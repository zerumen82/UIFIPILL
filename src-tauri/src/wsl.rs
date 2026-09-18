// Puente WSL2/Kali — motor RF dual (ver ROADMAP_WSL2.md Fase 5).
//
// La UI sigue en Windows; la RF que Windows no puede (inyección, cambio de
// canal, hcxdumptool…) se ejecuta en Kali-WSL2 con el RT3070 movido vía usbipd.
// Sin dependencias nuevas: usa tauri-plugin-shell como el resto de commands.rs.
// Respuestas en snake_case (este módulo es nuevo; el frontend las lee tal cual).
use serde::Serialize;
use std::time::Duration;
use tauri::{command, AppHandle, Emitter};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

/// Distro verificada en lab (Fase 3): Kali Rolling 2026.2 + herramientas RF.
pub const DEFAULT_DISTRO: &str = "kali-linux";

#[derive(Debug, Clone, Serialize)]
pub struct WslExecResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub message: String,
}

impl WslExecResult {
    fn ok(stdout: String, stderr: String, exit_code: Option<i32>, message: String) -> Self {
        WslExecResult { success: true, stdout, stderr, exit_code, message }
    }
    fn err(msg: String) -> Self {
        WslExecResult {
            success: false,
            stdout: String::new(),
            stderr: msg.clone(),
            exit_code: None,
            message: msg,
        }
    }
}

