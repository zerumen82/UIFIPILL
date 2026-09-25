// E2 del RX_PLAN.md: ciclo MCU_SLEEP → MCU_WAKEUP completo.
// H1: el vendor durmió el BBP al detach. Si el firmware requiere el ciclo
// completo (SLEEP con sus args exactos y LUEGO WAKEUP) para re-armar el BBP,
// esto lo despierta. No destructivo (peor caso: power-cycle).
//
// Variantes probadas en una pasada:
//   A: SLEEP(0xff,0xff,2) → WAKEUP(0xff,0,2)     [rt2800usb_set_state exacto]
//   B: SLEEP(0xff,0xff,2) → WAKEUP(0xff,0,0)
//   C: WAKEUP doble
// Tras cada variante: lectura bbp[0] (vivo si != 0x00/0xff).
mod common;

use common::*;

const MCU_SLEEP: u8 = 0x32; // rt2800.h (SLEEP=0x32, WAKEUP=0x31)

fn bbp_alive(h: &rusb::DeviceHandle<rusb::Context>) -> Option<u8> {
    let v = bbp_read(h, 0).unwrap_or(0xff);
    if v != 0x00 && v != 0xff { Some(v) } else { None }
}

fn main() {
    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[pre] MAC_CSR0 = {csr0:#010x}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo — power-cycle antes");
        std::process::exit(1);
    }

    let b0 = bbp_read(&h, 0).unwrap_or(0xff);
    println!("[pre] bbp[0] = {b0:#04x} ({})", if b0 == 0x00 || b0 == 0xff { "mudo" } else { "VIVO" });

    // ── Variante A: ciclo exacto de rt2800usb_set_state ──
    println!("\n[A] SLEEP(0xff,0xff,2) → WAKEUP(0xff,0,2)");
    match mcu_request_wait(&h, MCU_SLEEP, 0xff, 0xff, 2, 1000) {
        Ok(true) => println!("  SLEEP consumido"),
        _ => println!("  SLEEP sin confirmación"),
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
    match mcu_request_wait(&h, MCU_WAKEUP, 0xff, 0, 2, 1000) {
        Ok(true) => println!("  WAKEUP consumido"),
        _ => println!("  WAKEUP sin confirmación"),
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
    match bbp_alive(&h) {
        Some(v) => println!("  ✅ bbp[0] = {v:#04x} ¡VIVO! — EL CICLO DESPIERTA EL BBP"),
        None => println!("  ❌ bbp[0] sigue mudo"),
    }

    // ── Variante B: WAKEUP con arg1=0 ──
    println!("\n[B] SLEEP → WAKEUP(0xff,0,0)");
    let _ = mcu_request_wait(&h, MCU_SLEEP, 0xff, 0xff, 2, 1000);
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = mcu_request_wait(&h, MCU_WAKEUP, 0xff, 0, 0, 1000);
    std::thread::sleep(std::time::Duration::from_millis(10));
    match bbp_alive(&h) {
        Some(v) => println!("  ✅ bbp[0] = {v:#04x} ¡VIVO!"),
        None => println!("  ❌ mudo"),
    }

    // ── Variante C: WAKEUP doble ──
    println!("\n[C] WAKEUP ×2");
    let _ = mcu_request_wait(&h, MCU_WAKEUP, 0xff, 0, 2, 1000);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let _ = mcu_request_wait(&h, MCU_WAKEUP, 0xff, 0, 2, 1000);
    std::thread::sleep(std::time::Duration::from_millis(10));
    match bbp_alive(&h) {
        Some(v) => println!("  ✅ bbp[0] = {v:#04x} ¡VIVO!"),
        None => println!("  ❌ mudo"),
    }

    // ── Estado final del MCU ──
    let st = reg_read(&h, H2M_MAILBOX_STATUS).unwrap_or(0xffff_ffff);
    println!("\n[fin] MAILBOX_STATUS = {st:#010x}");
    println!("(sin kick — chip intacto salvo ciclo sleep/wake)");
}
