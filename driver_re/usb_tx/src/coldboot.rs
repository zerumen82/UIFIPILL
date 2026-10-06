// BOOT FRÍO REPLICADO DE LINUX con USB_MODE_UNPLUG(2) — codificación corregida.
//
// El watchdog de rt2x00usb usa DEVICE_MODE OUT UNPLUG para hacer que el chip
// SE DESCONECTE del bus y re-enumeración EN FRÍO (como un enchufado nuevo).
// Eso nos da en Windows lo que Linux tiene siempre en probe: un chip fresco,
// sin firmware del vendor, donde la secuencia rt2800usb_load_firmware es legal.
//
// Fases:
//  1. UNPLUG (wValue=2, wIndex=0) → el chip debe desaparecer del bus.
//  2. Esperar re-enumeración REAL (desaparición + reaparición).
//  3. Chip frío: autorun_detect + MCU test (¿de verdad está muerto?) + CSR.
//  4. Carga fw EXACTA de Linux + kick → msleep(10) → MAILBOX=0.
//     (En Linux NO hay re-enum tras el kick — si ocurre, lo detectamos.)
//  5. wait_csr_ready → init_registers → BOOT_SIGNAL → BBP → RFCSR → canal → RX.
mod common;

use common::*;
use rusb::UsbContext;
use std::time::Duration;

fn find_addr() -> Option<u8> {
    let ctx = rusb::Context::new().ok()?;
    for dev in ctx.devices().ok()?.iter() {
        if let Ok(d) = dev.device_descriptor() {
            if d.vendor_id() == VID_RALINK && d.product_id() == PID_RT3070 {
                return Some(dev.address());
            }
        }
    }
    None
}

