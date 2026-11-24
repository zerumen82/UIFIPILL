// E0-22 / E0-28: generadores de diccionarios. Escriben un fichero REAL en
// tools/wordlists/ y devuelven el número de líneas que hay en él.
// Regla del proyecto: sin "éxito" simulado; si el fichero no se escribe, falla.
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

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

/// Escribe la wordlist en el primer directorio existente y devuelve el
/// recuento REAL de líneas leídas del fichero (nunca del vector en memoria).
fn write_wordlist(base: &str, suffix: &str, words: Vec<String>) -> DictionaryResult {
    let fail = |m: String| DictionaryResult { success: false, path: String::new(), lines: 0, message: m };
    // En el instalado no existe tools/wordlists/ todavía: elegir el tools/
    // padre que sí existe (p. ej. <inst>/_up_/tools) y CREAR wordlists dentro.
    let dir = match wordlist_dirs().into_iter().find(|d| {
        d.exists() || d.parent().map(|p| p.exists()).unwrap_or(false)
    }) {
        Some(d) => d,
        None => return fail("No se encontró la carpeta tools/ donde guardar diccionarios.".into()),
    };
    if let Err(e) = fs::create_dir_all(&dir) { return fail(format!("No se pudo crear {}: {e}", dir.display())); }
    let name = format!("{}{}.txt", safe_name(base), suffix);
    let path = dir.join(&name);
    let body = words.join("\n") + "\n";
    if let Err(e) = fs::write(&path, &body) { return fail(format!("No se pudo escribir {}: {e}", path.display())); }
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

#[tauri::command]
pub fn generate_dictionary(ssid: String, bssid: String, hints: Option<String>) -> DictionaryResult {
    if ssid.trim().is_empty() && bssid.trim().is_empty() {
        return DictionaryResult { success: false, path: String::new(), lines: 0,
            message: "Sin SSID ni BSSID para generar candidatos.".into() };
    }
    let year = chrono_year();
    let words = build_candidates(&ssid, &bssid, hints.unwrap_or_default().as_str(), year);
    let base = if ssid.trim().is_empty() { &bssid } else { &ssid };
    write_wordlist(base, "", words)
}

/// E0-28: extrae candidatas de la respuesta del modelo. Acepta texto plano
/// (una por línea, con viñetas opcionales) o un array JSON; descarta
/// explicaciones, markdown y tokens con espacios (hashcat los necesita limpios).
fn add_candidate(out: &mut Vec<String>, seen: &mut HashSet<String>, s: &str, max: usize) {
    // Algunos modelos devuelven varias candidatas en una sola línea separadas
    // por comas; tratar cada token por separado.
    for token in s.split(|c: char| c == ',' || c == ';' || c == '|') {
        let t = token.trim().trim_matches(|c: char| c == '"' || c == '\'' || c == '`' || c == '*').trim();
        // WPA-PSK exige al menos 8 caracteres: descartar números sueltos y
        // recortes de numeración que el modelo a veces deja como líneas.
        if t.len() < 8 || t.len() > 64 { continue; }
        if !t.chars().all(|c| c.is_ascii_graphic()) { continue; }
        if seen.insert(t.to_string()) && out.len() < max { out.push(t.to_string()); }
    }
}

pub fn parse_ai_candidates(text: &str, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Array JSON: ["clave1","clave2"] o {"candidates":[...]}
    let trimmed = text.trim();
    if trimmed.starts_with('[') || trimmed.contains("\"candidates\"") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let arr: Option<&serde_json::Value> = if v.is_array() { Some(&v) } else { v.get("candidates") };
            if let Some(a) = arr.and_then(|x| x.as_array()) {
                for item in a {
                    if let Some(s) = item.as_str() { add_candidate(&mut out, &mut seen, s, max); }
                    if out.len() >= max { break; }
                }
                if !out.is_empty() { return out; }
            }
        }
    }
    for raw in text.lines() {
        let mut line = raw.trim();
        if line.starts_with("```") { continue; }
        if line.starts_with("- ") || line.starts_with("* ") { line = line[2..].trim(); }
        // Viñetas numéricas: "1. clave" / "1) clave"
        if let Some(pos) = line.find(|c: char| c == '.' || c == ')') {
            if pos > 0 && pos <= 3 && line[..pos].chars().all(|c| c.is_ascii_digit()) {
                line = line[pos + 1..].trim();
            }
        }
        add_candidate(&mut out, &mut seen, line, max);
    }
    out
}

