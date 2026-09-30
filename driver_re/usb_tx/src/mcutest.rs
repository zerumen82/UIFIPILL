// Test decisivo NO destructivo: ¿el MCU 8051 está corriendo cuando el CSR responde?
//
// Si el MCU corre → el chip está en el estado "post-vendor" y el kick de nuestra
// carga de fw compite con un firmware vivo (explicaría los modos de fallo A/B).
// Si el MCU NO corre (CSR vivo pero MCU parado) → el kick es correcto pero el
// arranque falla por otra causa.
//
// Pruebas: (1) MCU_CURRENT (0x30) con token — el MCU debe consumir OWNER.
//          (2) Lectura del mailbox tras el comando (respuesta del MCU).
//          (3) AUTOLOAD: DeviceID register (0x1000) vs MAC_CSR0.
// Sin kick, sin writes de fw — el chip queda intacto.
mod common;

use common::*;

fn main() {
    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[pre] MAC_CSR0 = {csr0:#010x}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo — power-cycle antes");
        std::process::exit(1);
    }

    // (1) MCU_CURRENT: rt2800_mcu_request clásico. Si el MCU corre, consume OWNER
    //     y deja respuesta en el mailbox status.
    println!("\n[MCU_CURRENT x3]");
    for i in 1..=3 {
        match mcu_request_wait(&h, MCU_CURRENT, 0xff, 0, 0, 1500) {
            Ok(true) => println!("  #{i} ✅ CONSUMIDO (el MCU ejecuta instrucciones)"),
            Ok(false) => println!("  #{i} ⚠️ no consumido (MCU parado o sin firmware)"),
            Err(e) => println!("  #{i} ❌ err {e}"),
        }
    }

    // (2) MAILBOX después: el MCU deja token/estado
    let mb = reg_read(&h, H2M_MAILBOX_CSR).unwrap_or(0xffff_ffff);
    let st = reg_read(&h, H2M_MAILBOX_STATUS).unwrap_or(0xffff_ffff);
    println!("\n[mailbox] CSR={mb:#010x} STATUS={st:#010x}");

    // (3) BOOT_SIGNAL también (el que usamos en enable_radio)
    println!("\n[MCU_BOOT_SIGNAL]");
    match mcu_request_wait(&h, MCU_BOOT_SIGNAL, 0, 0, 0, 1500) {
        Ok(true) => println!("  ✅ consumido"),
        Ok(false) => println!("  ⚠️ no consumido"),
        Err(e) => println!("  ❌ err {e}"),
    }

    // (4) PBF_SYS_CTRL: bit6 PBF_UIO? bit7 READY. En el boot real se usa para ver estado
    let pbf = reg_read(&h, PBF_SYS_CTRL).unwrap_or(0xffff_ffff);
    println!("\n[PBF_SYS_CTRL] = {pbf:#010x} (bit7 READY={})", (pbf >> 7) & 1);

    println!("\n(sin kick — chip intacto)");
}
