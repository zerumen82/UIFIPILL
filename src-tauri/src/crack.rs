// Estrategia de crack — inspector .22000 + assets locales + lanzador hashcat.
// Todo lo que hay aquí corre en Windows con el hashcat.exe nativo: el crack
// no necesita RF ni inyección, solo el hash (capturado donde sea, incluso en Linux).
use serde::Serialize;
use tauri::{command, AppHandle};
use tauri_plugin_shell::ShellExt;
use crate::commands::CmdResponse;
use crate::tools_detect::resolve_tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashKind {
    Pmkid,
    EapolChallenge,   // M1+M2: el AP aún no validó la clave
    EapolAuthorized,  // M2+M3 o superior: clave validada por ambos lados
}

#[derive(Debug, Clone, Serialize)]
pub struct HashSummary {
    pub total:      usize,
    pub pmkid:      usize,
    pub challenge:  usize,
    pub authorized: usize,
    pub essids:     Vec<String>,
}

/// Clasifica una línea .22000. Pura y testeable.
/// Formato: WPA*<01|02>*<hex>*<mac_ap>*<mac_sta>*<essid_hex>*...*<msgpair>
pub fn classify_line(line: &str) -> Option<(HashKind, Option<String>)> {
    let f: Vec<&str> = line.split('*').collect();
    if f.len() < 7 || f[0] != "WPA" {
        return None;
    }
    let essid = if f[5].is_empty() { None } else { hex_to_ascii(f[5]) };
    match f[1] {
        "01" => Some((HashKind::Pmkid, essid)),
        "02" => {
            let raw = f.last().unwrap_or(&"").trim();
            // El conversor escribe "00"; otros: "0".."5". Normalizar a un dígito.
            let norm = raw.trim_start_matches('0');
            let pair = if norm.is_empty() { "0" } else { norm };
            // Bits 2,1,0 del message pair: 000 = challenge (M1+M2), resto = authorized.
            let kind = match pair {
                "0" => HashKind::EapolChallenge,
                "1" | "2" | "3" | "4" | "5" => HashKind::EapolAuthorized,
                _ => return None,
            };
            Some((kind, essid))
        }
        _ => None,
    }
}

fn hex_to_ascii(h: &str) -> Option<String> {
    if h.len() % 2 != 0 || h.is_empty() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).ok())
        .collect();
    let bytes = bytes?;
    if bytes.iter().all(|b| (0x20..0x7f).contains(b)) {
        String::from_utf8(bytes).ok()
    } else {
        None
    }
}

/// Resume un .22000 ya convertido (no necesita hcxpcapngtool).
pub fn summarize_hashes(text: &str) -> HashSummary {
    let mut s = HashSummary { total: 0, pmkid: 0, challenge: 0, authorized: 0, essids: vec![] };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((kind, essid)) = classify_line(line) {
            s.total += 1;
            match kind {
                HashKind::Pmkid => s.pmkid += 1,
                HashKind::EapolChallenge => s.challenge += 1,
                HashKind::EapolAuthorized => s.authorized += 1,
            }
            if let Some(e) = essid {
                if !s.essids.contains(&e) {
                    s.essids.push(e);
                }
            }
        }
    }
    s
}

/// Comando: inspecciona un .22000 y dice qué contiene antes de quemar GPU.
#[command]
pub async fn inspect_hash(_app: AppHandle, hash_path: String) -> Result<HashSummary, String> {
    let text = std::fs::read_to_string(&hash_path)
        .map_err(|e| format!("No se pudo leer {}: {}", hash_path, e))?;
    let s = summarize_hashes(&text);
    if s.total == 0 {
        return Err("El archivo no contiene líneas WPA*01/WPA*02 válidas.".into());
    }
    Ok(s)
}

