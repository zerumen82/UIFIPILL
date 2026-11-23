// E0-25 fase B: extraer el PMKID de una (Re)Association Response 802.11
// capturada con el RT73 en monitor, y escribir una línea .22000 WPA*01 válida.
use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Serialize)]
pub struct AssocPmkidResult {
    pub success: bool,
    pub output_file: String,
    pub hashes: usize,
    pub message: String,
}

fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }
fn u16le(b: &[u8], o: usize) -> Option<u16> { Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?])) }
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}

/// Busca el primer PMKID no cero en el RSN IE de una Association Response.
pub fn pmkid_from_assoc(frame: &[u8], fc_off: usize, target: &[u8; 6]) -> Option<[u8; 16]> {
    if frame.len() < fc_off + 30 { return None; }
    let fc = u16le(frame, fc_off)?;
    let typ = (fc >> 2) & 0x03;
    let sub = (fc >> 4) & 0x0f;
    if typ != 0 || (sub != 1 && sub != 3) { return None; } // (Re)Assoc Response
    if frame.get(fc_off + 10..fc_off + 16)? != target { return None; } // SA = AP

    let mut p = fc_off + 30; // capability(2)+status(2)+aid(2) tras la cabecera 24
    while p + 2 <= frame.len() {
        let tag = frame[p];
        let len = frame[p + 1] as usize;
        let start = p + 2;
        let end = start.checked_add(len)?;
        if end > frame.len() { break; }
        if tag == 48 {
            // RSN IE: version2 + group4 + pairwise_count2 + n*4 + akm_count2 + m*4 + caps2 + pmkid_count2
            let mut q = start;
            q = q.checked_add(2 + 4)?;                       // version + group cipher
            let npair = u16le(frame, q)? as usize; q += 2;
            q = q.checked_add(npair.checked_mul(4)?)?;
            let nakm = u16le(frame, q)? as usize; q += 2;
            q = q.checked_add(nakm.checked_mul(4)?)?;
            q = q.checked_add(2)?;                           // RSN capabilities
            let npmkid = u16le(frame, q)? as usize; q += 2;
            for _ in 0..npmkid.min(4) {
                if q + 16 > end { break; }
                let mut k = [0u8; 16]; k.copy_from_slice(&frame[q..q + 16]);
                if k != [0u8; 16] { return Some(k); }
                q += 16;
            }
        }
        p = end;
    }
    None
}

pub fn extract(path: &str, ssid: &str, bssid: &[u8; 6]) -> Vec<String> {
    let bytes = match fs::read(path) { Ok(b) => b, Err(_) => return vec![] };
    if bytes.len() < 24 { return vec![]; }
    let magic = u32le(&bytes, 0).unwrap_or(0);
    let le = magic == 0xa1b2c3d4 || magic == 0xa1b23c4d;
    let rd32 = |o: usize| -> Option<u32> {
        let x = [*bytes.get(o)?, *bytes.get(o + 1)?, *bytes.get(o + 2)?, *bytes.get(o + 3)?];
        Some(if le { u32::from_le_bytes(x) } else { u32::from_be_bytes(x) })
    };
    let linktype = rd32(20).unwrap_or(127);
    let mut off = 24usize;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while off + 16 <= bytes.len() {
        let incl = rd32(off + 8).unwrap_or(0) as usize;
        let start = off + 16;
        let end = match start.checked_add(incl) { Some(e) if e <= bytes.len() => e, _ => break };
        let p = &bytes[start..end];
        let rt = if linktype == 127 {
            if p.len() < 4 { off = end; continue; }
            let r = u16::from_le_bytes([p[2], p[3]]) as usize;
            if r < 8 || r > p.len() { off = end; continue; }
            r
        } else { 0 };
        if let Some(pmkid) = pmkid_from_assoc(p, rt, bssid) {
            let sta = p.get(rt + 4..rt + 10).unwrap_or(&[0u8; 6]);
            let key = hex(&pmkid);
            if seen.insert(key.clone()) {
                out.push(format!("WPA*01*{}*{}*{}*{}***",
                    key, hex(bssid), hex(sta), hex(ssid.as_bytes())));
            }
        }
        off = end;
    }
    out
}

#[tauri::command]
pub fn convert_assoc_pmkid(
    pcap_path: String,
    ssid: String,
    bssid: String,
    output_path: Option<String>,
) -> AssocPmkidResult {
    let fail = |m: String| AssocPmkidResult { success: false, output_file: String::new(), hashes: 0, message: m };
    let p: Vec<&str> = bssid.split([':', '-']).collect();
    if p.len() != 6 { return fail("BSSID inválido".into()); }
    let mut ap = [0u8; 6];
    for (i, x) in p.iter().enumerate() {
        match u8::from_str_radix(x, 16) { Ok(v) => ap[i] = v, Err(_) => return fail("BSSID inválido".into()) }
    }
    if ssid.trim().is_empty() { return fail("SSID requerido".into()); }
    let lines = extract(&pcap_path, ssid.trim(), &ap);
    if lines.is_empty() { return fail("La Association Response no incluyó PMKID (o no se capturó).".into()); }
    let out = output_path.unwrap_or_else(|| format!("{}_pmkid.22000", std::path::Path::new(&pcap_path).file_stem().unwrap_or_default().to_string_lossy()));
    if fs::write(&out, lines.join("\n") + "\n").is_err() { return fail("No se pudo escribir el .22000".into()); }
    let n = fs::read_to_string(&out).map(|s| s.lines().filter(|l| l.starts_with("WPA*01")).count()).unwrap_or(0);
    AssocPmkidResult { success: n > 0, output_file: out.clone(), hashes: n,
        message: format!("✅ {n} PMKID real(es) escritos en {out}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_assoc_frames() {
        let ap = [0xaa,0xbb,0xcc,0xdd,0xee,0xff];
        let mut f = vec![0u8; 30];
        f[0] = 0x80; // beacon
        f[10..16].copy_from_slice(&ap);
        assert!(pmkid_from_assoc(&f, 0, &ap).is_none());
    }
    #[test]
    fn extracts_pmkid_from_assoc_resp() {
        let ap = [0xaa,0xbb,0xcc,0xdd,0xee,0xff];
        let mut body = vec![0u8; 30 + 2 + 42];
        body[0] = 0x10; // FC 0x0010 (subtype 1) en little-endian
        body[10..16].copy_from_slice(&ap);
        let mut ie = vec![48u8, 38];
        ie.extend_from_slice(&1u16.to_le_bytes());
        ie.extend_from_slice(&[0,0x0f,0xac,4]);
        ie.extend_from_slice(&1u16.to_le_bytes());
        ie.extend_from_slice(&[0,0x0f,0xac,4]);
        ie.extend_from_slice(&1u16.to_le_bytes());
        ie.extend_from_slice(&[0,0x0f,0xac,2]);
        ie.extend_from_slice(&0u16.to_le_bytes());
        ie.extend_from_slice(&1u16.to_le_bytes());
        ie.extend_from_slice(&[0x42; 16]);
        body[30..30+ie.len()].copy_from_slice(&ie);
        let got = pmkid_from_assoc(&body, 0, &ap).unwrap();
        assert_eq!(got, [0x42; 16]);
    }
}
