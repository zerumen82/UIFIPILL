// RX contador RT73: init_bbp + lectura EP 0x81 durante N segundos.
mod common;
use common::*;

fn bbp_write(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8) {
    let word: u32 = (val as u32) | ((reg as u32) << 8) | (1 << 16);
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, RT73_PHY_CSR3 as u16, &word.to_le_bytes(), common::REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_micros(10));
}

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    let h = match open_rt73() {
        Ok(h) => h,
        Err(e) => { eprintln!("❌ RT73: {e:?}"); std::process::exit(1); }
    };
    // init_bbp mínimo
    let bbp_vals: [(u8,u8);24] = [
        (3,0x80),(15,0x30),(21,0xc8),(22,0x38),(23,0x06),(24,0xfe),
        (25,0x0a),(26,0x0d),(32,0x0b),(34,0x12),(37,0x07),(39,0xf8),
        (41,0x60),(53,0x10),(54,0x18),(60,0x10),(61,0x04),(62,0x04),
        (75,0xfe),(86,0xfe),(88,0xfe),(90,0x0f),(99,0x00),(102,0x16),
    ];
    for (r,v) in bbp_vals { bbp_write(&h, r, v); }
    bbp_write(&h, 107, 0x04);
    // habilitar RX: TXRX_CSR0 DISABLE_RX=0
    let mut buf = [0u8;4];
    let _ = h.read_control(common::REQ_IN, common::USB_MULTI_READ, 0, 0x3040, &mut buf, common::FIRMWARE_TIMEOUT);
    let mut v = u32::from_le_bytes(buf);
    v &= !(1 << 16); // DISABLE_RX=0
    v |= 1 << 15; // AUTO_TX_SEQ
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, 0x3040, &v.to_le_bytes(), common::REGISTER_TIMEOUT);
    println!("✅ RX armado, escuchando {secs}s en EP 0x81…");
    let mut urbs = 0usize;
    let mut bytes = 0usize;
    let mut buf = vec![0u8; 4096];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, std::time::Duration::from_millis(200)) {
            Ok(n) if n > 0 => { urbs += 1; bytes += n; }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => { eprintln!("⚠️ {e:?}"); std::thread::sleep(std::time::Duration::from_millis(50)); }
        }
    }
    println!("URBs={urbs} bytes={bytes}");
    if urbs == 0 { std::process::exit(2); }
}