/// Nombres de distro WSL: letras, dígitos, guion, punto, guion bajo (máx. 64).
/// Evita que `distro` se convierta en vector de inyección hacia `wsl -d`.
fn valid_distro(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= 64
        && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// busid usbipd estilo `1-5`: dígitos y guiones (máx. 16).
fn valid_busid(b: &str) -> bool {
    !b.is_empty()
        && b.len() <= 16
        && b.chars().all(|c| c.is_ascii_digit() || c == '-')
        && b.chars().any(|c| c.is_ascii_digit())
}

/// Comprueba si WSL2 está disponible ejecutando `wsl --version`.
#[command]
pub async fn wsl_is_available(app: AppHandle) -> WslExecResult {
    let shell = app.shell();
    match tokio::time::timeout(Duration::from_secs(10), shell.command("wsl").args(["--version"]).output()).await {
        Ok(Ok(o)) => {
            let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
            let ok = o.status.success() || stdout.contains("WSL");
            WslExecResult::ok(stdout, stderr, o.status.code(), if ok { "WSL2 disponible".into() } else { "WSL2 no detectado".into() })
        }
        Ok(Err(e)) => WslExecResult::err(format!("No se pudo lanzar `wsl`: {}", e)),
        Err(_) => WslExecResult::err("Timeout: `wsl --version` tardó más de 10s.".into()),
    }
}

/// Ejecuta `comando + args` DENTRO de Kali sin shell intermedia (sin inyección
/// sh): `wsl -d {distro} -- {command} {args…}`. Timeout 5–600 s (def. 60 s).
#[command]
pub async fn wsl_exec(
    app: AppHandle,
    command: String,
    args: Option<Vec<String>>,
    distro: Option<String>,
    timeout_secs: Option<u64>,
) -> WslExecResult {
    let distro = distro.unwrap_or_else(|| DEFAULT_DISTRO.into());
    if !valid_distro(&distro) {
        return WslExecResult::err(format!("Distro WSL inválida: '{}'.", distro));
    }
    if command.trim().is_empty() {
        return WslExecResult::err("Comando vacío.".into());
    }
    let timeout = timeout_secs.unwrap_or(60).clamp(5, 600);
    let shell = app.shell();
    let mut cmd_args: Vec<String> =
        vec!["-d".into(), distro.clone(), "--".into(), command.clone()];
    cmd_args.extend(args.unwrap_or_default());
    let arg_refs: Vec<&str> = cmd_args.iter().map(|s| s.as_ref()).collect();
    let out = match tokio::time::timeout(
        Duration::from_secs(timeout),
        shell.command("wsl").args(&arg_refs).output(),
    )
    .await
    {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return WslExecResult::err(format!(
                "No se pudo lanzar `wsl` (¿WSL2 instalado? `wsl --version`): {}",
                e
            ))
        }
        Err(_) => {
            return WslExecResult::err(format!(
                "Timeout: `wsl -d {} -- {}` tardó más de {}s.",
                distro, command, timeout
            ))
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    WslExecResult::ok(
        stdout,
        stderr,
        out.status.code(),
        format!(
            "wsl -d {} -- {} → exit {}",
            distro,
            command,
            out.status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".into())
        ),
    )
}

/// Mueve un USB a Kali: `usbipd attach --wsl --busid {busid}` (p. ej. `1-5`
/// para el RT3070 148f:3070). Requiere usbipd-win (Fase 2, winget).
#[command]
pub async fn wsl_attach(app: AppHandle, busid: String) -> WslExecResult {
    if !valid_busid(&busid) {
        return WslExecResult::err(format!("busid inválido: '{}' (esperado estilo '1-5').", busid));
    }
    let shell = app.shell();
    let arg_refs = ["attach", "--wsl", "--busid", busid.as_str()];
    match tokio::time::timeout(
        Duration::from_secs(60),
        shell.command("usbipd").args(&arg_refs).output(),
    )
    .await
    {
        Ok(Ok(o)) => {
            let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
            let ok = o.status.success();
            WslExecResult {
                success: ok,
                stdout,
                stderr: stderr.clone(),
                exit_code: o.status.code(),
                message: if ok {
                    format!("USB {} movido a WSL (verifica con `iw dev` en Kali).", busid)
                } else {
                    format!("usbipd attach falló: {}", stderr.lines().next().unwrap_or(""))
                },
            }
        }
        Ok(Err(e)) => WslExecResult::err(format!(
            "No se pudo lanzar `usbipd` (¿usbipd-win instalado? `winget install usbipd`): {}",
            e
        )),
        Err(_) => WslExecResult::err("Timeout: `usbipd attach` tardó más de 60s.".into()),
    }
}

/// Devuelve un USB a Windows: `usbipd detach --busid {busid}`.
#[command]
pub async fn wsl_detach(app: AppHandle, busid: String) -> WslExecResult {
    if !valid_busid(&busid) {
        return WslExecResult::err(format!("busid inválido: '{}' (esperado estilo '1-5').", busid));
    }
    let shell = app.shell();
    let arg_refs = ["detach", "--busid", busid.as_str()];
    match tokio::time::timeout(
        Duration::from_secs(60),
        shell.command("usbipd").args(&arg_refs).output(),
    )
    .await
    {
        Ok(Ok(o)) => {
            let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
            let ok = o.status.success();
            WslExecResult {
                success: ok,
                stdout,
                stderr: stderr.clone(),
                exit_code: o.status.code(),
                message: if ok {
                    format!("USB {} devuelto a Windows (verifica con detect_adapters).", busid)
                } else {
                    format!("usbipd detach falló: {}", stderr.lines().next().unwrap_or(""))
                },
            }
        }
        Ok(Err(e)) => WslExecResult::err(format!("No se pudo lanzar `usbipd`: {}", e)),
        Err(_) => WslExecResult::err("Timeout: `usbipd detach` tardó más de 60s.".into()),
    }
}

/// `D:\capturas\x.pcapng` → `/mnt/d/capturas/x.pcapng` (para pasar rutas del
/// proyecto a los comandos Kali). Solo traduce letra de unidad + `\`→`/`; el
/// resto se deja intacto.
#[command]
pub fn wsl_from_win(path: String) -> String {
    win_to_wsl(&path)
}

/// `/mnt/d/capturas/x.pcapng` → `D:\capturas\x.pcapng`. Si no tiene forma
/// `/mnt/X/…`, se devuelve intacta.
#[command]
pub fn wsl_to_win(path: String) -> String {
    wsl_to_win_path(&path)
}

fn win_to_wsl(p: &str) -> String {
    let t = p.trim();
    let bytes = t.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest: String = t[2..].replace('\\', "/");
        format!("/mnt/{}{}", drive, rest)
    } else {
        t.replace('\\', "/")
    }
}

fn wsl_to_win_path(p: &str) -> String {
    let t = p.trim();
    if t.len() >= 7 && t.starts_with("/mnt/") {
        let drive = t.as_bytes()[5] as char;
        if drive.is_ascii_alphabetic() && t.as_bytes().get(6) == Some(&b'/') {
            let rest: String = t[6..].replace('/', "\\");
            return format!("{}:{}", drive.to_ascii_uppercase(), rest);
        }
    }
    t.into()
}

// ═══════════════════════════════════════════════════════════════════════════════
// FASE 6 — Recetas de ataque v1 (requieren RF funcional en Kali; ver ROADMAP).
// Todas usan `sudo -n` (NOPASSWD en /etc/sudoers.d/uifipill-lab, solo lab) +
// `timeout -s INT` DENTRO de la VM para autolimitarse (el timeout de wsl_exec
// es solo backstop: matar el cliente `wsl` NO mata el proceso en la VM).
// ═══════════════════════════════════════════════════════════════════════════════

/// Nombre de interfaz 802.11 (`wlan0`…): alfanumérico, máx. 16.
fn valid_iface(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 16
        && s.chars().all(|c| c.is_ascii_alphanumeric())
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// BSSID `AA:BB:CC:DD:EE:FF` (hex con `:`).
fn valid_bssid(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    parts.len() == 6
        && parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// RT3070 = 2.4 GHz: canales 1–14 (hcxdumptool exige sufijo de banda `Na`).
fn valid_channel_24(ch: u8) -> bool {
    (1..=14).contains(&ch)
}

/// Patrón para `pkill -INT` (nombre de herramienta): alfanumérico + `_-+.`.
fn valid_proc_pattern(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '+' || c == '.')
}

/// `sudo -n timeout -s INT {dur} {tool_args…}` vía wsl_exec (backstop dur+30).
async fn sudo_timeout(
    app: &AppHandle,
    tool_args: Vec<String>,
    dur: u64,
    what: &str,
) -> WslExecResult {
    let dur = dur.clamp(10, 600);
    let mut args = vec!["-n".into(), "/usr/bin/timeout".into(), "-s".into(), "INT".into(), dur.to_string()];
    args.extend(tool_args);
    let mut r = wsl_exec(app.clone(), "sudo".into(), Some(args), None, Some(dur + 30)).await;
    r.message = format!("{} ({}s en Kali): {}", what, dur, r.message);
    r
}

fn staging_path(prefix: &str, ext: &str) -> (String, String) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let win = std::env::temp_dir().join(format!("{}_{}.{}", prefix, ts, ext));
    let win_s = win.to_string_lossy().into_owned();
    (win_s.clone(), win_to_wsl(&win_s))
}

/// Diagnóstico completo del puente RF en Kali, en un tirón (solo lectura +
/// probe RX de 6 s con tcpdump sobre /dev/shm — NO transmite):
/// 1) distro arranca  2) iface existe  3) modo monitor  4) canal  5) RX VIVA.
/// La RX es el bloqueador medido del transporte usbipd (0 pkts donde Windows
/// captura cientos): sin este check la UI no distingue «sin attach» de
/// «attach pero radio muerta».
#[command]
pub async fn wsl_health(app: AppHandle, iface: Option<String>) -> WslExecResult {
    let iface = iface.unwrap_or_else(|| "wlan0".into());
    if !valid_iface(&iface) {
        return WslExecResult::err(format!("Interfaz inválida: '{}'.", iface));
    }
    let mut report = String::new();
    let mut ok_all = true;

    // 1) Distro + whoami
    let d = wsl_exec(app.clone(), "/usr/bin/whoami".into(), None, None, Some(20)).await;
    let who = d.stdout.trim().to_string();
    let distro_ok = d.success && (who == "root" || who == "kali");
    report.push_str(&format!("1. Distro kali-linux: {}\n", if distro_ok { format!("OK ({})", who) } else { "NO ARRANCA".to_string() }));
    ok_all &= distro_ok;
    if !distro_ok {
        return WslExecResult { success: false, message: format!("SALUD KALI: FALLO\n{}\n{}", report, d.stderr), stdout: report, stderr: d.stderr, exit_code: d.exit_code };
    }

    // 2) Interfaz presente
    let i = wsl_exec(app.clone(), "/usr/sbin/ip".into(), Some(vec!["link".into(), "show".into(), iface.clone()]), None, Some(20)).await;
    let iface_ok = i.success;
    report.push_str(&format!("2. {} existe: {}\n", iface, if iface_ok { "OK".to_string() } else { "AUSENTE — attach el USB (usbipd attach --wsl --busid)".to_string() }));
    ok_all &= iface_ok;
    if !iface_ok {
        return WslExecResult { success: false, message: format!("SALUD KALI: FALLO\n{}\n{}", report, i.stderr), stdout: report, stderr: i.stderr, exit_code: i.exit_code };
    }

    // 3) Tipo de interfaz (monitor?)
    let t = wsl_exec(app.clone(), "/usr/sbin/iw".into(), Some(vec!["dev".into(), iface.clone(), "info".into()]), None, Some(20)).await;
    let is_monitor = t.stdout.contains("type monitor");
    report.push_str(&format!("3. Modo: {}\n", if is_monitor { "monitor OK".to_string() } else { "managed (los ataques RF requieren monitor: usa 'Monitor de Kali' o sudo iw wlan0 set monitor control)".to_string() }));

    // 4) Canal actual
    let ch = t.stdout.lines().find(|l| l.trim().starts_with("channel")).map(|l| l.trim().to_string()).unwrap_or_else(|| "canal ?".into());
    report.push_str(&format!("4. Canal: {}\n", ch));

    // 5) Probe RX: tcpdump 6 s; VIVA si captura >0 paquetes.
    let rx = wsl_exec(
        app.clone(),
        "sudo".into(),
        Some(vec![
            "-n".into(), "/usr/bin/timeout".into(), "-s".into(), "INT".into(), "6".into(),
            "/usr/bin/tcpdump".into(), "-i".into(), iface.clone(), "-c".into(), "5".into(), "--immediate-mode".into(), "-n".into(),
        ]),
        None,
        Some(30),
    )
    .await;
    // tcpdump -c 5 exit 0 al llegar a 5 pkts; "0 packets captured" si muerta.
    let rx_viva = rx.success || rx.stdout.contains("packets captured") && !rx.stdout.contains("0 packets captured");
    report.push_str(&format!(
        "5. Recepción (RX): {}\n",
        if rx_viva { "VIVA — llegan paquetes: puedes atacar".to_owned() } else { "MUERTA — usbipd no entrega paquetes (bloqueador conocido; captura en Windows o VirtualHere con licencia)".to_string() }
    ));
    ok_all &= rx_viva;

    // Nota tcpdump ausente: sudo falla si tcpdump no está — lo reflejamos.
    if rx.stderr.contains("No such file") && !rx_viva {
        report.push_str("   (nota: tcpdump no está en esa ruta; apt install tcpdump)");
    }

    WslExecResult {
        success: ok_all,
        message: format!("SALUD KALI ({}): {}", iface, if ok_all { "TODO OK — RF operativa" } else { "CON FALLOS (ver pasos arriba)" }),
        stdout: report,
        stderr: rx.stderr,
        exit_code: if ok_all { Some(0) } else { Some(1) },
    }
}

/// Lanza una receta RF en Kali CON STREAMING en vivo hacia la UI:
/// emite `attack-progress` (id, stdout/stderr) por chunk igual que run_bin_bg,
/// `attack-started` al arrancar y `attack-completed` al terminar. El frontend
/// ya sabe pintar esos eventos — el verbose de Kali se ve en el panel en vivo.
/// Cancelable con cancel_attack porque el child se registra en running_attacks.
async fn wsl_stream_run(
    app: &AppHandle,
    state: &tauri::State<'_, crate::AppState>,
    attack_id: &str,
    tool_args: Vec<String>,
    dur: u64,
) -> WslExecResult {
    let dur = dur.clamp(10, 600);
    // Misma receta que sudo_timeout: sudo -n timeout -s INT {dur} {tool…}
    let mut full: Vec<String> = vec![
        "-n".into(), "/usr/bin/timeout".into(), "-s".into(), "INT".into(), dur.to_string(),
    ];
    full.extend(tool_args);
    let mut wsl_args: Vec<String> = vec!["-d".into(), DEFAULT_DISTRO.into(), "--".into(), "sudo".into()];
    wsl_args.extend(full);

    let cmdline = format!("wsl -d {} -- sudo{} {}", DEFAULT_DISTRO,
        wsl_args[3..].iter().map(|a| format!(" {}", a)).collect::<String>(),
        // (legible: los args ya llevan espacio delante)
        "");
    let _ = app.emit("attack-progress", serde_json::json!({
        "id": attack_id, "type": "stdout",
        "data": format!("$ (Kali) {}\n", cmdline)
    }));
    let _ = app.emit("attack-started", serde_json::json!({
        "id": attack_id, "bg": true, "cmdline": format!("(Kali) {}", cmdline)
    }));

    let shell = app.shell();
    let arg_refs: Vec<&str> = wsl_args.iter().map(|s| s.as_ref()).collect();
    let (mut rx, child) = match shell.command("wsl").args(&arg_refs).spawn() {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("No se pudo lanzar wsl: {}", e);
            let _ = app.emit("attack-error", serde_json::json!({ "id": attack_id, "error": msg }));
            return WslExecResult::err(msg);
        }
    };
    {
        let mut map = state.running_attacks.lock().unwrap();
        map.insert(attack_id.to_string(), child);
    }

    let mut stdout_all = String::new();
    let mut pend_out = String::new();
    let mut pend_err = String::new();
    let mut last_flush = std::time::Instant::now();
    let mut exit_code: Option<i32> = None;
    const FLUSH_BYTES: usize = 4096;
    const FLUSH_MS: u128 = 150;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(data) => {
                let txt = String::from_utf8_lossy(&data);
                stdout_all.push_str(&txt);
                pend_out.push_str(&txt);
                if pend_out.len() + pend_err.len() >= FLUSH_BYTES
                    || last_flush.elapsed().as_millis() >= FLUSH_MS
                {
                    let out = std::mem::take(&mut pend_out);
                    let err = std::mem::take(&mut pend_err);
                    crate::emit_stream(app, attack_id, &out, &err);
                    last_flush = std::time::Instant::now();
                }
            }
            CommandEvent::Stderr(data) => {
                let txt = String::from_utf8_lossy(&data);
                pend_err.push_str(&txt);
                if pend_out.len() + pend_err.len() >= FLUSH_BYTES
                    || last_flush.elapsed().as_millis() >= FLUSH_MS
                {
                    let out = std::mem::take(&mut pend_out);
                    let err = std::mem::take(&mut pend_err);
                    crate::emit_stream(app, attack_id, &out, &err);
                    last_flush = std::time::Instant::now();
                }
            }
            CommandEvent::Terminated(status) => {
                exit_code = status.code;
                break;
            }
            CommandEvent::Error(err) => {
                pend_err.push_str(&format!("Error: {}", err));
                break;
            }
            _ => {}
        }
    }
    // Resto pendiente
    {
        let out = std::mem::take(&mut pend_out);
        let err = std::mem::take(&mut pend_err);
        crate::emit_stream(app, attack_id, &out, &err);
    }
    {
        let mut map = state.running_attacks.lock().unwrap();
        map.remove(attack_id);
    }
    let ok = exit_code == Some(0);
    let _ = app.emit("attack-completed", serde_json::json!({
        "id": attack_id, "success": ok, "exit_code": exit_code
    }));
    WslExecResult {
        success: ok,
        stdout: stdout_all,
        stderr: String::new(),
        exit_code,
        message: format!("Kali: exit {:?} tras {}s", exit_code, dur),
    }
}

