// Pipeline de candidatas dirigido (E0-16/E0-17). Offline, sin RF.
// Éxito = .cracked escrito por hashcat (exit 0) o .cracked existente. Sin fakes.
use crate::commands::CmdResponse;
use crate::crack::valid_mask;
use crate::keygen::{self, KeygenAlg};
use crate::tools_detect::resolve_tool;
use crate::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{command, AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::ShellExt;

static PLAN_CANCEL: once_cell::sync::Lazy<Mutex<Option<std::sync::Arc<AtomicBool>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize)]
pub struct PlanStage {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub keyspace: Option<u64>,
    pub why: String,
    pub enabled: bool,
    pub skip_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrackPlan {
    pub hash_file: String,
    pub ssid: String,
    pub bssid: String,
    pub stages: Vec<PlanStage>,
    pub cracked_already: bool,
}

fn cracked_path(hash_file: &str) -> PathBuf {
    PathBuf::from(format!("{}.cracked", hash_file))
}

fn read_cracked(hash_file: &str) -> Option<String> {
    let p = cracked_path(hash_file);
    let txt = std::fs::read_to_string(p).ok()?;
    txt.lines().map(str::trim).find(|l| !l.is_empty()).map(|s| s.to_string())
}

fn hashcat_prog() -> Result<String, String> {
    let (found, hc, _) = resolve_tool("hashcat.exe");
    if found || Path::new("hashcat.exe").exists() {
        Ok(if found { hc } else { "hashcat.exe".into() })
    } else {
        Err("hashcat.exe no encontrado. Colócalo en tools/ o define HASHCAT_PATH.".into())
    }
}

/// Mayor de hashcat (p.ej. 8 para v8.x). None si no se puede determinar.
fn hashcat_major() -> Option<u32> {
    let prog = hashcat_prog().ok()?;
    for args in [["--version"].as_slice(), ["-V"].as_slice()] {
        if let Ok(out) = std::process::Command::new(&prog).args(args).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            let s = if s.trim().is_empty() { String::from_utf8_lossy(&out.stderr) } else { s };
            // Ej.: "v8.0.0", "hashcat (v7.1.2) starting..."
            if let Some(pos) = s.find(|c: char| c.is_ascii_digit()) {
                let rest = &s[pos..];
                let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = num.parse::<u32>() { return Some(n); }
            }
        }
    }
    None
}


fn supports_pcfg(major: Option<u32>) -> bool {
    major.map(|m| m >= 8).unwrap_or(false)
}

/// Hintword seguro para hashcat PCFG hints (coma-separado; sin espacios/inyección).
fn sanitize_hintword(ssid: &str) -> Option<String> {
    let s = ssid.trim();
    if s.is_empty() || s.len() > 32 { return None; }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
        Some(s.to_string())
    } else {
        None
    }
}

/// Estima keyspace de una máscara hashcat (charset built-in). None si inválida/desconocida.
pub fn mask_keyspace(mask: &str) -> Option<u64> {
    if !valid_mask(mask) { return None; }
    let mut total: u64 = 1;
    let mut it = mask.chars().peekable();
    while let Some(ch) = it.next() {
        let n: u64 = if ch == '?' {
            match it.next()? {
                'l' | 'u' => 26,
                'd' => 10,
                's' => 33,
                'a' => 95,
                'b' => 256,
                'h' | 'H' => 16,
                _ => return None,
            }
        } else {
            1
        };
        total = total.checked_mul(n)?;
    }
    Some(total)
}

fn sanitize_ssid_literal(ssid: &str) -> Option<String> {
    let s = ssid.trim();
    if s.is_empty() || s.len() > 12 { return None; }
    // Literales seguros para máscara hashcat (valid_mask permite estos)
    if s.chars().all(|c| c.is_ascii_graphic() && !"&;|$`<>(){}\n\r?".contains(c)) {
        Some(s.to_string())
    } else {
        None
    }
}

