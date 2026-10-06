// Scan de redes por USB crudo (EP 0x81) — como rt2800usb en Linux pero en
// Windows: salta los canales 2.4GHz, recoge beacons/probe-responses y parsea
// SSID (IE 0), canal (IE 3 DS Parameter Set), seguridad (IE 48 RSN → WPA2/WPA3
// según AKM SAE, IE 221 OUI WPA → WPA), BSSID y RSSI (RXWI W2).
// Salida para la app (una línea por red, dedup por BSSID, mejor RSSI):
//   AP|<ssid>|<bssid>|<canal>|<rssi%>|<seguridad>
// Uso: rt3070_scan <segundos> [ruta rt2870.bin (ignorado: init_radio no usa fw)]
mod common;

use common::*;
use std::collections::HashMap;

// RSSI crudo (0-255 típico ~30..120) → % estilo netsh (-100dBm..-40dBm).
// Fórmula de rt2800stats: RSSI es un valor sin dBm; la app usa %.
fn rssi_pct(rssi: u8) -> u8 {
    let dbm = -(rssi as i32); // aproximación RT3070: valor ≈ |dBm|
    (((dbm + 100) as f32 / 60.0 * 100.0).clamp(2.0, 100.0)) as u8
}

/// RSN IE (tag 48): ¿alguna AKM suite es SAE (OUI 00:0F:AC tipo 8)? → WPA3.
/// Layout: ver(2) group(4) pcCount(1) pc[4n] acCount(1) ac[4m] …
fn rsn_is_sae(ie: &[u8]) -> bool {
    if ie.len() < 8 { return false; }
    let n = ie[6] as usize;
    let j = 7 + 4 * n; // índice del akm count
    if j >= ie.len() { return false; }
    let m = ie[j] as usize;
    let mut k = j + 1;
    for _ in 0..m {
        if k + 4 > ie.len() { return false; }
        if ie[k..k + 3] == [0x00, 0x0F, 0xAC] && ie[k + 3] == 8 { return true; }
        k += 4;
    }
    false
}

