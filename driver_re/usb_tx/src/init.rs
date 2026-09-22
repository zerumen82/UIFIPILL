// Init completo del RT3070 por USB crudo (puerto del camino rt2800usb):
// reset → firmware → radio ON → canal → modo TX/RX.
// Uso: rt3070_init <canal 1-14> [ruta rt2870.bin]
mod common;

use common::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let channel: u8 = args.get(1).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);
    let fw_path = args.get(2).cloned().unwrap_or_else(|| "rt2870.bin".into());

    let h = open_rt3070().expect("abrir RT3070 (ver probe)");
    println!("✅ Device abierto");

    // ── 1. Chequeo CSR (informativo: sin firmware el MCU puede no responder) ──
    let mut csr_ok = false;
    for _ in 0..20 {
        let v = reg_read(&h, 0x1000).unwrap_or(0); // MAC_CSR0 / ASIC version
        if v != 0 && v != 0xffffffff {
            println!("✅ CSR ready: ASIC ver = {v:#010x}");
            csr_ok = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !csr_ok {
        // Sin firmware el MCU no atiende MULTI_READ — cargar firmware primero.
        println!("⚠️  CSR no responde aún (esperado sin firmware) — cargando firmware directamente");
    }

    // PBF_SYS_CTRL: limpiar bit 0x2000 (tolerante: sin firmware puede fallar)
    let pbf = reg_read(&h, PBF_SYS_CTRL).unwrap_or(0);
    if let Err(e) = reg_write(&h, PBF_SYS_CTRL, pbf & !0x0000_2000) { println!("   PBF write: {e:?}"); }

    // Reset MAC+BBP (tolerante)
    if let Err(e) = reg_write(&h, MAC_SYS_CTRL, 0x3) { println!("   MAC_SYS_CTRL=0x3: {e:?}"); }
    if let Err(e) = h.write_control(REQ_OUT, USB_DEVICE_MODE, 0, USB_MODE_RESET, &[], REGISTER_TIMEOUT) { println!("   DEVICE_MODE reset: {e:?}"); }
    std::thread::sleep(std::time::Duration::from_millis(100));
    if let Err(e) = reg_write(&h, MAC_SYS_CTRL, 0x0) { println!("   MAC_SYS_CTRL=0x0: {e:?}"); }
    println!("✅ Reset MAC/BBP enviado (tolerante a fallos)");

    // ── 2. Firmware ──
    let fw = std::fs::read(&fw_path).expect("leer rt2870.bin");
    println!("   firmware: {} bytes", fw.len());

    // autorun detect: USB_DEVICE_MODE IN con USB_MODE_AUTORUN
    let mut buf4 = [0u8; 4];
    let autorun = h.read_control(REQ_IN, USB_DEVICE_MODE, 0, USB_MODE_AUTORUN, &mut buf4, FIRMWARE_TIMEOUT)
        .map(|_| u32::from_le_bytes(buf4) & 3 == 2)
        .unwrap_or(false);
    if autorun {
        println!("   NIC en AutoRun: firmware no requerido");
    } else {
        // multiwrite FIRMWARE_IMAGE_BASE ← sección RT3070 (offset 0, 4096 B)
        let mut fw_ok = 0;
        for (i, chunk) in fw[FW_OFFSET..FW_OFFSET + FW_LENGTH].chunks(64).enumerate() {
            let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
            let (v, idx) = encode_reg_addr(addr);
            match h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, std::time::Duration::from_millis(500)) {
                Ok(_) => fw_ok += 1,
                Err(e) => {
                    println!("   chunk {i} @ {addr:#06x}: {e:?}");
                    if i == 0 {
                        println!("❌ El primer chunk de firmware no se pudo escribir — CPU del chip muerta o chip NO es un RT3070 real (¿clon?).");
                        std::process::exit(3);
                    }
                }
            }
        }
        println!("   firmware: {fw_ok}/64 chunks escritos");
        // H2M mailboxes + USB_DEVICE_MODE FIRMWARE
        reg_write(&h, H2M_MAILBOX_CID, !0u32).unwrap();
        reg_write(&h, H2M_MAILBOX_STATUS, !0u32).unwrap();
        h.write_control(REQ_OUT, USB_DEVICE_MODE, 0, USB_MODE_FIRMWARE, &[], FIRMWARE_TIMEOUT).unwrap();
        println!("✅ Comando FIRMWARE enviado — esperando a que arranque el MCU...");
        // El MCU tarda en arrancar: sondear MAC_SYS_CTRL hasta que responda
        let mut mcu_up = false;
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if reg_read(&h, MAC_SYS_CTRL).is_ok() {
                mcu_up = true;
                break;
            }
        }
        println!("{} MCU respondiendo tras firmware", if mcu_up { "✅" } else { "❌ no" });
        reg_write(&h, H2M_MAILBOX_CSR, 0).unwrap();
    }

    // ── 3. USB DMA + radio ON ──
    // USB_DMA_CFG: RX/TX bulk enable, agg timeout 128us (valor fijo de lab)
    reg_write(&h, USB_DMA_CFG, 0x0000_009C | (1 << 29) | (1 << 28)).unwrap(); // TX|RX bulk en bits 29/28 (aprox rt2800)
    std::thread::sleep(std::time::Duration::from_millis(10));

    // WAKEUP MCU (H2M_MAILBOX_CSR = 0x70 + token) — simplificado de lab
    reg_write(&h, H2M_MAILBOX_CSR, 0x70).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));

    // MAC_SYS_CTRL: enable TX + RX
    reg_write(&h, MAC_SYS_CTRL, 0x0C).unwrap();
    println!("✅ Radio ON (MAC_SYS_CTRL=0x0C)");

    // ── 4. Canal (port de rt2800_channel: RF + BBP + MAC) ──
    // Para RT3070: freq_offset = (chan - 1) * 5 + 0x0E (2.4GHz base 0x0E)
    let freq = ((channel as u32 - 1) * 5) + 0x0E;
    // Escrituras RF vía RF_CSR (registro 0x0500, RF_CSR_CFG)
    let rf_write = |h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8| {
        // RT3070 RF: 8bit reg + 8bit val en un word: bit15=0 (write), bits14-8=reg, bits7-0=val
        let word: u32 = (reg as u32) << 8 | val as u32; // formato corto; el driver real usa 0x0504 con R7..R10
        reg_write(h, 0x0500, word).ok();
    };
    // Secuencia RF mínima (RT3070): RF_R07=chan_low, RF_R09=freq_hi, RF_R11=resel
    rf_write(&h, 7, (freq & 0xFF) as u8);
    rf_write(&h, 9, ((freq >> 8) & 0xFF) as u8);
    rf_write(&h, 11, 0x02); // K RSSI / tune
    rf_write(&h, 0x4C, 0xB3); // txmix_bw etc (aprox)
    // BBP: TXFilter swap for channels >14? No — solo 2.4GHz aquí
    println!("✅ Canal {channel} programado (RF freq={freq:#x})");

    // BCN_TIME_CFG: TSF off, beacon off (modo manual)
    reg_write(&h, BCN_TIME_CFG, 0x0000_0000).unwrap();

    println!("\n➡️  Siguiente: rt3070_tx para lanzar beacons y verificar con netsh en la otra radio");
}
