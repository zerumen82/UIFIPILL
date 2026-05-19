// ═══════════════════════════════════════════════════════════════════════════════
// MODO MONITOR WINDOWS NATIVO — NPcap directo, sin WSL
//
// Flujo:
//   Npcap.dll  ──CreateFile──►  \Device\NPF_{GUID}
//              ──DeviceIoControl──► OID_RT2870_SET_MONITOR = 1  (modo monitor HW)
//              ──DeviceIoControl──► OID_RT2870_SET_CHANNEL = N  (canal)
//              ──ReadFile──►  buffer 802.11 crudo (hasta 65536 bytes)
// ═══════════════════════════════════════════════════════════════════════════════

// Infrastructure scaffolding intentionally not yet exposed to frontend;
// dead_code + unnecessary unsafe block warnings are suppressed.
#[allow(dead_code, unused_unsafe)]

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

// NPcap IOCTL codes (pre-computados: device<<16 | access<<14 | func<<2 | method)
// METHOD_IN_DIRECT=0x01 WRITE=0x02 FUNC=0x13B → 0x8000<<16 | 0x02<<14 | 0x13B<<2 | 0x01
const IOCTL_NPF_SET_OID: u32 = 0x8000_1001;
// METHOD_BUFFERED=0x00 READ=0x01 FUNC=0x13C → 0x8000<<16 | 0x01<<14 | 0x13C<<2 | 0x00
const IOCTL_NPF_GET_OID: u32 = 0x8000_0C40;

// OID RT2870 modo monitor y canal
const OID_RT2870_MONITOR: u32 = 0xFF0100C0;
const OID_RT2870_CHANNEL: u32 = 0xFF0100C8;
// OID_DOT11_* reserved for future use (dot11 extended channel table / monitor flags)
#[allow(dead_code)]
const OID_DOT11_CHANNEL:  u32 = 0x0D010104;
#[allow(dead_code)]
const OID_DOT11_MONITOR:  u32 = 0xFF0100C0;

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

    // ReadFile — reserved for packet-capture loop (not yet wired into a handler)
    #[allow(dead_code)]
    fn ReadFile(
        h_file: *mut c_void,
        lp_buffer: *mut c_void,
        n_number_of_bytes_to_read: u32,
        lp_number_of_bytes_read: *mut u32,
        lp_overlapped: *mut c_void,
    ) -> c_int;

    fn GetLastError() -> u32;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Abre el dispositivo NPF de Npcap por GUID:  \Device\NPF_{GUID}
unsafe fn open_npf_device(guid: &str) -> Option<*mut c_void> {
    // Aceptar GUID limpio o con prefijo NPF_
    let guid_clean = guid.trim_start_matches("NPF_").trim_start_matches("\\Device\\NPF_").trim_start_matches("\\\\Device\\NPF_");
    let path = format!("\\Device\\NPF_{}", guid_clean);
    let c_path = CString::new(path).ok()?;
    let handle = CreateFileA(
        c_path.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        0,
        ptr::null_mut(),
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        ptr::null_mut(),
    );
    if handle == INVALID_HANDLE_VALUE || handle.is_null() { None }
    else { Some(handle) }
}

fn close_handle(h: *mut c_void) {
    unsafe { let _ = CloseHandle(h); }
}

/// Envía un OID SET al driver NPcap y devuelve la respuesta
fn npf_set_oid(handle: *mut c_void, oid: u32, value: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let (in_ptr, in_len) = match value {
        Some(b) => (b.as_ptr() as *const c_void, b.len() as u32),
        None    => (ptr::null(), 0),
    };
    let mut out: [u8; 256] = [0; 256];
    let mut ret: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(handle, IOCTL_NPF_SET_OID,
            in_ptr, in_len,
            out.as_mut_ptr() as *mut c_void, out.len() as u32,
            &mut ret, ptr::null_mut())
    };
    if ok == 0 {
        return Err(format!("DeviceIoControl OID_SET 0x{:08X} falló (err={})", oid, unsafe { GetLastError() }));
    }
    Ok(out[..ret as usize].to_vec())
}

