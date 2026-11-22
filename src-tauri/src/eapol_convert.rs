// E0-24 fase B: convierte EAPOL-Key capturado en una interfaz ETHERNET (DLT 1)
// a una línea .22000 WPA*02 válida. El conversor histórico (pcap_convert.rs)
// espera 802.11+radiotap; aquí el auto-gatillo usa el RT73 en managed.
use serde::Serialize;
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone, Serialize)]
pub struct EapolConvertResult {
    pub success: bool,
    pub output_file: String,
    pub hashes: usize,
    pub message: String,
}

fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}

#[derive(Default)]
struct Hs {
    anonce: Option<[u8; 32]>,
    m1: Option<Vec<u8>>,
    m2: Option<Vec<u8>>,
}

/// Parsea el pcap ethernet y devuelve líneas hc22000 (una por STA).
pub fn convert_pcap(pcap_path: &str, ssid: &str, bssid: &[u8; 6]) -> Vec<String> {
    let bytes = match fs::read(pcap_path) { Ok(b) => b, Err(_) => return vec![] };
    if bytes.len() < 24 { return vec![]; }
    let magic = u32le(&bytes, 0).unwrap_or(0);
    let le = magic == 0xa1b2c3d4 || magic == 0xa1b23c4d;
    let rd32 = |o: usize| -> Option<u32> {
        let x = [*bytes.get(o)?, *bytes.get(o + 1)?, *bytes.get(o + 2)?, *bytes.get(o + 3)?];
        Some(if le { u32::from_le_bytes(x) } else { u32::from_be_bytes(x) })
    };
    let linktype = rd32(20).unwrap_or(1);
    if linktype != 1 && linktype != 127 { return vec![]; } // ethernet o 802.11+radiotap
    const LLC_SNAP: [u8; 8] = [0xaa, 0xaa, 0x03, 0x00, 0x00, 0x00, 0x88, 0x8e];

    let mut states: std::collections::BTreeMap<String, Hs> = std::collections::BTreeMap::new();
    let mut off = 24usize;
    while off + 16 <= bytes.len() {
        let incl = rd32(off + 8).unwrap_or(0) as usize;
        let start = off + 16;
        let end = match start.checked_add(incl) { Some(e) if e <= bytes.len() => e, _ => break };
        let p = &bytes[start..end];
        // Extraer (SA, DA, EAPOL) según el DLT del pcap.
        let parsed: Option<(Vec<u8>, Vec<u8>, &[u8])> = if linktype == 1 {
            if p.len() >= 14 && p[12] == 0x88 && p[13] == 0x8e {
                Some((p[6..12].to_vec(), p[0..6].to_vec(), &p[14..]))
            } else { None }
        } else if p.len() >= 8 {
            let rt = u16::from_le_bytes([p[2], p[3]]) as usize;
            if rt < 8 || rt > p.len() { None } else {
                let w = &p[rt..];
                if w.len() < 26 { None } else {
                    let fc = u16::from_le_bytes([w[0], w[1]]);
                    let ftype = (fc >> 2) & 0x03;
                    let fsub = (fc >> 4) & 0x0f;
                    if ftype != 2 || (fsub != 0 && fsub != 1) { None } else {
                        let mac = |o: usize| -> [u8; 6] {
                            let mut m = [0u8; 6];
                            if o + 6 <= w.len() { m.copy_from_slice(&w[o..o + 6]); }
                            m
                        };
                        let a1 = mac(4); let a2 = mac(10); let a3 = mac(16);
                        let to_ds = (fc >> 8) & 1 == 1;
                        let from_ds = (fc >> 9) & 1 == 1;
                        let (_da, sa) = match (to_ds, from_ds) {
                            (false, false) => (a1, a2),
                            (true, false) => (a3, a2),
                            (false, true) => (a1, a3),
                            (true, true) => (a3, [0; 6]),
                        };
                        let h = if (fc >> 15) & 1 == 1 { 26 } else { 24 };
                        if w.len() < h + 8 || w[h..h + 8] != LLC_SNAP { None } else {
                            Some((sa.to_vec(), _da.to_vec(), &w[h + 8..]))
                        }
                    }
                }
            }
        } else { None };

        if let Some((src, dst, eapol)) = parsed {
            let from_ap = src.as_slice() == bssid; // AP→STA: SA = BSSID (M1/M3)
            let to_ap = dst.as_slice() == bssid;   // STA→AP: DA = BSSID (M2/M4)
            if !from_ap && !to_ap { off = end; continue; }
            let sta = if from_ap { &dst } else { &src };
            if eapol.len() < 4 + 95 || eapol[0] != 1 || eapol[1] != 2 { off = end; continue; }
            let eapol_len = u16::from_be_bytes([eapol[2], eapol[3]]) as usize;
            if eapol.len() < 4 + eapol_len { off = end; continue; }
            let k = &eapol[4..];
            let key_info = u16::from_be_bytes([k[1], k[2]]);
            let install = key_info & 0x0040 != 0;
            let ack = key_info & 0x0080 != 0;
            let mic_ok = key_info & 0x0100 != 0;
            let pairwise = key_info & 0x0008 != 0;
            let full = eapol[..4 + eapol_len].to_vec();
            let st = states.entry(hex(sta)).or_default();
            if from_ap && !install && ack && !mic_ok && st.m1.is_none() {
                let mut n = [0u8; 32]; n.copy_from_slice(&k[13..45]);
                st.anonce = Some(n); st.m1 = Some(full);
            } else if to_ap && pairwise && mic_ok && !ack && st.m1.is_some() && st.m2.is_none() {
                st.m2 = Some(full);
            }
        }
        off = end;
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (sta_hex, hs) in states {
        let (Some(m2), Some(anonce)) = (hs.m2.as_ref(), hs.anonce.as_ref()) else { continue };
        let mic_off = 4 + 77;
        if m2.len() < mic_off + 16 { continue; }
        let mut blob = m2.clone();
        let orig_mic = blob[mic_off..mic_off + 16].to_vec();
        for b in &mut blob[mic_off..mic_off + 16] { *b = 0; }
        let ap = hex(bssid);
        let essid = hex(ssid.as_bytes());
        let line = format!("WPA*02*{}*{}*{}*{}*{}*{}*00",
            hex(&orig_mic), ap, sta_hex, essid, hex(anonce), hex(&blob));
        let dedupe = format!("{ap}|{sta_hex}|{}", hex(anonce));
        if seen.insert(dedupe) { out.push(line); }
    }
    out
}

#[tauri::command]
pub fn convert_ethernet_eapol(
    pcap_path: String,
    ssid: String,
    bssid: String,
    output_path: Option<String>,
) -> EapolConvertResult {
    let fail = |m: String| EapolConvertResult { success: false, output_file: String::new(), hashes: 0, message: m };
    let p: Vec<&str> = bssid.split([':', '-']).collect();
    if p.len() != 6 { return fail("BSSID inválido".into()); }
    let mut ap = [0u8; 6];
    for (i, x) in p.iter().enumerate() {
        match u8::from_str_radix(x, 16) { Ok(v) => ap[i] = v, Err(_) => return fail("BSSID inválido".into()) }
    }
    if ssid.trim().is_empty() { return fail("SSID requerido para el .22000".into()); }
    let lines = convert_pcap(&pcap_path, ssid.trim(), &ap);
    if lines.is_empty() { return fail("No se encontró un M1+M2 EAPOL válido en la captura ethernet.".into()); }
    let out = output_path.unwrap_or_else(|| format!("{}_eth.22000", std::path::Path::new(&pcap_path).file_stem().unwrap_or_default().to_string_lossy()));
    if fs::write(&out, lines.join("\n") + "\n").is_err() { return fail("No se pudo escribir el .22000".into()); }
    let n = fs::read_to_string(&out).map(|s| s.lines().filter(|l| l.starts_with("WPA*02")).count()).unwrap_or(0);
    EapolConvertResult { success: n > 0, output_file: out.clone(), hashes: n,
        message: format!("✅ {n} handshake(s) WPA*02 escritos en {out}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(src: [u8; 6], dst: [u8; 6], key_info: u16, nonce: u8) -> Vec<u8> {
        let mut p = vec![0u8; 14];
        p[0..6].copy_from_slice(&dst); p[6..12].copy_from_slice(&src);
        p[12] = 0x88; p[13] = 0x8e;
        p.extend_from_slice(&[1, 2, 0, 99]); // EAPOL header, len 99
        let mut k = vec![0u8; 99];
        k[0] = 254; // WPA2 key descriptor
        k[1..3].copy_from_slice(&key_info.to_be_bytes());
        for b in &mut k[13..45] { *b = nonce; }
        for b in &mut k[81..97] { *b = 0xAB; } // MIC presente
        p.extend_from_slice(&k);
        p
    }
    #[test]
    fn builds_22000_from_ethernet_m1_m2() {
        let ap = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        let sta = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        // M1: !install, ack, !mic ; M2: pairwise, mic, !ack
        let m1 = frame(ap, sta, 0x0008 | 0x0080, 0x11);
        let m2 = frame(sta, ap, 0x0008 | 0x0100, 0x22);
        let mut pcap = vec![0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
        for f in [m1, m2] {
            pcap.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
            pcap.extend_from_slice(&(f.len() as u32).to_le_bytes());
            pcap.extend_from_slice(&(f.len() as u32).to_le_bytes());
            pcap.extend_from_slice(&f);
        }
        let path = std::env::temp_dir().join("uifipill_eapol_eth_test.pcap");
        std::fs::write(&path, &pcap).unwrap();
        let lines = convert_pcap(path.to_str().unwrap(), "LABTEST", &ap);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("WPA*02*"));
        assert!(lines[0].contains("4c414254455354")); // SSID "LABTEST" en hex
        std::fs::remove_file(path).ok();
    }
}
