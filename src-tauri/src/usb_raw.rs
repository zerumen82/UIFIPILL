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
        // "éxito" honesto: frames escritos al chip != garantía de salida al aire.
        // Feedback real: las líneas "TXDONE success=true" de TX_STA_FIFO confirman
        // que el chip procesó (transmitió) los frames, no solo que el USB los aceptó.
        let written = r.output.lines().find(|l| l.contains("frames escritos")).map(|s| s.to_string());
        let txdone = r.output.lines().filter(|l| l.contains("TXDONE")).count();
        let txdone_ok = r.output.lines().any(|l| l.contains("TXDONE success=true"));
        let had_written = written.is_some();
        let mut msg = written.unwrap_or_else(|| r.message.clone());
        if txdone > 0 {
            msg.push_str(&format!(" | TXDONE: {txdone} confirmaciones{}", if txdone_ok { " (TX al aire OK)" } else { " (FALLOS de TX: ¿canal/PA?)" }));
        }
        UsbRawResult {
            success: r.success && had_written,
            message: msg,
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
        let txdone = r.output.lines().filter(|l| l.contains("TXDONE")).count();
        let txdone_ok = r.output.lines().any(|l| l.contains("TXDONE success=true"));
        let had_written = written.is_some();
        let mut msg = written.unwrap_or_else(|| r.message.clone());
        if txdone > 0 {
            msg.push_str(&format!(" | TXDONE: {txdone} confirmaciones{}", if txdone_ok { " (TX al aire OK)" } else { " (FALLOS de TX)" }));
        }
        UsbRawResult {
            success: r.success && had_written && r.output.contains("0/"),
            message: msg,
            output: r.output,
        }
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

/// Sniff: RX por EP 0x81 con el MISMO chip (sin Npcap). Vuelca pcap DLT 127.
#[command]
pub async fn usb_raw_sniff(duration_secs: u32, channel: u8, output_path: Option<String>) -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let secs = duration_secs.clamp(5, 300).to_string();
        let exe = match usb_tx_exe("rt3070_sniff.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        let out = output_path.unwrap_or_else(|| {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0);
            format!("%TEMP%\\uifipill_sniff_{t}.pcap")
        });
        let _ = channel; // el canal lo fija usb_raw_init antes
        let r = run_usb_tool(&exe, &[&secs, &out]);
        let mut res = r;
        // extraer ruta real del pcap del output
        if let Some(line) = res.output.lines().find(|l| l.contains("→")) {
            if let Some(p) = line.split("→").last() {
                res.message = format!("{} (pcap: {})", res.message, p.trim());
            }
        }
        res
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

// ── Scan por USB crudo (como Linux: beacons → lista de redes) ─────────────

#[derive(Serialize)]
pub struct UsbRawNet {
    pub ssid: String,
    pub bssid: String,
    pub channel: u8,
    pub signal: u8, // % estilo netsh
}

#[derive(Serialize)]
pub struct UsbRawScanResult {
    pub success: bool,
    pub message: String,
    pub networks: Vec<UsbRawNet>,
}

/// Escaneo de redes con el RT3070 en WinUSB (channel hopping 1-13 + parseo de
/// beacons). Devuelve la lista lista para la tabla de la UI. Duración fija
/// 20s (ronda completa de hopping con dwell 250ms/canal ~ 3 pasadas).
#[command]
pub async fn usb_raw_scan(duration_secs: Option<u32>) -> UsbRawScanResult {
    tauri::async_runtime::spawn_blocking(move || {
        let secs = duration_secs.unwrap_or(20).clamp(8, 120);
        let exe = match usb_tx_exe("rt3070_scan.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawScanResult { success: false, message: e, networks: vec![] },
        };
        let fw = firmware_path().unwrap_or_else(|| "rt2870.bin".into());
        let r = run_usb_tool(&exe, &[&secs.to_string(), &fw]);
        let mut networks = Vec::new();
        for l in r.output.lines() {
            if let Some(rest) = l.strip_prefix("AP|") {
                let p: Vec<&str> = rest.split('|').collect();
                if p.len() == 4 {
                    let bssid = p[1].trim().to_uppercase();
                    if bssid.len() == 17 && bssid.chars().filter(|c| *c == ':').count() == 5 {
                        networks.push(UsbRawNet {
                            ssid: p[0].trim().to_string(),
                            bssid,
                            channel: p[2].trim().parse().unwrap_or(0),
                            signal: p[3].trim().parse().unwrap_or(0),
                        });
                    }
                }
            }
        }
        let ok = r.success && !networks.is_empty();
        let msg = if !r.success {
            r.message.clone()
        } else if networks.is_empty() {
            // 2026-09-23: 0 frames con chip vivo = BBP mudo (bloqueo conocido,
            // ver AGENTS.md «TEST HARDWARE REAL 2026-09-23»). Mensaje honesto
            // con las 2 causas medidas, no genérico.
            "Sin redes: chip vivo pero 0 frames recibidos. Causas medidas: (1) BBP mudo — power-cycle (desenchufa 15s) y reintenta; (2) si persiste, mira rt3070_bbpdiag (efuse/EEPROM) — init BBP+RFCSR completo ya portado en common.rs".into()
        } else {
            format!("{} redes por USB crudo", networks.len())
        };
        UsbRawScanResult { success: ok, message: msg, networks }
    }).await.unwrap_or_else(|_| UsbRawScanResult { success: false, message: "join error".into(), networks: vec![] })
}
