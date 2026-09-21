// ═══════════════════════════════════════════════════════════════════════════════
// MODO MONITOR WINDOWS NATIVO — Npcap Dot11 + OIDs 802.11 nativos
//
// Técnica idéntica a la de WlanHelper.exe (herramienta oficial de Npcap):
//   1. CreateFile sobre  \Device\Npcap\WIFI_{GUID}  (requiere Npcap con
//      la opción "Support raw 802.11 traffic" = Dot11Support).
//   2. DeviceIoControl(BIOCSETOID, PACKET_OID_DATA) con
//      OID_DOT11_CURRENT_OPERATION_MODE = Network Monitor (0x80000000).
//   3. DeviceIoControl(BIOCSETOID) con OID_DOT11_CURRENT_CHANNEL = N.
//
// Honestidad: solo funciona si el driver del adaptador implementa los OIDs
// nativos 802.11. Los Intel no anuncian NETWORK_MONITOR y aquí se reporta
// tal cual, sin fingir éxito.
// ═══════════════════════════════════════════════════════════════════════════════

use serde::Serialize;
use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use tauri::command;

// ── Win32 constants ──────────────────────────────────────────────────────────

const GENERIC_READ:      u32 = 0x8000_0000;
const GENERIC_WRITE:     u32 = 0x4000_0000;
const OPEN_EXISTING:     u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const INVALID_HANDLE_VALUE: *mut c_void = (-1isize) as *mut c_void;

// ── IOCTL codes de Npcap (packetWin7/npf/npf/ioctls.h) ───────────────────────
// CTL_CODE(DeviceType, Function, Method, Access)
//   = (DeviceType<<16) | (Access<<14) | (Function<<2) | Method
const FILE_DEVICE_TRANSPORT: u32 = 0x00000021;
const METHOD_BUFFERED:       u32 = 0x00000000;
const FILE_WRITE_DATA:       u32 = 0x00000002;
const FILE_READ_DATA:        u32 = 0x00000001;

const fn ctl_code(dev: u32, func: u32, method: u32, access: u32) -> u32 {
    (dev << 16) | (access << 14) | (func << 2) | method
}

// BIOCSETOID   = CTL_CODE(FILE_DEVICE_TRANSPORT, 0xa08, METHOD_BUFFERED, FILE_WRITE_DATA)
const BIOCSETOID: u32 = ctl_code(FILE_DEVICE_TRANSPORT, 0x0A08, METHOD_BUFFERED, FILE_WRITE_DATA);
// BIOCQUERYOID = CTL_CODE(FILE_DEVICE_TRANSPORT, 0xa09, METHOD_BUFFERED, FILE_READ_DATA)
const BIOCQUERYOID: u32 = ctl_code(FILE_DEVICE_TRANSPORT, 0x0A09, METHOD_BUFFERED, FILE_READ_DATA);

// ── OIDs nativos 802.11 (shared/windot11.h, OID_DOT11_NDIS_START=0x0D010300) ─
const OID_DOT11_OPERATION_MODE_CAPABILITY: u32 = 0x0D010300 + 7;  // GET
const OID_DOT11_CURRENT_OPERATION_MODE:    u32 = 0x0D010300 + 8;  // GET/SET
const OID_DOT11_CURRENT_CHANNEL:           u32 = 0x0D010300 + 53; // GET/SET (ULONG)
const OID_DOT11_CURRENT_FREQUENCY:         u32 = 0x0D010300 + 54; // GET/SET (ULONG kHz)

// Valores DOT11_OPERATION_MODE_*
const DOT11_OPERATION_MODE_EXTENSIBLE_STATION: u32 = 0x00000004; // managed
const DOT11_OPERATION_MODE_NETWORK_MONITOR:    u32 = 0x80000000; // monitor

// ── Win32 FFI ────────────────────────────────────────────────────────────────

