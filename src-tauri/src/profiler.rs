// Perfilador de objetivo — veredicto de auditabilidad por red (solo lectura, sin TX).
// Usa `netsh wlan show networks mode=bssid` (disponible en Windows sin monitor mode)
// y clasifica auth/cipher en una vía de ataque recomendada con expectativas honestas.
//
// Limitaciones conocidas y declaradas en la UI:
// - netsh NO expone MFP/PMF (802.11w) ni WPS: esos campos salen como "unknown"
//   y deben confirmarse en Linux (tshark/wash) antes de un downgrade o Pixie Dust.
// - Ningún ataque activo se ejecuta desde aquí: esto solo perfila.
use serde::Serialize;
use tauri::{command, AppHandle};

#[derive(Debug, Clone, Serialize)]
pub struct TargetProfile {
    pub ssid:     String,
    pub bssid:    String,
    pub auth:     String,
    pub cipher:   String,
    pub channel:  Option<u8>,
    pub signal:   Option<u8>,
    /// Clave estable para la UI: open|wep|wpa_legacy|wpa2_psk|transition|wpa3_sae|enterprise|unknown
    pub verdict:  String,
    pub title:    String,
    pub detail:   String,
    /// WPS según netsh: siempre "unknown" (netsh no lo expone). Ver nota.
    pub wps:      String,
    pub wps_note: String,
    /// MFP/PMF según netsh: siempre "unknown". Ver nota.
    pub mfp:      String,
    pub mfp_note: String,
    /// Badges de honestidad: windows-ok | linux-required | exito-alto|medio|bajo|nulo
    pub badges:   Vec<String>,
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace(['-', '_', ' '], "")
}

/// Clasificación pura (testeable): (auth, cipher) de netsh → veredicto.
pub fn verdict_for(auth_raw: &str, cipher_raw: &str) -> (&'static str, &'static str, &'static str, Vec<&'static str>) {
    let a = norm(auth_raw);
    let c = norm(cipher_raw);

    // Abierta
    if a.contains("abierta") || a.contains("open") || a.contains("ninguna") || a == "open" {
        return ("open", "Red abierta — sin cifrado",
            "Tráfico sin cifrar. No hace falta crackear nada; el riesgo es de escucha directa. (Solo redes propias/lab.)",
            vec!["windows-ok", "exito-alto"]);
    }
    // Enterprise / 802.1X
    if a.contains("enterprise") || a.contains("802.1x") || a.contains("eap") || a.contains("wpa3enterprise") {
        return ("enterprise", "Enterprise (802.1X) — fuera del alcance PMKID/handshake",
            "Sin PSK que crackear offline. Vías reales: validar cert del servidor RADIUS, relay EAP (hostapd-mana, Linux) o portal. Requiere lab con RADIUS propio.",
            vec!["linux-required", "exito-bajo"]);
    }
    // WEP
    if a.contains("wep") || c.contains("wep") {
        return ("wep", "WEP roto — crack en minutos",
            "ChopChop / fragment + colección de IVs y crack estadístico. Requiere inyección → Linux con adaptador compatible.",
            vec!["linux-required", "exito-alto"]);
    }
    // WPA legacy (TKIP, sin WPA2)
    if (a.contains("wpa") && !a.contains("wpa2") && !a.contains("wpa3")) || c.contains("tkip") && !a.contains("wpa2") {
        return ("wpa_legacy", "WPA-TKIP legacy — handshake + diccionario",
            "Sin PMKID fiable en la mayoría de APs. Capturar handshake (deauth, Linux) y crack offline con reglas/máscaras.",
            vec!["linux-required", "exito-medio"]);
    }
    // Transición WPA2+WPA3 (el caso interesante de 2026)
    let has_psk = a.contains("wpa2") || a.contains("psk");
    let has_sae = a.contains("wpa3") || a.contains("sae");
    if has_psk && has_sae {
        return ("transition", "Transición WPA2+WPA3 — downgrade viable si MFP opcional",
            "Prioridad 1: confirmar MFP con tshark en Linux (MFPR=0 → deauth + rogue AP solo-WPA2 y handshake clásico). Prioridad 2: PMKID de la pata WPA2. MFP en netsh sale unknown: no intentar deauth a ciegas.",
            vec!["linux-required", "exito-medio"]);
    }
    // WPA3 puro
    if has_sae {
        return ("wpa3_sae", "WPA3-SAE puro — sin crack offline",
            "SAE no expone hash crackeable. Vías reales: Wacker online (lento, ruidoso, solo vs claves débiles), portal cautivo, o nada. Verificar que no haya pata WPA2 oculta con un scan RSN en Linux.",
            vec!["linux-required", "exito-bajo"]);
    }
    // WPA2-PSK (caso general)
    if a.contains("wpa2") {
        return ("wpa2_psk", "WPA2-PSK — PMKID primero, handshake fallback",
            "Capturar PMKID (sin cliente) y si el AP lo suprime, handshake vía deauth. Crack con hashcat -m 22000 + reglas/máscaras. Captura requiere Linux; el crack corre en este Windows.",
            vec!["linux-required", "exito-medio"]);
    }
    ("unknown", "Desconocido — reescanear",
        "netsh no devolvió auth/cipher reconocibles. Reescanea o confirma con airodump/tshark en Linux.",
        vec!["windows-ok", "exito-nulo"])
}