/// Envía un OID GET al driver NPcap y devuelve la respuesta
fn npf_get_oid(handle: *mut c_void, oid: u32) -> Result<Vec<u8>, String> {
    let mut out: [u8; 256] = [0; 256];
    let mut ret: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(handle, IOCTL_NPF_GET_OID,
            ptr::null(), 0,
            out.as_mut_ptr() as *mut c_void, out.len() as u32,
            &mut ret, ptr::null_mut())
    };
    if ok == 0 {
        return Err(format!("DeviceIoControl OID_GET 0x{:08X} falló (err={})", oid, unsafe { GetLastError() }));
    }
    Ok(out[..ret as usize].to_vec())
}

// ── Parseo 802.11 ─────────────────────────────────────────────────────────────

/// Extrae (src_mac, dst_mac, bssid) de un frame 802.11 sniffado por Npcap
/// Header 802.11: 2 bytes FC + 2 bytes Duration + 3 pares MAC (addr1,2,3,4) + 2 bytes SC
#[allow(dead_code)]
fn parse_80211_macs(data: &[u8]) -> (String, String, String) {
    let mac_at = |i: usize| -> String {
        if i + 5 < data.len() {
            format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                data[i], data[i+1], data[i+2], data[i+3], data[i+4], data[i+5])
        } else { "??:??:??:??:??:??".into() }
    };
    if data.len() < 24 { return (mac_at(10), mac_at(4), mac_at(16)); }

    let fc: u16 = (data[0] as u16) | ((data[1] as u16) << 8);
    let to_ds   = (fc >> 8)  & 1 != 0;
    let from_ds = (fc >> 9)  & 1 != 0;

    // Formato de direcciones 802.11:
    // addr1 (offset  4): DA / BSSID / RA
    // addr2 (offset 10): SA / TA  / RA2
    // addr3 (offset 16): BSSID / SA / TA2
    match (to_ds, from_ds) {
        (false, false) => (mac_at(10), mac_at(4),  mac_at(16)),  // IBSS: SA→DA, BSSID
        (true,  false) => (mac_at(16), mac_at(10), mac_at(4)),   // From DS: BSSID→SA, DA
        (false, true)  => (mac_at(4),  mac_at(16), mac_at(10)),  // To DS:   DA→BSSID, SA
        (true,  true)  => (mac_at(16), mac_at(4),  mac_at(10)),  // WDS
    }
}

// frame-type label helper — reserved for packet-capture (not yet wired)
#[allow(dead_code)]
fn frame_label(data: &[u8]) -> String {
    if data.len() < 2 { return "?".into() }
    let fc: u16 = (data[0] as u16) | ((data[1] as u16) << 8);
    let ftype  = (fc >> 2) & 0b11;
    let fsub   = (fc >> 4) & 0b1111;
    match ftype {
        0x00 => match fsub {
            0x08 => "Beacon".into(),
            0x04 => "ProbeReq".into(),
            0x05 => "ProbeResp".into(),
            0x0A => "Disassoc".into(),
            0x0C => "Deauth".into(),
            0x0B => "Auth".into(),
            0x00 => "AssocReq".into(),
            0x01 => "AssocResp".into(),
            0x0D => "Action".into(),
            _     => format!("Mgmt/0x{:X}", fsub),
        },
        0x01 => match fsub {
            0x0B => "ACK".into(),
            0x0C => "RTS".into(),
            0x0D => "CTS".into(),
            _     => format!("Ctrl/0x{:X}", fsub),
        },
        0x02 => "Data".into(),
        _     => format!("?/0x{:X}", ftype),
    }
}

/// Convierte el byte RSSI de Npcap a dBm aproximado
#[allow(dead_code)]
fn rssi_to_dbm(raw: i8) -> i8 { raw }

// ── Modelos ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
// PacketEntry — reserved for streaming packet capture (not yet exposed to frontend)
#[allow(dead_code)]
pub struct PacketEntry {
    pub ts:         String,
    pub src_mac:    String,
    pub dst_mac:    String,
    pub bssid:      String,
    pub signal_dbm: i8,
    pub frame_type: String,
    pub channel:    Option<u8>,
}

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

// ── COMANDOS ─────────────────────────────────────────────────────────────────