#[tauri::command]
pub async fn generate_dictionary_ai(
    ssid: String,
    bssid: String,
    hints: Option<String>,
    model: Option<String>,
    limit: Option<usize>,
) -> DictionaryResult {
    let fail = |m: String| DictionaryResult { success: false, path: String::new(), lines: 0, message: m };
    if ssid.trim().is_empty() && bssid.trim().is_empty() {
        return fail("Sin SSID ni BSSID para pedir candidatas a la IA.".into());
    }
    let max = limit.unwrap_or(400).clamp(10, 2000);
    let mdl: String = model.unwrap_or_else(|| "hermes3:8b".into())
        .chars().take(80).map(|c| if c.is_ascii_graphic() { c } else { '_' }).collect();
    if mdl.is_empty() { return fail("Modelo de Ollama inválido.".into()); }

    // no_proxy(): en este equipo hay un proxy de sistema que reqwest aplica a
    // 127.0.0.1; Ollama es LOCAL y debe hablarse directo (medido 2026-11-23).
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(120)).no_proxy().build() {
        Ok(c) => c,
        Err(e) => return fail(format!("No se pudo crear el cliente Ollama: {e}")),
    };
    let prompt = format!(
        "Eres un generador de candidatas para un laboratorio WiFi propio y autorizado. \
         Red: SSID={ssid}, BSSID={bssid}, pistas={hints}. \
         Propón hasta {max} contraseñas candidatas plausibles (variantes del SSID, fabricante, \
         años, números y palabras comunes en español). \
         Devuelve SOLO la lista, una contraseña por línea, sin numeración, sin viñetas, \
         sin Markdown y sin explicaciones. Cada contraseña: 8-64 caracteres ASCII sin espacios. \
         No afirmes conocer la contraseña real.",
        ssid = ssid.trim(), bssid = bssid.trim(), hints = hints.unwrap_or_default().trim(), max = max
    );
    let body = serde_json::json!({ "model": mdl, "prompt": prompt, "stream": false });
    // `localhost` (no 127.0.0.1): es lo que ya usa ollama_check/ollama_chat
    // en este equipo y lo que responde de forma estable (medido 2026-11-23).
    let response = match client.post("http://localhost:11434/api/generate").json(&body).send().await {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
            Ok(v) => v.get("response").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            Err(e) => return fail(format!("Ollama devolvió una respuesta ilegible: {e}")),
        },
        Ok(r) => return fail(format!("Ollama respondió HTTP {}. ¿Está arrancado en 127.0.0.1:11434?", r.status())),
        Err(e) => return fail(format!("No se pudo contactar con Ollama en 127.0.0.1:11434: {e}. Arráncalo con `ollama serve`.")),
    };
    let mut words = parse_ai_candidates(&response, max);
    if words.is_empty() {
        return fail(format!(
            "Ollama no devolvió candidatas válidas (recibido: {} chars). Reintenta con otro modelo.",
            response.trim().len()
        ));
    }
    // Mezclar el generador determinista como red de seguridad: nunca reduces
    // cobertura respecto a E0-22.
    let year = chrono_year();
    let mut fallback = build_candidates(&ssid, &bssid, "", year);
    let tail = words.len();
    for w in fallback.drain(..) {
        if words.len() >= (max + tail).min(2000) { break; }
        if !words.contains(&w) { words.push(w); }
    }
    let base = if ssid.trim().is_empty() { &bssid } else { &ssid };
    let mut r = write_wordlist(base, "-ia", words);
    if r.success {
        r.message = format!("{} (candidatas IA + patrones base)", r.message);
    }
    r
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
    #[test]
    fn ai_candidates_parse_plain_and_json() {
        let plain = "```\n- casa1234\n- casa1234\n2) perro2024\n123\nLa clave es secreto (no lo soy)\n```\n";
        let got = parse_ai_candidates(plain, 100);
        assert!(got.contains(&"casa1234".to_string()));
        assert!(got.contains(&"perro2024".to_string()));
        assert_eq!(got.iter().filter(|x| *x == "casa1234").count(), 1);
        assert!(!got.iter().any(|x| x.contains(' ')));
        let json = "{\"candidates\":[\"abc12345\",\"abc12345\",\"x y\"]}";
        let got = parse_ai_candidates(json, 100);
        assert_eq!(got, vec!["abc12345".to_string()]);
    }
}
