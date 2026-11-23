// TX por USB crudo al RT3070 (WinUSB) — envuelve los binarios de driver_re/usb_tx.
//
// Por qué existe: Npcap NO inyecta en Windows (Npcap #85) y la línea de RE del
// driver netr28ux quedó cerrada. La vía verificada el 2026-09-22 es el acceso
// WinUSB al device (rebind documentado en driver_re/usb_tx/) con el protocolo
// vendor del rt2800usb portado a Rust:
//   rt3070_probe → identidad del chip (ASIC 0x30700201)
//   rt3070_diag  → diagnóstico fino del protocolo vendor
//   rt3070_init  → init_radio SIN kick FIRMWARE(8) (prohibido bajo WinUSB):
//                  MCU vivo → radio ON + canal; MCU muerto → error accionable
//                  (power-cycle físico), nunca el kick.
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
            dirs.push(p.join("resources")); // NSIS aplana los resources a esta carpeta
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

/// Init radio: MCU vivo → radio ON + canal (1-14). SIN kick FIRMWARE(8)
/// (init_radio en common.rs lo tiene prohibido bajo WinUSB).
#[command]
pub async fn usb_raw_init(channel: u8) -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let chan = channel.clamp(1, 14);
        let exe = match usb_tx_exe("rt3070_init.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        run_usb_tool(&exe, &[&chan.to_string()])
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
pub async fn usb_raw_deauth(
    bssid: String,
    channel: u8,
    count: u32,
    target_mac: Option<String>,
    reason: Option<u8>,
    delay_ms: Option<u32>,
) -> UsbRawResult {
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
        let da = match target_mac {
            Some(m) => {
                let m = m.trim().to_uppercase();
                let ok = m.len() == 17 && m.chars().enumerate().all(|(i, c)| i % 3 == 2 || c.is_ascii_hexdigit());
                if !ok {
                    return UsbRawResult { success: false, message: format!("MAC de cliente inválida: {m}"), output: String::new() };
                }
                m
            }
            None => "broadcast".to_string(),
        };
        let reason = reason.unwrap_or(7).min(255).to_string();
        let delay = delay_ms.unwrap_or(100).clamp(20, 1000).to_string();
        let r = run_usb_tool(&exe, &[&b, &chan.to_string(), &n, &da, &reason, &delay]);
        let written = r.output.lines().find(|l| l.contains("deauth frames escritos")).map(|s| s.to_string());
        let txdone = r.output.lines().filter(|l| l.contains("TXDONE")).count();
        let txdone_ok = r.output.lines().any(|l| l.contains("TXDONE success=true"));
        // `contains("0/")` daba TRUE para "0/8" y FALSE para "2/8": invertido.
        let n_ok = written.as_deref()
            .and_then(|s| s.split_whitespace().next())
            .and_then(|x| x.split('/').next())
            .and_then(|x| x.parse::<u32>().ok())
            .unwrap_or(0);
        let mut msg = written.unwrap_or_else(|| r.message.clone());
        if txdone > 0 {
            msg.push_str(&format!(" | TXDONE: {txdone} confirmaciones{}", if txdone_ok { " (TX al aire OK)" } else { " (FALLOS de TX)" }));
        }
        UsbRawResult {
            success: r.success && n_ok > 0 && txdone_ok,
            message: msg,
            output: r.output,
        }
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

/// Sniff: RX por EP 0x81 con el MISMO chip (sin Npcap). Vuelca pcap DLT 127.
#[command]
pub async fn usb_raw_sniff(duration_secs: u32, channel: u8, output_path: Option<String>) -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let secs_n = duration_secs.clamp(5, 300);
        let secs = secs_n.to_string();
        let exe = match usb_tx_exe("rt3070_sniff.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        let out = output_path.unwrap_or_else(|| {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0);
            // Command NO expande %TEMP% (medido: la ruta literal impedía crear
            // el fichero) → resolver la ruta real del usuario.
            std::env::temp_dir().join(format!("uifipill_sniff_{t}.pcap"))
                .to_string_lossy().into_owned()
        });
        let _ = channel; // el canal lo fija usb_raw_init antes
        let r = run_usb_tool(&exe, &[&secs, &out]);
        let mut res = r;
        // Evidencia: la línea "Capturados N frames → ruta" manda sobre "OK".
        if let Some(line) = res.output.lines().find(|l| l.contains("Capturados")) {
            res.message = line.trim().to_string();
        }
        // Éxito honesto (regla 2): 0 frames NO es éxito, aunque el bin salga 0.
        if res.output.contains("Capturados 0 frames") {
            res.success = false;
            res.message = format!(
                "0 frames en {secs}s: RX no armado o canal sin tráfico. \
                 Ejecuta «Init radio» primero (init_radio sin kick) y usa el \
                 canal del AP; si init_radio reporta MCU muerto → power-cycle 15 s."
            );
        }
        res
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

/// Extrae "Capturados N frames" del stdout de rt3070_sniff; si el proceso fue
/// matado antes del resumen final (cancel ■ / watchdog), usa el último
/// `PROGRESS t=… frames=N`. `None` = no hay evidencia de frames.
fn sniff_frames(output: &str) -> Option<usize> {
    for l in output.lines().rev() {
        if let Some(rest) = l.split("Capturados ").nth(1) {
            if let Some(tok) = rest.split_whitespace().next() {
                if let Ok(n) = tok.parse::<usize>() {
                    return Some(n);
                }
            }
        }
        if let Some(rest) = l.split("frames=").nth(1) {
            if let Some(tok) = rest.split(|c: char| !c.is_ascii_digit()).next() {
                if let Ok(n) = tok.parse::<usize>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Sniff en BACKGROUND sostenido (épica E0-11): hasta 30 min de escucha con
/// streaming `attack-progress` (PROGRESS cada 10 s del bin), cancelable con
/// `cancel_attack` (■ Detener) y watchdog `secs+30` si el bin se cuelga.
/// Éxito honesto (regla 2): solo con frames > 0; el pcap se genera en `%TEMP%`
/// (resuelto con `std::env::temp_dir`) salvo que se pase `output_path`.
#[command]
pub async fn usb_raw_sniff_bg(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    duration_secs: u32,
    channel: u8,
    output_path: Option<String>,
) -> Result<UsbRawResult, String> {
    // ≥5 min (CA E0-11) y techo de 30 min; id fijo = el que claimAttack pone
    // en la UI ('usb_sniff_long') para que los eventos encajen (regla 6).
    // NOTA: async + State exige retorno Result (E0277 del macro de Tauri).
    let secs_n = duration_secs.clamp(10, 1800);
    let attack_id = "usb_sniff_long";
    let exe = match usb_tx_exe("rt3070_sniff.exe") {
        Ok(e) => e,
        Err(e) => return Ok(UsbRawResult { success: false, message: e, output: String::new() }),
    };
    let out = output_path.unwrap_or_else(|| {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs()).unwrap_or(0);
        std::env::temp_dir().join(format!("uifipill_sniff_long_{t}.pcap"))
            .to_string_lossy().into_owned()
    });
    let chan = channel.clamp(1, 14).to_string();
    let args = vec![secs_n.to_string(), out.clone(), chan];
    let r = crate::commands::run_path_bg(
        &app, &state, attack_id, &exe, &args, Some(secs_n as u64 + 30),
    ).await;
    let frames = sniff_frames(&r.output);
    let n = frames.unwrap_or(0);
    let mut message = match frames {
        Some(_) if n > 0 => format!("Capturados {n} frames → {out}"),
        Some(_) => format!(
            "0 frames en {secs_n}s: RX no armado («Init radio» primero, sin kick) \
             o canal sin tráfico; MCU muerto → power-cycle 15 s."
        ),
        None => format!("Sin resumen de frames (proceso matado antes de reportar) → pcap: {out}"),
    };
    if n > 0 && !r.success {
        message.push_str(" | detenido antes de tiempo (cancel/watchdog) — captura parcial conservada");
    }
    Ok(UsbRawResult { success: n > 0, message, output: r.output })
}

/// Devuelve la antena a WiFi normal (rebind netr28ux) lanzando el script de lab
/// auto-elevado (`restore_netr28ux_run.ps1` → `restore_netr28ux.ps1`) con UAC.
/// SOLO dispara el proceso (nunca un éxito simulado, regla 2): la VERIFICACIÓN
/// real de que la antena vuelve a ser WiFi la hace el frontend con
/// `detect_adapters` en polling. Log del script: `restore_run.log` (ruta absoluta).
#[command]
pub async fn usb_raw_restore() -> UsbRawResult {
    let dir = match usb_tx_dir() {
        Some(d) => d,
        None => {
            return UsbRawResult {
                success: false,
                message: "❌ Binarios usb_tx no encontrados: no hay dónde buscar restore_netr28ux.ps1".into(),
                output: String::new(),
            }
        }
    };
    let script = ["restore_netr28ux_run.ps1", "restore_netr28ux.ps1"]
        .iter()
        .map(|s| dir.join(s))
        .find(|p| p.exists());
    let script = match script {
        Some(p) => p,
        None => {
            return UsbRawResult {
                success: false,
                message: format!(
                    "❌ restore_netr28ux*.ps1 no está en {} — restauración manual: \
                     ejecuta ese script como administrador.",
                    dir.display()
                ),
                output: String::new(),
            }
        }
    };
    let script_s = script.to_string_lossy().replace('\'', "''");
    // RunAs sobre el wrapper: si el script interno se auto-eleva no habrá UAC
    // doble (guard IsInRole); si NO se auto-eleva, este RunAs es obligatorio.
    let cmd = format!(
        "Start-Process -FilePath powershell.exe -ArgumentList \
         '-NoProfile','-ExecutionPolicy','Bypass','-File','{}' -Verb RunAs",
        script_s
    );
    let spawn = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &cmd])
        .spawn();
    match spawn {
        Ok(_) => UsbRawResult {
            success: true,
            message: format!(
                "Elevación lanzada (UAC) → {script_s}. Acepta el prompt: la antena \
                 debe volver a ser WiFi; la app lo verifica con detect_adapters."
            ),
            output: format!("$ powershell -Command {cmd}\n"),
        },
        Err(e) => UsbRawResult {
            success: false,
            message: format!("❌ No se pudo lanzar la elevación: {e}"),
            output: String::new(),
        },
    }
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
    pub security: String, // detectada en el beacon: WPA3/WPA2/WPA/Abierta o WEP
}

#[derive(Serialize)]
pub struct UsbRawScanResult {
    pub success: bool,
    pub message: String,
    pub networks: Vec<UsbRawNet>,
}

/// Escaneo de redes con el RT3070 en WinUSB (channel hopping 1-13 + parseo de
/// beacons). Devuelve la lista lista para la tabla de la UI.
#[command]
pub async fn usb_raw_scan(duration_secs: Option<u32>) -> UsbRawScanResult {
    tauri::async_runtime::spawn_blocking(move || {
        let secs = duration_secs.unwrap_or(20).clamp(8, 120);
        let exe = match usb_tx_exe("rt3070_scan.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawScanResult { success: false, message: e, networks: vec![] },
        };
        let r = run_usb_tool(&exe, &[&secs.to_string()]);
        let mut networks = Vec::new();
        for l in r.output.lines() {
            if let Some(rest) = l.strip_prefix("AP|") {
                let p: Vec<&str> = rest.split('|').collect();
                if p.len() >= 4 {
                    let bssid = p[1].trim().to_uppercase();
                    if bssid.len() == 17 && bssid.chars().filter(|c| *c == ':').count() == 5 {
                        // 6º campo (2026-10-01): seguridad del beacon; los bins
                        // viejos de 5 campos → "Desconocida" (honesto, sin inventar).
                        let security = if p.len() >= 5 {
                            let s = p[4].trim().chars().take(16).filter(|c| c.is_ascii_alphanumeric() || " ?/-".contains(*c)).collect::<String>();
                            if s.is_empty() { "Desconocida".into() } else { s }
                        } else {
                            "Desconocida".into()
                        };
                        networks.push(UsbRawNet {
                            ssid: p[0].trim().to_string(),
                            bssid,
                            channel: p[2].trim().parse().unwrap_or(0),
                            signal: p[3].trim().parse().unwrap_or(0),
                            security,
                        });
                    }
                }
            }
        }
        let ok = r.success && !networks.is_empty();
        let msg = if !r.success {
            r.message.clone()
        } else if networks.is_empty() {
            // 0 beacons con chip vivo y radio inicializada = causa medida:
            // MCU muerto tras power-cycle (init_radio lo reporta y exige
            // power-cycle; el kick FIRMWARE(8) está prohibido bajo WinUSB).
            format!(
                "Sin redes en {secs}s: (1) radio sin init — pulsa «Init radio»/«Estado chip»; \
                 (2) MCU muerto tras power-cycle — desenchufa 15 s (init_radio NO kickea: \
                 FIRMWARE(8) mata el chip 5/5); (3) canal sin tráfico (raro)."
            )
        } else {
            format!("{} redes por USB crudo", networks.len())
        };
        UsbRawScanResult { success: ok, message: msg, networks }
    }).await.unwrap_or_else(|_| UsbRawScanResult { success: false, message: "join error".into(), networks: vec![] })
}

/// E0-25: enviar Authentication + Association Request por USB crudo para
/// provocar que el AP devuelva un PMKID en su Association Response.
#[command]
pub async fn usb_raw_assoc(ssid: String, bssid: String, channel: u8) -> UsbRawResult {
    tauri::async_runtime::spawn_blocking(move || {
        let exe = match usb_tx_exe("rt3070_assoc.exe") {
            Ok(e) => e,
            Err(e) => return UsbRawResult { success: false, message: e, output: String::new() },
        };
        let b = bssid.trim().to_uppercase();
        let hex_ok = b.len() == 17 && b.chars().enumerate().all(|(i, c)| i % 3 == 2 || c.is_ascii_hexdigit());
        if !hex_ok || ssid.trim().is_empty() {
            return UsbRawResult { success: false, message: "SSID o BSSID inválido".into(), output: String::new() };
        }
        let chan = channel.clamp(1, 14).to_string();
        // Igual que el deauth: sin init previo el FIFO TX hace timeout.
        let init = usb_tx_exe("rt3070_init.exe").ok()
            .map(|i| run_usb_tool(&i, &[&chan]))
            .unwrap_or(UsbRawResult { success: false, message: "init no encontrado".into(), output: String::new() });
        let mut out = init.output;
        let r = run_usb_tool(&exe, &[ssid.trim(), &b, &chan]);
        out.push_str("\n--- assoc ---\n");
        out.push_str(&r.output);
        UsbRawResult { success: r.success, message: r.message, output: out }
    }).await.unwrap_or_else(|_| UsbRawResult { success: false, message: "join error".into(), output: String::new() })
}

// ── E0-20 fase 1: preflight de TX (canal del RT3070 con read-back) ───────────
#[derive(Serialize)]
pub struct TxPreflightResult {
    pub ok: bool,
    pub requested: u8,
    pub actual: Option<u8>,
    pub monitor_channel: Option<u8>,
    pub message: String,
}

/// Fija el canal del RT3070 (WinUSB) al del objetivo y lo lee de vuelta desde
/// RFCSR2/3/6. Si no coincide con el canal REAL del monitor RT73, aborta:
/// transmitir en otro canal es quemar frames a ciegas. Nunca kick FIRMWARE(8).
#[command]
pub async fn usb_raw_tx_preflight(channel: u8, monitor_channel: Option<u8>) -> TxPreflightResult {
    tauri::async_runtime::spawn_blocking(move || {
        let requested = channel.clamp(1, 14);
        let exe = match usb_tx_exe("rt3070_chan_preflight.exe") {
            Ok(e) => e,
            Err(e) => return TxPreflightResult { ok: false, requested, actual: None, monitor_channel, message: e },
        };
        let r = run_usb_tool(&exe, &[&requested.to_string()]);
        let line = r.output.lines().find(|l| l.trim_start().starts_with('{')).unwrap_or("");
        #[derive(serde::Deserialize)]
        struct R { ok: Option<bool>, actual: Option<u8>, error: Option<String>, message: Option<String> }
        let parsed: Option<R> = serde_json::from_str(line.trim()).ok();
        let actual = parsed.as_ref().and_then(|p| p.actual);
        let bin_ok = parsed.as_ref().and_then(|p| p.ok).unwrap_or(false) && r.success;
        let mon_ok = monitor_channel.map(|m| Some(m) == actual).unwrap_or(true);
        let ok = bin_ok && mon_ok;
        let detail = parsed.as_ref().and_then(|p| p.error.clone().or_else(|| p.message.clone())).unwrap_or_default();
        let message = if !bin_ok {
            format!("❌ No se pudo fijar/leer el canal del RT3070 (pedido {requested}). {detail} {}",
                if r.output.is_empty() { String::new() } else { r.output.lines().take(2).collect::<Vec<_>>().join(" ") })
        } else if !mon_ok {
            format!("❌ Canal TX {actual:?} ≠ canal monitor {monitor_channel:?}. Abortando ráfaga: el deauth iría al aire equivocado.")
        } else {
            format!("✅ RT3070 en CH{} (pedido {requested}), coincide con el monitor. Listo para TX.", actual.unwrap_or(requested))
        };
        TxPreflightResult { ok, requested, actual, monitor_channel, message }
    }).await.unwrap_or_else(|_| TxPreflightResult {
        ok: false, requested: channel, actual: None, monitor_channel, message: "join error".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chan_preflight_json() {
        #[derive(serde::Deserialize)]
        struct R { ok: Option<bool>, actual: Option<u8>, error: Option<String>, message: Option<String> }
        let line = "{\"ok\":true,\"requested\":4,\"actual\":4,\"n\":242,\"r\":2,\"k\":7}";
        let p: R = serde_json::from_str(line).unwrap();
        assert_eq!(p.actual, Some(4));
        assert_eq!(p.ok, Some(true));
    }

    #[test]
    fn sniff_frames_from_final_summary() {
        let out = "✅ RX armado\n✅ Capturados 524 frames → C:\\tmp\\a.pcap\n   Convertir: pcap_to_22000";
        assert_eq!(sniff_frames(out), Some(524));
    }

    #[test]
    fn sniff_frames_zero_is_honest() {
        let out = "✅ Capturados 0 frames → C:\\tmp\\z.pcap\n⚠️ 0 frames en 300s";
        assert_eq!(sniff_frames(out), Some(0));
    }

    #[test]
    fn sniff_frames_falls_back_to_progress_when_killed() {
        // Proceso cancelado (kill): sin línea "Capturados" final.
        let out = "$ rt3070_sniff\nPROGRESS t=120s/300s frames=10342 pcap=1234567B\nPROGRESS t=130s/300s frames=11001 pcap=1310720B\n";
        assert_eq!(sniff_frames(out), Some(11001));
    }

    #[test]
    fn sniff_frames_missing_evidence() {
        assert_eq!(sniff_frames("No se pudo abrir RT3070"), None);
        assert_eq!(sniff_frames("⚠️ read_bulk: Pipe"), None);
    }
}