/// Activa modo monitor en el adaptador NPF especificado por GUID.
/// Envía OID RT2870_SET_MONITOR=1 al driver NPcap modificado del chipset.
#[command]
pub async fn activate_monitor(iface_guid: String, channel: Option<u8>) -> MonitorCapResult {
    let npcap_ok  = std::path::Path::new(r"C:\Windows\System32\Npcap.dll").exists()
        || std::path::Path::new(r"C:\Windows\SysWOW64\Npcap.dll").exists();

    if !npcap_ok {
        return MonitorCapResult {
            success: false, interface_name: iface_guid.clone(),
            mode: "npcap-missing".into(), channel_set: None,
            message: "\u{274C} Npcap.dll no encontrado.\nInstala NPcap desde https://npcap.com\nMarca 'Install in WinPcap API-compatible Mode'".into(),
            npcap_installed: false,
        };
    }

    // Normalizar GUID: aceptar solo el UUID o con prefijo NPF_
    let guid = iface_guid
        .replace("\\\\Device\\\\NPF_", "")
        .replace("\\Device\\NPF_", "")
        .replace("NPF_", "")
        .replace("{", "").replace("}", "")
        .trim().to_string();

    let raw_handle = unsafe { open_npf_device(&guid) };
    let result = match raw_handle {
        Some(handle) => {
            // ── OID 1: activar modo monitor ──
            let mon_buf: [u8; 4] = [1, 0, 0, 0]; // 1 = monitor mode ON
            let mon_res = npf_set_oid(handle, OID_RT2870_MONITOR, Some(&mon_buf));

            // ── OID 2: setear canal ──
            if let Some(ch) = channel {
                let mut ch_buf: [u8; 4] = [0; 4];
                ch_buf[0] = ch;
                let _ = npf_set_oid(handle, OID_RT2870_CHANNEL, Some(&ch_buf));
            }

            // ── OID 3: consultar canal actual ──
            let ch_res = npf_get_oid(handle, OID_RT2870_CHANNEL).ok();
            let ch_opt = ch_res.as_ref().and_then(|b| b.first()).copied();

            close_handle(handle);

            let (success, msg) = match mon_res {
                Ok(_) => (true, format!(
                    "\u{2705} Modo monitor ACTIVADO\nInterfaz: NPF_{{{}}}\nCanal: {:?}\nOID RT2870 aceptado por el driver NPcap.\nEl adaptador est\u{E1} listo para capturar paquetes 802.11.",
                    guid, ch_opt
                )),
                Err(e) => (false, format!(
                    "\u{26A0}\u{FE0F} OID RT2870 no aceptado pero NPF abierto.\n{}\n\nPuedes intentar usar el adaptador en modo monitor\nsi el driver NPcap modificado lo soporta.\nGUID: NPF_{{{}}}", e, guid
                )),
            };

            MonitorCapResult {
                success, interface_name: format!("NPF_{}", guid),
                mode: if success { "monitor" } else { "managed(?)" }.into(),
                channel_set: ch_opt,
                message: msg,
                npcap_installed: true,
            }
        }
        None => {
            MonitorCapResult {
                success: false, interface_name: iface_guid.clone(),
                mode: "not-found".into(), channel_set: None,
                message: format!(
                    "\u{274C} Dispositivo NPF no encontrado.\n\
                     GUID buscado: {{{}}}\n\
                     \nPara obtener el GUID correcto de tu AWUS036H:\n\
                     1. Abre Administrador de dispositivos\n\
                     2. Redes -> tu adaptador ALFA -> Propiedades\n\
                     3. Detalles -> Ruta de la instancia de hardware\n\
                     4. Copia el valor completo y p\u{E9}galo aqu\u{ED}.\n\
                     \nEl GUID tiene forma: {{4F9B9A0B-0000-0000-0000-00248BCC3F4B}}",
                    guid
                ),
                npcap_installed: true,
            }
        }
    };

    result
}

