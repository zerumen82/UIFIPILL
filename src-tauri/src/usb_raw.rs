// TX por USB crudo al RT3070 (WinUSB) — envuelve los binarios de driver_re/usb_tx.
//
// Por qué existe: Npcap NO inyecta en Windows (Npcap #85) y la línea de RE del
// driver netr28ux quedó cerrada. La vía verificada el 2026-09-22 es el acceso
// WinUSB al device (rebind documentado en driver_re/usb_tx/) con el protocolo
// vendor del rt2800usb portado a Rust:
//   rt3070_probe → identidad del chip (ASIC 0x30700201)
//   rt3070_diag  → diagnóstico fino del protocolo vendor
//   rt3070_init  → carga firmware (rt2870.bin) + radio ON + canal
//   rt3070_tx    → TX de beacons (TXINFO+TXWI+802.11 → EP 0x01)
//
// Los binarios viven en driver_re/usb_tx/target/release (dev) o
// <inst>/_up_/driver_re/usb_tx/bin + resources (NSIS, como el resto de tools).
// El firmware rt2870.bin se resuelve junto a los exes.
use serde::Serialize;
use tauri::command;

#[derive(Serialize)]
pub struct UsbRawResult {
    pub success: bool,
    pub message: String,
    pub output: String,
}

/// Resuelve la carpeta base de los binarios usb_tx.
fn usb_tx_dir() -> Option<std::path::PathBuf> {
    // 1) Junto al exe (dev: src-tauri/target/debug; NSIS: <inst> o <inst>/_up_)
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            dirs.push(p.join("usb_tx"));
            dirs.push(p.join("_up_").join("driver_re").join("usb_tx"));
            dirs.push(p.join("resources").join("_up_").join("driver_re").join("usb_tx"));
            dirs.push(p.join("resources").join("usb_tx"));
        }
    }
    // 2) Workspace (dev): <repo>/driver_re/usb_tx
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        if let Some(ws) = std::path::Path::new(&manifest).parent() {
            dirs.push(ws.join("driver_re").join("usb_tx"));
        }
    }
    // 3) CWD (cuando la app corre desde el repo)
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("driver_re").join("usb_tx"));
        dirs.push(cwd.join("..").join("driver_re").join("usb_tx"));
    }
    dirs.into_iter().find(|d| d.join("rt3070_probe.exe").exists() || d.join("target/release/rt3070_probe.exe").exists())
}

/// Ruta de un binario usb_tx (dev: target/release, instalado: junto a la app).
fn usb_tx_exe(name: &str) -> Result<String, String> {
    let dir = usb_tx_dir().ok_or_else(|| {
        "❌ Binarios usb_tx no encontrados (rt3070_probe.exe).\n\n\
         Compílalos: cd driver_re/usb_tx && cargo build --release\n\
         (o cópialos a <instalación>/driver_re/usb_tx/bin)"
    })?;
    for cand in [dir.join("target/release").join(name), dir.join("bin").join(name), dir.join(name)] {
        if cand.exists() {
            return Ok(cand.to_string_lossy().into_owned());
        }
    }
    Err(format!("❌ {} no encontrado en {}", name, dir.display()))
}

/// Ruta del firmware rt2870.bin (junto a los exes o en driver_re/usb_tx).
fn firmware_path() -> Option<String> {
    let dir = usb_tx_dir()?;
    let fw = dir.join("rt2870.bin");
    if fw.exists() {
        return Some(fw.to_string_lossy().into_owned());
    }
    None
}

/// Ejecuta un binario usb_tx de forma síncrona y captura salida.
fn run_usb_tool(exe: &str, args: &[&str]) -> UsbRawResult {
    use std::process::Command;
    let out = Command::new(exe).args(args).output();
    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
            let ok = o.status.success();
            UsbRawResult {
                success: ok,
                message: if ok { "OK".into() } else { format!("exit {:?}", o.status.code()) },
                output: format!("{stdout}{}", if stderr.is_empty() { String::new() } else { format!("\n[stderr]\n{stderr}") }),
            }
        }
        Err(e) => UsbRawResult { success: false, message: format!("spawn: {e}"), output: String::new() },
    }
}

/// Estado del RT3070 por USB crudo: ¿rebindeado a WinUSB? ¿chip vivo? ¿ASIC?
#[command]
pub async fn usb_raw_status() -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let exe = match usb_tx_exe("rt3070_probe.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        let r = run_usb_tool(&exe, &[]);
        let asic = r.output.contains("0x3070") || r.output.contains("30 70");
        UsbRawResult {
            success: r.success && asic,
            message: if r.success && asic {
                "RT3070 vivo por USB crudo (WinUSB) — TX posible".into()
            } else if r.output.contains("No se pudo abrir") {
                "Chip no accesible: ¿netr28ux reclamado? Rebindea a WinUSB (run_zadig.cmd) y re-enchufa".into()
            } else {
                r.message.clone()
            },
            output: r.output,
        }
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

/// Init completo: firmware + radio ON + canal (1-14).
#[command]
pub async fn usb_raw_init(channel: u8) -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let chan = channel.clamp(1, 14);
        let exe = match usb_tx_exe("rt3070_init.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        let fw = firmware_path().unwrap_or_else(|| "rt2870.bin".into());
        run_usb_tool(&exe, &[&chan.to_string(), &fw])
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

/// TX de N beacons con SSID arbitrario en el canal ya programado.
#[command]
pub async fn usb_raw_tx_beacon(ssid: String, channel: u8, count: u32) -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let chan = channel.clamp(1, 14);
        let exe = match usb_tx_exe("rt3070_tx.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        let n = count.clamp(1, 5000).to_string();
        let r = run_usb_tool(&exe, &[&ssid, &chan.to_string(), &n]);
        // "éxito" honesto: frames escritos al chip != garantía de salida al aire
        let written = r.output.lines().find(|l| l.contains("frames escritos")).map(|s| s.to_string());
        UsbRawResult {
            success: r.success && written.is_some(),
            message: written.unwrap_or_else(|| r.message.clone()),
            output: r.output,
        }
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

/// Deauth dirigido al BSSID objetivo por USB crudo (requiere init previo).
#[command]
pub async fn usb_raw_deauth(bssid: String, channel: u8, count: u32) -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let chan = channel.clamp(1, 14);
        let exe = match usb_tx_exe("rt3070_deauth.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        // Validación de formato BSSID (anti-inyección de args)
        let b = bssid.trim().to_uppercase();
        let hex_ok = b.len() == 17 && b.chars().enumerate().all(|(i, c)| i % 3 == 2 || c.is_ascii_hexdigit());
        if !hex_ok {
            return UsbRawResult { success: false, message: format!("BSSID inválido: {bssid}"), output: String::new() };
        }
        let n = count.clamp(1, 500).to_string();
        let r = run_usb_tool(&exe, &[&b, &chan.to_string(), &n]);
        let written = r.output.lines().find(|l| l.contains("deauth frames escritos")).map(|s| s.to_string());
        UsbRawResult {
            success: r.success && written.is_some() && r.output.contains("0/"),
            message: written.unwrap_or_else(|| r.message.clone()),
            output: r.output,
        }
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

/// Diagnóstico completo del protocolo vendor (para depurar estados raros).
#[command]
pub async fn usb_raw_diag() -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        match usb_tx_exe("rt3070_diag.exe") {
            Ok(e) => run_usb_tool(&e, &[]),
            Err(e) => UsbRawResult { success: false, message: e, output: String::new() },
        }
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}
