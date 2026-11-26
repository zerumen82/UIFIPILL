// E0-30: detección de clientes (STAs) asociados a un BSSID, a partir de una
// captura 802.11 en modo monitor. Solo lectura: no transmite nada.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ClientScanResult {
    pub success: bool,
    pub clients: Vec<String>,
    pub packets: u64,
    pub message: String,
}

fn mac_str(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(":")
}

/// Extrae MACs de tramas Data/QoS cuyo BSSID aparece en addr1/addr2/addr3.
/// Devuelve (clientes que pasan el umbral, número de STAs candidatos totales
/// y número de tramas Data con el BSSID). Sin inventar nada si no hay tramas.
#[allow(dead_code)]
pub fn clients_from_pcap(bytes: &[u8], bssid: &[u8; 6]) -> Vec<String> {
    let (out, _, _) = clients_from_pcap_full(bytes, bssid);
    out
}

pub fn clients_from_pcap_full(bytes: &[u8], bssid: &[u8; 6]) -> (Vec<String>, usize, usize) {
    let ap: &[u8] = bssid; // slice para comparas de prefijo seguras
    if bytes.len() < 24 { return (Vec::new(), 0, 0); }
    let le = bytes[0] == 0xd4;
    let rd32 = |o: usize| -> Option<u32> {
        let x = [*bytes.get(o)?, *bytes.get(o+1)?, *bytes.get(o+2)?, *bytes.get(o+3)?];
        Some(if le { u32::from_le_bytes(x) } else { u32::from_be_bytes(x) })
    };
    let dlt = rd32(20).unwrap_or(127);
    if dlt != 127 && dlt != 1 { return (Vec::new(), 0, 0); }
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut data_frames = 0usize;
    let mut off = 24usize;
    while off + 16 <= bytes.len() {
        let incl = rd32(off + 8).unwrap_or(0) as usize;
        let start = off + 16;
        let end = match start.checked_add(incl) { Some(e) if e <= bytes.len() => e, _ => break };
        let p = &bytes[start..end];
        let w = if dlt == 127 {
            if p.len() < 10 { off = end; continue; }
            let rt = u16::from_le_bytes([p[2], p[3]]) as usize;
            if rt < 8 || rt > p.len() { off = end; continue; }
            &p[rt..]
        } else { p };
        if w.len() < 24 { off = end; continue; }
        let fc = u16::from_le_bytes([w[0], w[1]]);
        let ftype = (fc >> 2) & 3;
        let fsub = (fc >> 4) & 15;
        let a1 = &w[4..10];
        let a2 = &w[10..16];
        let a3 = &w[16..22];
        // (Re)Association Request del STA al AP también prueba cliente real,
        // aunque aún no haya enviado datos (aca de asociarse).
        if ftype == 0 && (fsub == 0 || fsub == 2) && a1 == bssid && a2 != bssid {
            let s = mac_str(a2);
            *counts.entry(s).or_insert(0) += 1;
            off = end;
            continue;
        }
        // FIX 2026-11-26: un móvil CONECTADO pero en idle envía frames
        // Null (subtype 4) / QoS Null (subtype 12) para power-save (PS-Poll),
        // no Data (0/8). Antes se descartaban → "0 STAs candidatos" aunque
        // hubiera un cliente real asociado. Ahora también cuentan.
        if ftype != 2 || (fsub != 0 && fsub != 4 && fsub != 8 && fsub != 12) { off = end; continue; }
        let to_ds = (fc >> 8) & 1 == 1;
        let from_ds = (fc >> 9) & 1 == 1;
        // Solo tramas verdaderamente asociadas (STA↔AP): exactamente uno de
        // ToDS/FromDS. Descarta WDS/mesh y frames de otros APs con addr3 igual.
        if to_ds == from_ds { off = end; continue; }
        if a1 != bssid && a2 != bssid && a3 != bssid { off = end; continue; }
        data_frames += 1;
        // Dirección del STA, MEDIDA 2026-11-26 contra pcaps reales:
        //   ToDS=1:   a1=BSSID(AP), a2=STA, a3=DA (destino cableado/multicast)
        //   FromDS=1: a1=DA(STA), a2=BSSID(AP), a3=SA (origen cableado)
        // Antes se contaban a1/a2/a3 y salían como "clientes" hosts cableados
        // (p. ej. 48:22:54:88:29:D6: era el SA de downlink, no un STA) mientras
        // el deauth iba dirigido a ese MAC inútil.
        let sta: Option<&[u8]> = if to_ds {
            if a1 == bssid && a2 != bssid { Some(a2) } else { None }
        } else {
            if a2 == bssid && a1 != bssid { Some(a1) } else { None }
        };
        if let Some(m) = sta {
            if m.iter().all(|x| *x == 0) || m.iter().all(|x| *x == 0xff) { off = end; continue; }
            // Multicast/broadcast NO son clientes (p. ej. 33:33:xx = IPv6).
            if (m[0] & 0x01) != 0 { off = end; continue; }
            // Otra BSS del MISMO router (mismo OUI+prefijo, solo cambia el
            // último byte) no es un cliente: p. ej. sagemcomDEF0 vs _Plus.
            if m.len() >= 5 && m[..5] == ap[..5] { off = end; continue; }
            let s = mac_str(m);
            if s.split(':').count() == 6 {
                *counts.entry(s).or_insert(0) += 1;
            }
        }
        off = end;
    }
    let candidate_stas = counts.len();
    // Un solo frame suelto no prueba asociación: exigir varias tramas del
    // mismo STA hacia el AP para no cazar dispositivos transitorios.
    let mut out: Vec<String> = counts.into_iter()
        .filter(|(_, n)| *n >= 3)
        .map(|(m, _)| m)
        .collect();
    out.sort();
    (out, candidate_stas, data_frames)
}

