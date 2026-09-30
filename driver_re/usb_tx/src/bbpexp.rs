// Experimento quirúrgico: ¿por qué el BBP devuelve 0x00 siempre?
mod common;
use common::*;

fn bbp_read_raw(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, rw_mode: u32) -> rusb::Result<u8> {
    let word: u32 = ((reg as u32) << 8) | (1 << 16) | (1 << 17) | (rw_mode << 19);
    reg_write(h, BBP_CSR_CFG, word)?;
    std::thread::sleep(std::time::Duration::from_millis(1));
    let v = reg_read(h, BBP_CSR_CFG)?;
    Ok((v & 0xff) as u8)
}

fn bbp_write_raw(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8, rw_mode: u32) -> rusb::Result<()> {
    let word: u32 = (val as u32) | ((reg as u32) << 8) | (1 << 17) | (rw_mode << 19);
    reg_write(h, BBP_CSR_CFG, word)
}

fn main() {
    let h = open_rt3070().expect("abrir RT3070");

    println!("== 0. Identidad del chip ==");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0);
    println!("MAC_CSR0 = {csr0:#010x} → chipset={:#06x} rev={:#04x}", csr0 >> 16, csr0 & 0xffff);

    println!("\n== 0b. EFUSE map (config crítico) ==");
    // EEPROM word index: CHIP_ID=0, VERSION=1, MAC=2..4, NIC_CONF0=5, NIC_CONF1=6
    // NOTA: el efuse_dump escribe índice de word en ADDRESS_IN (bits17-25), así
    // que i es el índice de word, no el offset de byte.
    for i in 0u16..16 {
        let blk = efuse_read_block(&h, i).unwrap_or([0; 8]);
        for (k, w) in blk.iter().enumerate() {
            print!("w{:02}={:04x} ", i + k as u16, w);
        }
        println!();
    }
    // NIC_CONF0 = word 5 (0x0000 leído antes); RF_TYPE bits8-11
    let blk = efuse_read_block(&h, 4).unwrap_or([0; 8]);
    println!("words 4-11: {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x}",
        blk[0], blk[1], blk[2], blk[3], blk[4], blk[5], blk[6], blk[7]);

    println!("\n== 1. Readback MAC register ==");
    reg_write(&h, MAC_ADDR_DW0, 0x11223344).unwrap();
    let v = reg_read(&h, MAC_ADDR_DW0).unwrap_or(0);
    println!("MAC_ADDR_DW0: {} {}", if v == 0x11223344 { "✅" } else { "❌" }, { let _ = v; "" });

    println!("\n== 2. RFCSR completo ANTES de cualquier init (estado de fábrica) ==");
    print!("rfcsr: ");
    for r in 0u8..32 {
        let v = rfcsr_read(&h, r).unwrap_or(0xff);
        print!("{r}:{v:02x} ");
    }
    println!();

    println!("\n== 3. Firmware + BOOT_SIGNAL ==");
    let fw = std::fs::read("rt2870.bin").expect("leer rt2870.bin");
    let _ = reg_write(&h, PBF_SYS_CTRL, reg_read(&h, PBF_SYS_CTRL).unwrap_or(0) & !0x0000_2000);
    let _ = reg_write(&h, MAC_SYS_CTRL, 0x3);
    let _ = h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_RESET, 0, &[], REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = reg_write(&h, MAC_SYS_CTRL, 0x0);
    let _ = reg_write(&h, AUTOWAKEUP_CFG, 0);
    let mut ok = 0;
    for (i, chunk) in fw[0..4096].chunks(64).enumerate() {
        let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
        let (v, idx) = encode_reg_addr(addr);
        if h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, std::time::Duration::from_millis(500)).is_ok() { ok += 1; }
    }
    reg_write(&h, H2M_MAILBOX_CID, !0u32).ok();
    reg_write(&h, H2M_MAILBOX_STATUS, !0u32).ok();
    h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_FIRMWARE, 0, &[], FIRMWARE_TIMEOUT).ok();
    std::thread::sleep(std::time::Duration::from_millis(300));
    reg_write(&h, H2M_MAILBOX_CSR, 0).ok();
    let _ = mcu_request(&h, MCU_BOOT_SIGNAL, 0, 0, 0);
    std::thread::sleep(std::time::Duration::from_millis(5));
    println!("firmware {ok}/64 + BOOT_SIGNAL");

    println!("\n== 4. RFCSR completo DESPUÉS del firmware (¿el MCU tocó el RF?) ==");
    print!("rfcsr: ");
    for r in 0u8..32 {
        let v = rfcsr_read(&h, r).unwrap_or(0xff);
        print!("{r}:{v:02x} ");
    }
    println!();

    println!("\n== 5. RFCSR write test: escribir rfcsr[2]=0x80 y releer ==");
    let before = rfcsr_read(&h, 2).unwrap_or(0xff);
    rfcsr_write(&h, 2, 0x80).ok();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let after = rfcsr_read(&h, 2).unwrap_or(0xff);
    println!("rfcsr[2]: antes={before:#04x} → tras write 0x80: {after:#04x} {}", if after == 0x80 { "✅ pega" } else { "❌ NO pega" });

    println!("\n== 6. BBP RW_MODE=1 ==");
    for intento in 0..3 {
        let v = bbp_read_raw(&h, 0, 1).unwrap_or(0xff);
        println!("  read bbp0 ({intento}): {v:#04x}");
        if v != 0x00 && v != 0xff { break; }
    }
    bbp_write_raw(&h, 1, 0x5a, 1).ok();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let v = bbp_read_raw(&h, 1, 1).unwrap_or(0xff);
    println!("  write bbp1=0x5a → read {v:#04x} {}", if v == 0x5a { "✅" } else { "❌" });

    println!("\n== 7. Estado final BBP_CSR_CFG ==");
    let v = reg_read(&h, BBP_CSR_CFG).unwrap_or(0);
    println!("BBP_CSR_CFG = {v:#010x}");
}
