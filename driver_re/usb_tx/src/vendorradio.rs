// Radio sobre el MCU VIVO del vendor — SIN cargar firmware, SIN kick.
//
// Basado en la medición de mcutest: tras power-cycle el chip viene con
// MCU 8051 corriendo (MCU_CURRENT consumido, mailbox STATUS=0xf00f1587).
// Cargar fw + kick sobre ese estado MATA el chip (modos A/B medidos).
// Linux equivaldría a esto con autorun_detect=2: skip load → init_registers.
//
// Secuencia: set_state AWAKE (MCU_WAKEUP) → USB_DMA_CFG → init_registers
// → BOOT_SIGNAL → BBP init → RFCSR init → canal → PA → RX test.
mod common;

use common::*;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let chan: u8 = args.get(1).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);
    let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);

    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[pre] MAC_CSR0 = {csr0:#010x}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo — power-cycle antes");
        std::process::exit(1);
    }

    // sanity: ¿MCU vivo?
    match mcu_request_wait(&h, MCU_CURRENT, 0xff, 0, 0, 1000) {
        Ok(true) => println!("[mcu] ✅ vivo (MCU_CURRENT consumido)"),
        _ => println!("[mcu] ⚠️ sin confirmación — seguimos igualmente"),
    }

    // ── STATE_AWAKE (rt2800usb_set_device_state) ──
    match mcu_request_wait(&h, MCU_WAKEUP, 0xff, 0, 2, 1000) {
        Ok(true) => println!("[wake] ✅ MCU_WAKEUP consumido"),
        _ => println!("[wake] ⚠️ sin confirmación"),
    }
    std::thread::sleep(Duration::from_millis(1));

    // ── enable_radio (rt2800usb_enable_radio) ──
    println!("\n[radio] (SIN segundo reset MAC — el MCU del vendor ya corre)");
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
        Ok(_) => println!("  ✅ BBP init OK"),
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
    println!("  MAC enable TX+RX");
    config_channel_rt3070(&h, chan).expect("canal");
    enable_tx_pa(&h).unwrap();
    reg_write(&h, BCN_TIME_CFG, 0).unwrap();
    let _ = rx_filter_monitor(&h);
    println!("  canal {chan} + PA + RX monitor ON");

    // ── RX TEST ──
    println!("\n== RX {secs}s EP 0x81 ==");
    let mut frames = 0usize;
    let mut buf = vec![0u8; 8192];
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, Duration::from_millis(100)) {
            Ok(n) if n >= 36 => {
                let mut off = 0usize;
                while off + 36 <= n {
                    let dma = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap()) as usize;
                    if dma < 36 || dma > 4096 { break; }
                    if off + dma > n { break; }
                    let w0 = u32::from_le_bytes(buf[off + 4..off + 8].try_into().unwrap());
                    let mpdu = ((w0 >> 16) & 0x0fff) as usize;
                    if mpdu >= 26 && off + 4 + 32 + mpdu <= n { frames += 1; }
                    off += (dma + 3) & !3;
                }
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => { println!("  read err {e:?}"); break; }
        }
    }
    println!("\n📡 {frames} frames — {}", if frames > 0 {
        "🎉 ¡RX FUNCIONA sobre el MCU del vendor!"
    } else {
        "0 frames"
    });
}