/// Máscaras de bajo coste derivadas del SSID (heurísticas conservadoras, documentadas).
fn derive_masks(ssid: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    // Universales: patrones numéricos muy comunes en routers domésticos.
    for (m, why) in [
        ("?d?d?d?d?d?d?d?d", "8 dígitos (patrón numérico frecuente en routers)"),
        ("?d?d?d?d?d?d?d?d?d?d", "10 dígitos (teléfono/móvil como clave)"),
    ] {
        out.push((m.to_string(), why.to_string()));
    }
    // Derivado del SSID: literal + sufijo numérico corto, solo si el SSID es seguro como literal.
    if let Some(lit) = sanitize_ssid_literal(ssid) {
        for (suf, why) in [
            ("?d?d?d?d", "SSID + 4 dígitos (sufijo numérico corto)"),
            ("?d?d?d?d?d?d?d?d", "SSID + 8 dígitos"),
        ] {
            let m = format!("{}{}", lit, suf);
            if let Some(k) = mask_keyspace(&m) {
                // Solo si el espacio sigue siendo acotado (<= ~10^10)
                if k <= 10_000_000_000u64 {
                    out.push((m, format!("{} — derivado del SSID «{}»", why, lit)));
                }
            }
        }
    }
    out
}

fn wordlist_search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        if let Some(ws) = Path::new(&manifest).parent() {
            dirs.push(ws.join("tools"));
            dirs.push(ws.join("tools").join("wordlists"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            dirs.push(p.join("tools"));
            dirs.push(p.join("tools").join("wordlists"));
            dirs.push(p.join("_up_").join("tools"));
            dirs.push(p.join("_up_").join("tools").join("wordlists"));
        }
    }
    for d in crate::tools_detect::install_dirs_pub() {
        dirs.push(d.join("wordlists"));
        dirs.push(d);
    }
    dirs
}

fn find_wordlist(name_fragment: &str) -> Option<String> {
    let frag = name_fragment.to_lowercase();
    let mut candidates: Vec<PathBuf> = Vec::new();
    // exact name in known dirs
    for dir in wordlist_search_dirs() {
        candidates.push(dir.join(name_fragment));
    }
    for c in &candidates {
        if c.exists() { return Some(c.to_string_lossy().into_owned()); }
    }
    // fragment match
    for dir in wordlist_search_dirs() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("txt")).unwrap_or(false) {
                    if p.file_name().and_then(|n| n.to_str()).map(|n| n.to_lowercase().contains(&frag)).unwrap_or(false) {
                        return Some(p.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}

/// Busca wordlists adicionales y exportaciones de candidatas (p.ej. PassGAN/PassGPT)
/// colocadas por el usuario en tools/ o tools/wordlists/.
fn find_extra_wordlists(exclude: Option<&str>) -> Vec<(String, String)> {
    let names = [
        "probable-v2-top1575.txt",
        "probable-v2-top12000.txt",
        "10-million-password-list-top-1000000.txt",
        "darkweb2017-top10000.txt",
        "passwords.txt",
        "common.txt",
    ];
    let mut out: Vec<(String, String)> = Vec::new();
    let ex = exclude.unwrap_or("").to_lowercase();
    let mut push = |label: String, p: String| {
        if p.to_lowercase() == ex { return; }
        if out.iter().any(|(_, x): &(String, String)| x.to_lowercase() == p.to_lowercase()) { return; }
        out.push((label, p));
    };
    for n in names {
        if let Some(p) = find_wordlist(n) {
            push(n.to_string(), p);
        }
    }
    // Candidatas generadas externamente (ML/PCFG exportadas a fichero)
    let prefixes = ["candidates", "passgan", "passgpt", "generated", "samples", "wordgen"];
    for dir in wordlist_search_dirs() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if !p.is_file() { continue; }
                let fname = match p.file_name().and_then(|n| n.to_str()) { Some(s) => s.to_string(), None => continue };
                let lower = fname.to_lowercase();
                if !lower.ends_with(".txt") && !lower.ends_with(".lst") && !lower.ends_with(".dict") && !lower.ends_with(".wordlist") { continue; }
                if prefixes.iter().any(|pf| lower.starts_with(pf)) {
                    let full = p.to_string_lossy().into_owned();
                    push(fname, full);
                }
            }
        }
    }
    out
}

