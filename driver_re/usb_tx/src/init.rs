// Init completo del RT3070 por USB crudo (puerto del camino rt2800usb):
// reset → firmware → radio ON → canal → modo TX/RX.
// Uso: rt3070_init <canal 1-14> [ruta rt2870.bin]
mod common;

use common::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let channel: u8 = args.get(1).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);
    let fw_path = args.get(2).cloned().unwrap_or_else(|| "rt2870.bin".into());

    // Refactor 2026-09-23: la secuencia completa vive en common::init_radio
    // (compartida con rt3070_scan). Los pasos detallados se documentan ahí.
    let h = open_rt3070().expect("abrir RT3070 (ver probe)");
    let log = init_radio(&h, channel, &fw_path).unwrap_or_else(|e| {
        eprintln!("❌ init: {e}");
        std::process::exit(1);
    });
    for l in &log {
        println!("✅ {l}");
    }
    println!("\n➡️  Siguiente: rt3070_tx para lanzar beacons y verificar con netsh en la otra radio");
}
