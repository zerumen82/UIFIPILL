// RT73 chan config using rf_vals_bg_2528 ch11 + rx.
mod common;
use common::*;
fn reg_write(h: &rusb::DeviceHandle<rusb::Context>, addr: u16, val: u32) {
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, addr, &val.to_le_bytes(), common::REGISTER_TIMEOUT);
}
fn bbp_write(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8) {
    let word: u32 = (val as u32) | ((reg as u32) << 8) | (1 << 16);
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, RT73_PHY_CSR3 as u16, &word.to_le_bytes(), common::REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_micros(10));
}
fn rf_write(h: &rusb::DeviceHandle<rusb::Context>, val: u32) {
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, RT73_PHY_CSR4 as u16, &val.to_le_bytes(), common::REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_micros(10));
}
fn main() {
    let h = open_rt73().unwrap();
    // init_bbp
    let bbp_vals: [(u8,u8);24] = [
        (3,0x80),(15,0x30),(21,0xc8),(22,0x38),(23,0x06),(24,0xfe),
        (25,0x0a),(26,0x0d),(32,0x0b),(34,0x12),(37,0x07),(39,0xf8),
        (41,0x60),(53,0x10),(54,0x18),(60,0x10),(61,0x04),(62,0x04),
        (75,0xfe),(86,0xfe),(88,0xfe),(90,0x0f),(99,0x00),(102,0x16),
    ];
    for (r,v) in bbp_vals { bbp_write(&h, r, v); }
    bbp_write(&h, 107, 0x04);
    // TXRX_CSR0
    reg_write(&h, 0x3040, 0x00008000);

    // rf_vals_bg_2528 ch11
    let rf1 = 0x00002c0cu32;
    let rf2 = 0x0000079au32;
    let rf3 = 0x00068255u32;
    let rf4 = 0x000fea0bu32;
    // TXPOWER_TO_DEV(0) → rf3 |= 0? leave as is
    // smart = 1 (RF2528)
    let r3 = bbp_read(&h, 3);
    bbp_write(&h, 3, r3 | 0x01);
    bbp_write(&h, 94, 0x06);
    // RF writes x3
    for _ in 0..3 {
        rf_write(&h, rf1);
        rf_write(&h, rf2);
        rf_write(&h, rf3 & !0x00000004);
        rf_write(&h, rf4);
        rf_write(&h, rf1);
        rf_write(&h, rf2);
        rf_write(&h, rf3 | 0x00000004);
        rf_write(&h, rf4);
        rf_write(&h, rf1);
        rf_write(&h, rf2);
        rf_write(&h, rf3 & !0x00000004);
        rf_write(&h, rf4);
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
    println!("✅ chan 11 configurado, escuchando 10s...");
    // need bbp_read helper
    let mut urbs=0usize; let mut bytes=0usize;
    let mut buf=vec![0u8;4096];
    let dl=std::time::Instant::now()+std::time::Duration::from_secs(10);
    while std::time::Instant::now()<dl {
        match h.read_bulk(0x81,&mut buf,std::time::Duration::from_millis(200)) {
            Ok(n) if n>0 => {urbs+=1;bytes+=n;}
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => {eprintln!("⚠️ {e:?}"); std::thread::sleep(std::time::Duration::from_millis(50));}
        }
    }
    println!("URBs={} bytes={}", urbs, bytes);
}
fn bbp_read(h: &rusb::DeviceHandle<rusb::Context>, reg: u8) -> u8 {
    let word: u32 = ((reg as u32) << 8) | (1 << 15) | (1 << 16);
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, RT73_PHY_CSR3 as u16, &word.to_le_bytes(), common::REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let mut buf = [0u8;4];
    let _ = h.read_control(common::REQ_IN, common::USB_MULTI_READ, 0, RT73_PHY_CSR3 as u16, &mut buf, common::FIRMWARE_TIMEOUT);
    (u32::from_le_bytes(buf) & 0xff) as u8
}