fn find_rule(name: &str) -> Option<String> {
    let (found, hc, _) = resolve_tool("hashcat.exe");
    let base = if found { PathBuf::from(&hc).parent().map(|p| p.to_path_buf()).unwrap_or_default() } else { PathBuf::new() };
    let p = base.join("rules").join(name);
    if p.exists() { return Some(p.to_string_lossy().into_owned()); }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        if let Some(ws) = Path::new(&manifest).parent() {
            let q = ws.join("tools").join("rules").join(name);
            if q.exists() { return Some(q.to_string_lossy().into_owned()); }
        }
    }
    None
}

pub fn build_plan(hash_file: &str, ssid: &str, bssid: &str, external_generator: Option<&str>) -> CrackPlan {
    let cracked_already = read_cracked(hash_file).is_some();
    let hc_ok = hashcat_prog().is_ok();
    let mut stages: Vec<PlanStage> = Vec::new();

    // 1) Keygen determinista (CPU, sin GPU)
    let alg = keygen::detect_alg(ssid);
    match alg {
        KeygenAlg::Comtrend => {
            let (ks, why) = match keygen::comtrend_keys(ssid, bssid) {
                Ok(k) => (Some(k.len() as u64), format!("Comtrend (Jazztel_/WLAN_): {} candidato(s) determinista(s) desde SSID+BSSID — verificación directa.", k.len())),
                Err(e) => (None, format!("Comtrend detectado pero BSSID inválido: {}", e)),
            };
            stages.push(PlanStage {
                id: "keygen_comtrend".into(),
                label: "Keygen Comtrend (claves de fábrica)".into(),
                kind: "keygen".into(),
                keyspace: ks,
                why,
                enabled: hc_ok && ks.is_some(),
                path: None,
                skip_reason: if !hc_ok { Some("hashcat.exe no disponible".into()) } else if ks.is_none() { Some("BSSID inválido para este keygen".into()) } else { None },
            });
        }
        KeygenAlg::Thomson => {
            stages.push(PlanStage {
                id: "keygen_thomson".into(),
                label: "Keygen Thomson/SpeedTouch (claves de fábrica)".into(),
                kind: "keygen".into(),
                keyspace: Some(21_840_192),
                why: "Búsqueda Thomson (RouterKeygen, SHA1 CP+AÑO+SEMANA+dic 2004-2012). CPU con threads, verificación con hashcat.".into(),
                enabled: hc_ok,
                path: None,
                skip_reason: if !hc_ok { Some("hashcat.exe no disponible".into()) } else { None },
            });
        }
        KeygenAlg::None => {}
    }

    // 2) Máscaras derivadas (bajo coste → mayor)
    let mut masks = derive_masks(ssid);
    masks.sort_by_key(|(m, _)| mask_keyspace(m).unwrap_or(u64::MAX));
    for (i, (m, why)) in masks.into_iter().enumerate() {
        let ks = mask_keyspace(&m);
        let enabled = hc_ok && ks.map(|k| k > 0 && k <= 10_000_000_000u64).unwrap_or(false);
        stages.push(PlanStage {
            id: format!("mask_{}", i),
            label: format!("Máscara: {}", m),
            kind: "mask".into(),
            keyspace: ks,
            why,
            enabled,
            path: None,
            skip_reason: if !hc_ok { Some("hashcat.exe no disponible".into()) } else if !enabled { Some("Keyspace inválido o demasiado grande".into()) } else { None },
        });
    }

    // 3) Diccionarios locales
    let mut primary_wl: Option<String> = None;
    if let Some(wl) = find_wordlist("rockyou.txt") {
        primary_wl = Some(wl.clone());
        stages.push(PlanStage {
            id: "rockyou".into(),
            label: "Diccionario rockyou.txt".into(),
            kind: "wordlist".into(),
            keyspace: Some(14_344_391),
            why: format!("Wordlist local: {} (ataque -a 0).", wl),
            enabled: hc_ok,
            path: None,
            skip_reason: if !hc_ok { Some("hashcat.exe no disponible".into()) } else { None },
        });
        if let Some(rule) = find_rule("best64.rule") {
            stages.push(PlanStage {
                id: "rockyou_best64".into(),
                label: "rockyou.txt + best64.rule".into(),
                kind: "wordlist_rule".into(),
                keyspace: Some(14_344_391 * 64),
                why: format!("Wordlist {} con regla {} (mutaciones comunes).", wl, rule),
                enabled: hc_ok,
                path: None,
                skip_reason: if !hc_ok { Some("hashcat.exe no disponible".into()) } else { None },
            });
        }
        if let Some(rule) = find_rule("dive.rule") {
            stages.push(PlanStage {
                id: "rockyou_dive".into(),
                label: "rockyou.txt + dive.rule".into(),
                kind: "wordlist_rule".into(),
                keyspace: Some(14_344_391 * 100_000),
                why: format!("Wordlist {} con regla {} (más mutaciones; puede tardar bastante).", wl, rule),
                enabled: hc_ok,
                path: None,
                skip_reason: if !hc_ok { Some("hashcat.exe no disponible".into()) } else { None },
            });
        }
    } else {
        stages.push(PlanStage {
            id: "rockyou".into(),
            label: "Diccionario rockyou.txt".into(),
            kind: "wordlist".into(),
            keyspace: None,
            why: "rockyou.txt no encontrado en tools/ (descárgalo y colócalo en tools/ para habilitar esta etapa).".into(),
            enabled: false,
            path: None,
            skip_reason: Some("rockyou.txt no encontrado en tools/".into()),
        });
    }
    // Diccionarios/candidatas adicionales detectados (top-N, exports PassGAN/PassGPT, etc.)
    let cand_prefixes = ["candidates", "passgan", "passgpt", "generated", "samples", "wordgen"];
    for (name, path) in find_extra_wordlists(primary_wl.as_deref()) {
        let lower = name.to_lowercase();
        let is_cand = cand_prefixes.iter().any(|pf| lower.starts_with(pf));
        let id = format!("wl_{}", name.replace(|c: char| !c.is_ascii_alphanumeric(), "_").to_lowercase());
        let (label, why) = if is_cand {
            (format!("Candidatas {}", name), format!("Wordlist de candidatas detectada (p.ej. export PassGAN/PassGPT/PCFG): {} (ataque -a 0).", path))
        } else {
            (format!("Diccionario {}", name), format!("Wordlist local detectada: {} (ataque -a 0).", path))
        };
        stages.push(PlanStage {
            id,
            label,
            kind: "wordlist_path".into(),
            keyspace: None,
            why,
            enabled: hc_ok,
            path: Some(path.clone()),
            skip_reason: if !hc_ok { Some("hashcat.exe no disponible".into()) } else { None },
        });
    }

    // 4) PCFG (hashcat v8+, -a 4) — probabilístico, muy útil
    let major = hashcat_major();
    let pcfg_ok = hc_ok && supports_pcfg(major);
    let pcfg_reason = if !hc_ok {
        Some("hashcat.exe no disponible".into())
    } else if !supports_pcfg(major) {
        Some(format!("Requiere hashcat v8+ con ataque -a 4 PCFG (detectado: {}).", major.map(|m| format!("v{}", m)).unwrap_or_else(|| "versión desconocida".into())))
    } else {
        None
    };
    stages.push(PlanStage {
        id: "pcfg_default".into(),
        label: "PCFG (hashcat -a 4, gramática por defecto)".into(),
        kind: "pcfg".into(),
        keyspace: None,
        why: "Ataque probabilístico (hashcat v8+): genera candidatas por probabilidad desde gramática entrenada; bueno para contraseñas «humanas».".into(),
        enabled: pcfg_ok,
        path: None,
        skip_reason: pcfg_reason.clone(),
    });
    if let Some(hw) = sanitize_hintword(ssid) {
        stages.push(PlanStage {
            id: "pcfg_hints".into(),
            label: format!("PCFG dirigido (hints: {})", hw),
            kind: "pcfg_hints".into(),
            keyspace: None,
            why: format!("PCFG con pistas del objetivo (hintword='{}', derivado del SSID): prueba estructuras probables alrededor de esa palabra.", hw),
            enabled: pcfg_ok,
            path: None,
            skip_reason: pcfg_reason,
        });
    }

    // 5) Generador externo opcional (wordlist / feed plugin / slow-candidates)
    if let Some(ext) = external_generator {
        let p = Path::new(ext);
        let ok = p.exists();
        let lower = ext.to_lowercase();
        let (kind, label, why) = if lower.ends_with(".txt") || lower.ends_with(".lst") || lower.ends_with(".dict") || lower.ends_with(".wordlist") {
            ("external_wordlist".to_string(), "Generador externo (wordlist)".to_string(),
             format!("Wordlist de candidatas aportada por el usuario: {} (ataque -a 0). Útil para PassGAN/PassGPT muestreado a fichero.", ext))
        } else if lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib") || p.file_name().and_then(|n| n.to_str()).map(|n| n.to_lowercase().starts_with("feed")).unwrap_or(false) {
            ("external_feed".to_string(), "Generador externo (hashcat feed -a 8)".to_string(),
             format!("Feed plugin de hashcat aportado por el usuario: {} (ataque -a 8).", ext))
        } else {
            ("external_slow".to_string(), "Generador externo (--slow-candidates)".to_string(),
             format!("Ejecutable/generador stdout: {}. Se intenta --slow-candidates; si tu hashcat no lo soporta, exporta las candidatas a un .txt y vuelve a probar.", ext))
        };
        let (enabled, skip) = if !hc_ok {
            (false, Some("hashcat.exe no disponible".into()))
        } else if !ok {
            (false, Some("Ruta del generador no existe".into()))
        } else if kind == "external_feed" && !supports_pcfg(major) {
            (false, Some(format!("Feed -a 8 requiere hashcat v8+ (detectado: {}).", major.map(|m| format!("v{}", m)).unwrap_or_else(|| "versión desconocida".into()))))
        } else {
            (true, None)
        };
        stages.push(PlanStage {
            id: "external_gen".into(),
            label,
            kind,
            keyspace: None,
            why,
            enabled,
            path: None,
            skip_reason: skip,
        });
    }

    CrackPlan {
        hash_file: hash_file.to_string(),
        ssid: ssid.to_string(),
        bssid: bssid.to_string(),
        stages,
        cracked_already,
    }
}

