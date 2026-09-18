// ═══════════════════════════════════════════════════════════════════════════════
// DETECCIÓN DE HERRAMIENTAS  (tools_detect.rs)
// ═══════════════════════════════════════════════════════════════════════════════

use serde::Serialize;
use tauri::{command, AppHandle};
use crate::commands::CmdResponse;
use which;
use dirs;

/// Resuelve la ruta completa de una herramienta:
/// 1) variable de entorno NAME_PATH
/// 2) carpetas de instalación de UIFIPILL
/// 3) which / PATH del sistema
pub(crate) fn resolve_tool(name: &str) -> (bool, String, String) {
    // 1) Variable de entorno
    let env_var = format!("{}_PATH", name.to_uppercase().replace('.', "_"));
    if let Ok(p) = std::env::var(&env_var) {
        let pstr = p.trim().to_string();
        if std::path::Path::new(&pstr).exists() {
            return (true, pstr, "variable de entorno".into());
        }
    }

    // 1b) Ruta relativa al CWD (dev: project root tiene tools/)
    if std::path::Path::new(name).exists() {
        return (true, name.to_string(), "ruta relativa".into());
    }

    // 2) Rutas de instalación de UIFIPILL
    for dir in install_dirs() {
        let candidate = dir.join(name);
        if candidate.exists() {
            return (true, candidate.display().to_string(), "carpeta UIFIPILL tools".into());
        }
        let stem = name.trim_end_matches(".exe");
        let candidate2 = dir.join(stem);
        if candidate2.exists() {
            return (true, candidate2.display().to_string(), "carpeta UIFIPILL tools".into());
        }
    }

    // 3) which / PATH
    if let Ok(full) = which::which(name) {
        return (true, full.display().to_string(), "PATH del sistema".into());
    }
    let stem = name.trim_end_matches(".exe");
    if let Ok(full) = which::which(stem) {
        return (true, full.display().to_string(), "PATH del sistema".into());
    }

    (false, String::new(), String::new())
}

pub(crate) fn install_dirs_pub() -> Vec<std::path::PathBuf> { install_dirs() }

fn install_dirs() -> Vec<std::path::PathBuf> {
    let workspace = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf());

    let mut dirs: Vec<std::path::PathBuf> = vec![
        dirs::data_local_dir()
            .map(|p| p.join("UIFIPILL").join("tools")),
        // Tauri resource dir (installed app) — resources/ subdir
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("resources"))),
        // <exe_dir>/tools/ (dev or portable)
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("tools"))),
        // Workspace tools/ (dev mode)
        workspace.as_ref().map(|p| p.join("tools")),
        // aircrack-ng Windows binaries subdirectory
        workspace.as_ref().map(|p| p.join("tools").join("aircrack-ng-win")),
        // Tauri resources/tools/ + resources/tools/aircrack-ng-win/
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("resources").join("tools"))),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("resources").join("tools").join("aircrack-ng-win"))),
    ].into_iter().filter_map(|o| o).collect();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() { dirs.push(p.to_path_buf()); }
        // NSIS/Tauri v2 aplana rutas "../x" en una carpeta literal "_up_":
        // <inst>/_up_/tools/reaver.exe (verificado en %LOCALAPPDATA%\UIFIPILL).
        if let Some(p) = exe.parent() {
            dirs.push(p.join("_up_").join("tools"));
            dirs.push(p.join("_up_").join("tools").join("aircrack-ng-win"));
            dirs.push(p.join("resources").join("_up_").join("tools"));
        }
    }
    dirs
}

