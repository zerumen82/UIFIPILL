// E0-20 fase 1: gate MFPC/MFPR (802.11w) antes de cualquier deauth.
// Lee un .pcap clásico con radiotap (DLT 127) y extrae del RSN IE (tag 48)
// las capabilities: bit6=MFPR (obligatorio), bit7=MFPC (opcional).
// Fail-closed: si no hay beacon del BSSID o no hay RSN IE, MFPR/MFPC = None
// y el frontend debe BLOQUEAR el ataque con mensaje accionable (usar Kali).
use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PmfInfo {
    pub success: bool,
    pub bssid: String,
    /// Some(true) = MFPR activo: el deauth sin protección se ignora.
    pub mfpr: Option<bool>,
    /// Some(true) = MFPC activo (PMF opcional).
    pub mfpc: Option<bool>,
    /// AKM 00-0F-AC:8 detectado (WPA3/SAE).
    pub sae: bool,
    /// AKM 00-0F-AC:2 detectado (WPA2-PSK).
    pub psk: bool,
    pub beacons: usize,
    pub error: Option<String>,
}

fn u16le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn u32_any(b: &[u8], o: usize, le: bool) -> Option<u32> {
    let x = [*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?];
    Some(if le { u32::from_le_bytes(x) } else { u32::from_be_bytes(x) })
}

/// Parsea un RSN IE (tag 48, sin tag/len). Devuelve (mfpc, mfpr, sae, psk).
pub fn parse_rsn_ie(ie: &[u8]) -> Option<(bool, bool, bool, bool)> {
    let _version = u16le(ie, 0)?;
    let pairwise_count = u16le(ie, 6)? as usize;
    let mut off = 8usize.checked_add(pairwise_count.checked_mul(4)?)?;
    let akm_count = u16le(ie, off)? as usize;
    off = off.checked_add(2)?.checked_add(akm_count.checked_mul(4)?)?;
    let cap = u16le(ie, off)?;
    let mut sae = false;
    let mut psk = false;
    let mut akm_off = 8usize.checked_add(pairwise_count.checked_mul(4)?)?.checked_add(2)?;
    for _ in 0..akm_count {
        let suite = ie.get(akm_off..akm_off + 4)?;
        if suite[0..3] == [0x00, 0x0f, 0xac] {
            if suite[3] == 0x08 { sae = true; }
            if suite[3] == 0x02 { psk = true; }
        }
        akm_off += 4;
    }
    Some(((cap >> 7) & 1 == 1, (cap >> 6) & 1 == 1, sae, psk))
}

/// ¿Es un beacon/probe-response del BSSID buscado?
pub fn is_target_beacon(frame: &[u8], target: &[u8; 6]) -> bool {
    if frame.len() < 36 { return false; }
    let Some(fc) = u16le(frame, 0) else { return false; };
    let typ = (fc >> 2) & 0x03;
    let subtype = (fc >> 4) & 0x0f;
    if typ != 0 || (subtype != 8 && subtype != 5) { return false; }
    frame.get(16..22) == Some(target) // addr3 = BSSID
}

/// Extrae el RSN IE de un beacon/probe-response 802.11 (sin radiotap).
pub fn parse_beacon_rsn(frame: &[u8], target: &[u8; 6]) -> Option<(bool, bool, bool, bool)> {
    if !is_target_beacon(frame, target) { return None; }
    let mut off = 36usize;                                          // body tras IEs fijos
    while off + 2 <= frame.len() {
        let tag = frame[off];
        let len = frame[off + 1] as usize;
        let start = off + 2;
        let end = start.checked_add(len)?;
        if end > frame.len() { break; }
        if tag == 48 {
            if let Some(r) = parse_rsn_ie(&frame[start..end]) { return Some(r); }
        }
        off = end;
    }
    None
}

/// Parsea el .pcap y devuelve el veredicto para el BSSID.
pub fn parse_pmf_file(path: &str, bssid: &str) -> PmfInfo {
    let mut info = PmfInfo {
        success: false, bssid: bssid.to_string(), mfpr: None, mfpc: None,
        sae: false, psk: false, beacons: 0, error: None,
    };
    let target = match parse_mac(bssid) {
        Some(m) => m,
        None => { info.error = Some("BSSID inválido".into()); return info; }
    };
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => { info.error = Some(format!("No se pudo leer {path}: {e}")); return info; }
    };
    if bytes.len() < 24 { info.error = Some("pcap demasiado corto".into()); return info; }
    let magic = u32_any(&bytes, 0, true).unwrap_or(0);
    let (le, ok_magic) = match magic {
        0xa1b2c3d4 => (true, true),
        0xd4c3b2a1 => (false, true),
        0xa1b23c4d => (true, true),
        0x4d3cb2a1 => (false, true),
        _ => (true, false),
    };
    if !ok_magic { info.error = Some("No es un pcap clásico".into()); return info; }
    let linktype = u32_any(&bytes, 20, le).unwrap_or(0);
    let mut off = 24usize;
    while off + 16 <= bytes.len() {
        let incl = u32_any(&bytes, off + 8, le).unwrap_or(0) as usize;
        let start = off + 16;
        let end = match start.checked_add(incl) { Some(e) if e <= bytes.len() => e, _ => break };
        let pkt = &bytes[start..end];
        // Radiotap: version, pad, len(u16 LE). DLT 105 es 802.11 sin radiotap.
        let wifi = if linktype == 127 && pkt.len() >= 8 {
            let rt_len = u16le(pkt, 2).unwrap_or(0) as usize;
            if rt_len < 8 || rt_len > pkt.len() { off = end; continue; }
            &pkt[rt_len..]
        } else if linktype == 105 {
            pkt
        } else { off = end; continue };
        if is_target_beacon(wifi, &target) { info.beacons += 1; }
        if info.mfpr.is_none() {
            if let Some((mfpc, mfpr, sae, psk)) = parse_beacon_rsn(wifi, &target) {
                info.mfpc = Some(mfpc); info.mfpr = Some(mfpr);
                info.sae = sae; info.psk = psk;
            }
        }
        off = end;
    }
    info.success = info.mfpr.is_some();
    if info.mfpr.is_none() {
        info.error = Some(if info.beacons == 0 {
            "No se vieron beacons del BSSID en la captura; reintenta con más tiempo o comprueba el canal del monitor.".into()
        } else {
            "Los beacons del BSSID no incluyen RSN IE; no se puede confirmar MFPC/MFPR. Fail-closed: usar Kali.".into()
        });
    }
    info
}

fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = s.split([':', '-']).collect();
    if parts.len() != 6 { return None; }
    let mut out = [0u8; 6];
    for (i, p) in parts.iter().enumerate() { out[i] = u8::from_str_radix(p, 16).ok()?; }
    Some(out)
}

#[tauri::command]
pub fn parse_pmf(pcap_path: String, bssid: String) -> PmfInfo {
    parse_pmf_file(&pcap_path, &bssid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beacon(bssid: &[u8; 6], rsn: Option<&[u8]>, subtype: u8) -> Vec<u8> {
        let mut f = vec![0u8; 36];
        f[0] = subtype << 4; // mgmt: subtype 8 = 0x80, probe resp 5 = 0x50
        f[16..22].copy_from_slice(bssid);
        if let Some(ie) = rsn {
            f.push(48); f.push(ie.len() as u8); f.extend_from_slice(ie);
        }
        f
    }
    fn rsn(cap: u16, akms: &[u8]) -> Vec<u8> {
        let mut r = vec![0u8; 8]; // version + group + pairwise count(0)
        r.extend_from_slice(&(akms.len() as u16).to_le_bytes());
        for a in akms { r.extend_from_slice(&[0x00, 0x0f, 0xac, *a]); }
        r.extend_from_slice(&cap.to_le_bytes());
        r
    }
    fn pcap(pkts: &[Vec<u8>]) -> Vec<u8> {
        let mut b = vec![0xd4, 0xc3, 0xb2, 0xa1, 0x02, 0x00, 0x04, 0x00,
                         0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0x00, 0x00, 0x7f, 0, 0, 0];
        for p in pkts {
            b.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
            b.extend_from_slice(&(p.len() as u32).to_le_bytes());
            b.extend_from_slice(&(p.len() as u32).to_le_bytes());
            b.extend_from_slice(p);
        }
        b
    }
    fn with_rt(frame: &[u8]) -> Vec<u8> {
        let mut p = vec![0, 0, 8, 0, 0, 0, 0, 0];
        p.extend_from_slice(frame);
        p
    }

    #[test]
    fn parses_rsn_capabilities_and_akms() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        // MFPR(bit6)=1 + MFPC(bit7)=1, PSK(2) + SAE(8)
        let cap = (1 << 6) | (1 << 7);
        let frame = beacon(&mac, Some(&rsn(cap, &[0x02, 0x08])), 8);
        let bytes = pcap(&[with_rt(&frame)]);
        let path = std::env::temp_dir().join("uifipill_pmf_test.pcap");
        std::fs::write(&path, &bytes).unwrap();
        let r = parse_pmf_file(path.to_str().unwrap(), "AA:BB:CC:DD:EE:FF");
        assert_eq!(r.mfpr, Some(true));
        assert_eq!(r.mfpc, Some(true));
        assert!(r.psk && r.sae);
        assert_eq!(r.beacons, 1);
        assert!(r.success);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn optional_pmf_and_no_rsn_fail_closed() {
        let mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let cap = 1 << 7; // MFPC only
        let frame = beacon(&mac, Some(&rsn(cap, &[0x02])), 5);
        let bytes = pcap(&[with_rt(&frame)]);
        let path = std::env::temp_dir().join("uifipill_pmf_test2.pcap");
        std::fs::write(&path, &bytes).unwrap();
        let r = parse_pmf_file(path.to_str().unwrap(), "01:02:03:04:05:06");
        assert_eq!(r.mfpc, Some(true));
        assert_eq!(r.mfpr, Some(false));
        assert!(r.psk && !r.sae);
        let bytes2 = pcap(&[with_rt(&beacon(&mac, None, 8))]);
        std::fs::write(&path, &bytes2).unwrap();
        let r2 = parse_pmf_file(path.to_str().unwrap(), "01:02:03:04:05:06");
        assert_eq!(r2.mfpr, None);
        assert_eq!(r2.beacons, 1);
        assert!(!r2.success);
        std::fs::remove_file(path).ok();
    }
}