/// Receta PMKID con streaming: hcxdumptool en Kali, verbose en vivo en la UI.
#[command]
pub async fn wsl_pmkid_stream(
    app: AppHandle,
    state: tauri::State<'_, crate::AppState>,
    iface: String,
    channel: Option<u8>,
    duration_secs: Option<u64>,
) -> Result<WslExecResult, String> {
    if !valid_iface(&iface) {
        return Ok(WslExecResult::err(format!("Interfaz inválida: '{}'.", iface)));
    }
    let dur = duration_secs.unwrap_or(60).clamp(10, 600);
    let mut tool = vec![
        "/usr/bin/hcxdumptool".into(), "-i".into(), iface.clone(),
        "--enable_status=1".into(), "--status_interval=2".into(),
    ];
    if let Some(ch) = channel {
        if !valid_channel_24(ch) { return Ok(WslExecResult::err(format!("Canal inválido: {}", ch))); }
        tool.push("-c".into()); tool.push(ch.to_string());
    }
    Ok(wsl_stream_run(&app, &state, "wsl_pmkid_stream", tool, dur).await)
}

/// Receta airodump con streaming (verbose en vivo).
#[command]
pub async fn wsl_airodump_stream(
    app: AppHandle,
    state: tauri::State<'_, crate::AppState>,
    iface: String,
    channel: Option<u8>,
    duration_secs: Option<u64>,
) -> Result<WslExecResult, String> {
    if !valid_iface(&iface) {
        return Ok(WslExecResult::err(format!("Interfaz inválida: '{}'.", iface)));
    }
    let dur = duration_secs.unwrap_or(30).clamp(10, 600);
    let mut tool = vec!["/usr/sbin/airodump-ng".into(), iface.clone()];
    if let Some(ch) = channel {
        if !valid_channel_24(ch) { return Ok(WslExecResult::err(format!("Canal inválido: {}", ch))); }
        tool.push("-c".into()); tool.push(ch.to_string());
    }
    Ok(wsl_stream_run(&app, &state, "wsl_airodump_stream", tool, dur).await)
}

