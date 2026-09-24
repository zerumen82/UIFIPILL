// Diagnóstico: RF type del EEPROM/EFUSE + volcado efuse + test BBP fino
mod common;
use common::*;

fn main() {
    let h = open_rt3070().expect("abrir RT3070");
    println!("== ASIC ==");
    match reg_read(&h, MAC_CSR0) {
        Ok(v) => println!("MAC_CSR0 = {v:#010x}"),
        Err(e) => println!("MAC_CSR0 err {e:?}"),
    }

    println!("\n== EFUSE ==");
    let present = efuse_present(&h);
    println!("EFUSE present: {present}");
    if present {
        for l in efuse_dump(&h, 64) {
            println!("{l}");
        }
    }

    println!("\n== EEPROM vendor (comparativa) ==");
    for off in [0x00u16, 0x02, 0x04, 0x1a, 0x1c, 0x1e] {
        let mut b = [0u8; 2];
        match h.read_control(REQ_IN, USB_EEPROM_READ, 0, off, &mut b, std::time::Duration::from_millis(500)) {
            Ok(_) => println!("eeprom[@{off:#04x}] = {:02x} {:02x} → {:#06x}", b[0], b[1], u16::from_le_bytes(b)),
            Err(e) => println!("eeprom[@{off:#04x}] err {e:?}"),
        }
    }

    println!("\n== BBP write/readback test ==");
    // Escribir un valor conocido en BBP reg 1 y leerlo de vuelta
    for (reg, val) in [(1u8, 0x11u8), (62, 0x37), (4, 0x00)] {
        match (bbp_write(&h, reg, val), bbp_read(&h, reg)) {
            (Ok(_), Ok(v)) => println!("bbp[{reg}]: write 0x{val:02x} → read 0x{v:02x} {}", if v == val { "✅ pega" } else { "❌ NO pega" }),
            (w, r) => println!("bbp[{reg}]: write {w:?} read {r:?}"),
        }
    }

    println!("\n== RFCSR readback test ==");
    for reg in [0u8, 1, 3, 30] {
        match rfcsr_read(&h, reg) {
            Ok(v) => println!("rfcsr[{reg}] = 0x{v:02x}"),
            Err(e) => println!("rfcsr[{reg}] err {e:?}"),
        }
    }
}
