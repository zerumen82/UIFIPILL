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
            let pair = f.last().unwrap_or(&"").trim();
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
    ]
    .into_iter()
    .flatten()
    {
        for name in ls(&extra, "txt").into_iter().chain(ls(&extra, "dict")) {
            let full = extra.join(&name).display().to_string();
            if !wordlists.contains(&full) {
                wordlists.push(full);
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
    if !std::path::Path::new(&hash_file).exists() {
        return CmdResponse {
            success: false,
            output: String::new(),
            stderr: format!("No existe el hash: {}", hash_file),
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
    let cracked_out = format!("{}.cracked", hash_file);
    let mut args: Vec<String> = vec![
        "-m".into(), "22000".into(),
        "-a".into(), attack.to_string(),
        "-o".into(), cracked_out.clone(),
        "--force".into(), "--status".into(), "--status-timer=10".into(),
        "--session".into(), session.unwrap_or_else(|| "uifipill".into()),
    ];
    if attack == 0 {
        if let Some(r) = rule {
            if !std::path::Path::new(&r).exists() {
                return CmdResponse {
                    success: false, output: String::new(),
                    stderr: format!("No existe la regla: {}", r), exit_code: None,
                };
            }
            args.push("-r".into());
            args.push(r);
        }
    }
    args.push(hash_file.clone());
    match attack {
        0 => args.push(wordlist.unwrap()),
        1 => {
            args.push(wordlist.clone().unwrap());
            args.push(wordlist.unwrap());
        }
        3 => args.push(mask.unwrap()),
        6 => {
            args.push(wordlist.unwrap());
            args.push(mask.unwrap());
        }
        7 => {
            args.push(mask.unwrap());
            args.push(wordlist.unwrap());
        }
        _ => unreachable!(),
    }

    let out = match shell
        .command("hashcat")
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
    let pw = stdout
        .lines()
        .find(|l| l.contains(':') && !l.starts_with('#'))
        .map(|s| s.to_string());
    let found = pw.is_some();
    let body = match pw {
        Some(p) => format!("PASSWORD: {}\n\n{}", p, stdout),
        None => format!("Sin resultado aún.\nHash: {}  Ataque: -a{}\n\n{}", hash_file, attack, stdout),
    };
    CmdResponse {
        success: found || out.status.success(),
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