/// Receta deauth aireplay-ng con streaming (SOLO contra AP propio).
#[command]
pub async fn wsl_deauth_stream(
    app: AppHandle,
    state: tauri::State<'_, crate::AppState>,
    iface: String,
    bssid: String,
    count: Option<u32>,
    channel: Option<u8>,
) -> Result<WslExecResult, String> {
    if !valid_iface(&iface) { return Ok(WslExecResult::err(format!("Interfaz inválida: '{}'.", iface))); }
    if !valid_bssid(&bssid) { return Ok(WslExecResult::err(format!("BSSID inválido: '{}'.", bssid))); }
    let n = count.unwrap_or(10).clamp(1, 100);
    let tool = vec![
        "/usr/sbin/aireplay-ng".into(), "--deauth".into(), n.to_string(),
        "-a".into(), bssid.clone(), iface.clone(),
    ];
    if let Some(ch) = channel {
        if !valid_channel_24(ch) { return Ok(WslExecResult::err(format!("Canal inválido: {}", ch))); }
        // Fijar canal antes: iw es la vía soportada (airodump no está delante).
        let _ = wsl_exec(app.clone(), "sudo".into(), Some(vec![
            "-n".into(), "/usr/sbin/iw".into(), "dev".into(), iface.clone(), "set".into(), "channel".into(), ch.to_string(),
        ]), None, Some(20)).await;
    }
    Ok(wsl_stream_run(&app, &state, "wsl_deauth_stream", tool, 30).await)
}

/// PMKID: `hcxdumptool -i {iface} -w {staging.pcapng} [-c {ch}a] --rds=1`
/// durante `dur` s. El .pcapng se convierte después con `pcap_to_22000`.
#[command]
pub async fn wsl_pmkid_capture(
    app: AppHandle,
    iface: String,
    channel: Option<u8>,
    duration_secs: Option<u64>,
) -> WslExecResult {
    if !valid_iface(&iface) {
        return WslExecResult::err(format!("Interfaz inválida: '{}'.", iface));
    }
    let dur = duration_secs.unwrap_or(60).clamp(10, 600);
    if let Some(ch) = channel {
        if !valid_channel_24(ch) {
            return WslExecResult::err(format!("Canal 2.4 GHz inválido: {} (1–14).", ch));
        }
    }
    let (win_pcapng, wsl_pcapng) = staging_path("wsl_pmkid", "pcapng");
    let mut tool = vec![
        "/usr/bin/hcxdumptool".into(),
        "-i".into(),
        iface.clone(),
        "-w".into(),
        wsl_pcapng,
        "--rds=1".into(),
    ];
    if let Some(ch) = channel {
        tool.push("-c".into());
        tool.push(format!("{}a", ch));
    }
    let mut r = sudo_timeout(&app, tool, dur, "hcxdumptool").await;
    let exists = std::path::Path::new(&win_pcapng).exists();
    r.message = format!(
        "{}\nSalida: {}{}",
        r.message,
        win_pcapng,
        if exists { " (fichero presente)" } else { " (fichero AUSENTE: ¿RF caída?)" }
    );
    r
}