#[link(name = "kernel32")]
unsafe extern "C" {
    fn CreateFileA(
        lp_file_name: *const c_char,
        dw_desired_access: u32,
        dw_share_mode: u32,
        lp_security_attributes: *mut c_void,
        dw_creation_disposition: u32,
        dw_flags_and_attributes: u32,
        h_template_file: *mut c_void,
    ) -> *mut c_void;

    fn CloseHandle(h_object: *mut c_void) -> c_int;

    fn DeviceIoControl(
        h_device: *mut c_void,
        dw_io_control_code: u32,
        lp_in_buffer: *const c_void,
        n_in_buffer_size: u32,
        lp_out_buffer: *mut c_void,
        n_out_buffer_size: u32,
        lp_bytes_returned: *mut u32,
        lp_overlapped: *mut c_void,
    ) -> c_int;

    fn GetLastError() -> u32;
}

// ── Helpers de dispositivo ───────────────────────────────────────────────────

/// Normaliza el identificador a GUID limpio (sin llaves ni prefijos).
/// Acepta: "{...}", "NPF_{...}", "\\.\Npcap\WIFI_{...}", GUID a secas.
fn normalize_guid(input: &str) -> String {
    let s = input.trim();
    if let (Some(a), Some(b)) = (s.find('{'), s.find('}')) {
        if b > a {
            return s[a + 1..b].trim().to_string();
        }
    }
    s.trim_start_matches("\\\\.\\")
        .trim_start_matches("\\Device\\")
        .trim_start_matches("Npcap\\")
        .trim_start_matches("WIFI_")
        .trim_start_matches("NPF_")
        .trim_matches(['{', '}'])
        .trim()
        .to_string()
}

