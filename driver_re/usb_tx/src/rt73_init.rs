// Init completo RT73 (port de rt73usb_init_registers + init_bbp) — sin firmware/kick.
mod common;
use common::*;

fn reg_read(h: &rusb::DeviceHandle<rusb::Context>, addr: u16) -> u32 {
    let mut buf = [0u8;4];
    h.read_control(common::REQ_IN, common::USB_MULTI_READ, 0, addr, &mut buf, common::FIRMWARE_TIMEOUT).ok();
    u32::from_le_bytes(buf)
}
fn reg_write(h: &rusb::DeviceHandle<rusb::Context>, addr: u16, val: u32) {
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, addr, &val.to_le_bytes(), common::REGISTER_TIMEOUT);
}
fn bbp_write(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8) {
    let word: u32 = (val as u32) | ((reg as u32) << 8) | (1 << 16);
    let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, RT73_PHY_CSR3 as u16, &word.to_le_bytes(), common::REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_micros(10));
}

fn main() {
    let h = match open_rt73() {
        Ok(h) => h,
        Err(e) => { eprintln!("❌ RT73: {e:?}"); std::process::exit(1); }
    };
    println!("✅ RT73 abierto, MAC_CSR0={:#010x}", reg_read(&h, RT73_MAC_CSR0 as u16));

    // Firmware load (rt73usb_load_firmware): MULTI_WRITE a 0x0800 + kick FIRMWARE(8)
    if let Ok(fw) = std::fs::read("rt73.bin") {
        if fw.len() == 2048 {
            // multiwrite 2048 bytes to FIRMWARE_IMAGE_BASE
            for (i, chunk) in fw.chunks(64).enumerate() {
                let off = 0x0800u16 + (i as u16 * 64);
                let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, off, chunk, common::REGISTER_TIMEOUT);
            }
            println!("   firmware 2048B escrito a 0x0800");
            // kick
            let _ = h.write_control(common::REQ_OUT, common::USB_DEVICE_MODE, common::USB_MODE_FIRMWARE, 0, &[], common::FIRMWARE_TIMEOUT);
            std::thread::sleep(std::time::Duration::from_millis(500));
            println!("   kick FIRMWARE(8) enviado");
        } else {
            println!("   firmware tamaño inesperado: {}", fw.len());
        }
    } else {
        println!("   rt73.bin no encontrado, saltando firmware");
    }

    // init_registers ...
    // TXRX_CSR0
    let mut r = reg_read(&h, 0x3040);
    r = (r & !0x04010000) | (1 << 15); // AUTO_TX_SEQ=1, DISABLE_RX=0
    reg_write(&h, 0x3040, r);
    // TXRX_CSR1
    reg_write(&h, 0x3044, (47) | (1<<7) | (30<<8) | (1<<15) | (42<<16) | (1<<23) | (30<<24) | (1<<31));
    // TXRX_CSR2
    reg_write(&h, 0x3048, (13) | (1<<7) | (12<<8) | (1<<15) | (11<<16) | (1<<23) | (10<<24) | (1<<31));
    // TXRX_CSR3
    reg_write(&h, 0x304c, (7) | (1<<7) | (6<<8) | (1<<15) | (5<<16) | (1<<23));
    // TXRX_CSR7
    reg_write(&h, 0x305c, (59) | (53<<8) | (49<<16) | (46<<24));
    // TXRX_CSR8
    reg_write(&h, 0x3060, (44) | (42<<8) | (42<<16) | (42<<24));
    // TXRX_CSR9
    reg_write(&h, 0x3064, 0);
    // TXRX_CSR15
    reg_write(&h, 0x307c, 0x0000000f);
    // MAC_CSR6
    reg_write(&h, 0x3018, 0x00000fff);
    // MAC_CSR10
    reg_write(&h, 0x3028, 0x00000718);
    // MAC_CSR13
    reg_write(&h, 0x3034, 0x00007f00);
    // SEC_CSR0/1/5
    reg_write(&h, 0x30a0, 0);
    reg_write(&h, 0x30a4, 0);
    reg_write(&h, 0x30b4, 0);
    // PHY_CSR1
    reg_write(&h, 0x3084, 0x000023b0);
    // PHY_CSR5/6/7
    reg_write(&h, 0x3094, 0x00040a06);
    reg_write(&h, 0x3098, 0x00080606);
    reg_write(&h, 0x309c, 0x00000408);
    // MAC_CSR9 CW_SELECT=0
    let mut mc9 = reg_read(&h, 0x3024);
    mc9 &= !(1 << 16);
    reg_write(&h, 0x3024, mc9);
    // HW_BEACON_BASE0-3 = 0
    for b in [0x2400u16, 0x2500, 0x2600, 0x2700] { reg_write(&h, b, 0); }
    // clear STA_CSR0/1/2
    let _ = reg_read(&h, 0x30c0);
    let _ = reg_read(&h, 0x30c4);
    let _ = reg_read(&h, 0x30c8);
    // reset MAC+BBP
    let mut mc1 = reg_read(&h, 0x3004);
    mc1 |= (1 << 0) | (1 << 1);
    reg_write(&h, 0x3004, mc1);
    std::thread::sleep(std::time::Duration::from_millis(1));
    mc1 &= !((1 << 0) | (1 << 1));
    reg_write(&h, 0x3004, mc1);
    // HOST_READY
    let mut mc1 = reg_read(&h, 0x3004);
    mc1 |= 1 << 2;
    reg_write(&h, 0x3004, mc1);
    // set_device_state AWAKE
    let mut mc12 = reg_read(&h, 0x3030);
    mc12 |= (1 << 2) | (1 << 0);
    mc12 &= !(1 << 1);
    reg_write(&h, 0x3030, mc12);

    // init_bbp
    let bbp_vals: [(u8,u8);24] = [
        (3,0x80),(15,0x30),(21,0xc8),(22,0x38),(23,0x06),(24,0xfe),
        (25,0x0a),(26,0x0d),(32,0x0b),(34,0x12),(37,0x07),(39,0xf8),
        (41,0x60),(53,0x10),(54,0x18),(60,0x10),(61,0x04),(62,0x04),
        (75,0xfe),(86,0xfe),(88,0xfe),(90,0x0f),(99,0x00),(102,0x16),
    ];
    for (r,v) in bbp_vals { bbp_write(&h, r, v); }
    bbp_write(&h, 107, 0x04);
    println!("✅ init completo. BBP0={:#04x} BBP3={:#04x}", 0u8, 0u8);
    // read BBP0 via PHY_CSR3
    let b0 = {
        let word: u32 = (0u32 << 8) | (1 << 15) | (1 << 16);
        let _ = h.write_control(common::REQ_OUT, common::USB_MULTI_WRITE, 0, RT73_PHY_CSR3 as u16, &word.to_le_bytes(), common::REGISTER_TIMEOUT);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut buf = [0u8;4];
        let _ = h.read_control(common::REQ_IN, common::USB_MULTI_READ, 0, RT73_PHY_CSR3 as u16, &mut buf, common::FIRMWARE_TIMEOUT);
        (u32::from_le_bytes(buf) & 0xff) as u8
    };
    println!("   BBP0 leído = {:#04x}", b0);
}