/// Airodump: CSV en staging (`{prefix}-01.csv`) durante `dur` s.
#[command]
pub async fn wsl_airodump(
    app: AppHandle,
    iface: String,
    channel: Option<u8>,
    duration_secs: Option<u64>,
) -> WslExecResult {
    if !valid_iface(&iface) {
        return WslExecResult::err(format!("Interfaz inválida: '{}'.", iface));
    }
    let dur = duration_secs.unwrap_or(20).clamp(10, 600);
    if let Some(ch) = channel {
        if !valid_channel_24(ch) {
            return WslExecResult::err(format!("Canal 2.4 GHz inválido: {} (1–14).", ch));
        }
    }
    let (win_prefix, wsl_prefix) = staging_path("wsl_airodump", "csv");
    let win_prefix = win_prefix.trim_end_matches(".csv").to_string();
    let wsl_prefix = wsl_prefix.trim_end_matches(".csv").to_string();
    let mut tool = vec![
        "/usr/sbin/airodump-ng".into(),
        iface.clone(),
        "-w".into(),
        wsl_prefix,
        "--output-format".into(),
        "csv".into(),
    ];
    if let Some(ch) = channel {
        tool.push("-c".into());
        tool.push(ch.to_string());
    }
    let mut r = sudo_timeout(&app, tool, dur, "airodump-ng").await;
    let csv = format!("{}-01.csv", win_prefix);
    r.message = format!(
        "{}\nCSV: {}{}",
        r.message,
        csv,
        if std::path::Path::new(&csv).exists() { " (presente)" } else { " (AUSENTE)" }
    );
    r
}

/// Wash (WPS discovery): `wash -i {iface} [-c {ch}]` durante `dur` s.
#[command]
pub async fn wsl_wash(app: AppHandle, iface: String, channel: Option<u8>, duration_secs: Option<u64>) -> WslExecResult {
    if !valid_iface(&iface) {
        return WslExecResult::err(format!("Interfaz inválida: '{}'.", iface));
    }
    let dur = duration_secs.unwrap_or(20).clamp(10, 600);
    if let Some(ch) = channel {
        if !valid_channel_24(ch) {
            return WslExecResult::err(format!("Canal 2.4 GHz inválido: {} (1–14).", ch));
        }
    }
    let mut tool = vec!["/usr/bin/wash".into(), "-i".into(), iface.clone()];
    if let Some(ch) = channel {
        tool.push("-c".into());
        tool.push(ch.to_string());
    }
    sudo_timeout(&app, tool, dur, "wash").await
}

/// Deauth dirigida (SOLO contra AP propio): fija canal con `iw` y luego
/// `aireplay-ng --deauth {count} -a {bssid} [-c {client}] {iface}`.
#[command]
pub async fn wsl_deauth(
    app: AppHandle,
    iface: String,
    bssid: String,
    client: Option<String>,
    count: Option<u32>,
    channel: Option<u8>,
) -> WslExecResult {
    if !valid_iface(&iface) {
        return WslExecResult::err(format!("Interfaz inválida: '{}'.", iface));
    }
    if !valid_bssid(&bssid) {
        return WslExecResult::err(format!("BSSID inválido: '{}'.", bssid));
    }
    if let Some(c) = &client {
        if !c.is_empty() && !valid_bssid(c) {
            return WslExecResult::err(format!("Cliente inválido: '{}'.", c));
        }
    }
    let count = count.unwrap_or(10).clamp(1, 100);
    if let Some(ch) = channel {
        if !valid_channel_24(ch) {
            return WslExecResult::err(format!("Canal 2.4 GHz inválido: {} (1–14).", ch));
        }
        let pin = wsl_exec(
            app.clone(),
            "sudo".into(),
            Some(vec![
                "-n".into(),
                "/usr/sbin/iw".into(),
                "dev".into(),
                iface.clone(),
                "set".into(),
                "channel".into(),
                ch.to_string(),
            ]),
            None,
            Some(30),
        )
        .await;
        if !pin.success {
            return WslExecResult::err(format!(
                "No se pudo fijar el canal {} ({}). Deauth cancelada.",
                ch, pin.stderr.lines().next().unwrap_or("")
            ));
        }
    }
    let mut tool = vec![
        "/usr/sbin/aireplay-ng".into(),
        "--deauth".into(),
        count.to_string(),
        "-a".into(),
        bssid.clone(),
    ];
    if let Some(c) = client {
        if !c.is_empty() {
            tool.push("-c".into());
            tool.push(c);
        }
    }
    tool.push(iface.clone());
    sudo_timeout(&app, tool, u64::from(count) + 15, "aireplay-ng --deauth").await
}

/// Reaver WPS (SOLO contra AP propio): `reaver -i {iface} -b {bssid} -c {ch}
/// [-K 1] -vv` durante `dur` s (sonda: asocia e intenta M1; el PIN completo
/// tarda horas y se deja al operador con `wsl_kill` para detener).
#[command]
pub async fn wsl_reaver(
    app: AppHandle,
    iface: String,
    bssid: String,
    channel: u8,
    duration_secs: Option<u64>,
    pixie: Option<bool>,
) -> WslExecResult {
    if !valid_iface(&iface) {
        return WslExecResult::err(format!("Interfaz inválida: '{}'.", iface));
    }
    if !valid_bssid(&bssid) {
        return WslExecResult::err(format!("BSSID inválido: '{}'.", bssid));
    }
    if !valid_channel_24(channel) {
        return WslExecResult::err(format!("Canal 2.4 GHz inválido: {} (1–14).", channel));
    }
    let dur = duration_secs.unwrap_or(60).clamp(10, 600);
    let mut tool = vec![
        "/usr/bin/reaver".into(),
        "-i".into(),
        iface.clone(),
        "-b".into(),
        bssid.clone(),
        "-c".into(),
        channel.to_string(),
        "-vv".into(),
    ];
    if pixie.unwrap_or(false) {
        tool.push("-K".into());
        tool.push("1".into());
    }
    sudo_timeout(&app, tool, dur, "reaver").await
}

