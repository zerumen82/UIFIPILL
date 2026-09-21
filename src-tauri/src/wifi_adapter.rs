// wifi_adapter.rs — detección de adaptador WiFi + chipset (AWUS036H y compatibles)
use serde::Serialize;
use tauri::{command, AppHandle};

// chipset: (nombre_legible, vid_pid, monitor_capable, hint_instalacion, ruta_base)
const CHIPS: &[(&str, &str, bool, &str, &str)] = &[
    ("Atheros AR9271",        "0CF3:9271", true, "Driver NPcap modificado",       "https://github.com/aircrack-ng/aircrack-ng"),
    ("Realtek RTL8812AU",     "2357:0109", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8812au"),
    // AWUS036ACH y dongles genéricos: VID Realtek (el 2357 es TP-Link)
    ("Realtek RTL8812AU",     "0BDA:8812", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8812au"),
    ("Realtek RTL8812AU",     "0BDA:881A", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8812au"),
    ("Realtek RTL8811AU",     "0BDA:0811", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8812au"),
    ("Realtek RTL8814AU",     "2357:0107", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8814au"),
    ("Realtek RTL8188CUS",    "0BDA:818C", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8188cus"),
    ("Realtek RTL8188L",      "0BDA:818B", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8188"),
    ("Realtek RTL8187",       "0BDA:8187", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8187"),
    ("Realtek RTL8192CU",     "0BDA:8176", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8192cu"),
    ("Realtek RTL8811CU",     "0BDA:C811", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8811cu"),
    ("Realtek RTL8812BU",     "0BDA:B812", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl8812bu"),
    ("Ralink RT2870/RT3070",  "148F:5370", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl93xx"),
    ("Ralink RT3070",         "148F:3070", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl93xx"),
    ("Ralink RT3071/RT3072",  "148F:3071", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl93xx"),
    ("Ralink RT5572",         "148F:5572", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/rtl93xx"),
    ("MediaTek MT7612U",      "0E8D:7612", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/mt7612u"),
    ("MediaTek MT7921",       "0E8D:7921", true, "Driver NPcap modificado",        "https://github.com/aircrack-ng/mt76"),
    ("Intel AX200",           "8086:2723", false,"No soporta modo monitor en Windows nativo. Driver Intel limitado.",               ""),
    ("Intel AX210",           "8086:7922", false,"No soporta modo monitor en Windows nativo. Driver Intel limitado.",               ""),
    ("Intel AX201",           "8086:0026", false,"No soporta modo monitor en Windows nativo. Driver Intel limitado.",               ""),
];

#[derive(Debug, Clone, Serialize)]
pub struct AdapterInfo {
    pub name:            String,
    pub status:          String,
    pub mac:             String,
    /// GUID de la interfaz ({...}) para activate_monitor / set_monitor_channel.
    pub guid:            String,
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

/// Parsea una línea CSV de PowerShell (campos entre comillas, "" = comilla literal).
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = vec![];
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => {
                fields.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    fields.push(cur.trim().to_string());
    fields
}

/// Columnas: Name, InterfaceDescription, Status, MacAddress, InterfaceGuid, PnPDeviceID
fn parse_csv(raw: &str) -> Vec<AdapterInfo> {
    let mut v = vec![];
    let mut lines = raw.lines();
    let header = match lines.next() {
        Some(h) => h,
        None => return v,
    };
    // Localiza índices por nombre (tolerante al orden/idioma parcial).
    let cols: Vec<String> = csv_fields(header).iter().map(|c| c.to_lowercase()).collect();
    let col = |names: &[&str]| {
        cols.iter()
            .position(|c| names.iter().any(|n| c.contains(n)))
            .unwrap_or(usize::MAX)
    };
    let (i_name, i_desc, i_status, i_mac, i_guid, i_pnp) = (
        col(&["name"]),
        col(&["interfacedescription"]),
        col(&["status"]),
        col(&["macaddress"]),
        col(&["interfaceguid"]),
        col(&["pnpdeviceid", "pnp"]),
    );
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f = csv_fields(line);
        let get = |i: usize| f.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        let name = get(i_name);
        let desc = get(i_desc);
        if name.is_empty() && desc.is_empty() {
            continue;
        }
        let match_text = format!("{} {} {}", name, desc, get(i_pnp));

        let (cname, compat, hint, _route_base) = chipset_name(&match_text)
            .or_else(|| chipset_hwid(&match_text))
            .unwrap_or(("Desconocido", false,
                        "Chip WiFi desconocido. Se requiere AWUS036H (RTL8812AU) u otro con driver NPcap modificado.",
                        "scan-only"));

        let (vp, vp_clean) = vid_pid_from_hw(&match_text)
            .map(|vp| (vp.clone(), vp.clone()))
            .unwrap_or_else(|| ("—".into(), "—".into()));

        let full_chip = if cname != "Desconocido" { cname.to_string() }
            else if vp_clean != "—" { format!("Desconocido ({})", vp_clean) }
            else { "Desconocido".into() };

        let route = if compat { "windows".into() } else { "scan-only".into() };
        let mac = get(i_mac).replace('-', ":").to_uppercase();
        let status = get(i_status);
        let guid = get(i_guid);

        v.push(AdapterInfo {
            name: if name.is_empty() { desc.clone() } else { format!("{} ({})", name, desc) },
            status: if status.is_empty() { "desconocido".into() } else { status },
            mac: if mac.is_empty() { "—".into() } else { mac },
            guid,
            chipset: full_chip,
            chipset_key: vp,
            monitor_capable: compat && vp_clean != "—",
            monitor_active: false, // se resuelve abajo con monitor_status real
            driver_hint: hint.into(),
            route_supported: route,
        });
    }
    v
}

async fn get_adapters() -> Vec<AdapterInfo> {
    // Una sola consulta con todo lo necesario (CSV = robusto ante espacios en nombres).
    let ps = r#"
Get-NetAdapter 2>$null | Where-Object { $_.MediaType -eq 'Native 802.11' } |
Select-Object Name, InterfaceDescription, Status, MacAddress, InterfaceGuid, PnPDeviceID |
ConvertTo-Csv -NoTypeInformation
"#;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", ps])
        .output();
    let mut adapters = match out {
        Ok(ref o) if o.status.success() => parse_csv(&String::from_utf8_lossy(&o.stdout)),
        _ => vec![],
    };
    // Estado real de modo monitor por adaptador (GET OID, sin cambios).
    for a in &mut adapters {
        if a.guid.is_empty() {
            continue;
        }
        let st = crate::monitor_mode::monitor_status(a.guid.clone()).await;
        a.monitor_active = st.success && st.mode == "monitor";
    }
    adapters
}

// ── COMANDOS ────────────────────────────────────────────────────────────────

/// Escanea adaptadores WiFi USB
#[command]
pub async fn detect_adapters() -> AdapterReport {
    let adapters = get_adapters().await;
    let monitor_ready = adapters.iter().any(|a| a.monitor_capable);

    let recommended = if monitor_ready {
        "windows-driver".into()
    } else {
        "scan-only".into()
    };

    AdapterReport {
        adapters, monitor_ready, recommended_route: recommended,
    }
}

/// Activa modo monitor en el adaptador vía NPcap OID (delega en monitor_mode::activate_monitor)
#[command]
pub async fn set_monitor_mode(_app: AppHandle, iface: String) -> MonitorModeResult {
    let result = crate::monitor_mode::activate_monitor(iface.clone(), None).await;
    MonitorModeResult {
        success: result.success,
        interface: result.interface_name,
        current_mode: result.mode,
        message: result.message,
    }
}

/// Restaura modo managed vía NPcap OID (delega en monitor_mode::restore_managed)
#[command]
pub async fn set_managed_mode(_app: AppHandle, iface: String) -> MonitorModeResult {
    let result = crate::monitor_mode::restore_managed(iface.clone()).await;
    MonitorModeResult {
        success: result.success,
        interface: result.interface_name,
        current_mode: result.mode,
        message: result.message,
    }
}


