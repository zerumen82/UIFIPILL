// E0-23: memoria local de la IA. Persiste notas/preferencias/lecciones en
// %LOCALAPPDATA%\UIFIPILL\ia-learned.json (fuera del repo, no se commitea).
// Nunca guarda credenciales: las notas obviamente sensibles se rechazan.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub key: String,
    pub value: String,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryResult {
    pub success: bool,
    pub message: String,
    pub count: usize,
}

fn memory_path() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("UIFIPILL").join("ia-learned.json")
}

fn load() -> Vec<Note> {
    std::fs::read_to_string(memory_path()).ok()
        .and_then(|s| serde_json::from_str::<Vec<Note>>(&s).ok())
        .unwrap_or_default()
}

fn save(notes: &[Note]) -> Result<(), String> {
    let p = memory_path();
    if let Some(dir) = p.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    std::fs::write(&p, serde_json::to_string_pretty(notes).map_err(|e| e.to_string())?)
        .map_err(|e| format!("No se pudo escribir {}: {e}", p.display()))
}

fn looks_sensitive(s: &str) -> bool {
    let l = s.to_lowercase();
    ["password", "contraseña", "contrasena", "psk:", "wpa*02", "clave wifi"].iter()
        .any(|k| l.contains(k))
}

fn now() -> String {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string()).unwrap_or_else(|_| "0".into())
}

#[tauri::command]
pub fn ia_memory_add(key: String, value: String) -> MemoryResult {
    let (key, value) = (key.trim().to_string(), value.trim().to_string());
    if key.is_empty() || value.is_empty() {
        return MemoryResult { success: false, message: "Nota vacía: indica clave y contenido.".into(), count: 0 };
    }
    if key.len() > 60 || value.len() > 600 {
        return MemoryResult { success: false, message: "Nota demasiado larga (clave 60 / valor 600).".into(), count: 0 };
    }
    if looks_sensitive(&key) || looks_sensitive(&value) {
        return MemoryResult { success: false, message: "Rechazado: la memoria no guarda credenciales ni hashes.".into(), count: 0 };
    }
    let mut notes = load();
    if let Some(n) = notes.iter_mut().find(|n| n.key == key) {
        n.value = value; n.ts = now();
    } else {
        notes.push(Note { key, value, ts: now() });
    }
    if notes.len() > 100 { notes.remove(0); }
    let count = notes.len();
    match save(&notes) {
        Ok(_) => MemoryResult { success: true, message: "Nota guardada en la memoria local de la IA.".into(), count },
        Err(e) => MemoryResult { success: false, message: e, count: 0 },
    }
}

#[tauri::command]
pub fn ia_memory_list() -> Vec<Note> {
    let mut n = load();
    n.sort_by(|a, b| b.ts.cmp(&a.ts));
    n.into_iter().take(20).collect()
}

#[tauri::command]
pub fn ia_memory_clear() -> MemoryResult {
    match save(&[]) {
        Ok(_) => MemoryResult { success: true, message: "Memoria local de la IA borrada.".into(), count: 0 },
        Err(e) => MemoryResult { success: false, message: e, count: 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_credentials() {
        assert!(looks_sensitive("mi password es 1234"));
        assert!(looks_sensitive("WPA*02:abcd"));
        assert!(!looks_sensitive("usar PMKID antes que handshake"));
    }
}