/// Limpieza: `pkill -INT {pattern}` (p. ej. `hcxdumptool`) para detener restos
/// huérfanos en la VM (el timeout del cliente `wsl` no los mata).
#[command]
pub async fn wsl_kill(app: AppHandle, pattern: String) -> WslExecResult {
    if !valid_proc_pattern(&pattern) {
        return WslExecResult::err(format!("Patrón inválido: '{}'.", pattern));
    }
    let r = wsl_exec(
        app,
        "sudo".into(),
        Some(vec!["-n".into(), "/usr/bin/pkill".into(), "-INT".into(), pattern.clone()]),
        None,
        Some(30),
    )
    .await;
    WslExecResult::ok(
        r.stdout,
        r.stderr,
        r.exit_code,
        format!("pkill -INT {} → exit {:?}", pattern, r.exit_code),
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// FASE 6b — VirtualHere (transporte USB alternativo a usbipd; ver ROADMAP).
// El servidor Windows (trial: 1 dispositivo) expone el RT3070 en el puerto
// 7575; el cliente consola `vhclientx86_64` en Kali lo reclama con USE.
// OJO licencia: el USE por cliente consola exige servidor con licencia de
// pago (respuesta oficial en foro VirtualHere #4683); con trial responde
// `FAILED: API Timeout` y así se reporta en el mensaje.
// El demonio y los controles corren como root en la VM (`wsl -u root`, sin
// contraseña desde Windows); los inputs de usuario se validan con charset
// estricto ANTES de llegar a root. Sin shell intermedia en ningún paso.
// ═══════════════════════════════════════════════════════════════════════════════

/// URL oficial del cliente consola Linux amd64 (estático, verificado 2026-09-15).
pub const VH_CLIENT_URL: &str =
    "https://www.virtualhere.com/sites/default/files/usbclient/vhclientx86_64";
/// Ruta del cliente dentro de Kali (home del usuario de lab).
pub const VH_CLIENT_PATH: &str = "/home/kali/vhclientx86_64";
/// Puerto del servidor VirtualHere Windows.
pub const VH_DEFAULT_PORT: u16 = 7575;

/// Host del servidor: letras, dígitos, `.` y `-` (IPv4 o nombre NetBIOS).
fn valid_server_host(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && s.chars().any(|c| c.is_ascii_alphanumeric())
}

/// Dirección de dispositivo VirtualHere `<servidor>.<n>` (p. ej. el
/// `DESKTOP-33B27GN.8` que muestra el `LIST`): servidor + punto + 1–8 dígitos.
fn valid_vh_address(s: &str) -> bool {
    match s.rsplit_once('.') {
        Some((srv, dev)) => {
            valid_server_host(srv)
                && !dev.is_empty()
                && dev.len() <= 8
                && dev.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// Ejecuta como root en la VM: `wsl -d {distro} -u root -- {command} {args…}`.
/// SOLO lo llaman los comandos vh_* con binarios fijos y args de charset
/// validado (sin shell, sin interpolación). Timeout 5–150 s.
async fn wsl_exec_root(
    app: &AppHandle,
    command: &str,
    args: Vec<String>,
    timeout_secs: u64,
) -> WslExecResult {
    let shell = app.shell();
    let mut cmd_args: Vec<String> = vec![
        "-d".into(),
        DEFAULT_DISTRO.into(),
        "-u".into(),
        "root".into(),
        "--".into(),
        command.into(),
    ];
    cmd_args.extend(args);
    let arg_refs: Vec<&str> = cmd_args.iter().map(|s| s.as_ref()).collect();
    let timeout = timeout_secs.clamp(5, 150);
    let out = match tokio::time::timeout(
        Duration::from_secs(timeout),
        shell.command("wsl").args(&arg_refs).output(),
    )
    .await
    {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return WslExecResult::err(format!("No se pudo lanzar `wsl -u root`: {}", e))
        }
        Err(_) => {
            return WslExecResult::err(format!(
                "`wsl -u root -- {}` tardó más de {}s.",
                command, timeout
            ))
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let code = out.status.code();
    WslExecResult::ok(
        stdout,
        stderr,
        code,
        format!(
            "wsl -u root -- {} → exit {}",
            command,
            code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
        ),
    )
}

/// TCP connect con timeout (bloqueante; se llama dentro de spawn_blocking).
fn tcp_connect_once(target: &str, timeout_secs: u64) -> Result<(), String> {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let timeout = Duration::from_secs(timeout_secs);
    let addrs = target.to_socket_addrs().map_err(|e| e.to_string())?;
    let mut last_err = "sin direcciones".to_string();
    for a in addrs {
        match TcpStream::connect_timeout(&a, timeout) {
            Ok(_) => return Ok(()),
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(last_err)
}

/// ¿Responde el servidor VH en `{ip}:7575`? (TCP connect 5 s, sin WSL.)
#[command]
pub async fn vh_server_check(server_ip: String) -> WslExecResult {
    if !valid_server_host(&server_ip) {
        return WslExecResult::err(format!("Servidor inválido: '{}'.", server_ip));
    }
    let target = format!("{}:{}", server_ip, VH_DEFAULT_PORT);
    let res = tokio::task::spawn_blocking(move || tcp_connect_once(&target, 5)).await;
    match res {
        Ok(Ok(())) => WslExecResult::ok(
            String::new(),
            String::new(),
            Some(0),
            format!(
                "Servidor VirtualHere {}:{} accesible (siguiente: Provision).",
                server_ip, VH_DEFAULT_PORT
            ),
        ),
        Ok(Err(e)) => WslExecResult {
            success: false,
            stdout: String::new(),
            stderr: e.clone(),
            exit_code: None,
            message: format!(
                "Servidor {}:{} NO accesible: {} (¿vhusbd corriendo en Windows? ¿firewall?).",
                server_ip, VH_DEFAULT_PORT, e
            ),
        },
        Err(e) => WslExecResult::err(format!("Error interno de tarea: {}", e)),
    }
}

/// Descarga el cliente consola a `~/` si falta (`wget URL -O path` + chmod +x).
#[command]
pub async fn vh_provision(app: AppHandle) -> WslExecResult {
    let chk = wsl_exec(
        app.clone(),
        "test".into(),
        Some(vec!["-x".into(), VH_CLIENT_PATH.into()]),
        None,
        Some(15),
    )
    .await;
    if chk.exit_code == Some(0) {
        return WslExecResult::ok(
            chk.stdout,
            chk.stderr,
            chk.exit_code,
            format!(
                "Cliente VH ya presente en {} (siguiente: Demonio).",
                VH_CLIENT_PATH
            ),
        );
    }
    let dl = wsl_exec(
        app.clone(),
        "wget".into(),
        Some(vec![
            VH_CLIENT_URL.into(),
            "-O".into(),
            VH_CLIENT_PATH.into(),
        ]),
        None,
        Some(120),
    )
    .await;
    if dl.exit_code != Some(0) {
        return WslExecResult::err(format!(
            "Descarga del cliente VH falló: {} (¿red en Kali?).",
            dl.stderr.lines().next().unwrap_or("?")
        ));
    }
    let ch = wsl_exec(
        app.clone(),
        "chmod".into(),
        Some(vec!["+x".into(), VH_CLIENT_PATH.into()]),
        None,
        Some(15),
    )
    .await;
    if ch.exit_code == Some(0) {
        WslExecResult::ok(
            ch.stdout,
            ch.stderr,
            ch.exit_code,
            format!("Cliente VH instalado en {} (siguiente: Demonio).", VH_CLIENT_PATH),
        )
    } else {
        WslExecResult::err(format!(
            "chmod +x falló: {}",
            ch.stderr.lines().next().unwrap_or("?")
        ))
    }
}

/// Arranca el demonio: `modprobe vhci-hcd` + `{client} -n` + verificación.
#[command]
pub async fn vh_daemon(app: AppHandle) -> WslExecResult {
    let mp = wsl_exec_root(&app, "/sbin/modprobe", vec!["vhci-hcd".into()], 30).await;
    if mp.exit_code != Some(0) {
        return WslExecResult::err(format!(
            "modprobe vhci-hcd falló: {} (¿kernel sin USBIP_VHCI?).",
            mp.stderr.lines().next().unwrap_or("?")
        ));
    }
    let _ = wsl_exec_root(&app, VH_CLIENT_PATH, vec!["-n".into()], 30).await;
    let pg = wsl_exec_root(
        &app,
        "/usr/bin/pgrep",
        vec!["-f".into(), "vhclientx86_64".into()],
        15,
    )
    .await;
    if pg.exit_code == Some(0) {
        WslExecResult::ok(
            pg.stdout,
            pg.stderr,
            pg.exit_code,
            "Demonio vhclient en marcha (vhci-hcd cargado). Siguiente: +Hub.".into(),
        )
    } else {
        WslExecResult::err(
            "El demonio vhclient no quedó en marcha (repite Provision o revisa el binario)."
                .into(),
        )
    }
}

/// `MANUAL HUB ADD,<ip>` contra el demonio local.
#[command]
pub async fn vh_hub_add(app: AppHandle, server_ip: String) -> WslExecResult {
    if !valid_server_host(&server_ip) {
        return WslExecResult::err(format!("Servidor inválido: '{}'.", server_ip));
    }
    let mut r = wsl_exec_root(
        &app,
        VH_CLIENT_PATH,
        vec!["-t".into(), format!("MANUAL HUB ADD,{}", server_ip)],
        30,
    )
    .await;
    r.message = format!("Hub {}: {}", server_ip, r.message);
    r
}

/// `LIST` de dispositivos compartidos por el servidor.
#[command]
pub async fn vh_list(app: AppHandle) -> WslExecResult {
    let mut r = wsl_exec_root(&app, VH_CLIENT_PATH, vec!["-t".into(), "LIST".into()], 30).await;
    r.message = format!("LIST: {}", r.message);
    r
}

/// `USE,<addr>`: reclama el USB en Kali (lo quita de Windows).
/// Con servidor trial responde `FAILED: API Timeout` (exige licencia de pago).
#[command]
pub async fn vh_use(app: AppHandle, address: String) -> WslExecResult {
    if !valid_vh_address(&address) {
        return WslExecResult::err(format!(
            "Dirección inválida: '{}' (esperado estilo 'SERVIDOR.8' del LIST).",
            address
        ));
    }
    let mut r = wsl_exec_root(
        &app,
        VH_CLIENT_PATH,
        vec!["-t".into(), format!("USE,{}", address)],
        30,
    )
    .await;
    if r.stdout.contains("API Timeout") || r.stderr.contains("API Timeout") {
        r.message = format!(
            "{}\nUSE rechazado: el servidor trial NO permite cliente consola (exige licencia de pago, foro VirtualHere #4683). Alternativa gratis: cliente GUI Linux bajo WSLg, o seguir con usbipd.",
            r.message
        );
    } else {
        r.message = format!("USE {}: {} (verifica con RF-check).", address, r.message);
    }
    r
}

/// `STOP USING,<addr>`: devuelve el USB a Windows.
#[command]
pub async fn vh_stop(app: AppHandle, address: String) -> WslExecResult {
    if !valid_vh_address(&address) {
        return WslExecResult::err(format!(
            "Dirección inválida: '{}' (esperado estilo 'SERVIDOR.8' del LIST).",
            address
        ));
    }
    let mut r = wsl_exec_root(
        &app,
        VH_CLIENT_PATH,
        vec!["-t".into(), format!("STOP USING,{}", address)],
        30,
    )
    .await;
    if r.stdout.contains("API Timeout") || r.stderr.contains("API Timeout") {
        r.message = format!(
            "{}\nSTOP no aplicado (el dispositivo no estaba en uso en Kali o el servidor no respondió). Nada que devolver.",
            r.message
        );
    } else {
        r.message = format!("STOP {}: {} (el USB vuelve a Windows).", address, r.message);
    }
    r
}

/// Chequeo RF post-USE: `iw dev {iface} info` + `ip link set up` +
/// `tcpdump -c 30` durante `dur` s + líneas de firmware del dmesg.
/// Veredicto VIVA/MUERTA según paquetes capturados.
#[command]
pub async fn vh_rf_check(
    app: AppHandle,
    iface: String,
    duration_secs: Option<u64>,
) -> WslExecResult {
    if !valid_iface(&iface) {
        return WslExecResult::err(format!("Interfaz inválida: '{}'.", iface));
    }
    let dur = duration_secs.unwrap_or(15).clamp(10, 60);
    let info = wsl_exec_root(
        &app,
        "/usr/sbin/iw",
        vec!["dev".into(), iface.clone(), "info".into()],
        20,
    )
    .await;
    if info.exit_code != Some(0) {
        return WslExecResult::err(format!(
            "Sin {} en Kali (¿USE hecho? `iw dev` vacío). STDERR: {}",
            iface,
            info.stderr.lines().next().unwrap_or("nl80211 not found")
        ));
    }
    let up = wsl_exec_root(
        &app,
        "/usr/sbin/ip",
        vec!["link".into(), "set".into(), iface.clone(), "up".into()],
        20,
    )
    .await;
    let cap = wsl_exec_root(
        &app,
        "/usr/bin/timeout",
        vec![
            "-s".into(),
            "INT".into(),
            dur.to_string(),
            "/usr/bin/tcpdump".into(),
            "-i".into(),
            iface.clone(),
            "-e".into(),
            "-n".into(),
            "-c".into(),
            "30".into(),
        ],
        dur + 20,
    )
    .await;
    let dm = wsl_exec_root(&app, "/usr/bin/dmesg", vec![], 20).await;
    let fw: Vec<String> = dm
        .stdout
        .lines()
        .filter(|l| {
            let ll = l.to_lowercase();
            ll.contains("rt28") || ll.contains("rt2x00") || ll.contains("firmware")
        })
        .map(|l| l.to_string())
        .collect();
    let fw_tail = fw.iter().rev().take(5).rev().cloned().collect::<Vec<_>>().join("\n");
    let captured_zero = cap.stderr.contains("0 packets captured");
    let captured_some = cap.stderr.contains("packets captured") && !captured_zero;
    let verdict = if captured_some {
        "RF VIVA (hay paquetes 802.11 en Kali)"
    } else if captured_zero {
        "RF MUERTA (0 paquetes: mismo síntoma que usbipd)"
    } else {
        "RF INCIERTA (tcpdump sin resumen; revisa salida)"
    };
    let stdout = format!(
        "── iw dev {} info ──\n{}\n── ip link set up: exit {:?} ──\n── tcpdump ({}s) ──\n{}\n{}\n── firmware (dmesg) ──\n{}",
        iface,
        info.stdout.trim(),
        up.exit_code,
        dur,
        cap.stdout.trim(),
        cap.stderr.lines().last().unwrap_or(""),
        if fw_tail.is_empty() { "(sin líneas rt28/firmware)" } else { &fw_tail },
    );
    WslExecResult::ok(
        stdout,
        String::new(),
        cap.exit_code,
        format!("RF-check {} → {}.", iface, verdict),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_paths_translate() {
        assert_eq!(
            win_to_wsl(r"D:\capturas\x.pcapng"),
            "/mnt/d/capturas/x.pcapng"
        );
        assert_eq!(win_to_wsl("E:/a/b"), "/mnt/e/a/b");
        assert_eq!(win_to_wsl("/ya/wsl"), "/ya/wsl");
    }

    #[test]
    fn wsl_paths_translate() {
        assert_eq!(
            wsl_to_win_path("/mnt/d/capturas/x.pcapng"),
            r"D:\capturas\x.pcapng"
        );
        assert_eq!(wsl_to_win_path("relativo/x"), "relativo/x");
    }

    #[test]
    fn validators_reject_injection() {
        assert!(valid_distro("kali-linux"));
        assert!(!valid_distro("kali; rm -rf /"));
        assert!(!valid_distro(""));
        assert!(valid_busid("1-5"));
        assert!(!valid_busid("1-5; evil"));
        assert!(!valid_busid(""));
    }

    #[test]
    fn recipe_validators() {
        assert!(valid_iface("wlan0"));
        assert!(!valid_iface("wlan0;evil"));
        assert!(!valid_iface(""));
        assert!(valid_bssid("AA:BB:CC:DD:EE:FF"));
        assert!(valid_bssid("aa:bb:cc:dd:ee:ff"));
        assert!(!valid_bssid("AA:BB:CC"));
        assert!(!valid_bssid("GG:BB:CC:DD:EE:FF"));
        assert!(valid_channel_24(11));
        assert!(!valid_channel_24(36));
        assert!(valid_proc_pattern("hcxdumptool"));
        assert!(!valid_proc_pattern("a; rm -rf /"));
    }

    #[test]
    fn vh_validators() {
        assert!(valid_server_host("172.29.80.1"));
        assert!(valid_server_host("DESKTOP-33B27GN"));
        assert!(!valid_server_host(""));
        assert!(!valid_server_host("a; rm -rf /"));
        assert!(!valid_server_host("ip con espacios"));
        assert!(valid_vh_address("DESKTOP-33B27GN.8"));
        assert!(valid_vh_address("172.29.80.1.8"));
        assert!(!valid_vh_address("DESKTOP-33B27GN"));
        assert!(!valid_vh_address("srv.x"));
        assert!(!valid_vh_address("srv.8;evil"));
        assert!(!valid_vh_address(""));
        assert_eq!(VH_DEFAULT_PORT, 7575);
    }

    #[test]
    fn vh_tcp_check_hits_local_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(tcp_connect_once(&format!("127.0.0.1:{}", port), 5).is_ok());
        // Puerto 1 en loopback: connection refused determinista.
        assert!(tcp_connect_once("127.0.0.1:1", 2).is_err());
        assert!(tcp_connect_once("256.256.256.256:7575", 2).is_err());
    }
}
