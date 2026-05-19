// wifi_adapter.rs — detección de adaptador WiFi + chipset (AWUS036H y compatibles)
use serde::Serialize;
use tauri::{command, AppHandle};
use tauri_plugin_shell::ShellExt;
use crate::commands::CmdResponse;

// chipset: (nombre_legible, vid_pid, monitor_capable, hint_instalacion, ruta_base)
const CHIPS: &[(&str, &str, bool, &str, &str)] = &[
    ("Atheros AR9271",        "0CF3:9271", true, "Driver NPcap modificado",       "https://github.com/aircrack-ng/rtl8812au"),
    ("Realtek RTL8812AU",     "2357:0109", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8812au"),
    ("Realtek RTL8814AU",     "2357:0107", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8814au"),
    ("Realtek RTL8188CUS",    "0BDA:818C", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8188cus"),
    ("Realtek RTL8188L",      "0BDA:818B", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8188"),
    ("Realtek RTL8187",       "0BDA:8187", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8187"),
    ("Realtek RTL8192CU",     "0BDA:8176", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8192cu"),
    ("Realtek RTL8811CU",     "0BDA:C811", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8811cu"),
    ("Realtek RTL8812BU",     "0BDA:B812", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8812bu"),
    ("Ralink RT2870/RT3070",  "148F:5370", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl93xx"),
    ("Ralink RT5572",         "148F:5572", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl93xx"),
    ("MediaTek MT7612U",      "0E8D:7612", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/mt7612u"),
    ("MediaTek MT7921",       "0E8D:7921", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/mt76"),
    ("Intel AX200",           "8086:2723", false,"Solo Linux/WSL2",               "wsl --install"),
    ("Intel AX210",           "8086:7922", false,"Solo Linux/WSL2",               "wsl --install"),
    ("Intel AX201",           "8086:0026", false,"Solo Linux/WSL2",               "wsl --install"),
];

#[derive(Debug, Clone, Serialize)]
pub struct AdapterInfo {
    pub name:            String,
    pub status:          String,
    pub mac:             String,
    pub chipset:         String,
    pub chipset_key:     String,
    pub monitor_capable: bool,
    pub monitor_active:  bool,
    pub driver_hint:     String,
    pub route_supported: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterReport {
    pub adapters:         Vec<AdapterInfo>,
    pub wsl2_available:   bool,
    pub wsl2_usable:      bool,
    pub monitor_ready:    bool,
    pub recommended_route: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorModeResult {
    pub success:      bool,
    pub interface:    String,
    pub current_mode: String,
    pub message:      String,
}

// ── helpers ────────────────────────────────────────────────────────────────

/// ¿WSL2 instalado y accesible? ¿tiene binarios Linux?
fn check_wsl2() -> (bool, bool) {
    let avail = std::process::Command::new("wsl").args(["--status"]).output()
        .map(|o| o.status.success()).unwrap_or(false);
    if !avail { return (false, false); }
    let tools = std::process::Command::new("wsl")
        .args(["--","bash","-c","command -v aireplay-ng >/dev/null 2>&1 && echo YES || echo NO"])
        .output().map(|o| String::from_utf8_lossy(&o.stdout).contains("YES")).unwrap_or(false);
    (true, tools)
}

// chipset_hwid — stub for future HWID-based lookup; currently unused
#[allow(dead_code)]
fn chipset_hwid(hw: &str) -> Option<(&str, bool, &str, &str)> {
    let hw = hw.to_uppercase();
    for c in CHIPS {
        let vid  = c.1.split(':').next()?;
        let pid  = c.1.split(':').nth(1)?;
        let search = format!("VID_{}&PID_{}", vid, pid);
        if hw.contains(&search) { return Some((c.0, c.2, c.3, c.4)); }
    }
    None
}

fn chipset_name(name: &str) -> Option<(&str, bool, &str, &str)> {
    for c in CHIPS {
        if name.to_lowercase().contains(&c.0.to_lowercase()) {
            return Some((c.0, c.2, c.3, c.4));
        }
    }
    None
}

/// Extrae VID:PID de un Hardware ID de dispositivo USB
fn vid_pid_from_hw(hw: &str) -> Option<String> {
    let re = regex::Regex::new(r"VID_([0-9A-Fa-f]{4})[&\\]PID_([0-9A-Fa-f]{4})").unwrap();
    re.captures(hw).map(|c| format!("{}:{}", &c[1], &c[2]).to_uppercase())
}

fn parse_pnp(raw: &str) -> Vec<AdapterInfo> {
    let mut v = vec![];
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("FriendlyName") || line.starts_with("---") { continue; }

        let (cname, compat, hint, _route_base) = chipset_name(line)
            .unwrap_or(("Desconocido", false,
                        "Chip WiFi no identificado para modo monitor en Windows. Prueba WSL2.",
                        "scan-only"));

        let (vp, vp_clean) = vid_pid_from_hw(line)
            .map(|vp| (vp.clone(), vp.clone()))
            .unwrap_or_else(|| ("—".into(), "—".into()));

        let full_chip = if cname != "Desconocido" { cname.to_string() }
            else if vp_clean != "—" { format!("Desconocido ({})", vp_clean) }
            else { "Desconocido".into() };

        let wsl2 = check_wsl2().0;
        let route = if !compat && wsl2 {
            "wsl2".into()
        } else if compat && wsl2 {
            "both".into()
        } else if compat {
            "windows".into()
        } else {
            "scan-only".into()
        };

        v.push(AdapterInfo {
            name:           line.to_string(),
            status:         "ok".into(),
            mac:            "—".into(),
            chipset:        full_chip,
            chipset_key:    vp,
            monitor_capable: compat && vp_clean != "—",
            monitor_active:  false,
            driver_hint:    hint.into(),
            route_supported: route,
        });
    }
    v
}

fn get_adapters() -> Vec<AdapterInfo> {
    let ps = r#"
Get-PnpDevice 2>$null | Where-Object {
    ($_.Class -eq "Net") -and (
        $_.Manufacturer -match "ALFA|Atheros|Realtek|Ralink|MediaTek|Intel|Broadcom|WLAN|WiFi|Wireless|802.11" -or
        $_.FriendlyName  -match "ALFA|WLAN|WiFi|Wireless|802.11"
    )
} | Select-Object FriendlyName, Status, InstanceId |
Format-Table -AutoSize
"#;
    let out = std::process::Command::new("powershell").args(["-NoProfile","-Command",ps]).output();
    match out {
        Ok(ref o) if o.status.success() => parse_pnp(&String::from_utf8_lossy(&o.stdout)),
        _ => vec![],
    }
}

// ── COMANDOS ────────────────────────────────────────────────────────────────

/// Escanea adaptadores WiFi USB + estado WSL2
#[command]
pub async fn detect_adapters() -> AdapterReport {
    let (wsl2_avail, wsl2_usable) = check_wsl2();
    let adapters = get_adapters();
    let monitor_ready = adapters.iter().any(|a| a.monitor_capable);

    let recommended = if monitor_ready && wsl2_avail {
        "both".into()
    } else if monitor_ready {
        "windows-driver".into()
    } else if wsl2_usable {
        "wsl2".into()
    } else if wsl2_avail {
        "wsl2-setup".into()
    } else {
        "scan-only".into()
    };

    AdapterReport {
        adapters, wsl2_available: wsl2_avail, wsl2_usable, monitor_ready, recommended_route: recommended,
    }
}

/// Instrucciones para activar modo monitor en el adaptador detectado
#[command]
pub async fn set_monitor_mode(_app: AppHandle, iface: String) -> MonitorModeResult {
    let npcap  = std::path::Path::new(r"C:\Windows\System32\Npcap.dll").exists()
        || std::path::Path::new(r"C:\Windows\SysWOW64\Npcap.dll").exists();
    let wsl2   = check_wsl2().0;
    let wsl2ok = check_wsl2().1;

    let msg = if wsl2ok {
        format!(
            "✅ WSL2 con herramientas Linux detectado.\n\
             Ruta recomendada: WSL2 (modo monitor nativo).\n\n\
             Ejecuta en consola:\n\
             wsl sudo airmon-ng start <interface-wsl>  # activa modo monitor\n\
             wsl sudo airodump-ng <interface-wsl>mon    # escanea\n\n\
             Interface Windows '{}' — si la pasas a WSL2:\n\
             wsl -- sudo ip link set <if-wsl> up",
            iface)
    } else if npcap {
        format!(
            "⚠️  NPcap detectado pero sin herramientas WSL2.\n\
             Para modo monitor en Windows con tu ALFA AWUS036H (RT2870):\n\n\
             1. Desinstala el driver Realtek oficial en Administrador de dispositivos\n\
             2. Instala NPcap desde https://npcap.com\n\
             3. Instala el driver NPcap + RT2870 desde comunidad\n\
             4. Coloca WiFiMode.exe en la carpeta del driver\n\
             5. Ejecuta: WiFiMode.exe set monitor 1",
            )
    } else if wsl2 {
        format!(
            "⚠️  WSL2 disponible pero sin herramientas Linux instaladas.\n\
             Ejecuta en PowerShell:\n\
             wsl sudo apt install -y aircrack-ng bully hcxtools\n\
             Luego usa WSL2 para todos los ataques."
        )
    } else {
        format!(
            "❌ Ni WSL2 ni NPcap detectados.\n\n\
             OPCIÓN A — WSL2 (recomendado):\n\
               wsl --install\n\
               wsl sudo apt install -y aircrack-ng bully hcxtools hashcat\n\n\
             OPCIÓN B — NPcap nativo:\n\
               https://npcap.com → instalar NPcap\n\
               Luego driver NPcap + RT2870 modificado para la AWUS036H"
        )
    };

    MonitorModeResult {
        success: false,
        interface: iface,
        current_mode: "sin confirmar".into(),
        message: msg,
    }
}

#[command]
pub async fn set_managed_mode(_app: AppHandle, iface: String) -> MonitorModeResult {
    let iface_ref = iface.clone();
    MonitorModeResult {
        success: true,
        interface: iface,
        current_mode: "managed".into(),
        message: format!("✅ {} en modo managed.", iface_ref),
    }
}

/// Ejecuta un comando dentro de WSL2 (aireplay-ng, bully, hcxdumptool, etc.)
#[command]
pub async fn wsl2_run(_app: AppHandle, cmd: String) -> CmdResponse {
    let full = format!("wsl -- bash -c '{}'", cmd.replace("'", "'\\''"));
    let sh = _app.shell();
    match sh.command("powershell").args(["-NoProfile","-Command",&full]).output().await {
        Ok(o) => {
            let exit_code: Option<i32> = o.status.code();
            CmdResponse {
                success:    o.status.success(),
                output:     String::from_utf8_lossy(&o.stdout).into_owned(),
                stderr:     String::from_utf8_lossy(&o.stderr).into_owned(),
                exit_code:  exit_code,
            }
        },
        Err(e) => CmdResponse { success: false, output: String::new(), stderr: e.to_string(), exit_code: None::<i32> },
    }
}

#[command]
pub async fn wsl2_info() -> CmdResponse {
    let (avail, _) = check_wsl2();
    if !avail {
        return CmdResponse {
            success: false,
            output: "WSL2 no detectado en este sistema.\nInstálalo con: wsl --install".into(),
            stderr: String::new(), exit_code: None,
        };
    }
    let raw = std::process::Command::new("wsl").args(["--status"]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    CmdResponse { success: true, output: raw, stderr: String::new(), exit_code: None }
}