/// Comando: filtra un .22000 por clase y escribe el resultado.
#[command]
pub async fn filter_hash(
    _app: AppHandle,
    hash_path: String,
    kind: String,
    output_path: Option<String>,
) -> Result<CmdResponse, String> {
    let want = match kind.as_str() {
        "pmkid" => HashKind::Pmkid,
        "challenge" => HashKind::EapolChallenge,
        "authorized" => HashKind::EapolAuthorized,
        _ => return Err("kind debe ser pmkid|challenge|authorized".into()),
    };
    let text = std::fs::read_to_string(&hash_path)
        .map_err(|e| format!("No se pudo leer {}: {}", hash_path, e))?;
    let kept: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| classify_line(l).map(|(k, _)| k == want).unwrap_or(false))
        .collect();
    if kept.is_empty() {
        return Err(format!("Ninguna línea de clase '{}' en {}", kind, hash_path));
    }
    let out = output_path.unwrap_or_else(|| format!("{}.{}", hash_path, kind));
    std::fs::write(&out, kept.join("\n"))
        .map_err(|e| format!("No se pudo escribir {}: {}", out, e))?;
    Ok(CmdResponse {
        success: true,
        output: format!("Filtradas {} líneas '{}' → {}", kept.len(), kind, out),
        stderr: String::new(),
        exit_code: Some(0),
    })
}

/// Preflight del backend de crack (D-07): hashcat sin OpenCL agota etapas con
/// `./OpenCL/: No such file or directory`. Avisar al arrancar, no a mitad.
#[derive(Debug, Clone, Serialize)]
pub struct CrackBackend {
    pub ready: bool,
    pub hashcat: String,
    pub opencl: bool,
    pub message: String,
}

#[command]
pub async fn crack_backend_check() -> CrackBackend {
    tauri::async_runtime::spawn_blocking(move || {
        let (found, hc, _) = resolve_tool("hashcat.exe");
        if !found {
            return CrackBackend { ready: false, hashcat: String::new(), opencl: false,
                message: "hashcat.exe no encontrado en tools/. El crack no está disponible.".into() };
        }
        // hashcat busca `./OpenCL/` relativo al CWD: hay que lanzarlo desde tools/.
        let wd = std::path::Path::new(&hc).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let out = crate::process_util::hidden_command(&hc).arg("-I").current_dir(wd).output();
        let (text, ok) = match out {
            Ok(o) => (String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr), o.status.success()),
            Err(e) => (format!("spawn: {e}"), false),
        };
        // Ojo: `./OpenCL/: No such file or directory` CONTIENE "OpenCL" pero
        // significa exactamente lo contrario. Solo cuenta un Platform/Device real.
        let broken = text.contains("./OpenCL/: No such file") || text.contains("No devices found") || text.contains("No platforms");
        let listed = text.contains("Platform #") || text.contains("Device #");
        let opencl = listed && !broken;
        let ready = ok && opencl;
        let message = if ready {
            "Backend OpenCL disponible: el crack puede ejecutarse.".into()
        } else {
            format!("Sin backend OpenCL (hashcat v7.x sin runtime). Las etapas fallarán con './OpenCL/: No such file or directory'. Instala hashcat v8+ con OpenCL; hasta entonces el éxito del pipeline es un .22000 válido guardado. {text}")
        };
        CrackBackend { ready, hashcat: hc, opencl, message }
    }).await.unwrap_or_else(|_| CrackBackend {
        ready: false, hashcat: String::new(), opencl: false, message: "preflight error".into()
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct CrackAssets {
    pub hashcat:  String,
    pub rules:    Vec<String>,
    pub masks:    Vec<String>,
    pub wordlists: Vec<String>,
    pub rockyou:  bool,
}

/// Comando: lista reglas, máscaras y wordlists REALMENTE presentes (no promesas).
#[command]
pub async fn list_crack_assets() -> CrackAssets {
    let (found, hc, _) = resolve_tool("hashcat.exe");
    let hc_path = std::path::PathBuf::from(&hc);
    let base = hc_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();

    let mut rules = ls(&base.join("rules"), "rule");
    rules.sort();
    let mut masks = ls(&base.join("masks"), "hcmask");
    masks.sort();
    // Wordlists: carpeta tools de UIFIPILL + example.dict de hashcat.
    let mut wordlists: Vec<String> = vec![];
    for dir in [&base, &base.parent().unwrap_or(&base).to_path_buf()] {
        for name in ls(dir, "txt").into_iter().chain(ls(dir, "dict")) {
            let full = dir.join(&name).display().to_string();
            if !wordlists.contains(&full) {
                wordlists.push(full);
            }
        }
    }
    for extra in [
        dirs::data_local_dir().map(|p| p.join("UIFIPILL").join("tools")),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join("tools"))),
        // Workspace tools/ (modo dev: hashcat.exe, rockyou.txt, reglas)
        Some(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(|p| p.join("tools"))
                .unwrap_or_default(),
        ),
    ]
    .into_iter()
    .flatten()
    {
        for d in [extra.clone(), extra.join("wordlists")] {
            for name in ls(&d, "txt").into_iter().chain(ls(&d, "dict")) {
                let full = d.join(&name).display().to_string();
                if !wordlists.contains(&full) {
                    wordlists.push(full);
                }
            }
        }
    }
    wordlists.sort();
    let rockyou = wordlists.iter().any(|w| w.to_lowercase().contains("rockyou"));

    CrackAssets {
        hashcat: if found { hc } else { "no encontrado".into() },
        rules,
        masks,
        wordlists,
        rockyou,
    }
}

