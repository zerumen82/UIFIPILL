// Gatillo EAPOL para captura pasiva en Windows (dos antenas): asocia una interfaz
// en modo managed con contrasena incorrecta para provocar M1/M2 (y PMKID si el
// AP lo expone) mientras otra antena captura en monitor. Sin inyeccion Npcap.
// Solo laboratorio / redes propias o con autorizacion explícita.
use crate::commands::{CmdResponse, emit_chunked};
use crate::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{command, AppHandle, Emitter, State};
use tauri_plugin_shell::ShellExt;

static TRIGGER_CANCEL: once_cell::sync::Lazy<Mutex<Option<std::sync::Arc<AtomicBool>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn emit(app: &AppHandle, id: &str, msg: &str) {
    emit_chunked(app, id, "stdout", msg);
}

async fn netsh(app: &AppHandle, args: &[&str]) -> (i32, String, String) {
    let shell = app.shell();
    let out = shell.command("netsh").args(args).output().await;
    match out {
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            (
                code,
                String::from_utf8_lossy(&o.stdout).into_owned(),
                String::from_utf8_lossy(&o.stderr).into_owned(),
            )
        }
        Err(e) => (-1, String::new(), e.to_string()),
    }
}

#[command]
pub async fn wpa2_trigger_bg(
    app: AppHandle,
    _state: State<'_, AppState>,
    ssid: String,
    interface: String,
    attempts: Option<u32>,
    delay_seconds: Option<u32>,
    password: Option<String>,
) -> Result<serde_json::Value, String> {
    let attempts = attempts.unwrap_or(5).max(1);
    let delay = delay_seconds.unwrap_or(3);
    let password = password.unwrap_or_else(|| "clavefalsa123".to_string());
    let profile_name = ssid.clone();

    if password.len() < 8 {
        return Err("La contrasena falsa debe tener al menos 8 caracteres (WPA2-PSK).".into());
    }

    let attack_id = "wpa2_trigger".to_string();
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    {
        let mut g = TRIGGER_CANCEL.lock().unwrap();
        *g = Some(cancel.clone());
    }

    let _ = app.emit("attack-started", serde_json::json!({
        "id": attack_id,
        "bg": true,
        "cmdline": format!("[trigger] SSID='{}' iface='{}' attempts={} delay={}s", ssid, interface, attempts, delay)
    }));

    let app2 = app.clone();
    let cancel2 = cancel.clone();
    let attack_id_ret = attack_id.clone();
    let ssid2 = ssid.clone();
    let interface2 = interface.clone();
    let password2 = password.clone();
    let profile_name2 = profile_name.clone();

    tauri::async_runtime::spawn(async move {
        let mut attempts_done: u32 = 0;
        let mut lines: Vec<String> = Vec::new();
        let mut ok = false;

        let xml = format!(
            r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>{profile}</name>
  <SSIDConfig><SSID><name>{ssid}</name></SSID></SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>manual</connectionMode>
  <MSM><security>
    <authEncryption>
      <authentication>WPA2PSK</authentication>
      <encryption>AES</encryption>
      <useOneX>false</useOneX>
    </authEncryption>
    <sharedKey>
      <keyType>passPhrase</keyType>
      <protected>false</protected>
      <keyMaterial>{pass}</keyMaterial>
    </sharedKey>
  </security></MSM>
</WLANProfile>"#,
            profile = xml_escape(&profile_name2),
            ssid = xml_escape(&ssid2),
            pass = xml_escape(&password2)
        );

        let tmp_path = std::env::temp_dir().join(format!(
            "wpa2_trigger_{}.xml",
            std::process::id()
        ));
        if let Err(e) = std::fs::write(&tmp_path, &xml) {
            let msg = format!("[trigger] ERROR: no se pudo escribir el perfil temporal: {}", e);
            emit(&app2, &attack_id, &format!("{}\n", msg));
            lines.push(msg);
            finish(&app2, &attack_id, &cancel2, attempts_done, false, &lines);
            return;
        }

        let msg = format!(
            "[trigger] SSID='{}' interfaz='{}' intentos={} espera={}s (solo lab)\n[trigger] Asegura que la antena de captura (monitor, canal del AP) ya esta capturando.\n",
            ssid2, interface2, attempts, delay
        );
        emit(&app2, &attack_id, &msg);
        lines.push(msg.trim_end().to_string());

        let _ = netsh(&app2, &["wlan", "delete", "profile", &format!("name={}", profile_name2), &format!("interface={}", interface2)]).await;

        let (code, so, se) = netsh(
            &app2,
            &[
                "wlan",
                "add",
                "profile",
                &format!("filename={}", tmp_path.to_string_lossy()),
                &format!("interface={}", interface2),
            ],
        )
        .await;
        if code != 0 {
            let msg = format!(
                "[trigger] ERROR: netsh wlan add profile fallo (exit {}). {}\n",
                code,
                se.lines().next().unwrap_or(&so)
            );
            emit(&app2, &attack_id, &msg);
            lines.push(msg.trim_end().to_string());
            let _ = std::fs::remove_file(&tmp_path);
            finish(&app2, &attack_id, &cancel2, attempts_done, false, &lines);
            return;
        }
        emit(&app2, &attack_id, "[trigger] Perfil temporal anadido.\n");

        for i in 1..=attempts {
            if cancel2.load(Ordering::Relaxed) {
                lines.push("Plan cancelado.".into());
                emit(&app2, &attack_id, "Plan cancelado.\n");
                break;
            }

            let msg = format!("[trigger] Intento {}/{}: netsh wlan connect name=\"{}\" interface=\"{}\"", i, attempts, profile_name2, interface2);
            emit(&app2, &attack_id, &format!("{}\n", msg));
            lines.push(msg);

            let (code, so, se) = netsh(
                &app2,
                &[
                    "wlan",
                    "connect",
                    &format!("name={}", profile_name2),
                    &format!("interface={}", interface2),
                ],
            )
            .await;
            let detail = if se.trim().is_empty() {
                so.trim().to_string()
            } else {
                se.trim().to_string()
            };
            if !detail.is_empty() {
                emit(&app2, &attack_id, &format!("{}\n", detail));
                lines.push(detail);
            }
            let _ = code;

            attempts_done = i;

            if i < attempts {
                for _ in 0..delay {
                    if cancel2.load(Ordering::Relaxed) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                if cancel2.load(Ordering::Relaxed) {
                    lines.push("Plan cancelado.".into());
                    emit(&app2, &attack_id, "Plan cancelado.\n");
                    break;
                }
                let _ = netsh(&app2, &["wlan", "disconnect", &format!("interface={}", interface2)]).await;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        let _ = netsh(&app2, &["wlan", "delete", "profile", &format!("name={}", profile_name2), &format!("interface={}", interface2)]).await;
        let _ = std::fs::remove_file(&tmp_path);

        if attempts_done >= attempts {
            ok = true;
        }

        let summary = if cancel2.load(Ordering::Relaxed) {
            format!(
                "Gatillo cancelado ({} de {} intentos).\n\n{}",
                attempts_done,
                attempts,
                lines.join("\n")
            )
        } else {
            format!(
                "Gatillo completado: {} intento(s) de asociacion con contrasena incorrecta.\n\
                 Revisa la captura (antena en monitor, canal del AP) y convierte a .22000 (pcap_to_22000 / hcxpcapngtool).\n\
                 Verifica que exista WPA*01* (PMKID) o WPA*02* (M1+M2) antes de crackear.\n\n{}",
                attempts_done,
                lines.join("\n")
            )
        };

        emit(&app2, &attack_id, &format!("[trigger] {}\n", summary));
        finish(&app2, &attack_id, &cancel2, attempts_done, ok, &lines);
    });

    Ok(serde_json::json!({
        "attack_id": attack_id_ret,
        "attempts": attempts,
        "delay_seconds": delay
    }))
}

fn finish(
    app: &AppHandle,
    attack_id: &str,
    cancel: &std::sync::Arc<AtomicBool>,
    attempts_done: u32,
    success: bool,
    lines: &[String],
) {
    let summary = if cancel.load(Ordering::Relaxed) {
        format!("Gatillo cancelado ({} intento(s)).\n\n{}", attempts_done, lines.join("\n"))
    } else if success {
        format!(
            "Gatillo completado: {} intento(s).\n\n{}",
            attempts_done,
            lines.join("\n")
        )
    } else {
        format!(
            "Gatillo terminado ({} intento(s)).\n\n{}",
            attempts_done,
            lines.join("\n")
        )
    };
    let _ = app.emit("attack-completed", serde_json::json!({
        "id": attack_id,
        "success": success,
        "stdout": summary,
        "stderr": "",
        "exit_code": if success { 0 } else { 1 }
    }));
    let mut g = TRIGGER_CANCEL.lock().unwrap();
    *g = None;
}

#[command]
pub async fn wpa2_trigger_cancel() -> CmdResponse {
    let g = TRIGGER_CANCEL.lock().unwrap();
    if let Some(c) = g.as_ref() {
        c.store(true, Ordering::Relaxed);
        CmdResponse {
            success: true,
            output: "Cancelacion del gatillo solicitada.".into(),
            stderr: String::new(),
            exit_code: Some(-1),
        }
    } else {
        CmdResponse {
            success: false,
            output: "No hay gatillo en ejecucion.".into(),
            stderr: String::new(),
            exit_code: None,
        }
    }
}
