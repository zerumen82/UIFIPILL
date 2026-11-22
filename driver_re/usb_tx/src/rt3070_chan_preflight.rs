// E0-20 / fase 1: fija el canal del RT3070 (WinUSB) y lo lee de vuelta antes
// de cualquier ráfaga de deauth. NUNCA kick FIRMWARE(8) bajo WinUSB.
// Uso: rt3070_chan_preflight <canal 1..14>
// Salida: JSON {"ok":bool,"requested":u8,"actual":u8|null,"n":u8,"r":u8,"k":u8}
mod common;
use common::*;
use std::process::exit;

fn main() {
    let ch: u8 = match std::env::args().nth(1).and_then(|s| s.parse().ok()) {
        Some(c) if (1..=14).contains(&c) => c,
        _ => { eprintln!("Uso: rt3070_chan_preflight <canal 1..14>"); exit(2); }
    };
    let h = match open_rt3070() {
        Ok(h) => h,
        Err(e) => { println!("{{\"ok\":false,\"error\":\"open: {e}\"}}"); exit(1); }
    };
    if let Err(e) = config_channel_rt3070(&h, ch) {
        println!("{{\"ok\":false,\"error\":\"set_channel: {e}\"}}");
        exit(1);
    }
    std::thread::sleep(std::time::Duration::from_millis(60));
    let n = match rfcsr_read(&h, 2) { Ok(v) => v, Err(e) => { println!("{{\"ok\":false,\"error\":\"RFCSR2: {e}\"}}"); exit(1); } };
    let k = match rfcsr_read(&h, 3) { Ok(v) => v, Err(e) => { println!("{{\"ok\":false,\"error\":\"RFCSR3: {e}\"}}"); exit(1); } };
    let r = match rfcsr_read(&h, 6) { Ok(v) => v, Err(e) => { println!("{{\"ok\":false,\"error\":\"RFCSR6: {e}\"}}"); exit(1); } };
    let actual = channel_from_rf(n, r, k);
    let ok = actual == Some(ch);
    println!(
        "{{\"ok\":{},\"requested\":{},\"actual\":{},\"n\":{},\"r\":{},\"k\":{}}}",
        ok,
        ch,
        actual.map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
        n, r & 0x03, k & 0x0f
    );
    if !ok { exit(1); }
}