async fn run_hashcat(
    app: &AppHandle,
    args: Vec<String>,
    attack_id: &str,
    cancel: &std::sync::Arc<AtomicBool>,
) -> (i32, String, String) {
    let prog = match hashcat_prog() { Ok(p) => p, Err(e) => return (-1, String::new(), e) };
    let shell = app.shell();
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (mut rx, child) = match shell.command(&prog).args(&arg_refs).spawn() {
        Ok(r) => r,
        Err(e) => return (-1, String::new(), e.to_string()),
    };
    // Registrar para cancelación externa (■ Detener / cancel_attack sobre crack_plan)
    {
        let st = app.state::<crate::AppState>();
        let mut map = st.running_attacks.lock().unwrap();
        map.insert(attack_id.to_string(), child);
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = -1i32;
    use tauri_plugin_shell::process::CommandEvent;
    while let Some(event) = rx.recv().await {
        if cancel.load(Ordering::Relaxed) {
            let st = app.state::<crate::AppState>();
            if let Some(ch) = st.running_attacks.lock().unwrap().remove(attack_id) {
                let _ = ch.kill();
            }
            return (-2, stdout, "cancelado".into());
        }
        match event {
            CommandEvent::Stdout(d) => {
                let t = String::from_utf8_lossy(&d).into_owned();
                stdout.push_str(&t);
            }
            CommandEvent::Stderr(d) => {
                let t = String::from_utf8_lossy(&d).into_owned();
                stderr.push_str(&t);
            }
            CommandEvent::Terminated(s) => { exit_code = s.code.unwrap_or(-1); break; }
            CommandEvent::Error(e) => { stderr.push_str(&e); exit_code = -1; break; }
            _ => {}
        }
    }
    {
        let st = app.state::<crate::AppState>();
        let mut map = st.running_attacks.lock().unwrap();
        map.remove(attack_id);
    }
    (exit_code, stdout, stderr)
}

fn emit_progress(app: &AppHandle, attack_id: &str, msg: &str) {
    let _ = app.emit("attack-progress", serde_json::json!({ "id": attack_id, "type": "stdout", "data": msg }));
}

async fn run_stage(
    app: &AppHandle,
    attack_id: &str,
    stage: &PlanStage,
    hash_file: &str,
    ssid: &str,
    bssid: &str,
    external_generator: Option<&str>,
    cancel: &std::sync::Arc<AtomicBool>,
) -> Result<(bool, String), String> {
    if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
    if let Some(pw) = read_cracked(hash_file) { return Ok((true, format!("Ya crackeado: {}", pw))); }

    let cracked_out = format!("{}.cracked", hash_file);
    match stage.kind.as_str() {
        "keygen" => {
            let candidates: Vec<String> = if stage.id == "keygen_comtrend" {
                keygen::comtrend_keys(ssid, bssid).unwrap_or_default()
            } else {
                let ssid_owned = ssid.to_string();
                tauri::async_runtime::spawn_blocking(move || {
                    keygen::thomson_search(&ssid_owned, 50, 4..=12, 1..=52)
                })
                .await
                .map_err(|e| e.to_string())?
                .map(|(hits, _)| hits.into_iter().map(|h| h.key).collect())
                .unwrap_or_else(|e| { let _ = e; Vec::new() })
            };
            if candidates.is_empty() {
                return Ok((false, "Sin candidatos de keygen.".into()));
            }
            let tmp = std::env::temp_dir().join(format!("uifipill_keygen_{}.txt", std::process::id()));
            std::fs::write(&tmp, candidates.join("\n")).map_err(|e| e.to_string())?;
            emit_progress(app, attack_id, &format!("[plan] {}: verificando {} candidato(s)…\n", stage.label, candidates.len()));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "0".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), hash_file.to_string(), tmp.to_string_lossy().into_owned()];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            let _ = std::fs::remove_file(&tmp);
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada por keygen.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Keygen agotado sin coincidencia (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "mask" => {
            let mask = stage.label.strip_prefix("Máscara: ").unwrap_or(&stage.label).to_string();
            if !valid_mask(&mask) { return Err("Máscara inválida".into()); }
            emit_progress(app, attack_id, &format!("[plan] {}…\n", stage.label));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "3".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string(), mask];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada por máscara.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Máscara agotada sin coincidencia (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "wordlist" => {
            let wl = find_wordlist("rockyou.txt").ok_or("rockyou.txt no encontrado")?;
            emit_progress(app, attack_id, &format!("[plan] {}…\n", stage.label));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "0".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string(), wl];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada en diccionario.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Diccionario agotado sin coincidencia (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "wordlist_path" => {
            let wl = stage.path.clone()
                .filter(|p| Path::new(p).exists())
                .or_else(|| {
                    let name = stage.label.strip_prefix("Diccionario ").unwrap_or("");
                    find_wordlist(name)
                })
                .ok_or("Wordlist no encontrada")?;
            emit_progress(app, attack_id, &format!("[plan] {}…\n", stage.label));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "0".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string(), wl];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada en diccionario.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Diccionario agotado sin coincidencia (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "wordlist_rule" => {
            let wl = find_wordlist("rockyou.txt").ok_or("rockyou.txt no encontrado")?;
            let rule_name = if stage.id == "rockyou_dive" { "dive.rule" } else { "best64.rule" };
            let rule = find_rule(rule_name).ok_or(format!("{} no encontrada", rule_name))?;
            emit_progress(app, attack_id, &format!("[plan] {}…\n", stage.label));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "0".into(), "-r".into(), rule, "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string(), wl];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada con regla.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Diccionario+regla agotado (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "pcfg" => {
            emit_progress(app, attack_id, &format!("[plan] {}…\n", stage.label));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "4".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string()];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if se.contains("Unknown option") || (se.contains("-a") && se.contains("4")) || se.contains("Invalid attack mode") {
                return Err("hashcat no soporta -a 4 (PCFG) en esta instalación (etapa omitida).".into());
            }
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada con PCFG.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("PCFG agotado sin coincidencia (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "pcfg_hints" => {
            let hint = sanitize_hintword(ssid).ok_or("SSID no utilizable como hint")?;
            emit_progress(app, attack_id, &format!("[plan] {}…\n", stage.label));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "4".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string(), "hints".into(), format!("hintwords={}", hint)];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if se.contains("Unknown option") || se.contains("Invalid attack mode") || se.contains("ruleset") && se.contains("No such file") {
                return Err("hashcat no soporta PCFG+hints en esta instalación (etapa omitida).".into());
            }
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada con PCFG dirigido (hints).".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("PCFG hints agotado sin coincidencia (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "external_wordlist" => {
            let ext = external_generator.ok_or("Sin wordlist externa")?;
            if !Path::new(ext).exists() { return Err("Wordlist externa no encontrada".into()); }
            emit_progress(app, attack_id, &format!("[plan] {}…\n", stage.label));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "0".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string(), ext.to_string()];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada con wordlist externa.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Wordlist externa agotada (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "external_feed" => {
            let ext = external_generator.ok_or("Sin feed externo")?;
            if !Path::new(ext).exists() { return Err("Feed externo no encontrado".into()); }
            emit_progress(app, attack_id, &format!("[plan] Feed externo {}…\n", ext));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "8".into(), ext.to_string(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string()];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if se.contains("Unknown option") || se.contains("Invalid attack mode") || se.contains("No such file or directory") && se.contains("feed") {
                return Err("hashcat no soporta este feed -a 8 en esta instalación (etapa omitida).".into());
            }
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada con feed externo.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Feed externo agotado (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        "external_slow" | "external" => {
            let ext = external_generator.ok_or("Sin generador externo")?;
            if !Path::new(ext).exists() { return Err("Generador externo no encontrado".into()); }
            emit_progress(app, attack_id, &format!("[plan] Generador externo {}…\n", ext));
            let args = vec!["-m".into(), "22000".into(), "-a".into(), "0".into(), "--slow-candidates".into(), "-o".into(), cracked_out, "--force".into(), "--potfile-disable".into(), "--status".into(), "--status-timer=10".into(), hash_file.to_string(), ext.to_string()];
            let (code, _so, se) = run_hashcat(app, args, attack_id, cancel).await;
            if se.contains("Unknown option") || se.contains("--slow-candidates") {
                return Err("hashcat no soporta --slow-candidates: exporta las candidatas a .txt (PassGAN/PassGPT) o usa un feed -a 8 (etapa omitida).".into());
            }
            if read_cracked(hash_file).is_some() { return Ok((true, "Clave encontrada por generador externo.".into())); }
            if cancel.load(Ordering::Relaxed) { return Err("cancelado".into()); }
            Ok((false, format!("Generador externo agotado (exit {}). {}", code, se.lines().next().unwrap_or(""))))
        }
        _ => Err("Tipo de etapa desconocido".into()),
    }
}

#[command]
pub async fn crack_plan_preview(hash_file: String, ssid: String, bssid: String, external_generator: Option<String>) -> Result<CrackPlan, String> {
    if !Path::new(&hash_file).exists() {
        return Err(format!("No existe el hash: {}", hash_file));
    }
    Ok(build_plan(&hash_file, &ssid, &bssid, external_generator.as_deref()))
}

#[command]
pub async fn crack_plan_run_bg(
    app: AppHandle,
    _state: State<'_, AppState>,
    hash_file: String,
    ssid: String,
    bssid: String,
    external_generator: Option<String>,
) -> Result<serde_json::Value, String> {
    if !Path::new(&hash_file).exists() {
        return Err(format!("No existe el hash: {}", hash_file));
    }
    let attack_id = "crack_plan".to_string();
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    {
        let mut g = PLAN_CANCEL.lock().unwrap();
        *g = Some(cancel.clone());
    }
    let _ = app.emit("attack-started", serde_json::json!({ "id": attack_id, "bg": true, "cmdline": "[plan] crack_plan_run_bg" }));
    let plan = build_plan(&hash_file, &ssid, &bssid, external_generator.as_deref());
    let stages_count = plan.stages.len();
    let attack_id_ret = attack_id.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut cracked = false;
        let mut won_stage: Option<String> = None;
        let mut lines: Vec<String> = Vec::new();
        if plan.cracked_already {
            cracked = true;
            lines.push(format!("Ya crackeado: {}", read_cracked(&hash_file).unwrap_or_default()));
        } else {
            for st in &plan.stages {
                if cancel.load(Ordering::Relaxed) {
                    lines.push("Plan cancelado.".into());
                    break;
                }
                if !st.enabled {
                    let reason = st.skip_reason.clone().unwrap_or_else(|| "omitida".into());
                    let msg = format!("[plan] SKIP {} — {}", st.label, reason);
                    emit_progress(&app2, &attack_id, &format!("{}\n", msg));
                    lines.push(msg);
                    continue;
                }
                match run_stage(&app2, &attack_id, st, &hash_file, &ssid, &bssid, external_generator.as_deref(), &cancel).await {
                    Ok((true, msg)) => {
                        let line = format!("[plan] OK {} — {}", st.label, msg);
                        emit_progress(&app2, &attack_id, &format!("{}\n", line));
                        lines.push(line);
                        cracked = true;
                        won_stage = Some(st.id.clone());
                        break;
                    }
                    Ok((false, msg)) => {
                        let line = format!("[plan] -- {} — {}", st.label, msg);
                        emit_progress(&app2, &attack_id, &format!("{}\n", line));
                        lines.push(line);
                    }
                    Err(e) if e == "cancelado" => {
                        lines.push("Plan cancelado.".into());
                        break;
                    }
                    Err(e) => {
                        let line = format!("[plan] !! {} — {}", st.label, e);
                        emit_progress(&app2, &attack_id, &format!("{}\n", line));
                        lines.push(line);
                    }
                }
            }
        }
        let pw = read_cracked(&hash_file);
        let summary = if cracked {
            format!("CRACKED: {}\nEtapa: {}\n\n{}", pw.clone().unwrap_or_default(), won_stage.clone().unwrap_or_else(|| "previo".into()), lines.join("\n"))
        } else if cancel.load(Ordering::Relaxed) {
            format!("Plan cancelado.\n\n{}", lines.join("\n"))
        } else {
            format!("Sin resultado: todas las etapas agotadas u omitidas.\n\n{}", lines.join("\n"))
        };
        // Limpieza defensiva del child
        {
            let st = app2.state::<crate::AppState>();
            let _ = st.running_attacks.lock().unwrap().remove(&attack_id);
        }
        let _ = app2.emit("attack-completed", serde_json::json!({
            "id": attack_id,
            "success": cracked,
            "stdout": summary,
            "stderr": "",
            "exit_code": if cracked { 0 } else { 1 }
        }));
        let mut g = PLAN_CANCEL.lock().unwrap();
        *g = None;
    });
    Ok(serde_json::json!({ "attack_id": attack_id_ret, "stages": stages_count }))
}

#[command]
pub async fn crack_plan_cancel() -> CmdResponse {
    let g = PLAN_CANCEL.lock().unwrap();
    if let Some(c) = g.as_ref() {
        c.store(true, Ordering::Relaxed);
        CmdResponse { success: true, output: "Cancelación del plan solicitada.".into(), stderr: String::new(), exit_code: Some(-1) }
    } else {
        CmdResponse { success: false, output: "No hay plan en ejecución.".into(), stderr: String::new(), exit_code: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_keyspace_basic() {
        assert_eq!(mask_keyspace("?d?d?d?d"), Some(10_000));
        assert_eq!(mask_keyspace("?d?d?d?d?d?d?d?d"), Some(100_000_000));
        assert_eq!(mask_keyspace("A?d?d"), Some(100));
        assert!(mask_keyspace("?x").is_none());
        assert!(mask_keyspace("").is_none());
    }

    #[test]
    fn derive_masks_has_universal() {
        let m = derive_masks("MiRed");
        assert!(m.iter().any(|(x, _)| x == "?d?d?d?d?d?d?d?d"));
        assert!(m.iter().any(|(x, _)| x.starts_with("MiRed")));
    }

    #[test]
    fn derive_masks_rejects_unsafe_ssid() {
        let m = derive_masks("bad;ssid");
        assert!(!m.iter().any(|(x, _)| x.contains("bad")));
        assert!(!m.is_empty());
    }

    #[test]
    fn sanitize_hintword_basic() {
        assert_eq!(sanitize_hintword("MiRed_2.4"), Some("MiRed_2.4".into()));
        assert_eq!(sanitize_hintword("Mi Red"), None);
        assert_eq!(sanitize_hintword("a,b"), None);
        assert_eq!(sanitize_hintword(""), None);
    }

    #[test]
    fn supports_pcfg_versions() {
        assert!(!supports_pcfg(None));
        assert!(!supports_pcfg(Some(7)));
        assert!(supports_pcfg(Some(8)));
        assert!(supports_pcfg(Some(9)));
    }
}