fn get_version(exe: &str) -> String {
    for args in [&["--version"][..], &["-V"][..], &["-h"][..]] {
        let out = std::process::Command::new(exe).args(args).output();
        match out {
            Ok(ref o) if o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty() => {
                let mut s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() { s = String::from_utf8_lossy(&o.stderr).trim().to_string(); }
                // reaver/wash -h vuelcan ayuda larga: quedarse con la primera línea útil
                let first = s.lines().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("").to_string();
                if !first.is_empty() {
                    // recortar ayuda a 90 chars
                    let short: String = first.chars().take(90).collect();
                    return short;
                }
            }
            _ => {}
        }
    }
    "—".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name:         String,
    pub found:        bool,
    pub full_path:    String,
    pub version:      String,
    pub source:       String,
    pub install_hint: String,
    pub required:     bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolsReport {
    pub tools:         Vec<ToolInfo>,
    pub all_ready:     bool,
    pub ready_count:   usize,
    pub total:         usize,
    pub missing_names: Vec<String>,
}

const TOOL_LIST: &[(&str, bool)] = &[
    // WPS brute-force (ruta por defecto Windows)
    ("reaver.exe", true),
    // WPS scanner
    ("wash.exe", true),
    // WPS brute-force opcional (fallback externo/WSL, no bloquea)
    ("bully.exe", false),
    // Pixie Dust helper opcional
    ("pixiewps.exe", false),
    // aircrack-ng suite (captura + inyeccion + rogue AP)
    ("airodump-ng.exe", true),
    ("aireplay-ng.exe", true),
    ("airbase-ng.exe", true),
    // cracking
    ("hashcat.exe", true),
    // wordlists
    ("rockyou.txt", true),
];

const INSTALL_HINTS: &[&str] = &[
    // 0  reaver.exe
    "tools/reaver.exe — compilado desde fuente con MSYS2/MinGW64 (ver tools/build/BUILD.md). O REAVER_PATH=<ruta>",
    // 1  wash.exe
    "tools/wash.exe — compilado desde fuente con MSYS2/MinGW64 (ver tools/build/BUILD.md). O WASH_PATH=<ruta>",
    // 2  bully.exe (opcional)
    "bully.exe OPCIONAL — Opción C: solo si existe (BULLY_PATH o tools/bully.exe o PATH). Sin él se usa reaver.exe. No portable a MinGW puro (linux/wireless.h).",
    // 3  pixiewps.exe (opcional)
    "pixiewps.exe OPCIONAL — helper Pixie Dust externo. Sin él reaver -K 1 sigue funcionando.",
    // 4  airodump-ng
    "tools/aircrack-ng-win/airodump-ng.exe — Official 1.7 Windows build (Cygwin)",
    // 5  aireplay-ng
    "tools/aircrack-ng-win/aireplay-ng.exe — Official 1.7 Windows build (Cygwin)",
    // 6  airbase-ng
    "tools/aircrack-ng-win/airbase-ng.exe — Official 1.7 Windows build (Cygwin)",
    // 7  hashcat
    "https://github.com/hashcat/hcat/releases  (descarga hashcat-*.7z, extrae hashcat.exe)",
    // 8  rockyou.txt
    "Descarga rockyou.txt (145 MB) y copiala en %APPDATA%\\UIFIPILL\\tools\\",
];

/// Hint de instalación para un ejecutable concreto (usado en preflight de commands.rs).
pub(crate) fn install_hint_for(exe: &str) -> &'static str {
    let base = exe.rsplit(['/', '\\']).next().unwrap_or(exe).to_lowercase();
    match base.as_str() {
        "reaver.exe" => INSTALL_HINTS[0],
        "wash.exe" => INSTALL_HINTS[1],
        "bully.exe" => INSTALL_HINTS[2],
        "pixiewps.exe" => INSTALL_HINTS[3],
        "airodump-ng.exe" => INSTALL_HINTS[4],
        "aireplay-ng.exe" => INSTALL_HINTS[5],
        "airbase-ng.exe" => INSTALL_HINTS[6],
        "hashcat.exe" => INSTALL_HINTS[7],
        "hcxdumptool.exe" => "hcxdumptool NO es portable a Windows (necesita nl80211/Linux). \
            Captura en Kali/WSL2 con hcxdumptool y convierte aquí con pcap_to_22000 (nativo), \
            o define HCXDUMPTOOL_PATH a un binario externo.",
        "hcxpcapngtool.exe" => "hcxpcapngtool NO es portable a Windows. \
            Usa la conversión NATIVA (pcap_to_22000, sin binario externo) desde la tarjeta \
            «Convertir .pcapng (Rust nativo)».",
        "mdk3.exe" => "mdk3 NO es portable a MinGW puro (sockets POSIX). Fuentes en tools/build/mdk3/. \
            Alternativa: beacon flood desde Kali, o define MDK3_PATH a un binario externo.",
        _ => "Binario no encontrado. Revisa la pestaña de herramientas (detect_tools).",
    }
}

/// Devuelve el estado de todas las herramientas. El frontend lo consulta al iniciar.
#[command]
pub async fn detect_tools() -> ToolsReport {
    let mut tools: Vec<ToolInfo> = vec![];
    let mut missing = vec![];

    for (i, (name, required)) in TOOL_LIST.iter().enumerate() {
        let (found, path, source) = resolve_tool(name);
        let version = if found { get_version(&path) } else { "—".into() };
        if !found && *required { missing.push((*name).into()); }
        tools.push(ToolInfo {
            name:       (*name).into(),
            found,
            full_path:  if found { path } else { "no encontrado".into() },
            version,
            source:     if found { source } else { "—".into() },
            install_hint: INSTALL_HINTS[i].into(),
            required:   *required,
        });
    }

    let ready_count = tools.iter().filter(|t| t.found).count();
    let required_ok = tools.iter().filter(|t| t.required).all(|t| t.found);
    let total = tools.len();
    ToolsReport {
        tools,
        all_ready:     required_ok,
        ready_count,
        total,
        missing_names: missing,
    }
}

/// Busca una herramienta concreta y devuelve la ruta.
#[command]
pub async fn find_tool_cmd(_app: AppHandle, name: String) -> CmdResponse {
    let (found, path, source) = resolve_tool(&name);
    if found {
        let ver = get_version(&path);
        let msg = format!("✅ ENCONTRADO\n  Ruta   : {}\n  Origen : {}\n  Versión: {}", path, source, ver);
        CmdResponse { success: true, output: msg, stderr: String::new(), exit_code: None }
    } else {
        let searched = install_dirs().iter()
            .map(|d| format!("  · {}", d.display()))
            .collect::<Vec<_>>()
            .join("\n");
        let hint = INSTALL_HINTS.get(0).unwrap_or(&"—").to_string();
        let msg = format!("❌ {} NO encontrado\n\nRutas buscadas:\n{}\n\nHint: {}", name, searched, hint);
        CmdResponse { success: false, output: msg, stderr: String::new(), exit_code: None }
    }
}

/// Comprueba si una herramienta existe antes de intentar usarla.
#[command]
pub async fn check_tool(_app: AppHandle, name: String) -> CmdResponse {
    let (found, path, _source) = resolve_tool(&name);
    CmdResponse {
        success: found,
        output:  if found { path } else { format!("{} no encontrado", name) },
        stderr:  String::new(),
        exit_code: None,
    }
}
