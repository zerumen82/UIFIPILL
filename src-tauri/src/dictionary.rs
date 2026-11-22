// E0-22: generador de diccionarios dirigido desde la IA. Escribe un fichero
// REAL en tools/wordlists/ y devuelve el número de líneas que hay en él.
// Regla del proyecto: sin "éxito" simulado; si el fichero no se escribe, falla.
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct DictionaryResult {
    pub success: bool,
    pub path: String,
    pub lines: usize,
    pub message: String,
}

fn wordlist_dirs() -> Vec<PathBuf> {
    let mut d = Vec::new();
    // Primero la instalación: el dictamen debe caer donde crack_plan lo busca
    // (<exe>/_up_/tools/wordlists). En dev ese padre no existe y se usa el repo.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            d.push(p.join("_up_").join("tools").join("wordlists"));
            d.push(p.join("tools").join("wordlists"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        d.push(cwd.join("tools").join("wordlists"));
        d.push(cwd.join("..").join("tools").join("wordlists"));
    }
    for x in crate::tools_detect::install_dirs_pub() { d.push(x.join("wordlists")); }
    d
}

fn safe_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(40).collect::<String>().trim_matches('_').to_string();
    if n.is_empty() { "target".into() } else { n.to_lowercase() }
}

fn leet(s: &str) -> String {
    s.chars().map(|c| match c.to_ascii_lowercase() {
        'a' => '4', 'e' => '3', 'i' => '1', 'o' => '0', 's' => '5', _ => c,
    }).collect()
}

/// Genera candidatos deterministas a partir de datos REALES del objetivo.
fn push_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, s: String) {
    if !s.is_empty() && s.len() <= 64 && seen.insert(s.clone()) { out.push(s); }
}

pub fn build_candidates(ssid: &str, bssid: &str, hints: &str, year: i32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let ssid = ssid.trim();
    push_unique(&mut out, &mut seen, ssid.to_string());
    push_unique(&mut out, &mut seen, leet(ssid));
    let parts: Vec<&str> = ssid.split(|c: char| !c.is_ascii_alphanumeric()).filter(|p| p.len() > 1).collect();
    for p in &parts {
        push_unique(&mut out, &mut seen, (*p).to_string());
        push_unique(&mut out, &mut seen, leet(p));
    }
    if parts.len() > 1 {
        push_unique(&mut out, &mut seen, parts.concat());
        push_unique(&mut out, &mut seen, parts.join("-"));
        push_unique(&mut out, &mut seen, parts.join("_"));
    }
    let hex: String = bssid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    push_unique(&mut out, &mut seen, hex.clone());
    if hex.len() >= 4 { push_unique(&mut out, &mut seen, hex[hex.len() - 4..].to_string()); }
    if hex.len() >= 6 { push_unique(&mut out, &mut seen, hex[..6].to_string()); } // OUI
    for h in hints.split(|c: char| c == ',' || c == ' ' || c == ';').filter(|x| x.len() > 1) {
        push_unique(&mut out, &mut seen, h.to_string());
    }
    let bases: Vec<String> = out.iter().take(16).cloned().collect();
    for b in &bases {
        for y in (year - 6)..=year {
            push_unique(&mut out, &mut seen, format!("{b}{y}"));
            push_unique(&mut out, &mut seen, format!("{b}-{y}"));
        }
        for s in ["123", "1234", "12345", "01", "007", "2024", "2025", "2026", "admin", "wifi", "lab"] {
            push_unique(&mut out, &mut seen, format!("{b}{s}"));
        }
    }
    out
}

#[tauri::command]
pub fn generate_dictionary(ssid: String, bssid: String, hints: Option<String>) -> DictionaryResult {
    let fail = |m: String| DictionaryResult { success: false, path: String::new(), lines: 0, message: m };
    if ssid.trim().is_empty() && bssid.trim().is_empty() {
        return fail("Sin SSID ni BSSID para generar candidatos.".into());
    }
    let year = chrono_year();
    let words = build_candidates(&ssid, &bssid, hints.unwrap_or_default().as_str(), year);
    // En el instalado no existe tools/wordlists/ todavía: elegir el tools/
    // padre que sí existe (p. ej. <inst>/_up_/tools) y CREAR wordlists dentro.
    let dir = match wordlist_dirs().into_iter().find(|d| {
        d.exists() || d.parent().map(|p| p.exists()).unwrap_or(false)
    }) {
        Some(d) => d,
        None => return fail("No se encontró la carpeta tools/ donde guardar diccionarios.".into()),
    };
    if let Err(e) = fs::create_dir_all(&dir) { return fail(format!("No se pudo crear {}: {e}", dir.display())); }
    let name = format!("{}.txt", safe_name(if ssid.trim().is_empty() { &bssid } else { &ssid }));
    let path = dir.join(&name);
    let body = words.join("\n") + "\n";
    if let Err(e) = fs::write(&path, &body) { return fail(format!("No se pudo escribir {}: {e}", path.display())); }
    // Conteo REAL releyendo el fichero (nunca el vector en memoria).
    let lines = fs::read_to_string(&path).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    if lines == 0 { return fail("El fichero se escribió vacío (no se inventa éxito).".into()); }
    DictionaryResult {
        success: true,
        path: path.to_string_lossy().into_owned(),
        lines,
        message: format!("✅ {lines} candidatos únicos escritos en {}", path.display()),
    }
}

fn chrono_year() -> i32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| 1970 + (d.as_secs() / 31_536_000) as i32)
        .unwrap_or(2026)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidates_are_deterministic_and_deduped() {
        let a = build_candidates("MiWiFi-2024", "AA:BB:CC:DD:EE:FF", "perro, gato", 2026);
        let b = build_candidates("MiWiFi-2024", "AA:BB:CC:DD:EE:FF", "perro, gato", 2026);
        assert_eq!(a, b);
        assert!(a.iter().any(|x| x == "MiWiFi-2024"));
        assert!(a.iter().any(|x| x == "EEFF"));
        assert!(a.iter().any(|x| x == "AABBCC"));
        let mut u = a.clone(); u.sort(); u.dedup();
        assert_eq!(u.len(), a.len());
    }
    #[test]
    fn safe_name_blocks_paths() {
        assert_eq!(safe_name("../../etc/passwd"), "etc_passwd");
        assert_eq!(safe_name(""), "target");
    }
}
