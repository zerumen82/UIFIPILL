// Sonda de acceso USB crudo al RT3070: lectura de registros MAC.
// NO escribe nada — solo demuestra que el device responde al protocolo
// vendor del rt2800usb cuando su driver Windows NO lo tiene reclamado
// (tras Zadig/WinUSB) o cuando está deshabilitado en PnP.
mod common;

fn main() {
    let mut h = match common::open_rt3070() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("❌ No se pudo abrir el RT3070 ({e:?}).");
            eprintln!("   Causas probables:");
            eprintln!("   1. netr28ux lo tiene reclamado → deshabilitar en PnP o Zadig→WinUSB");
            eprintln!("   2. No está conectado");
            std::process::exit(1);
        }
    };
    println!("✅ Device abierto, interfaz 0 reclamada");

    let desc = h.device().device_descriptor().unwrap();
    println!("   USB {}:{}, {} configuraciones", desc.vendor_id(), desc.product_id(), desc.num_configurations());

    // Endpoints de la config activa
    let cfg = h.device().active_config_descriptor().unwrap();
    for ifc in cfg.interfaces() {
        for desc in ifc.descriptors() {
            for ep in desc.endpoint_descriptors() {
                println!("   EP {:#04x} {:?} pkt={}", ep.address(), ep.transfer_type(), ep.max_packet_size());
            }
        }
    }

    // Diagnóstico: ¿el pipe de control responde a requests estándar?
    let mut st = [0u8; 2];
    match h.read_control(0x80, 0x00, 0, 0, &mut st, common::FIRMWARE_TIMEOUT) {
        Ok(_) => println!("✅ GET_STATUS estándar OK ({:#06x}) — pipe de control vivo", u16::from_le_bytes(st)),
        Err(e) => println!("⚠️  GET_STATUS falló: {e:?} — control pipe sin respuesta"),
    }

    // ⚠️ NO enviar USB_DEVICE_MODE(reset): deja la CPU del chip parada esperando
    // firmware y las MULTI_READ dejan de responder (hallazgo 2026-09-22).
    // rt2x00usb NUNCA lo hace en probe; solo en watchdog con firmware restart.
    println!("   (sin DEVICE_MODE reset — lectura directa de registros)");

    // ¿Autorun? — codificación EXACTA de rt2x00usb_check_autoload:
    // usb_control_msg(USB_DEVICE_MODE, IN, value=USB_MODE_AUTORUN, index=0, buf=1 byte)
    let mut ab = [0u8; 1];
    match h.read_control(common::REQ_IN, common::USB_DEVICE_MODE, common::USB_MODE_AUTORUN, 0, &mut ab, std::time::Duration::from_millis(500)) {
        Ok(1) => println!("   AUTORUN = {:#04x} (bit1: {}) — {}", ab[0], (ab[0] >> 1) & 1,
            if ab[0] & 2 == 2 { "en AUTOLOAD" } else { "NO autoload" }),
        Ok(n) => println!("   AUTORUN: respuesta corta ({n})"),
        Err(e) => println!("   AUTORUN: {e:?}"),
    }

    // Prueba clave: MAC_SYS_CTRL debe responder. Codificación rt2x00usb con timeout largo
    // (el chip ya respondió al AUTORUN, así que está vivo; el read puede tardar más tras reset)
    let mut ok_val: Option<u32> = None;
    for (name, wv, wi, tmo) in [
        ("MULTI_READ 2s", 0u16, common::MAC_SYS_CTRL, 2000u64),
        ("MULTI_READ 5s", 0u16, common::MAC_SYS_CTRL, 5000u64),
    ] {
        for attempt in 1..=2 {
            let mut buf = [0u8; 4];
            match h.read_control(common::REQ_IN, common::USB_MULTI_READ, wv, wi, &mut buf, std::time::Duration::from_millis(tmo)) {
                Ok(4) => {
                    let v = u32::from_le_bytes(buf);
                    println!("✅ MAC_SYS_CTRL (0x0004) vía {name} (intento {attempt}) = {v:#010x} — el chip HABLA");
                    ok_val = Some(v);
                    break;
                }
                Ok(n) => println!("   {name} #{attempt}: respuesta corta ({n} bytes)"),
                Err(e) => println!("   {name} #{attempt}: {e:?}"),
            }
        }
        if ok_val.is_some() { break; }
    }

    // Último recurso: reset del puerto USB (re-enumeración interna) y reabrir
    if ok_val.is_none() {
        println!("⚠️  Sin respuesta — probando reset del puerto USB...");
        if let Err(e) = h.reset() {
            println!("   reset: {e:?} (normal si Windows re-enumera)");
        }
        std::thread::sleep(std::time::Duration::from_millis(1500));
        match common::open_rt3070() {
            Ok(h2) => h = h2,
            Err(e) => {
                println!("   reabrir tras reset: {e:?} (¿Windows re-enumerando? reejecuta probe)");
                std::process::exit(2);
            }
        }
        let mut buf = [0u8; 4];
        match h.read_control(common::REQ_IN, common::USB_MULTI_READ, 0, common::MAC_SYS_CTRL, &mut buf, common::FIRMWARE_TIMEOUT) {
            Ok(4) => {
                let v = u32::from_le_bytes(buf);
                println!("✅ MAC_SYS_CTRL tras reset = {v:#010x} — el chip HABLA");
                ok_val = Some(v);
            }
            Ok(n) => println!("   tras reset: respuesta corta ({n})"),
            Err(e) => println!("   tras reset: {e:?}"),
        }
    }
    if ok_val.is_none() {
        eprintln!("❌ reg_read falló en todos los modos — ¿chip en modo sleep o vendor requests bloqueadas?");
        eprintln!("   Siguiente: prueba de re-enchufar la antena y correr probe otra vez.");
        std::process::exit(2);
    }

    // Algunos registros más de identidad
    for (name, addr) in [
        ("PBF_SYS_CTRL", common::PBF_SYS_CTRL),
        ("USB_DMA_CFG", common::USB_DMA_CFG),
        ("BCN_TIME_CFG", common::BCN_TIME_CFG),
    ] {
        match common::reg_read(&h, addr) {
            Ok(v) => println!("   {name} (0x{addr:04x}) = {v:#010x}"),
            Err(e) => println!("   {name} (0x{addr:04x}) = ERR {e:?}"),
        }
    }
}