fn wait_reenum(secs: u64, addr_before: u8) -> Option<rusb::DeviceHandle<rusb::Context>> {
    // FASE 1: desaparición REAL (device fuera del bus O cambia de addr).
    // addr_before se captura ANTES del UNPLUG (bug v1: se capturaba después y
    // la nueva addr ya era la «vieja» cuando empezamos a mirar).
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    let mut disappeared = false;
    while std::time::Instant::now() < deadline {
        match find_addr() {
            None => { if !disappeared { println!("  · device fuera del bus"); } disappeared = true; }
            Some(a) if a != addr_before => { disappeared = true; println!("  · device volvió con nueva addr {a}"); }
            Some(_) => {}
        }
        if disappeared {
            // FASE 2: intentar abrir (WinUSB puede tardar en reconfigurarse)
            if let Ok(h) = open_rt3070() {
                return Some(h);
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let chan: u8 = args.get(1).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);

    println!("=== COLD BOOT vía UNPLUG (réplica del punto de partida de Linux) ===\n");
    let h = open_rt3070().expect("abrir RT3070 (la antena debe estar conectada con WinUSB)");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[pre] MAC_CSR0 = {csr0:#010x}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo antes de empezar — power-cycle primero");
        std::process::exit(1);
    }

    // ── FASE 1: UNPLUG → desconexión forzada ──
    println!("\n[unplug] DEVICE_MODE OUT UNPLUG(2) — el chip debe desconectarse…");
    let addr_before = find_addr().unwrap_or(0);
    match h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_UNPLUG, 0, &[], Duration::from_millis(1000)) {
        Ok(_) => println!("  enviado ✅"),
        Err(e) => println!("  err {e:?} (puede ser normal: el chip corta al instante)"),
    }
    drop(h);
    std::thread::sleep(Duration::from_millis(500));

    // ── FASE 2: re-enumeración ──
    println!("[re-enum] esperando que el chip vuelva EN FRÍO (60 s)…");
    let h = match wait_reenum(60, addr_before) {
        Some(h) => h,
        None => {
            println!("❌ el chip no volvió en 60 s. Re-enchúfalo (power-cycle) y reintenta.");
            std::process::exit(2);
        }
    };
    println!("  ✅ chip RE-ENUMERADO (frío)");

    // ── FASE 3: verificación de chip frío ──
    std::thread::sleep(Duration::from_millis(500));
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("\n[cold] MAC_CSR0 = {csr0:#010x}");
    let mut ab = [0u8; 4];
    match h.read_control(REQ_IN, USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, &mut ab, FIRMWARE_TIMEOUT) {
        Ok(_) => {
            let raw = u32::from_le_bytes(ab);
            println!("[cold] autorun_detect = {raw:#010x} → {}",
                if raw & 3 == 2 { "AUTORUN (no cargar fw)" } else { "NORMAL (cargar fw)" });
            if raw & 3 == 2 { println!("  (autorun: el chip trae fw de fábrica — saltamos carga)"); }
        }
        Err(e) => { println!("[cold] autorun_detect err: {e:?}"); }
    }

    // ── FASE 4: carga fw EXACTA de Linux ──
    println!("\n[load_firmware]");
    let _ = reg_write(&h, AUTOWAKEUP_CFG, 0);
    let fw = std::fs::read("rt2870.bin").expect("leer rt2870.bin");
    let sec = &fw[FW_OFFSET..FW_OFFSET + FW_LENGTH];
    let mut ok = 0;
    for (i, chunk) in sec.chunks(64).enumerate() {
        let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
        let (v, idx) = encode_reg_addr(addr);
        if h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, Duration::from_millis(500)).is_ok() { ok += 1; }
    }
    println!("  firmware {ok}/64 chunks");
    if ok < 64 { println!("  ⚠️ carga incompleta — el chip puede no estar frío"); }
    let _ = reg_write(&h, H2M_MAILBOX_CID, !0u32);
    let _ = reg_write(&h, H2M_MAILBOX_STATUS, !0u32);
    match h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_FIRMWARE, 0, &[], FIRMWARE_TIMEOUT) {
        Ok(_) => println!("  kick FIRMWARE(8) enviado"),
        Err(e) => println!("  kick err: {e:?}"),
    }
    std::thread::sleep(Duration::from_millis(10));
    let _ = reg_write(&h, H2M_MAILBOX_CSR, 0);
    // En Linux NO hay re-enum tras el kick. Si el chip desaparece, lo sabremos
    // en el wait_csr_ready (todo dará timeout).

    // ── FASE 5: rt2800usb_init_registers → enable_radio ──
    println!("\n[wait_csr_ready]");
    let mut csr_ok = false;
    for i in 0..100 {
        let v = reg_read(&h, MAC_CSR0).unwrap_or(0);
        if v != 0 && v != 0xffff_ffff {
            println!("  csr_ready (i={i}) = {v:#010x} ✅");
            csr_ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    if !csr_ok {
        println!("  ❌ CSR no ready tras kick (¿el chip re-enumeró? ¿se murió?)");
        println!("  estado del bus:");
        match find_addr() {
            Some(a) => println!("    device presente en addr {a} (¿re-enum falsa o chip vivo?)"),
            None => println!("    device FUERA del bus (re-enum del fw — el modo A otra vez)"),
        }
        std::process::exit(3);
    }

    println!("\n[radio]");
    // init_registers (con el segundo reset MAC+BBP de Linux — ahora es legal:
    // acabamos de arrancar el fw nosotros, igual que rt2800usb_init_registers)
    let pbf = reg_read(&h, PBF_SYS_CTRL).unwrap_or(0);
    let _ = reg_write(&h, PBF_SYS_CTRL, pbf & !0x0000_2000);
    let _ = reg_write(&h, MAC_SYS_CTRL, 0x0000_0003);
    let _ = h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_RESET, 0, &[], REGISTER_TIMEOUT);
    std::thread::sleep(Duration::from_millis(10));
    let _ = reg_write(&h, MAC_SYS_CTRL, 0x0000_0000);
    println!("  segundo reset MAC+BBP (rt2800usb_init_registers)");
    reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE).expect("USB_DMA_CFG");
    std::thread::sleep(Duration::from_millis(10));
    init_registers_rt3070(&h).expect("init_registers");
    println!("  init_registers OK");
    let _ = wait_busy(&h, MAC_STATUS_CFG, 0x0000_0003, 0);
    reg_write(&h, H2M_BBP_AGENT, 0).unwrap();
    reg_write(&h, H2M_MAILBOX_CSR, 0).unwrap();
    reg_write(&h, H2M_INT_SRC, 0).unwrap();
    let _ = mcu_request(&h, MCU_BOOT_SIGNAL, 0, 0, 0);
    println!("  BOOT_SIGNAL");
    match init_bbp_rt3070(&h) {
        Ok(_) => println!("  ✅ BBP init OK — ¡EL BBP DESPIERTA EN BOOT FRÍO!"),
        Err(e) => { println!("  ❌ BBP: {e}"); std::process::exit(4); }
    }
    match init_rfcsr_rt3070(&h) {
        Ok(_) => println!("  ✅ RFCSR init OK"),
        Err(e) => { println!("  ❌ RFCSR: {e}"); std::process::exit(4); }
    }
    let _ = mcu_request(&h, MCU_CURRENT, 0, 0, 0);
    reg_write(&h, MAC_SYS_CTRL, 0x04).unwrap();
    std::thread::sleep(Duration::from_millis(1));
    reg_write(&h, MAC_SYS_CTRL, 0x0C).unwrap();
    config_channel_rt3070(&h, chan).expect("canal");
    enable_tx_pa(&h).unwrap();
    reg_write(&h, BCN_TIME_CFG, 0).unwrap();
    let _ = rx_filter_monitor(&h);
    println!("  canal {chan} + PA + RX monitor");

    // ── RX TEST ──
    println!("\n== RX 15s EP 0x81 ==");
    let mut frames = 0usize;
    let mut buf = vec![0u8; 8192];
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, Duration::from_millis(100)) {
            Ok(n) if n > 0 => {
                // Layout MEDIDO (vendor.pcap): [4 len][RXWI 16][802.11 @20]
                frames += walk_rx(&buf[..n]).len();
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => { println!("  read err {e:?}"); break; }
        }
    }
    println!("\n📡 {frames} frames — {}", if frames > 0 {
        "🎉 ¡RX FUNCIONA EN BOOT FRÍO (la misma antena, sin Kali)!"
    } else {
        "0 frames"
    });
}