/// IE 221 (vendor): OUI WPA 00:50:F2 tipo 01 (WPA1).
fn is_wpa_vendor(body: &[u8]) -> bool {
    body.len() >= 4 && body[0..3] == [0x00, 0x50, 0xF2] && body[3] == 0x01
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.get(1).map(|s| s.parse().unwrap_or(20)).unwrap_or(20);
    let fw_path = args.get(2).cloned().unwrap_or_else(|| "rt2870.bin".into());

    let h = open_rt3070().unwrap_or_else(|e| {
        eprintln!("❌ abrir RT3070: {e} (¿rebindeado a WinUSB? ver run_zadig.cmd)");
        std::process::exit(1);
    });
    println!("✅ Device abierto");

    // init en canal inicial (firmware + radio ON + canal + PA)
    let log = init_radio(&h, 1, &fw_path).unwrap_or_else(|e| {
        eprintln!("❌ init: {e}");
        std::process::exit(2);
    });
    for l in &log {
        println!("✅ {l}");
    }

    // RX ya queda habilitado por init_radio (MAC_SYS_CTRL=0x0C + USB_DMA_CFG).
    // Modo monitor: aceptar control/not-to-me (beacons pasan igual, pero así
    // capturamos todo como un sniffer).
    let _ = rx_filter_monitor(&h);
    println!("🔍 Escaneando {secs}s por los canales 1-13 (USB crudo, como rt2800usb)…");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    // bssid → (ssid, canal, mejor rssi, seguridad)
    let mut nets: HashMap<[u8; 6], (String, u8, u8, String)> = HashMap::new();
    let mut buf = vec![0u8; 8192];
    let mut cur_ch: u8 = 1;
    let mut ch_since: std::time::Instant = std::time::Instant::now();
    const CH_DWELL_MS: u64 = 250; // por canal: beacons cada ~102ms, sobra
    let mut frame_count = 0usize;

    while std::time::Instant::now() < deadline {
        // channel hopping: 1..13
        if ch_since.elapsed().as_millis() as u64 >= CH_DWELL_MS {
            cur_ch = if cur_ch >= 13 { 1 } else { cur_ch + 1 };
            let _ = config_channel_rt3070(&h, cur_ch);
            ch_since = std::time::Instant::now();
        }

        match h.read_bulk(0x81, &mut buf, std::time::Duration::from_millis(50)) {
            Ok(n) if n > 0 => {
                // Layout MEDIDO (vendor.pcap): [4 len][RXWI 16][802.11 @20],
                // stride = len+8. El parser anterior ([4][32 rxwi], FC@36)
                // no encontraba nunca el frame → 0 redes.
                for f in walk_rx(&buf[..n]) {
                    frame_count += 1;
                    let body = if f.data.len() > 4 { &f.data[..f.data.len() - 4] } else { f.data };
                    parse_mgmt(body, f.rssi, cur_ch, &mut nets);
                }
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    }

    println!("\n📡 {frame_count} frames en {secs}s — {} redes:", nets.len());
    let mut list: Vec<_> = nets.iter().collect();
    list.sort_by_key(|(_, (_, _, r, _))| std::cmp::Reverse(*r));
    for (bssid, (ssid, ch, rssi, sec)) in &list {
        let mac: Vec<String> = bssid.iter().map(|b| format!("{b:02X}")).collect();
        println!("AP|{}|{}|{}|{}|{}", ssid, mac.join(":"), ch, rssi_pct(*rssi), sec);
    }
    if nets.is_empty() {
        println!("(sin redes — ¿chip vivo? usa rt3070_diag)");
    }
}

/// Parsea un frame management y lo añade al mapa de redes (dedup por BSSID).
fn parse_mgmt(frame: &[u8], rssi: u8, rx_ch: u8, nets: &mut HashMap<[u8; 6], (String, u8, u8, String)>) {
    if frame.len() < 24 { return; }
    let fc = u16::from_le_bytes([frame[0], frame[1]]);
    if fc & 0x03 != 0 { return; }                 // solo management
    let st = (fc >> 4) & 0xF;
    if st != 8 && st != 5 { return; }             // beacon(8) / probe-response(5)
    // ID v1: addr1=DA, addr2=SA(=BSSID en beacons), addr3=BSSID
    let bssid = [frame[10], frame[11], frame[12], frame[13], frame[14], frame[15]];
    // cuerpo: tras MAC header 24 bytes + fixed params 12 (beacon) → IEs
    let ie_off = if st == 8 { 36 } else { 36 };   // probe-resp: mismos 12 fixed
    if frame.len() < ie_off { return; }
    let mut ssid: Option<String> = None;
    let mut chan: u8 = rx_ch;                      // fallback: canal donde escuchamos
    let mut sec: &str = "";                        // "" = sin IE de cifrado visto
    let mut i = ie_off;
    while i + 2 <= frame.len() {
        let tag = frame[i];
        let len = frame[i + 1] as usize;
        if i + 2 + len > frame.len() { break; }
        if tag == 0 && len > 0 {
            // SSID: filtrar no-UTF8/controles
            if let Ok(s) = std::str::from_utf8(&frame[i + 2..i + 2 + len]) {
                if s.chars().all(|c| !c.is_control()) {
                    ssid = Some(s.to_string());
                }
            }
        } else if tag == 3 && len >= 1 {
            let c = frame[i + 2];
            if (1..=14).contains(&c) { chan = c; } // DS Parameter Set
        } else if tag == 48 && len >= 2 {
            // RSN: WPA3 si alguna AKM es SAE (00:0F:AC:08), si no WPA2
            sec = if rsn_is_sae(&frame[i + 2..i + 2 + len]) { "WPA3" } else { "WPA2" };
        } else if tag == 221 && is_wpa_vendor(&frame[i + 2..i + 2 + len]) {
            if sec.is_empty() { sec = "WPA"; }
        }
        if tag == 0 && ssid.is_some() && chan != rx_ch { break; }
        i += 2 + len;
    }
    // Sin IE de cifrado: honesto — o abierta o WEP (WEP no expone IE).
    if sec.is_empty() { sec = "Abierta o WEP"; }
    let e = nets.entry(bssid).or_insert_with(|| (String::new(), chan, 0, String::new()));
    if let Some(s) = ssid {
        if e.0.is_empty() { e.0 = s; }
    }
    if chan >= 1 && chan <= 14 { e.1 = chan; }     // prevalece el del IE
    if rssi > e.2 { e.2 = rssi; }                  // mejor señal vista
    if e.3.is_empty() { e.3 = sec.to_string(); }
}
