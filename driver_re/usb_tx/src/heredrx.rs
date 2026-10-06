// HEREDRX: RX sobre el estado heredado del vendor SIN tocar BBP ni MCU.
//
// Idea: el chip del vendor viene con su init completo (BBP corriendo para él,
// canal del vendor). Solo nos faltan las piezas USB-side que el vendor apaga al
// detach, y que Linux enciende en rt2800usb_enable_radio:
//   1. USB_DMA_CFG = RX|TX_BULK_EN + AGG como Linux (el heredado suele venir a 0)
//   2. MAC_SYS_CTRL ENABLE_RX (ya suele venir)
//   3. RX_FILTER_CFG monitor
//   4. leer EP 0x81
// SIN bbp_read/write, SIN rfcsr, SIN mailbox, SIN resets. Nada que pueda
// molestar al estado del vendor.
mod common;

use common::*;
use std::time::Duration;
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);

    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[pre] MAC_CSR0 = {csr0:#010x}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo — power-cycle antes");
        std::process::exit(1);
    }

    // Estado heredado relevante (solo lectura, diagnóstico)
    let msc = reg_read(&h, MAC_SYS_CTRL).unwrap_or(0);
    let dma = reg_read(&h, USB_DMA_CFG).unwrap_or(0);
    println!("[heredado] MAC_SYS_CTRL={msc:#010x} USB_DMA_CFG={dma:#010x}");

    // 1. USB_DMA_CFG como rt2800usb_enable_radio (AGG_EN=0, TIMEOUT=128,
    //    LIMIT=301, RX|TX_BULK_EN) — el valor medido-bueno del hito 2026-09-23.
    reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE).expect("USB_DMA_CFG");
    let dma2 = reg_read(&h, USB_DMA_CFG).unwrap_or(0);
    println!("[set] USB_DMA_CFG = {dma2:#010x}");

    // 2. ENABLE_RX (OR, sin tocar lo demás)
    if msc & 0x08 == 0 {
        reg_write(&h, MAC_SYS_CTRL, msc | 0x08).unwrap();
        println!("[set] MAC_SYS_CTRL |= ENABLE_RX");
    } else {
        println!("[heredado] ENABLE_RX ya activo");
    }

    // 3. filtro monitor
    let _ = rx_filter_monitor(&h);
    println!("[set] RX_FILTER_CFG monitor");

    // 4. RX
    println!("\n== RX {secs}s EP 0x81 (estado heredado intacto) ==");
    let mut frames = 0usize;
    let mut nets: HashMap<[u8; 6], (String, u8, u8)> = HashMap::new();
    let mut buf = vec![0u8; 8192];
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, Duration::from_millis(100)) {
            Ok(n) if n > 0 => {
                // Layout MEDIDO (vendor.pcap): [4 len][RXWI 16][802.11 @20],
                // stride = len+8. El parser anterior ([4][32 rxwi]) no
                // encontraba nunca el frame → "0 frames" era un falso negativo.
                for f in walk_rx(&buf[..n]) {
                    frames += 1;
                    if let Some((ssid, ch, bssid)) = parse_beacon(f.data) {
                        let rssi = (f.rssi as i32) - 110;
                        nets.insert(bssid, (ssid, ch, rssi.clamp(0, 100) as u8));
                    }
                }
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => { println!("  read err {e:?}"); break; }
        }
    }

    println!("\n📡 {frames} frames en {secs}s — {}", if frames > 0 {
        "🎉 ¡RX FUNCIONA SOBRE EL ESTADO HEREDADO!"
    } else {
        "0 frames"
    });
    if !nets.is_empty() {
        println!("\nAPs vistos:");
        let mut list: Vec<_> = nets.iter().collect();
        list.sort_by_key(|(_, (_, _, r))| std::cmp::Reverse(*r));
        for (bssid, (ssid, ch, rssi)) in &list {
            let mac: Vec<String> = bssid.iter().map(|b| format!("{b:02X}")).collect();
            println!("AP|{}|{}|ch{}|rssi{}", ssid, mac.join(":"), ch, rssi);
        }
    }
}