static RE_BSSID_LINE: once_cell::sync::Lazy<regex::Regex> =
    once_cell::sync::Lazy::new(|| regex::Regex::new(r"^BSSID\s+\d*\s*:\s*([0-9A-Fa-f:]{17})").unwrap());

/// Vuelca netsh (compartido con wpa3.rs). Puro IO, sin parseo.
pub(crate) async fn netsh_dump(app: &tauri::AppHandle) -> Result<String, String> {
    use std::time::Duration;
    use tauri_plugin_shell::ShellExt;
    let shell = app.shell();
    let out = tokio::time::timeout(
        Duration::from_secs(10),
        shell
            .command("powershell")
            .args(["-NoProfile", "-Command", "netsh wlan show networks mode=bssid"])
            .output(),
    )
    .await
    .map_err(|_| "Timeout: netsh wlan tardó más de 10s.".to_string())?
    .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Busca el bloque netsh del BSSID exacto y devuelve (ssid, auth, cipher, channel, signal).
/// Versión pública-para-el-crate (la usa wpa3.rs).
pub(crate) fn find_target(
    raw: &str,
    want: &str,
) -> Option<(String, String, String, Option<u8>, Option<u8>)> {
    let want_lc = want.to_lowercase();
    let mut ssid = String::new();
    let mut auth = String::new();
    let mut cipher = String::new();

    let mut cur_bssid = String::new();
    let mut cur_sig: Option<u8> = None;
    let mut cur_chan: Option<u8> = None;

    // Estado del bloque SSID actual + candidatos por BSSID
    let mut hit: Option<(String, String, String, Option<u8>, Option<u8>)> = None;

    for line in raw.lines() {
        let s = line.trim();
        if let Some(rest) = s.strip_prefix("SSID") {
            // Nuevo bloque: el BSSID pendiente del bloque anterior queda evaluado
            if cur_bssid.to_lowercase() == want_lc {
                hit = Some((ssid.clone(), auth.clone(), cipher.clone(), cur_chan, cur_sig));
            }
            // "SSID 1 : nombre" — nuevo bloque; guarda el anterior si hubo hit (ya hecho abajo)
            if let Some(colon) = rest.find(':') {
                ssid = rest[colon + 1..].trim().to_string();
            }
            auth.clear(); cipher.clear();
            cur_bssid.clear(); cur_sig = None; cur_chan = None;
            continue;
        }
        let low = s.to_lowercase();
        if low.starts_with("autenticaci") || low.starts_with("authentication") {
            if let Some(colon) = s.find(':') { auth = s[colon + 1..].trim().to_string(); }
        } else if low.starts_with("cifrado") || low.starts_with("encryption") {
            if let Some(colon) = s.find(':') { cipher = s[colon + 1..].trim().to_string(); }
        } else if let Some(c) = RE_BSSID_LINE.captures(s) {
            // Nuevo BSSID dentro del bloque: el anterior queda evaluado
            if cur_bssid.to_lowercase() == want_lc {
                hit = Some((ssid.clone(), auth.clone(), cipher.clone(), cur_chan, cur_sig));
            }
            cur_bssid = c[1].to_string();
            cur_sig = None; cur_chan = None;
        } else if low.starts_with("se") && low.contains("al") && s.contains('%') {
            // "Señal : 82%" / "Signal : 82%"
            if let Some(colon) = s.find(':') {
                cur_sig = s[colon + 1..].trim().trim_end_matches('%').trim().parse().ok();
            }
        } else if low.starts_with("canal") || low.starts_with("channel") {
            if let Some(colon) = s.find(':') {
                cur_chan = s[colon + 1..].trim().split_whitespace().next().unwrap_or("").parse().ok();
            }
        }
    }
    // Último BSSID del volcado
    if cur_bssid.to_lowercase() == want_lc {
        hit = Some((ssid, auth, cipher, cur_chan, cur_sig));
    }
    hit
}

/// Comando Tauri: perfila un BSSID (solo lectura).
#[command]
pub async fn profile_target(app: AppHandle, bssid: String) -> Result<TargetProfile, String> {
    let raw = netsh_dump(&app).await?;
    let (ssid, auth, cipher, channel, signal) = find_target(&raw, &bssid)
        .ok_or_else(|| format!("BSSID {} no visible en el último scan. Reescanea.", bssid))?;

    let (verdict, title, detail, badges) = verdict_for(&auth, &cipher);
    Ok(TargetProfile {
        ssid: if ssid.is_empty() { "Red oculta".into() } else { ssid },
        bssid: bssid.to_uppercase(),
        auth: if auth.is_empty() { "Desconocida".into() } else { auth },
        cipher: if cipher.is_empty() { "Desconocido".into() } else { cipher },
        channel, signal,
        verdict: verdict.into(),
        title: title.into(),
        detail: detail.into(),
        wps: "unknown".into(),
        wps_note: "netsh no expone WPS. Confirmar con wash (Linux) antes de Pixie Dust.".into(),
        mfp: "unknown".into(),
        mfp_note: "netsh no expone MFP/PMF. Confirmar MFPC/MFPR con tshark en Linux antes de deauth.".into(),
        badges: badges.into_iter().map(|s| s.to_string()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_matrix() {
        assert_eq!(verdict_for("Abierta", "Ninguna").0, "open");
        assert_eq!(verdict_for("WEP", "WEP").0, "wep");
        assert_eq!(verdict_for("WPA-Personal", "TKIP").0, "wpa_legacy");
        assert_eq!(verdict_for("WPA2-Personal", "CCMP").0, "wpa2_psk");
        assert_eq!(verdict_for("WPA3-Personal", "GCMP-256").0, "wpa3_sae");
        assert_eq!(verdict_for("WPA2-Personal WPA3-Personal", "CCMP").0, "transition");
        assert_eq!(verdict_for("WPA2-Enterprise", "CCMP").0, "enterprise");
        assert_eq!(verdict_for("???", "").0, "unknown");
    }

    #[test]
    fn finds_bssid_block() {
        let raw = "SSID 1 : Casa\n    Autenticación           : WPA2-Personal\n    Cifrado                 : CCMP\n    BSSID 1                 : aa:bb:cc:dd:ee:ff\n         Señal             : 82%\n         Canal            : 6\nSSID 2 : Bar\n    Autenticación           : Abierta\n    Cifrado                 : Ninguna\n    BSSID 1                 : 11:22:33:44:55:66\n         Señal             : 60%\n         Canal            : 1\n";
        let (ssid, auth, cipher, ch, sig) = find_target(raw, "AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(ssid, "Casa");
        assert_eq!(auth, "WPA2-Personal");
        assert_eq!(cipher, "CCMP");
        assert_eq!(ch, Some(6));
        assert_eq!(sig, Some(82));
        assert!(find_target(raw, "00:00:00:00:00:00").is_none());
    }
}
