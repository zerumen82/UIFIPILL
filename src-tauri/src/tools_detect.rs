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
fn resolve_tool(name: &str) -> (bool, String, String) {
    // 1) Variable de entorno
    let env_var = format!("{}_PATH", name.to_uppercase().replace('.', "_"));
    if let Ok(p) = std::env::var(&env_var) {
        let pstr = p.trim().to_string();
        if std::path::Path::new(&pstr).exists() {
            return (true, pstr, "variable de entorno".into());
        }
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

fn install_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = vec![
        dirs::data_local_dir()
            .map(|p| p.join("UIFIPILL").join("tools")),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("tools"))),
    ].into_iter().filter_map(|o| o).collect();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() { dirs.push(p.to_path_buf()); }
    }
    dirs
}

fn get_version(exe: &str) -> String {
    let out = std::process::Command::new(exe)
        .args(["--version"])
        .output();
    match out {
        Ok(ref o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { "—".into() } else { s.lines().next().unwrap_or("?").into() }
        }
        Ok(_) => "sin --version".into(),
        Err(_) => "no ejecutable".into(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name:         String,
    pub found:        bool,
    pub full_path:    String,
    pub version:      String,
    pub source:       String,
    pub install_hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolsReport {
    pub tools:         Vec<ToolInfo>,
    pub all_ready:     bool,
    pub ready_count:   usize,
    pub missing_names: Vec<String>,
}

const TOOL_LIST: &[&str] = &[
    // hcxtools
    "hcxdumptool",
    "hcxpcapngtool",
    // hashcat
    "hashcat",
    // WPS
    "bully",
    "reaver-wps-fork-t6x",
    // aircrack-ng suite
    "airodump-ng",
    "aireplay-ng",
    "airbase-ng",
    "mdk3",
    "aircrack-ng",
    // wordlists
    "rockyou.txt",
];

const INSTALL_HINTS: &[&str] = &[
    "https://github.com/cipsas-uifipill/hcxtools/releases   (Win64 .exe)",
    "Mismo paquete .zip que hcxdumptool",
    "https://hashcat.net/hashcat/   (SE Self-Extracting .exe)",
    "https://github.com/T6X/bully   (Win64 .exe)",
    "https://github.com/T6X/reaver-wps-fork-t6x   (Win64 .exe)",
    "https://github.com/aircrack-ng/aircrack-ng/releases   (Win64 .exe)",
    "https://github.com/aircrack-ng/aircrack-ng/releases   (Win64 .exe)",
    "https://github.com/aircrack-ng/aircrack-ng/releases   (Win64 .exe)",
    "https://github.com/aircrack-ng/aircrack-ng/releases   (Win64 .exe)",
    "https://github.com/aircrack-ng/aircrack-ng/releases   (Win64 .exe)",
    "Descarga rockyou.txt y copiala en %APPDATA%\\UIFIPILL\\tools\\",
];

/// Devuelve el estado de todas las herramientas. El frontend lo consulta al iniciar.
#[command]
pub async fn detect_tools() -> ToolsReport {
    let mut tools: Vec<ToolInfo> = vec![];
    let mut missing = vec![];

    for (i, name) in TOOL_LIST.iter().enumerate() {
        let (found, path, source) = resolve_tool(name);
        let version = if found { get_version(&path) } else { "—".into() };
        if !found { missing.push((*name).into()); }
        tools.push(ToolInfo {
            name:       (*name).into(),
            found,
            full_path:  if found { path } else { "no encontrado".into() },
            version,
            source:     if found { source } else { "—".into() },
            install_hint: INSTALL_HINTS[i].into(),
        });
    }

    let ready_count = tools.iter().filter(|t| t.found).count();
    ToolsReport {
        tools,
        all_ready:     ready_count == TOOL_LIST.len(),
        ready_count,
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
