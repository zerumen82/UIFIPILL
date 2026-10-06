// Secuencia de arranque como Linux: firmware PRIMERO, sin reset previo.
// v2: firmware en UNA SOLA MULTI_WRITE de 4096B (como rt2x00usb: el buffer
// pasa entero y el driver lo trocea en CSR_CACHE_SIZE=64 SIN pausas).
mod common;

use common::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let chan: u8 = args.get(1).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);
    let fw_path = args.get(2).filter(|s| !s.starts_with("--")).cloned().unwrap_or_else(|| "rt2870.bin".into());
    let mode = args.iter().position(|a| a == "--single").is_some();

    let h = open_rt3070().expect("abrir RT3070");
    println!("== Estado inicial ==");
    println!("MAC_CSR0 = {:#010x}", reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff));

    let _ = reg_write(&h, AUTOWAKEUP_CFG, 0);
    println!("[0] AUTOWAKEUP_CFG=0");

    let fw = std::fs::read(&fw_path).expect("leer rt2870.bin");
    let sec = &fw[FW_OFFSET..FW_OFFSET + FW_LENGTH];

    if mode {
        // UNA SOLA MULTI_WRITE de 4096B (rt2x00usb_register_multiwrite entero)
        let (v, idx) = encode_reg_addr(FIRMWARE_IMAGE_BASE);
        match h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, sec, std::time::Duration::from_millis(3000)) {
            Ok(_) => println!("[1] firmware 4096B en UNA transferencia ✅"),
            Err(e) => { println!("❌ MULTI_WRITE 4096B: {e:?}"); std::process::exit(3); }
        }
    } else {
        let mut ok = 0;
        for (i, chunk) in sec.chunks(64).enumerate() {
            let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
            let (v, idx) = encode_reg_addr(addr);
            match h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, std::time::Duration::from_millis(500)) {
                Ok(_) => ok += 1,
                Err(e) if i == 0 => { println!("❌ chunk 0 rechazado: {e:?}"); std::process::exit(3); }
                Err(_) => {}
            }
        }
        println!("[1] firmware {ok}/64 chunks (modo chunks)");
    }

    reg_write(&h, H2M_MAILBOX_CID, !0u32).unwrap();
    reg_write(&h, H2M_MAILBOX_STATUS, !0u32).unwrap();
    h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_FIRMWARE, 0, &[], FIRMWARE_TIMEOUT)
        .expect("DEVICE_MODE FIRMWARE");
    println!("[2] DEVICE_MODE FIRMWARE(8) — 10ms silencio…");
    std::thread::sleep(std::time::Duration::from_millis(10));
    reg_write(&h, H2M_MAILBOX_CSR, 0).ok();

    // PBF ready (100 x 1ms)
    let mut pbf_ok = false;
    for i in 0..100 {
        if let Ok(pbf) = reg_read(&h, PBF_SYS_CTRL) {
            if pbf & 0x80 != 0 { pbf_ok = true; println!("[3] PBF READY (intento {i}) = {pbf:#010x}"); break; }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    if !pbf_ok { println!("⚠️ PBF nunca READY"); }

    // BOOT_SIGNAL tolerante
    reg_write(&h, H2M_BBP_AGENT, 0).ok();
    reg_write(&h, H2M_MAILBOX_CSR, 0).ok();
    reg_write(&h, H2M_INT_SRC, 0).ok();
    match mcu_request_wait(&h, MCU_BOOT_SIGNAL, 0, 0, 0, 2000) {
        Ok(true) => println!("[4] BOOT_SIGNAL CONSUMIDO ✅"),
        Ok(false) => println!("⚠️ BOOT_SIGNAL sin confirmación"),
        Err(e) => println!("⚠️ err: {e}"),
    }
    std::thread::sleep(std::time::Duration::from_millis(2));

    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0);
    println!("[5] MAC_CSR0 = {csr0:#010x}");

    println!("\n== BBP ==");
    for r in [0u8, 1, 4] {
        let v = bbp_read(&h, r).unwrap_or(0xff);
        println!("bbp[{r}] = {v:#04x} {}", if v == 0x00 || v == 0xff { "❌" } else { "✅ VIVO" });
    }

    let bbp0 = bbp_read(&h, 0).unwrap_or(0);
    if bbp0 != 0x00 && bbp0 != 0xff {
        println!("\n✅✅ BBP VIVO — enable_radio + canal {chan} + RX test");
        init_registers_rt3070(&h).expect("init_registers");
        let _ = wait_busy(&h, MAC_STATUS_CFG, 0x0000_0003, 0);
        reg_write(&h, H2M_BBP_AGENT, 0).unwrap();
        reg_write(&h, H2M_MAILBOX_CSR, 0).unwrap();
        reg_write(&h, H2M_INT_SRC, 0).unwrap();
        let _ = mcu_request(&h, MCU_BOOT_SIGNAL, 0, 0, 0);
        match init_bbp_rt3070(&h) { Ok(_) => println!("  BBP init OK"), Err(e) => println!("  ⚠️ {e}") }
        match init_rfcsr_rt3070(&h) { Ok(_) => println!("  RFCSR init OK"), Err(e) => println!("  ⚠️ {e}") }
        let _ = mcu_request(&h, MCU_CURRENT, 0, 0, 0);
        reg_write(&h, MAC_SYS_CTRL, 0x04).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
        reg_write(&h, MAC_SYS_CTRL, 0x0C).unwrap();
        config_channel_rt3070(&h, chan).expect("canal");
        enable_tx_pa(&h).unwrap();
        reg_write(&h, BCN_TIME_CFG, 0).unwrap();
        let _ = rx_filter_monitor(&h);
        reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE).unwrap();
        println!("  radio ON + canal {chan}");

        println!("\n== RX 12s EP 0x81 ==");
        let mut frames = 0usize;
        let mut buf = vec![0u8; 8192];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
        while std::time::Instant::now() < deadline {
            match h.read_bulk(0x81, &mut buf, std::time::Duration::from_millis(100)) {
                Ok(n) if n > 0 => {
                    // Layout MEDIDO (vendor.pcap): [4 len][RXWI 16][802.11 @20],
                    // stride = len+8. El parser anterior ([4][32 rxwi]) no
                    // encontraba nunca el frame → 0 frames falsos.
                    frames += walk_rx(&buf[..n]).len();
                }
                Ok(_) => {}
                Err(rusb::Error::Timeout) => {}
                Err(e) => { println!("  read err {e:?}"); break; }
            }
        }
        println!("\n📡 {frames} frames — {}", if frames > 0 { "¡RX FUNCIONA!" } else { "0 frames" });
    }
}