fn ls(dir: &std::path::Path, ext: &str) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x.eq_ignore_ascii_case(ext))
                        .unwrap_or(false)
                })
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Máscara hashcat válida: literales ASCII seguros + placeholders ?l?u?d?s?a?b?h?H.
pub fn valid_mask(mask: &str) -> bool {
    if mask.is_empty() || mask.len() > 64 {
        return false;
    }
    let mut it = mask.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '?' {
            match it.next() {
                Some(q) if "ludsbahH".contains(q) => {}
                _ => return false,
            }
        } else if ch.is_ascii_graphic() && !"&;|$`<>(){}\n\r".contains(ch) {
            // literal seguro (argv directo, sin shell — aun así se filtra)
        } else {
            return false;
        }
    }
    true
}

/// Comando: lanza hashcat -m 22000 con estrategia estructurada (sin shell, argv directo).
/// attack: 0=diccionario, 1=combinator, 3=máscara, 6=dicc+máscara, 7=máscara+dicc.
#[command]
pub async fn crack_custom(
    app: AppHandle,
    hash_file: String,
    attack: u8,
    wordlist: Option<String>,
    mask: Option<String>,
    rule: Option<String>,
    session: Option<String>,
) -> CmdResponse {
    if !matches!(attack, 0 | 1 | 3 | 6 | 7) {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: "attack debe ser 0, 1, 3, 6 o 7".into(),
            exit_code: None,
        };
    }
    // M-16: hashcat se lanza con current_dir(tools/). Una ruta relativa como
    // "capture.22000" existe respecto al CWD de la app, pero hashcat la busca
    // en tools/ → "No hashes loaded", y el .cracked acabaría en tools/ en vez
    // de junto al hash. Resolver TODO a absoluto antes de pasar los argumentos.
    let to_abs = |p: &str| -> String {
        let path = std::path::Path::new(p);
        if path.is_absolute() { p.to_string() }
        else { std::env::current_dir().unwrap_or_default().join(path).to_string_lossy().into_owned() }
    };
    let hash_abs = to_abs(&hash_file);
    if !std::path::Path::new(&hash_abs).exists() {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: format!("No existe el hash: {}", hash_abs),
            exit_code: None,
        };
    }
    let needs_wordlist = matches!(attack, 0 | 1 | 6 | 7);
    let needs_mask = matches!(attack, 3 | 6 | 7);
    if needs_wordlist && wordlist.as_deref().map(str::is_empty).unwrap_or(true) {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: "Este modo necesita wordlist".into(),
            exit_code: None,
        };
    }
    if needs_mask {
        match mask.as_deref() {
            Some(m) if valid_mask(m) => {}
            _ => {
                return CmdResponse {
                    success: false,
                    output: String::new(),
                    stderr: "Máscara inválida o ausente (?l?u?d?s?a?b?h?H + literales)".into(),
                    exit_code: None,
                }
            }
        }
    }

    let shell = app.shell();
    let cracked_out = format!("{}.cracked", hash_abs);
    // Igual para la wordlist: si el usuario escribe un nombre relativo, debe
    // resolverse respecto al CWD de la app, no al de hashcat.
    let wordlist_abs = wordlist.as_deref().map(|w| to_abs(w));
    let mut args: Vec<String> = vec![
        "-m".into(), "22000".into(),
        "-a".into(), attack.to_string(),
        "-o".into(), cracked_out.clone(),
        "--force".into(), "--status".into(), "--status-timer=10".into(),
        // Sin esto, un hash ya presente en hashcat.potfile se omite y NO se
        // escribe .cracked: perderíamos la evidencia real exigida por el proyecto.
        "--potfile-disable".into(),
        "--session".into(), session.unwrap_or_else(|| "uifipill".into()),
    ];
    if attack == 0 {
        if let Some(r) = rule {
            // El desplegable de la UI devuelve SOLO el nombre ("dive.rule");
            // crack_plan_run_bg resuelve con find_rule, pero crack_custom
            // antes exigía una ruta ya existente y fallaba con "No existe
            // la regla: dive.rule". Aceptar ambos: ruta absoluta o nombre.
            let rule_path = if std::path::Path::new(&r).exists() {
                r.clone()
            } else {
                match crate::crack_plan::find_rule(&r) {
                    Some(p) => p,
                    None => return CmdResponse {
                        success: false, output: String::new(),
                        stderr: format!("No existe la regla '{}' en tools/rules/. Pulsa «Assets» para recargar.", r),
                        exit_code: None,
                    },
                }
            };
            args.push("-r".into());
            args.push(rule_path);
        }
    }
    args.push(hash_abs.clone());
    let wl = wordlist_abs.unwrap_or_default();
    match attack {
        0 => args.push(wl),
        1 => {
            args.push(wl.clone());
            args.push(wl);
        }
        3 => args.push(mask.unwrap()),
        6 => {
            args.push(wl);
            args.push(mask.unwrap());
        }
        7 => {
            args.push(mask.unwrap());
            args.push(wl);
        }
        _ => unreachable!(),
    }

    // hashcat resuelve ./OpenCL/ y los kernels respecto a su CWD: lanzarlo
    // desde tools/ evita el error "./OpenCL/: No such file or directory".
    let hc_prog = resolve_tool("hashcat.exe").1;
    let wd = std::path::Path::new(&hc_prog).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let out = match shell
        .command(&hc_prog)
        .current_dir(wd)
        .args(args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            return CmdResponse {
                success: false, output: String::new(),
                stderr: e.to_string(), exit_code: None,
            }
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    // Éxito REAL = fichero .cracked escrito por hashcat. Buscar la contraseña
    // en stdout daba falsos positivos (p. ej. "* Device #1: CUDA...").
    let cracked = std::fs::read_to_string(&cracked_out).ok().and_then(|s| {
        s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).last()
    });
    let pw = cracked.as_ref().and_then(|l| l.rsplit(':').next()).filter(|s| !s.is_empty());
    let found = pw.is_some();
    let body = match pw {
        Some(p) => format!("PASSWORD: {}\n\n{}", p, stdout),
        None => format!("Sin resultado aún.\nHash: {}  Ataque: -a{}\n\n{}", hash_file, attack, stdout),
    };
    CmdResponse {
        success: found,
        output: body,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        exit_code: out.status.code(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PMKID: &str = "WPA*01*abc123*112233445566*aabbccddeeff*4d694e6574776f726b***";
    const CHALL: &str = "WPA*02*mic001*112233445566*aabbccddeeff*4d694e6574776f726b*anonce*eframe*0";
    const AUTHZ: &str = "WPA*02*mic002*112233445566*aabbccddeeff*4d694e6574776f726b*anonce*eframe*2";

    #[test]
    fn classifies_all_kinds() {
        assert_eq!(classify_line(PMKID).unwrap().0, HashKind::Pmkid);
        assert_eq!(classify_line(CHALL).unwrap().0, HashKind::EapolChallenge);
        assert_eq!(classify_line(AUTHZ).unwrap().0, HashKind::EapolAuthorized);
        assert_eq!(classify_line(PMKID).unwrap().1.as_deref(), Some("MiNetwork"));
        assert!(classify_line("basura").is_none());
        assert!(classify_line("WPA*03*x").is_none());
    }

    #[test]
    fn summarizes_mixed_file() {
        let text = format!("{}\n{}\n{}\n# comentario\n\nlínea rota\n", PMKID, CHALL, AUTHZ);
        let s = summarize_hashes(&text);
        assert_eq!((s.total, s.pmkid, s.challenge, s.authorized), (3, 1, 1, 1));
        assert_eq!(s.essids, vec!["MiNetwork".to_string()]);
    }

    #[test]
    fn mask_validation() {
        assert!(valid_mask("?d?d?d?d?d?d?d?d"));
        assert!(valid_mask("?u?l?l?l?l?l?l?d?d"));
        assert!(valid_mask("Casa?d?d?d?d"));
        assert!(!valid_mask(""));
        assert!(!valid_mask("?x?d"));
        assert!(!valid_mask("a?"));
        assert!(!valid_mask("pass;rm"));
        assert!(!valid_mask("a$(b)"));
    }
}