#[tauri::command]
pub async fn detect_clients(
    iface_guid: String,
    bssid: String,
    duration_secs: Option<u64>,
) -> ClientScanResult {
    let fail = |m: String| ClientScanResult { success: false, clients: vec![], packets: 0, message: m };
    let parts: Vec<&str> = bssid.split(':').collect();
    if parts.len() != 6 { return fail("BSSID inválido".into()); }
    let mut ap = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        match u8::from_str_radix(p, 16) { Ok(v) => ap[i] = v, Err(_) => return fail("BSSID inválido".into()) }
    }
    let secs = duration_secs.unwrap_or(30).clamp(5, 120);
    let cap = match crate::capture::native_capture(iface_guid, Some(secs), None).await {
        c if c.success => c,
        c => return fail(format!("captura falló: {}", c.message)),
    };
    let bytes = match std::fs::read(&cap.output_file) {
        Ok(b) => b,
        Err(e) => return fail(format!("no se pudo leer {}: {e}", cap.output_file)),
    };
    let (clients, candidate_stas, data_frames) = clients_from_pcap_full(&bytes, &ap);
    let msg = if clients.is_empty() {
        format!("Sin clientes de {} en {}s ({} paquetes, {} frames Data con BSSID, {} STAs candidatos). No se puede capturar un handshake real sin un cliente asociado.", bssid, secs, cap.packets, data_frames, candidate_stas)
    } else {
        format!("{} cliente(s) de {}: {} ({} frames Data, {} candidatos)", clients.len(), bssid, clients.join(", "), data_frames, candidate_stas)
    };
    ClientScanResult { success: true, clients, packets: cap.packets, message: msg }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_sta_not_ap() {
        let ap = [0x44, 0xd4, 0x54, 0xbc, 0xde, 0xf6];
        let sta = [0x00, 0xc0, 0xca, 0x59, 0xa7, 0xd8];
        let mut frame = vec![0u8; 24];
        frame[0] = 0x08; // Data (little-endian: ToDS se codifica en el byte alto)
        frame[1] = 0x01; // ToDS=1 → cliente → AP, asociado
        frame[4..10].copy_from_slice(&ap);   // addr1 = AP
        frame[10..16].copy_from_slice(&sta); // addr2 = STA
        frame[16..22].copy_from_slice(&ap);  // addr3 = AP
        let mut pcap = vec![0xd4,0xc3,0xb2,0xa1,2,0,4,0,0,0,0,0,0,0,0,0,127,0,0,0,1,0,0,0];
        pcap.extend_from_slice(&[0,0,0,0,0,0,0,0]);
        pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        pcap.extend_from_slice(&frame);
        for _ in 0..5 {
            pcap.extend_from_slice(&[0,0,0,0,0,0,0,0]);
            pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            pcap.extend_from_slice(&frame);
        }
        let got = clients_from_pcap(&pcap, &ap);
        assert_eq!(got, vec!["00:C0:CA:59:A7:D8".to_string()]);
    }

    // Medido 2026-11-26: en downlink (FromDS) addr3 = SA cableado. Antes se
    // contaba como "cliente" (48:22:54:88:29:D6) y el deauth iba a ese MAC.
    #[test]
    fn downlink_wired_sa_is_not_client_sta_is_addr1() {
        let ap = [0xd8, 0xa7, 0x56, 0x31, 0xbf, 0x13];
        let wired_sa = [0x48, 0x22, 0x54, 0x88, 0x29, 0xd6];
        let sta = [0x50, 0x8b, 0xb9, 0x61, 0x38, 0xb3];
        let build = |a1: &[u8; 6], a2: &[u8; 6], a3: &[u8; 6]| -> Vec<u8> {
            let mut pcap = vec![0xd4,0xc3,0xb2,0xa1,2,0,4,0,0,0,0,0,0,0,0,0,127,0,0,0,1,0,0,0];
            for _ in 0..5 {
                let mut frame = vec![0u8; 24];
                frame[0] = 0x08;
                frame[1] = 0x02; // FromDS=1 → AP → STA
                frame[4..10].copy_from_slice(a1);  // DA (el STA)
                frame[10..16].copy_from_slice(a2);  // BSSID (AP)
                frame[16..22].copy_from_slice(a3); // SA (cableado)
                pcap.extend_from_slice(&[0,0,0,0,0,0,0,0]);
                pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
                pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
                pcap.extend_from_slice(&frame);
            }
            pcap
        };
        // Downlink broadcast con SA cableado: 0 clientes.
        let broadcast = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert!(clients_from_pcap(&build(&broadcast, &ap, &wired_sa), &ap).is_empty());
        // Downlink unicast al STA real: ese sí es cliente (a3 cableado ignorado).
        assert_eq!(clients_from_pcap(&build(&sta, &ap, &wired_sa), &ap), vec!["50:8B:B9:61:38:B3".to_string()]);
    }

    // Medido 2026-11-26 en red real: el móvil BE:FD:76:27:52:16 conectado
    // enviaba 26 frames Null (subtype 4, ToDS=1) para power-save. Antes del fix
    // se descartaban (solo se contaban sub 0/8) → "0 STAs candidatos" aunque
    // hubiera un cliente real. Este test simula ESOS frames y exige que se detecte.
    #[test]
    fn idle_phone_sends_null_frames_is_detected() {
        let ap = [0x44, 0xd4, 0x54, 0xbc, 0xde, 0xf6];
        let sta = [0xbe, 0xfd, 0x76, 0x27, 0x52, 0x16];
        let mut pcap = vec![0xd4,0xc3,0xb2,0xa1,2,0,4,0,0,0,0,0,0,0,0,0,127,0,0,0,1,0,0,0];
        for _ in 0..26 {
            let mut frame = vec![0u8; 24];
            // subtype 4 (Null) = 0100 << 4 = 0x40; type Data = 0x08
            frame[0] = 0x48;
            frame[1] = 0x01; // ToDS=1 → STA → AP
            frame[4..10].copy_from_slice(&ap);   // addr1 = AP
            frame[10..16].copy_from_slice(&sta); // addr2 = STA (el móvil)
            frame[16..22].copy_from_slice(&ap);  // addr3 = AP
            pcap.extend_from_slice(&[0,0,0,0,0,0,0,0]);
            pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            pcap.extend_from_slice(&frame);
        }
        assert_eq!(clients_from_pcap(&pcap, &ap), vec!["BE:FD:76:27:52:16".to_string()]);
    }

    #[test]
    fn ignores_multicast_addresses() {
        let ap = [0x44, 0xd4, 0x54, 0xbc, 0xde, 0xf6];
        let mut frame = vec![0u8; 24];
        frame[0] = 0x08;
        frame[1] = 0x01; // ToDS=1
        frame[4..10].copy_from_slice(&ap);
        frame[10..16].copy_from_slice(&[0x33, 0x33, 0, 0, 0, 0xfb]);
        frame[16..22].copy_from_slice(&ap);
        let mut pcap = vec![0xd4,0xc3,0xb2,0xa1,2,0,4,0,0,0,0,0,0,0,0,0,127,0,0,0,1,0,0,0];
        pcap.extend_from_slice(&[0,0,0,0,0,0,0,0]);
        pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        pcap.extend_from_slice(&frame);
        assert!(clients_from_pcap(&pcap, &ap).is_empty());
    }
}
