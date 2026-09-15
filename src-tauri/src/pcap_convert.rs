// PCAP → hashcat .22000 converter
// Replaces hcxpcapngtool by extracting PMKID + EAPOL handshakes from pcap captures.
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::command;

// Nota: el parsing del header global pcap y de cada paquete se hace manualmente
// (little/big-endian según magic); no se usan structs packed para evitar UB de
// alineamiento en slices.

// radiotap header: version(1) + pad(1) + len(2) + present(4) + optional fields
const RADIOTAP_LEN_OFFSET: usize = 2;

// 802.11 frame control
fn frame_type_fc(fc: u16) -> u8  { ((fc >> 2) & 0b11) as u8 }
fn frame_subtype_fc(fc: u16) -> u8 { ((fc >> 4) & 0b1111) as u8 }
fn to_ds(fc: u16) -> bool { (fc >> 8) & 1 != 0 }
fn from_ds(fc: u16) -> bool { (fc >> 9) & 1 != 0 }

// LLC/SNAP header for EAPOL
const LLC_SNAP: [u8; 8] = [0xAA, 0xAA, 0x03, 0x00, 0x00, 0x00, 0x88, 0x8E];

fn mac_str(b: &[u8], off: usize) -> String {
    if off + 6 > b.len() { return "??:??:??:??:??:??".into() }
    format!("{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[off], b[off+1], b[off+2], b[off+3], b[off+4], b[off+5])
}

fn mac_str_colon(b: &[u8], off: usize) -> String {
    if off + 6 > b.len() { return "??:??:??:??:??:??".into() }
    format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        b[off], b[off+1], b[off+2], b[off+3], b[off+4], b[off+5])
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

// (eliminado: parser RSNE de beacons — hcxpcapngtool no saca PMKIDs de beacons,
//  el PMKID útil vive en el PMKID-KDE del EAPOL M1)

// — PMKID obtenido del PMKID-KDE dentro del EAPOL M1 (AP→STA) —
// (hcxpcapngtool nunca saca el PMKID del RSNE de beacons: solo de M1,
//  (Re)AssocReq o EAPOL M2 con KDE. El RSNE de beacon lleva lista vacía.)
#[derive(Debug, Clone)]
struct PmkidEntry {
    apmac:   [u8; 6],
    stmac:   [u8; 6],
    pmkid:   [u8; 16],
}

// — Handshake state per (APMAC, STMAC) pair —
#[derive(Debug, Clone)]
struct HandshakeState {
    apmac:      [u8; 6],
    stmac:      [u8; 6],
    anonce:     Option<[u8; 32]>,
    snonce:     Option<[u8; 32]>,
    eapol_m1:   Option<Vec<u8>>,
    eapol_m2:   Option<Vec<u8>>,
    eapol_m3:   Option<Vec<u8>>,
    eapol_m4:   Option<Vec<u8>>,
    keyver:     u8,  // 1=WPA, 2=WPA2, 0=unknown
}

fn hs_key(ap: &[u8], st: &[u8]) -> [u8; 12] {
    let mut k = [0u8; 12];
    k[..6].copy_from_slice(ap);
    k[6..].copy_from_slice(st);
    k
}

// — Result —
#[derive(Debug, Clone, Serialize)]
pub struct HashLine {
    pub line:          String,
    pub ap_mac:        String,
    pub client_mac:    String,
    pub hash_type:     String, // PMKID or EAPOL
    pub pmkid:         Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConvertResult {
    pub success:       bool,
    pub output_file:   String,
    pub hash_count:    usize,
    pub pmkid_count:   usize,
    pub handshake_count: usize,
    pub messages:      Vec<String>,
}

/// Comando Tauri: convierte .pcap → .22000 (reemplaza hcxpcapngtool.exe)
#[command]
pub async fn pcap_to_22000(pcap_path: String, output_dir: Option<String>) -> ConvertResult {
    let out_dir = output_dir.as_deref().unwrap_or(".");
    let stem = std::path::Path::new(&pcap_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("out");
    let output_path = format!("{}/{}.22000", out_dir, stem);
    convert_pcap_to_22000(pcap_path, output_path).await
}

/// Convierte un archivo pcap a hashcat .22000
pub async fn convert_pcap_to_22000(pcap_path: String, output_path: String) -> ConvertResult {
    let data = match std::fs::read(&pcap_path) {
        Ok(d) => d,
        Err(e) => return ConvertResult {
            success: false, output_file: output_path,
            hash_count: 0, pmkid_count: 0, handshake_count: 0,
            messages: vec![format!("Error leyendo {}: {}", pcap_path, e)],
        },
    };

    if data.len() < 24 {
        return ConvertResult {
            success: false, output_file: output_path,
            hash_count: 0, pmkid_count: 0, handshake_count: 0,
            messages: vec!["Archivo demasiado pequeño para ser un pcap".into()],
        };
    }

    // Parse global header
    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let network: u32;
    let swap: bool;
    if magic == 0xa1b2c3d4 {
        swap = false;
        network = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    } else if magic == 0xd4c3b2a1 {
        swap = true;
        network = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    } else {
        return ConvertResult {
            success: false, output_file: output_path,
            hash_count: 0, pmkid_count: 0, handshake_count: 0,
            messages: vec![format!("Magic pcap inválido: 0x{:08X} (no es un archivo .pcap clásico)", magic)],
        };
    }

    let raws = if swap { u32::from_be_bytes } else { u32::from_le_bytes };
    let _raws16 = if swap { u16::from_be_bytes } else { u16::from_le_bytes };

    let mut pmkid_entries: Vec<PmkidEntry> = vec![];
    let mut handshakes: BTreeMap<[u8; 12], HandshakeState> = BTreeMap::new();

    let mut offset = 24; // skip global header
    let mut pkt_num = 0;
    let mut messages = vec![];

    while offset + 16 <= data.len() {
        let incl_len = raws([data[offset+8], data[offset+9], data[offset+10], data[offset+11]]) as usize;
        let _ts_sec = raws([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
        let _ts_usec = raws([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]);

        offset += 16;
        if offset + incl_len > data.len() { break }

        let pkt = &data[offset..offset+incl_len];
        pkt_num += 1;

        // Skip radiotap header
        if network == 105 || network == 127 { // IEEE802_11_RADIOTAP or IEEE802_11_RADIOTAP_AIRPCAP
            if pkt.len() < 4 { offset += incl_len; continue }
            let rt_len = u16::from_le_bytes([pkt[RADIOTAP_LEN_OFFSET], pkt[RADIOTAP_LEN_OFFSET+1]]) as usize;
            if rt_len > pkt.len() { offset += incl_len; continue }
            process_80211_frame(pkt, rt_len, &mut pmkid_entries, &mut handshakes);
        }

        offset += incl_len;
    }

    // Build output (formato hc22000: WPA*01*PMKID*MAC_AP*MAC_STA*ESSID_HEX***)
    let mut out_lines: Vec<String> = vec![];
    let mut pmkid_lines: Vec<HashLine> = vec![];
    for e in &pmkid_entries {
        let pmkid_hex = hex(&e.pmkid);
        let line = format!(
            "WPA*01*{}*{}*{}***",
            pmkid_hex,
            mac_str(&e.apmac, 0),
            mac_str(&e.stmac, 0),
        );
        pmkid_lines.push(HashLine {
            line: line.clone(),
            ap_mac: mac_str_colon(&e.apmac, 0),
            client_mac: mac_str_colon(&e.stmac, 0),
            hash_type: "PMKID".into(),
            pmkid: Some(pmkid_hex),
        });
        out_lines.push(line);
    }
    for (_key, hs) in &handshakes {
        if let Some(line) = build_hash_line(hs) {
            out_lines.push(line);
        }
    }

    let hash_count = out_lines.len();
    let pmkid_count = pmkid_lines.len();
    let handshake_count = handshakes.len();

    if let Err(e) = std::fs::write(&output_path, out_lines.join("\n")) {
        return ConvertResult {
            success: false, output_file: output_path.clone(),
            hash_count: 0, pmkid_count: 0, handshake_count: 0,
            messages: vec![format!("Error escribiendo {}: {}", output_path, e)],
        };
    }

    messages.push(format!("Procesados {} paquetes", pkt_num));
    messages.push(format!("Encontrados {} PMKIDs, {} handshakes", pmkid_count, handshake_count));
    messages.push(format!("Escrito: {}", output_path));

    ConvertResult {
        success: true, output_file: output_path,
        hash_count, pmkid_count, handshake_count, messages,
    }
}

fn mac_at(pkt: &[u8], off: usize) -> [u8; 6] {
    let mut m = [0u8; 6];
    if off + 6 <= pkt.len() { m.copy_from_slice(&pkt[off..off+6]); }
    m
}

fn process_80211_frame(
    pkt: &[u8], rt_len: usize,
    pmkids: &mut Vec<PmkidEntry>,
    handshakes: &mut BTreeMap<[u8; 12], HandshakeState>,
) {
    let fc_off = rt_len;
    if fc_off + 2 > pkt.len() { return }
    let fc = u16::from_le_bytes([pkt[fc_off], pkt[fc_off+1]]);
    let ftype = frame_type_fc(fc);
    let fsub  = frame_subtype_fc(fc);

    let a1 = mac_at(pkt, fc_off + 4);
    let a2 = mac_at(pkt, fc_off + 10);
    let a3 = mac_at(pkt, fc_off + 16);

    let (da, sa, bssid) = match (to_ds(fc), from_ds(fc)) {
        (false, false) => (a1, a2, a3),
        (true,  false) => (a3, a2, a1),
        (false, true)  => (a1, a3, a2),
        (true,  true)  => (a3, [0; 6], a1),
    };

    // Mgmt frames: no producen hashes aquí (el PMKID real viene del EAPOL M1
    // o de (Re)AssocReq; el RSNE de beacon lleva el PMKID list vacío).
    let _ = (ftype, fsub, da, sa, bssid);

    // EAPOL (Data frame, subtype 0x00 o QoS-Data 0x01)
    if ftype == 2 && (fsub == 0 || fsub == 1) {
        // After 802.11 header (24 bytes min), look for LLC/SNAP + EAPOL
        // Check for QoS (FC byte 1, bit 7)
        let qos = (fc >> 15) & 1 != 0;
        let hdr_len = if qos { 26 } else { 24 };
        let llc_off = fc_off + hdr_len;

        if llc_off + 8 + 4 > pkt.len() { return }
        // Check LLC/SNAP for EAPOL
        if &pkt[llc_off..llc_off+8] != LLC_SNAP { return }

        let eapol_off = llc_off + 8;
        if eapol_off + 4 > pkt.len() { return }
        let _eapol_ver = pkt[eapol_off];
        let eapol_type = pkt[eapol_off+1];
        let eapol_len = u16::from_be_bytes([pkt[eapol_off+2], pkt[eapol_off+3]]) as usize;
        if eapol_off + 4 + eapol_len > pkt.len() { return }

        if eapol_type != 2 { return } // EAPOL-Key only
        if eapol_len < 95 { return }   // minimum EAPOL-Key size

        let key_off = eapol_off + 4;

        // Determinar (apmac, stmac) según la dirección del frame:
        //   STA→DS (to_ds=1):  TA=SA es el cliente, RA=A1 es el AP
        //   DS→STA (from_ds=1): TA=SA es el AP,     DA=A1 es el cliente
        //   ad-hoc/IBSS:        bssid (a3) es el AP,  DA es el cliente
        let (apmac, stmac) = if to_ds(fc) && !from_ds(fc) {
            (bssid, sa)
        } else if from_ds(fc) && !to_ds(fc) {
            (sa, da)
        } else {
            (bssid, da)
        };

        let desc_type = pkt[key_off]; // 1=WPA, 2=WPA2, 254=WPA2
        let key_info = u16::from_be_bytes([pkt[key_off+1], pkt[key_off+2]]);
        let _key_len  = u16::from_be_bytes([pkt[key_off+3], pkt[key_off+4]]);
        let key_nonce = &pkt[key_off+13..key_off+13+32];
        let _key_mic   = &pkt[key_off+81..key_off+81+16];
        let _key_data_len = u16::from_be_bytes([pkt[key_off+97], pkt[key_off+98]]) as usize;
        let _key_data_off = key_off + 99;

        // Determine if Install flag is set (bit 6, 0x0040)
        let install = (key_info & 0x0040) != 0;

        let k = hs_key(&apmac, &stmac);
        let state = handshakes.entry(k).or_insert(HandshakeState {
            apmac, stmac,
            anonce: None, snonce: None,
            eapol_m1: None, eapol_m2: None, eapol_m3: None, eapol_m4: None,
            keyver: 0,
        });

        // Detect message number from pairwise bit and install flag
        let pairwise = (key_info & 0x0008) != 0; // bit 3
        let key_ack = (key_info & 0x0080) != 0;  // bit 7
        let key_mic_valid = (key_info & 0x0100) != 0; // bit 8

        let full_eapol = pkt[eapol_off..eapol_off+4+eapol_len].to_vec();

        // M1: Install=0, Ack=1, MIC=0, Pairwise=1
        // M2: Install=0, Ack=0, MIC=1, Pairwise=1
        // M3: Install=1, Ack=1, MIC=1, Pairwise=1
        // M4: Install=0, Ack=0, MIC=1, Pairwise=1 (replay counter matches M3)

        if !install && key_ack && !key_mic_valid {
            // M1: guardar EAPOL + ANONCE y extraer el PMKID-KDE si existe
            if state.eapol_m1.is_none() {
                state.eapol_m1 = Some(full_eapol.clone());
                state.anonce = {
                    let mut n = [0u8; 32];
                    n.copy_from_slice(key_nonce);
                    Some(n)
                };
                state.keyver = if desc_type == 1 { 1 } else { 2 };
            }
            // PMKID-KDE: en key_data, IE vendor 00:0F:AC type 4, len 0x14 (20)
            let kd_off = key_off + 99;
            if kd_off + 2 <= pkt.len() {
                let kd_len = u16::from_be_bytes([pkt[key_off+97], pkt[key_off+98]]) as usize;
                let kd_end = (kd_off + kd_len).min(pkt.len());
                let mut p = kd_off;
                while p + 8 <= kd_end {
                    let ie_type = pkt[p];
                    let ie_len  = pkt[p + 1] as usize;
                    if ie_len < 20 || p + 2 + ie_len > kd_end { break }
                    // OUI 00:0F:AC (802.11) + type 4 = PMKID KDE
                    if ie_type == 0xDD && pkt[p+2] == 0x00 && pkt[p+3] == 0x0F && pkt[p+4] == 0xAC && pkt[p+5] == 0x04 {
                        let mut kid = [0u8; 16];
                        kid.copy_from_slice(&pkt[p+6..p+6+16]);
                        // Descartar PMKID cero o todos-iguales (corrupto)
                        if kid != [0u8; 16] {
                            if !pmkids.iter().any(|e| e.apmac == apmac && e.stmac == stmac && e.pmkid == kid) {
                                pmkids.push(PmkidEntry { apmac, stmac, pmkid: kid });
                            }
                        }
                        break;
                    }
                    p += 2 + ie_len;
                }
            }
        } else if pairwise && key_mic_valid && !key_ack {
            // M2 or M4
            if state.eapol_m1.is_some() && state.eapol_m2.is_none() {
                state.eapol_m2 = Some(full_eapol.clone());
                state.snonce = {
                    let mut n = [0u8; 32];
                    n.copy_from_slice(key_nonce);
                    Some(n)
                };
            } else if state.eapol_m2.is_some() && state.eapol_m4.is_none() {
                state.eapol_m4 = Some(full_eapol.clone());
            }
        } else if install && key_ack && key_mic_valid {
            // M3
            if state.eapol_m3.is_none() {
                state.eapol_m3 = Some(full_eapol.clone());
            }
        }
    }
}

/// Construye la línea hc22000 de un handshake M1+M2 (message pair 00 = challenge).
/// Formato (hcxpcapngtool): WPA*02*MIC*MAC_AP*MAC_STA*ESSID_HEX*ANONCE*EAPOL_MIC_CERO*MP
/// - EAPOL = mensaje completo (header EAPOL + Key) con los 16 bytes del MIC a 0.
/// - MP 00 = M1+M2 con EAPOL tomado de M2 (challenge).
fn build_hash_line(hs: &HandshakeState) -> Option<String> {
    let m1 = hs.eapol_m1.as_ref()?;
    let m2 = hs.eapol_m2.as_ref()?;
    let anonce = hs.anonce.as_ref()?;

    let ap_mac = mac_str(&hs.apmac, 0);
    let st_mac = mac_str(&hs.stmac, 0);

    // EAPOL del M2 con MIC zeroed (los 16 bytes del MIC en el Key frame)
    let mut eapol = m2.clone();
    // Key frame: type(1)+key_info(2)+key_len(2)+replay(8)+nonce(32)+iv(16)+rsc(8)+id(8) = 77 → MIC en [77..93] del Key
    // full EAPOL = header(4) + Key → MIC en [4+77 .. 4+93]
    let mic_off = 4 + 77;
    if eapol.len() >= mic_off + 16 {
        for b in &mut eapol[mic_off..mic_off + 16] { *b = 0; }
    } else {
        return None; // EAPOL demasiado corto: no es un M2 válido
    }

    let mic_hex = hex(&eapol[mic_off..mic_off + 16]); // cero a cero tras el wipe (hashcat recalcula)
    let essid_hex = String::new(); // ESSID desconocido en capturas crudas
    let _ = m1;

    Some(format!(
        "WPA*02*{}*{}*{}*{}*{}*{}*00",
        mic_hex, ap_mac, st_mac, essid_hex, hex(anonce), hex(&eapol)
    ))
}
