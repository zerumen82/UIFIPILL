// Conexión a la red atacada — el objetivo final (lab).
// Windows no acepta SSID+clave por CLI directa: se genera un perfil XML
// temporal, se importa con `netsh wlan add profile` y se conecta con
// `netsh wlan connect`. 100% nativo, sin inyección ni monitor mode.
use tauri::{command, AppHandle};
use tauri_plugin_shell::ShellExt;
use crate::commands::CmdResponse;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Perfil WLAN para WPA2-PSK/AES (el caso general; WEP/abierta fuera de alcance).
pub fn wlan_profile_xml(ssid: &str, password: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?>\n\
         <WLANProfile xmlns=\"http://www.microsoft.com/networking/WLAN/profile/v1\">\n\
         \x20   <name>{ssid}</name>\n\
         \x20   <SSIDConfig>\n\
         \x20       <SSID>\n\
         \x20           <name>{ssid}</name>\n\
         \x20       </SSID>\n\
         \x20   </SSIDConfig>\n\
         \x20   <connectionType>ESS</connectionType>\n\
         \x20   <connectionMode>auto</connectionMode>\n\
         \x20   <MSM>\n\
         \x20       <security>\n\
         \x20           <authEncryption>\n\
         \x20               <authentication>WPA2PSK</authentication>\n\
         \x20               <encryption>AES</encryption>\n\
         \x20               <useOneX>false</useOneX>\n\
         \x20           </authEncryption>\n\
         \x20           <sharedKey>\n\
         \x20               <keyType>passPhrase</keyType>\n\
         \x20               <protected>false</protected>\n\
         \x20               <keyMaterial>{pass}</keyMaterial>\n\
         \x20           </sharedKey>\n\
         \x20       </security>\n\
         \x20   </MSM>\n\
         </WLANProfile>\n",
        ssid = xml_escape(ssid),
        pass = xml_escape(password),
    )
}

async fn netsh(app: &AppHandle, args: &[&str]) -> Result<(bool, String), String> {
    let shell = app.shell();
    let out = tokio::time::timeout(std::time::Duration::from_secs(20), shell.command("netsh").args(args).output())
        .await
        .map_err(|_| "Timeout: netsh tardó más de 20s.".to_string())?
        .map_err(|e| e.to_string())?;
    Ok((out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// Comando: conecta a un SSID con clave (perfil temporal, se deja instalado).
#[command]
pub async fn wifi_connect(app: AppHandle, ssid: String, password: String) -> CmdResponse {
    if ssid.trim().is_empty() || password.is_empty() {
        return CmdResponse {
            success: false, output: String::new(),
            stderr: "SSID y clave requeridos".into(), exit_code: None,
        };
    }
    // La clave viaja en fichero temporal, nunca en argv.
    let xml_path = std::env::temp_dir().join(format!("uifipill_wlan_{}.xml", std::process::id()));
    if std::fs::write(&xml_path, wlan_profile_xml(ssid.trim(), &password)).is_err() {
        return CmdResponse {
            success: false, output: String::new(),
            stderr: "No se pudo escribir el perfil temporal".into(), exit_code: None,
        };
    }
    let xml = xml_path.display().to_string();
    let (ok_add, out_add) = match netsh(&app, &["wlan", "add", "profile", &format!("filename=\"{}\"", xml), "user=current"]).await {
        Ok(r) => r,
        Err(e) => {
            let _ = std::fs::remove_file(&xml_path);
            return CmdResponse { success: false, output: String::new(), stderr: e, exit_code: None };
        }
    };
    let _ = std::fs::remove_file(&xml_path);
    if !ok_add {
        return CmdResponse {
            success: false,
            output: out_add.clone(),
            stderr: format!("netsh add profile falló:\n{}", out_add),
            exit_code: None,
        };
    }
    // Conectar por nombre de perfil (= SSID).
    let profile = ssid.trim();
    let (ok_con, out_con) = match netsh(&app, &["wlan", "connect", &format!("name=\"{}\"", profile)]).await {
        Ok(r) => r,
        Err(e) => return CmdResponse { success: false, output: out_add, stderr: e, exit_code: None },
    };
    // Verificar estado real de la interfaz.
    let connected = match netsh(&app, &["wlan", "show", "interfaces"]).await {
        Ok((_, info)) => {
            let low = info.to_lowercase();
            low.contains(&profile.to_lowercase()) && (low.contains("conectado") || low.contains("connected"))
        }
        Err(_) => false,
    };
    let ok = ok_con && connected;
    CmdResponse {
        success: ok,
        output: format!(
            "{}\n---\n{}\n---\nEstado: {}",
            out_add.trim(),
            out_con.trim(),
            if ok { format!("CONECTADO a {}", profile) } else { "NO verificado: revisa interfaces".to_string() }
        ),
        stderr: String::new(),
        exit_code: Some(if ok { 0 } else { 1 }),
    }
}

/// Comando: desconecta la interfaz Wi-Fi.
#[command]
pub async fn wifi_disconnect(app: AppHandle) -> CmdResponse {
    match netsh(&app, &["wlan", "disconnect"]).await {
        Ok((ok, out)) => CmdResponse { success: ok, output: out, stderr: String::new(), exit_code: Some(ok as i32) },
        Err(e) => CmdResponse { success: false, output: String::new(), stderr: e, exit_code: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escapes_and_shapes() {
        let x = wlan_profile_xml("Mi&Red<1>", "p\"a's<s>");
        assert!(x.contains("<name>Mi&amp;Red&lt;1&gt;</name>"));
        assert!(x.contains("<keyMaterial>p&quot;a&apos;s&lt;s&gt;</keyMaterial>"));
        assert!(x.contains("<authentication>WPA2PSK</authentication>"));
        assert!(!x.contains("Mi&Red"));
    }
}
