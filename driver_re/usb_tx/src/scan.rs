// Scan de redes por USB crudo (EP 0x81) — como rt2800usb en Linux pero en
// Windows: salta los canales 2.4GHz, recoge beacons/probe-responses y parsea
// SSID (IE 0), canal (IE 3 DS Parameter Set), BSSID y RSSI (RXWI W2).
// Salida para la app (una línea por red, dedup por BSSID, mejor RSSI):
//   AP|<ssid>|<bssid>|<canal>|<rssi%>
// Uso: rt3070_scan <segundos> [ruta rt2870.bin]
mod common;

use common::*;
use std::collections::HashMap;

// RSSI crudo (0-255 típico ~30..120) → % estilo netsh (-100dBm..-40dBm).
// Fórmula de rt2800stats: RSSI es un valor sin dBm; la app usa %.
fn rssi_pct(rssi: u8) -> u8 {
    let dbm = -(rssi as i32); // aproximación RT3070: valor ≈ |dBm|
    (((dbm + 100) as f32 / 60.0 * 100.0).clamp(2.0, 100.0)) as u8
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
    // bssid → (ssid, canal, mejor rssi)
    let mut nets: HashMap<[u8; 6], (String, u8, u8)> = HashMap::new();
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
            Ok(n) if n >= 36 => {
                let mut off = 0usize;
                // Agregación USB: [dma len le32][RXWI 32B][802.11]… alineado a 4
                while off + 36 <= n {
                    let dma = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap()) as usize;
                    if dma < 36 || dma > 4096 { break; }
                    if off + dma > n { break; }
                    let wi = off + 4;              // RXWI 32B
                    let fs = wi + 32;              // 802.11 frame
                    // RXWI_W0_MPDU_TOTAL_BYTE_COUNT bits 16-27 = frame+FCS
                    let w0 = u32::from_le_bytes(buf[wi..wi + 4].try_into().unwrap());
                    let mpdu = ((w0 >> 16) & 0x0fff) as usize;
                    if mpdu < 26 || fs + mpdu > n { break; } // sin cabecera 802.11 mínima
                    let frame = &buf[fs..fs + mpdu.saturating_sub(4)]; // sin FCS
                    frame_count += 1;

                    // RSSI: RXWI_W2_RSSI0 bits 0-7
                    let w2 = u32::from_le_bytes(buf[wi + 8..wi + 12].try_into().unwrap());
                    let rssi = (w2 & 0xff) as u8;

                    parse_mgmt(frame, rssi, cur_ch, &mut nets);
                    off += (dma + 3) & !3;
                }
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    }

    println!("\n📡 {frame_count} frames en {secs}s — {} redes:", nets.len());
    let mut list: Vec<_> = nets.iter().collect();
    list.sort_by_key(|(_, (_, _, r))| std::cmp::Reverse(*r));
    for (bssid, (ssid, ch, rssi)) in &list {
        let mac: Vec<String> = bssid.iter().map(|b| format!("{b:02X}")).collect();
        println!("AP|{}|{}|{}|{}", ssid, mac.join(":"), ch, rssi_pct(*rssi));
    }
    if nets.is_empty() {
        println!("(sin redes — ¿chip vivo? usa rt3070_diag)");
    }
}

/// Parsea un frame management y lo añade al mapa de redes (dedup por BSSID).
fn parse_mgmt(frame: &[u8], rssi: u8, rx_ch: u8, nets: &mut HashMap<[u8; 6], (String, u8, u8)>) {
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
        }
        if tag == 0 && ssid.is_some() && chan != rx_ch { break; }
        i += 2 + len;
    }
    let e = nets.entry(bssid).or_insert_with(|| (String::new(), chan, 0));
    if let Some(s) = ssid {
        if e.0.is_empty() { e.0 = s; }
    }
    if chan >= 1 && chan <= 14 { e.1 = chan; }     // prevalece el del IE
    if rssi > e.2 { e.2 = rssi; }                  // mejor señal vista
}