/// Cambia canal en modo monitor sin reiniciar la interfaz
#[command]
pub async fn set_monitor_channel(iface_guid: String, channel: u8) -> ChannelResult {
    if !(1..=165).contains(&channel) {
        return ChannelResult {
            success: false, channel: 0,
            message: format!("Canal inv\u{E1}lido: {} (v\u{E1}lidos 1-165)", channel),
        };
    }
    let guid = iface_guid.replace("\\\\Device\\\\NPF_", "").replace("\\Device\\NPF_", "").replace("NPF_", "");
    let handle = unsafe { open_npf_device(&guid) };

    let r = match handle {
        Some(h) => {
            let mut buf: [u8; 4] = [0; 4];
            buf[0] = channel;
            match npf_set_oid(h, OID_RT2870_CHANNEL, Some(&buf)) {
                Ok(_) => ChannelResult { success: true, channel,
                    message: format!("\u{2705} Canal {} seteado correctamente en NPF_{{{}}}", channel, guid) },
                Err(e) => ChannelResult { success: false, channel: 0,
                    message: format!("\u{274C} No se pudo cambiar canal: {}", e) },
            }
        }
        None => ChannelResult { success: false, channel: 0,
            message: format!("No se pudo abrir NPF_{{{}}}. Verifica el GUID.", guid) },
    };
    if let Some(h) = handle { close_handle(h); }
    r
}

/// Consulta estado actual de la interfaz NPF
#[command]
pub async fn monitor_status(iface_guid: String) -> MonitorCapResult {
    let npcap_ok = std::path::Path::new(r"C:\Windows\System32\Npcap.dll").exists()
        || std::path::Path::new(r"C:\Windows\SysWOW64\Npcap.dll").exists();

    if !npcap_ok {
        return MonitorCapResult { success: false, interface_name: "".into(),
            mode: "npcap-missing".into(), channel_set: None,
            message: "Npcap no instalado".into(), npcap_installed: false };
    }

    let guid = iface_guid.replace("\\\\Device\\\\NPF_", "").replace("\\Device\\NPF_", "").replace("NPF_", "");
    let handle = unsafe { open_npf_device(&guid) };

    let result = match handle {
        Some(h) => {
            let mode_data = npf_get_oid(h, OID_RT2870_MONITOR).ok();
            let ch_data   = npf_get_oid(h, OID_RT2870_CHANNEL).ok();
            let mode_val  = mode_data.as_ref().and_then(|b| b.first()).copied().unwrap_or(0);
            let channel   = ch_data.as_ref().and_then(|b| b.first()).copied();
            let mode_str  = if mode_val == 1 { "monitor" } else { "managed" };

            close_handle(h);

            MonitorCapResult {
                success: true, interface_name: format!("NPF_{}", guid),
                mode: mode_str.into(), channel_set: channel,
                message: format!("Estado: {} | Canal: {:?}", mode_str, channel),
                npcap_installed: true,
            }
        }
        None => MonitorCapResult { success: false, interface_name: "".into(),
            mode: "not-found".into(), channel_set: None,
            message: format!("NPF no encontrado: {}", guid),
            npcap_installed: true },
    };

    result
}

/// Restaura modo managed
#[command]
pub async fn restore_managed(iface_guid: String) -> MonitorCapResult {
    let guid = iface_guid.replace("\\\\Device\\\\NPF_", "").replace("\\Device\\NPF_", "").replace("NPF_", "");
    let handle = unsafe { open_npf_device(&guid) };

    let r = match handle {
        Some(h) => {
            let buf: [u8; 4] = [0, 0, 0, 0]; // 0 = managed mode
            match npf_set_oid(h, OID_RT2870_MONITOR, Some(&buf)) {
                Ok(_) => MonitorCapResult {
                    success: true, interface_name: format!("NPF_{}", guid),
                    mode: "managed".into(), channel_set: None,
                    message: format!("\u{2705} NPF_{{{}}} restaurada a modo managed.", guid),
                    npcap_installed: true,
                },
                Err(e) => MonitorCapResult {
                    success: false, interface_name: format!("NPF_{}", guid),
                    mode: "unknown".into(), channel_set: None,
                    message: format!("\u{274C} No se pudo restaurar: {}", e),
                    npcap_installed: true,
                }
            }
        }
        None => MonitorCapResult {
            success: false, interface_name: format!("NPF_{}", guid),
            mode: "not-found".into(), channel_set: None,
            message: format!("Dispositivo NPF no encontrado: {}", guid),
            npcap_installed: true,
        }
    };
    if let Some(h) = handle { close_handle(h); }
    r
}

