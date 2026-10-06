// Experimento de arranque con diagnóstico fino de la re-enumeración:
// 1. escribir fw + kick FIRMWARE(8) (formato verificado contra rt2x00usb.h)
// 2. esperar desaparición REAL del device
// 3. al reaparecer: sondear GET_DESCRIPTOR estándar (ep0, sin claim, sin reset)
//    y vendor MULTI_READ con timeouts largos — distinguir sordera USB vs vendor.
mod common;

use common::*;
use rusb::UsbContext;
use std::time::Duration;

fn find_addr() -> Option<(u8, u8, u16, u16)> {
    let ctx = rusb::Context::new().ok()?;
    for dev in ctx.devices().ok()?.iter() {
        if let Ok(d) = dev.device_descriptor() {
            if d.vendor_id() == VID_RALINK && d.product_id() == PID_RT3070 {
                return Some((dev.bus_number(), dev.address(), d.vendor_id(), d.product_id()));
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let chan: u8 = args.get(1).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);

    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[pre] MAC_CSR0 = {csr0:#010x}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo — power-cycle antes");
        std::process::exit(1);
    }
    let addr_before = find_addr().map(|(_, a, _, _)| a).unwrap_or(0);
    println!("[pre] addr={addr_before}");

    // ── 1. fw + kick ──
    let fw = std::fs::read("rt2870.bin").expect("leer rt2870.bin");
    let sec = &fw[FW_OFFSET..FW_OFFSET + FW_LENGTH];
    let mut ok = 0;
    for (i, chunk) in sec.chunks(64).enumerate() {
        let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
        let (v, idx) = encode_reg_addr(addr);
        if h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, Duration::from_millis(500)).is_ok() { ok += 1; }
    }
    println!("[fw] {ok}/64 chunks");
    let _ = reg_write(&h, H2M_MAILBOX_CID, !0u32);
    let _ = reg_write(&h, H2M_MAILBOX_STATUS, !0u32);
    match h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_FIRMWARE, 0, &[], FIRMWARE_TIMEOUT) {
        Ok(_) => println!("[kick] FIRMWARE(8) enviado"),
        Err(e) => println!("[kick] err: {e:?}"),
    }
    drop(h);

    // ── 2. esperar desaparición y reaparición ──
    println!("[wait] esperando desaparición…");
    let mut disappeared = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    let mut h2 = None;
    while std::time::Instant::now() < deadline {
        match find_addr() {
            Some((bus, addr, vid, pid)) => {
                if !disappeared {
                    if addr != addr_before { disappeared = true; } // re-enum directa
                } else {
                    println!("[re-enum] de vuelta: bus={bus} addr={addr} {vid:04x}:{pid:04x}");
                    // abrir SIN claim primero: GET_DESCRIPTOR no requiere interfaz
                    h2 = open_rt3070().ok();
                    break;
                }
            }
            None => { disappeared = true; }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    let h = match h2 {
        Some(h) => h,
        None => { println!("❌ device no reapareció en 90 s"); std::process::exit(2); }
    };

    // ── 3. sondeos sin reset ──
    std::thread::sleep(Duration::from_millis(1000));
    println!("\n[probe post-enum]");

    // (a) GET_DESCRIPTOR estándar del DEVICE (ep0 puro, nada de vendor)
    let mut dbuf = [0u8; 18];
    match h.read_control(0x80, 0x06, 0x0100, 0, &mut dbuf, Duration::from_millis(3000)) {
        Ok(n) => println!("  GET_DESCRIPTOR = {n}B {:02x?}… {}", &dbuf[..4.min(n)], if n == 18 { "✅ ep0 USB VIVO" } else { "⚠️ corto" }),
        Err(e) => println!("  GET_DESCRIPTOR: {e:?} ❌ ep0 USB sordo"),
    }

    // (b) autorun_detect vendor (DEVICE_MODE IN 0x11)
    let mut ab = [0u8; 4];
    match h.read_control(REQ_IN, USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, &mut ab, Duration::from_millis(3000)) {
        Ok(_) => println!("  autorun_detect = {:#010x} ✅ vendor VIVO", u32::from_le_bytes(ab)),
        Err(e) => println!("  autorun_detect: {e:?} ❌ vendor sordo"),
    }

    // (c) MAC_CSR0 con timeout largo
    let mut mb = [0u8; 4];
    match h.read_control(REQ_IN, USB_MULTI_READ, 0, MAC_CSR0, &mut mb, Duration::from_millis(5000)) {
        Ok(_) => println!("  MAC_CSR0 = {:#010x} ✅ registros VIVOS", u32::from_le_bytes(mb)),
        Err(e) => println!("  MAC_CSR0: {e:?} ❌ registros sordos"),
    }

    // (d) si el vendor responde, seguir directamente la cola de la radio
    if h.read_control(REQ_IN, USB_MULTI_READ, 0, MAC_CSR0, &mut mb, Duration::from_millis(2000)).is_ok() {
        println!("\n[sigue] CSR ready → after radio (canal {chan})");
        // NO reimprimimos todo: apuntamos al final del linuxfull
        after_radio_minimal(&h, chan);
    }
}

// Cola mínima de radio reutilizando common (misma que after_csr_ready)
fn vendor_sw_retry(h: &rusb::DeviceHandle<rusb::Context>, req: u8, value: u16, total_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_millis(total_ms);
    while std::time::Instant::now() < deadline {
        if h.write_control(REQ_OUT, req, value, 0, &[], Duration::from_millis(200)).is_ok() {
            return true;
        }
    }
    false
}

fn after_radio_minimal(h: &rusb::DeviceHandle<rusb::Context>, chan: u8) {
    let _ = reg_write(h, PBF_SYS_CTRL, reg_read(h, PBF_SYS_CTRL).unwrap_or(0) & !0x0000_2000);
    let _ = reg_write(h, MAC_SYS_CTRL, 0x0000_0003);
    let _ = vendor_sw_retry(h, USB_DEVICE_MODE, USB_MODE_RESET, 200);
    std::thread::sleep(Duration::from_millis(10));
    let _ = reg_write(h, MAC_SYS_CTRL, 0x0000_0000);
    println!("  reset MAC+BBP post-fw");
    reg_write(h, USB_DMA_CFG, USB_DMA_CFG_VALUE).ok();
    std::thread::sleep(Duration::from_millis(10));
    init_registers_rt3070(h).expect("init_registers");
    let _ = wait_busy(h, MAC_STATUS_CFG, 0x0000_0003, 0);
    reg_write(h, H2M_BBP_AGENT, 0).ok();
    reg_write(h, H2M_MAILBOX_CSR, 0).ok();
    reg_write(h, H2M_INT_SRC, 0).ok();
    let _ = mcu_request(h, MCU_BOOT_SIGNAL, 0, 0, 0);
    match init_bbp_rt3070(h) {
        Ok(_) => println!("  ✅ BBP init OK"),
        Err(e) => { println!("  ❌ BBP: {e}"); return; }
    }
    init_rfcsr_rt3070(h).expect("rfcsr");
    let _ = mcu_request(h, MCU_CURRENT, 0, 0, 0);
    reg_write(h, MAC_SYS_CTRL, 0x04).ok();
    std::thread::sleep(Duration::from_millis(1));
    reg_write(h, MAC_SYS_CTRL, 0x0C).ok();
    config_channel_rt3070(h, chan).expect("canal");
    enable_tx_pa(h).ok();
    reg_write(h, BCN_TIME_CFG, 0).ok();
    let _ = rx_filter_monitor(h);
    println!("  radio ON canal {chan}");

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
    println!("\n📡 {frames} frames — {}", if frames > 0 { "🎉 ¡RX FUNCIONA!" } else { "0 frames" });
}