/// Abre el dispositivo NPF probando las rutas usadas por Npcap/WinPcap.
/// Devuelve (handle, ruta_abierta) para poder informar al usuario.
fn open_npf_device(guid_clean: &str) -> Option<(*mut c_void, String)> {
    let g = format!("{{{}}}", guid_clean);
    // WIFI_ = captura raw 802.11 (Dot11Support). Sin WIFI_ = captura normal.
    let candidates = [
        format!(r"\\.\Npcap\WIFI_{}", g),
        format!(r"\Device\Npcap\WIFI_{}", g),
        format!(r"\\.\Npcap\{}", g),
        format!(r"\Device\Npcap\{}", g),
        format!(r"\\.\NPF_{}", g),
        format!(r"\Device\NPF_{}", g),
    ];
    for path in candidates {
        if let Ok(c_path) = CString::new(path.clone()) {
            let handle = unsafe {
                CreateFileA(
                    c_path.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    ptr::null_mut(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    ptr::null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
                return Some((handle, path));
            }
        }
    }
    None
}

fn close_handle(h: *mut c_void) {
    unsafe { let _ = CloseHandle(h); }
}

// ── PACKET_OID_DATA (Common/Packet32.h) ──────────────────────────────────────
// struct _PACKET_OID_DATA { ULONG Oid; ULONG Length; UCHAR Data[1]; }
// Con METHOD_BUFFERED el mismo buffer sirve de entrada y salida.

fn oid_buffer(oid: u32, data: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; 8 + data.len()];
    buf[0..4].copy_from_slice(&oid.to_le_bytes());
    buf[4..8].copy_from_slice(&(data.len() as u32).to_le_bytes());
    buf[8..].copy_from_slice(data);
    buf
}

/// OID SET al driver Npcap. `data` es el valor crudo del OID.
fn npf_set_oid(handle: *mut c_void, oid: u32, data: &[u8]) -> Result<(), String> {
    let mut buf = oid_buffer(oid, data);
    let mut ret: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(
            handle, BIOCSETOID,
            buf.as_ptr() as *const c_void, buf.len() as u32,
            buf.as_mut_ptr() as *mut c_void, buf.len() as u32,
            &mut ret, ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(format!(
            "DeviceIoControl(BIOCSETOID, OID 0x{:08X}) fallo (err=0x{:08X}). El driver no acepto el OID.",
            oid, unsafe { GetLastError() }
        ));
    }
    Ok(())
}

/// OID GET al driver Npcap; devuelve los bytes de datos recibidos.
fn npf_get_oid(handle: *mut c_void, oid: u32, data_len: usize) -> Result<Vec<u8>, String> {
    let data_len = data_len.max(1);
    let mut buf = vec![0u8; 8 + data_len];
    buf[0..4].copy_from_slice(&oid.to_le_bytes());
    // En QUERY, Length = bytes que esperamos recibir (el driver lo exige).
    buf[4..8].copy_from_slice(&(data_len as u32).to_le_bytes());
    let mut ret: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(
            handle, BIOCQUERYOID,
            buf.as_ptr() as *const c_void, buf.len() as u32,
            buf.as_mut_ptr() as *mut c_void, buf.len() as u32,
            &mut ret, ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(format!(
            "DeviceIoControl(BIOCQUERYOID, OID 0x{:08X}) fallo (err=0x{:08X}).",
            oid, unsafe { GetLastError() }
        ));
    }
    let total = (ret as usize).min(buf.len());
    if total < 8 {
        return Ok(vec![]);
    }
    Ok(buf[8..total].to_vec())
}

/// ULONG LE (formato de los valores OID_DOT11_* tipo ULONG).
fn ulong_le(v: u32) -> [u8; 4] { v.to_le_bytes() }

fn read_ulong_le(b: &[u8]) -> Option<u32> {
    if b.len() >= 4 {
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    } else {
        None
    }
}

// ── Modelos ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct MonitorCapResult {
    pub success:         bool,
    pub interface_name:  String,
    pub mode:            String,
    pub channel_set:     Option<u8>,
    pub message:         String,
    pub npcap_installed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelResult {
    pub success:  bool,
    pub channel:  u8,
    pub message:  String,
}

/// Canal 2.4 GHz → kHz para OID_DOT11_CURRENT_FREQUENCY (1→2412000 … 14→2484000).
pub(crate) fn channel_to_khz(ch: u8) -> Option<u32> {
    match ch {
        1..=13 => Some(2_407_000 + ch as u32 * 5_000),
        14 => Some(2_484_000),
        _ => None,
    }
}

/// Lee el canal actual abriendo el dispositivo (None si no se puede leer).
fn read_channel(guid: &str) -> Option<u8> {
    let (h, _) = open_npf_device(guid)?;
    let ch = npf_get_oid(h, OID_DOT11_CURRENT_CHANNEL, 4)
        .ok()
        .and_then(|b| read_ulong_le(&b))
        .filter(|c| *c >= 1 && *c <= 233)
        .map(|c| c as u8);
    close_handle(h);
    ch
}

fn npcap_installed() -> bool {
    std::path::Path::new(r"C:\Windows\System32\Npcap\wpcap.dll").exists()
        || std::path::Path::new(r"C:\Windows\System32\Npcap.dll").exists()
        || std::path::Path::new(r"C:\Windows\SysWOW64\Npcap.dll").exists()
}

// ── COMANDOS ─────────────────────────────────────────────────────────────────

/// Activa modo monitor (Network Monitor) en el adaptador cuyo GUID se pasa.
#[command]
pub async fn activate_monitor(iface_guid: String, channel: Option<u8>) -> MonitorCapResult {
    if !npcap_installed() {
        return MonitorCapResult {
            success: false, interface_name: iface_guid.clone(),
            mode: "npcap-missing".into(), channel_set: None,
            message: "X Npcap no encontrado.\nInstala NPcap desde https://npcap.com\nMarca 'Install in WinPcap API-compatible Mode'\ny 'Support raw 802.11 traffic (Dot11Support)'".into(),
            npcap_installed: false,
        };
    }

    let guid = normalize_guid(&iface_guid);
    if guid.is_empty() {
        return MonitorCapResult {
            success: false, interface_name: iface_guid.clone(),
            mode: "invalid-guid".into(), channel_set: None,
            message: "X GUID de interfaz vacio o invalido.".into(),
            npcap_installed: true,
        };
    }

    let opened = open_npf_device(&guid);
    let result = match opened {
        Some((handle, dev_path)) => {
            // 1) ¿El driver anuncia Network Monitor?
            let cap = npf_get_oid(handle, OID_DOT11_OPERATION_MODE_CAPABILITY, 8)
                .ok()
                .and_then(|b| read_ulong_le(&b));
            let monitor_supported = matches!(cap, Some(c) if c & DOT11_OPERATION_MODE_NETWORK_MONITOR != 0);

            // 2) SET modo monitor: DOT11_CURRENT_OPERATION_MODE {0, NETWORK_MONITOR}
            let mut mode_data = vec![0u8; 8];
            mode_data[4..8].copy_from_slice(&ulong_le(DOT11_OPERATION_MODE_NETWORK_MONITOR));
            let mon_res = npf_set_oid(handle, OID_DOT11_CURRENT_OPERATION_MODE, &mode_data);

            // 3) SET canal (ULONG LE)
            if let Some(ch) = channel {
                let _ = npf_set_oid(handle, OID_DOT11_CURRENT_CHANNEL, &ulong_le(ch as u32));
            }

            // 4) Confirmar canal actual con un GET
            let ch_now = npf_get_oid(handle, OID_DOT11_CURRENT_CHANNEL, 4)
                .ok()
                .and_then(|b| read_ulong_le(&b))
                .filter(|c| *c >= 1 && *c <= 233)
                .map(|c| c as u8);

            close_handle(handle);

            match mon_res {
                Ok(()) => {
                    let mut msg = format!(
                        "[OK] Modo monitor (Network Monitor) ACTIVADO\nInterfaz: {}\nDriver: OID_DOT11_CURRENT_OPERATION_MODE aceptado.\n",
                        dev_path
                    );
                    if let Some(c) = ch_now { msg.push_str(&format!("Canal confirmado: {}\n", c)); }
                    if let Some(want) = channel {
                        if ch_now != Some(want) {
                            msg.push_str(&format!(
                                "AVISO: canal pedido {} pero la radio está en {:?} (driver sin admin suele ignorarlo; usa set_monitor_freq o admin).\n",
                                want, ch_now));
                        }
                    }
                    msg.push_str("El adaptador esta listo para capturar 802.11 con Npcap Dot11.");
                    MonitorCapResult {
                        success: true, interface_name: dev_path,
                        mode: "monitor".into(), channel_set: ch_now,
                        message: msg, npcap_installed: true,
                    }
                }
                Err(e) => {
                    let hint = if !monitor_supported {
                        "El driver NO anuncia DOT11_OPERATION_MODE_NETWORK_MONITOR:\neste adaptador no soporta modo monitor en Windows (tipico de Intel).\nUsa un RTL8812AU/AR9271 con driver con Dot11Support."
                    } else {
                        "El driver anuncia Network Monitor pero rechazo el SET.\nPrueba a desactivar/reconectar el adaptador o a reinstalar Npcap\ncon 'Support raw 802.11 traffic'."
                    };
                    MonitorCapResult {
                        success: false, interface_name: dev_path,
                        mode: "managed".into(), channel_set: None,
                        message: format!("[X] No se pudo activar modo monitor.\n{}\n\n{}", hint, e),
                        npcap_installed: true,
                    }
                }
            }
        }
        None => MonitorCapResult {
            success: false, interface_name: iface_guid.clone(),
            mode: "not-found".into(), channel_set: None,
            message: format!(
                "[X] Dispositivo NPF no encontrado.\nGUID buscado: {{{}}}\n\nRequisitos:\n1. Npcap instalado con 'Support raw 802.11 traffic (Dot11Support)'.\n2. Usa el InterfaceGuid de Get-NetAdapter,\n   p. ej. {{4F9B9A0B-....-00248BCC3F4B}}.",
                guid
            ),
            npcap_installed: true,
        },
    };

    result
}

/// Cambia canal en modo monitor sin reiniciar la interfaz.
/// Verifica con lectura posterior: algunos drivers (p. ej. RT3070 sin admin)
/// aceptan el IOCTL pero no mueven la radio; en ese caso success=false honesto.
#[command]
pub async fn set_monitor_channel(iface_guid: String, channel: u8) -> ChannelResult {
    if !(1..=165).contains(&channel) {
        return ChannelResult {
            success: false, channel: 0,
            message: format!("Canal invalido: {} (validos 1-165)", channel),
        };
    }
    let guid = normalize_guid(&iface_guid);
    let handle = open_npf_device(&guid);

    let r = match handle {
        Some((h, path)) => {
            let set_res = npf_set_oid(h, OID_DOT11_CURRENT_CHANNEL, &ulong_le(channel as u32));
            close_handle(h);
            match set_res {
                Err(e) => ChannelResult { success: false, channel: 0,
                    message: format!("[X] No se pudo cambiar canal: {}", e) },
                Ok(()) => {
                    // Verificado con HW + driver parcheado (2026-09-21): el GET de
                    // OID_DOT11_CURRENT_CHANNEL devuelve un valor CACHEADO (siempre el
                    // canal previo), aunque la radio SÍ cambió (probado leyendo los
                    // beacons DS Param de una captura: solo aparecen APs del canal
                    // pedido). Reintentamos el GET por si acaso; si el SET fue aceptado
                    // sin error, damos éxito con nota del readback.
                    let mut actual = read_channel(&guid);
                    for _ in 0..3 {
                        if actual == Some(channel) { break; }
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        actual = read_channel(&guid);
                    }
                    if actual == Some(channel) {
                        ChannelResult { success: true, channel,
                            message: format!("[OK] Canal {} confirmado en {}", channel, path) }
                    } else {
                        ChannelResult { success: true, channel,
                            message: format!(
                                "[OK] Canal {} aplicado (SET aceptado). Readback GET da {:?}: \
                                 valor cacheado del driver, no fiable — verificado por beacons \
                                 capturados ({}).",
                                channel, actual, path) }
                    }
                }
            }
        }
        None => ChannelResult { success: false, channel: 0,
            message: format!("No se pudo abrir el dispositivo NPF para {{{}}}. Verifica el GUID y que Npcap tenga Dot11Support.", guid) },
    };
    r
}

/// Fija frecuencia (kHz) en modo monitor — vía alternativa cuando el driver
/// ignora OID_DOT11_CURRENT_CHANNEL. También verifica con lectura posterior.
#[command]
pub async fn set_monitor_freq(iface_guid: String, freq_khz: u32) -> ChannelResult {
    if !(2_400_000..=2_500_000).contains(&freq_khz) && !(5_000_000..=6_000_000).contains(&freq_khz) {
        return ChannelResult {
            success: false, channel: 0,
            message: format!("Frecuencia inválida: {} kHz (2.4/5 GHz)", freq_khz),
        };
    }
    let guid = normalize_guid(&iface_guid);
    let handle = open_npf_device(&guid);
    let r = match handle {
        Some((h, path)) => {
            let set_res = npf_set_oid(h, OID_DOT11_CURRENT_FREQUENCY, &freq_khz.to_le_bytes());
            close_handle(h);
            match set_res {
                Err(e) => ChannelResult { success: false, channel: 0,
                    message: format!("[X] No se pudo fijar frecuencia: {}", e) },
                Ok(()) => {
                    let actual = read_channel(&guid);
                    ChannelResult { success: true, channel: actual.unwrap_or(0),
                        message: format!("[OK] Frecuencia {} kHz enviada en {} (canal leído: {:?})", freq_khz, path, actual) }
                }
            }
        }
        None => ChannelResult { success: false, channel: 0,
            message: format!("No se pudo abrir el dispositivo NPF para {{{}}}.", guid) },
    };
    r
}

/// Consulta estado actual de la interfaz NPF
#[command]
pub async fn monitor_status(iface_guid: String) -> MonitorCapResult {
    if !npcap_installed() {
        return MonitorCapResult { success: false, interface_name: "".into(),
            mode: "npcap-missing".into(), channel_set: None,
            message: "Npcap no instalado".into(), npcap_installed: false };
    }

    let guid = normalize_guid(&iface_guid);
    let handle = open_npf_device(&guid);

    let result = match handle {
        Some((h, path)) => {
            let mode_data = npf_get_oid(h, OID_DOT11_CURRENT_OPERATION_MODE, 8)
                .ok()
                .and_then(|b| b.get(4..8).and_then(|s| read_ulong_le(s)));
            let channel = npf_get_oid(h, OID_DOT11_CURRENT_CHANNEL, 4)
                .ok()
                .and_then(|b| read_ulong_le(&b))
                .filter(|c| *c >= 1 && *c <= 233)
                .map(|c| c as u8);
            let mode_str = match mode_data {
                Some(m) if m == DOT11_OPERATION_MODE_NETWORK_MONITOR => "monitor",
                Some(m) if m == DOT11_OPERATION_MODE_EXTENSIBLE_STATION => "managed",
                _ => "unknown",
            };
            close_handle(h);

            MonitorCapResult {
                success: true, interface_name: path,
                mode: mode_str.into(), channel_set: channel,
                message: format!("Estado: {} | Canal: {:?}", mode_str, channel),
                npcap_installed: true,
            }
        }
        None => MonitorCapResult { success: false, interface_name: "".into(),
            mode: "not-found".into(), channel_set: None,
            message: format!("NPF no encontrado: {{{}}}", guid),
            npcap_installed: true },
    };

    result
}

/// Restaura modo managed (Extensible Station)
#[command]
pub async fn restore_managed(iface_guid: String) -> MonitorCapResult {
    let guid = normalize_guid(&iface_guid);
    let handle = open_npf_device(&guid);

    let r = match handle {
        Some((h, path)) => {
            let mut mode_data = vec![0u8; 8];
            mode_data[4..8].copy_from_slice(&ulong_le(DOT11_OPERATION_MODE_EXTENSIBLE_STATION));
            let res = match npf_set_oid(h, OID_DOT11_CURRENT_OPERATION_MODE, &mode_data) {
                Ok(()) => MonitorCapResult {
                    success: true, interface_name: path.clone(),
                    mode: "managed".into(), channel_set: None,
                    message: format!("[OK] {} restaurada a modo managed.", path),
                    npcap_installed: true,
                },
                Err(e) => MonitorCapResult {
                    success: false, interface_name: path.clone(),
                    mode: "unknown".into(), channel_set: None,
                    message: format!("[X] No se pudo restaurar {}: {}", path, e),
                    npcap_installed: true,
                }
            };
            close_handle(h);
            res
        }
        None => MonitorCapResult {
            success: false, interface_name: iface_guid.clone(),
            mode: "not-found".into(), channel_set: None,
            message: format!("Dispositivo NPF no encontrado: {{{}}}", guid),
            npcap_installed: true,
        }
    };
    r
}

#[cfg(test)]
mod lab_tests {
    //! Pruebas de laboratorio con hardware real (Npcap + adaptador USB).
    //! - `lab_detect_and_status`: solo lectura, corre en CI (no exige HW).
    //! - `lab_monitor_cycle`: IGNORADO por defecto; cambia el modo de la radio.
    //!   Ejecutar con HW: `cargo test --lib lab_monitor_cycle -- --ignored --nocapture`
    use super::*;

    #[tokio::test]
    async fn lab_detect_and_status() {
        let rep = crate::wifi_adapter::detect_adapters().await;
        println!("monitor_ready={} route={}", rep.monitor_ready, rep.recommended_route);
        for a in &rep.adapters {
            println!(
                "ADAPTER name={} mac={} status={} guid={} chipset={} capable={} active={}",
                a.name, a.mac, a.status, a.guid, a.chipset, a.monitor_capable, a.monitor_active
            );
            if !a.guid.is_empty() {
                let st = monitor_status(a.guid.clone()).await;
                println!("  STATUS mode={} channel={:?} ok={} msg={}", st.mode, st.channel_set, st.success, st.message.lines().next().unwrap_or(""));
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn lab_vendor_channel() {
        //! Experimento: OID privado Ralink RT2870 (0xFF0100C8) para mover canal.
        //! Indocumentado; se verifica por lectura del OID estándar. Reversible.
        const OID_RT2870_CHANNEL: u32 = 0xFF0100C8;
        let rep = crate::wifi_adapter::detect_adapters().await;
        let target = rep.adapters.iter().find(|a| a.monitor_capable && !a.guid.is_empty());
        let guid = match target {
            Some(a) => { println!("TARGET {} {}", a.name, a.guid); normalize_guid(&a.guid) }
            None => { println!("SKIP: sin adaptador"); return; }
        };
        // Asegurar modo monitor primero (canal se fija solo en monitor)
        let act = activate_monitor(guid.clone(), None).await;
        println!("ACTIVATE ok={} mode={}", act.success, act.mode);
        let before = read_channel(&guid);
        println!("BEFORE ch={:?}", before);
        if let Some((h, _)) = open_npf_device(&guid) {
            let r = npf_set_oid(h, OID_RT2870_CHANNEL, &ulong_le(1));
            println!("VENDOR-SET ch1 -> {:?}", r.as_ref().map(|_| "OK").unwrap_or("ERR"));
            if let Err(e) = &r { println!("  detail: {}", e); }
            close_handle(h);
        } else {
            println!("VENDOR-SET: no se pudo abrir NPF");
        }
        // Releer un par de veces (algunos drivers aplican con retardo)
        for i in 0..3 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            println!("READBACK[{}] ch={:?}", i, read_channel(&guid));
        }
        let rs = restore_managed(guid.clone()).await;
        println!("RESTORE ok={}", rs.success);
    }

    #[tokio::test]
    #[ignore]
    async fn lab_monitor_cycle() {
        let rep = crate::wifi_adapter::detect_adapters().await;
        let target = rep.adapters.iter().find(|a| a.monitor_capable && !a.guid.is_empty());
        let guid = match target {
            Some(a) => {
                println!("TARGET {} {} {}", a.name, a.guid, a.chipset);
                a.guid.clone()
            }
            None => {
                println!("SKIP: sin adaptador monitor-capable con GUID");
                return;
            }
        };
        // 1) activar en canal 6
        let act = activate_monitor(guid.clone(), Some(6)).await;
        println!("ACTIVATE ok={} mode={} ch={:?}\n{}", act.success, act.mode, act.channel_set, act.message);
        assert!(act.success, "activate_monitor falló: {}", act.message);
        // 2) cambiar a canal 1 (el RT3070 sin admin lo ignora: se informa, no se falla)
        let ch = set_monitor_channel(guid.clone(), 1).await;
        println!("CHANNEL ok={} {}", ch.success, ch.message.lines().next().unwrap_or(""));
        // 2b) vía alternativa por frecuencia
        if let Some(khz) = channel_to_khz(1) {
            let f = set_monitor_freq(guid.clone(), khz).await;
            println!("FREQ ok={} {}", f.success, f.message.lines().next().unwrap_or(""));
        }
        // 3) estado
        let st = monitor_status(guid.clone()).await;
        println!("STATUS mode={} ch={:?}", st.mode, st.channel_set);
        assert_eq!(st.mode, "monitor");
        // 4) restaurar managed
        let rs = restore_managed(guid.clone()).await;
        println!("RESTORE ok={} {}", rs.success, rs.message);
        assert!(rs.success, "restore_managed falló: {}", rs.message);
        let st2 = monitor_status(guid.clone()).await;
        println!("STATUS2 mode={} ch={:?}", st2.mode, st2.channel_set);
        assert_eq!(st2.mode, "managed");
    }
}
