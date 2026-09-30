// Captura 802.11 nativa vía wpcap.dll (Npcap) — sin binarios externos.
//
// Por qué existe: airodump-ng/aireplay-ng Windows NO soportan interfaces Npcap
// (solo AirPcap HW) y Npcap no implementa inyección. Pero la CAPTURA sí funciona:
// con el adaptador en modo monitor (ver monitor_mode.rs), wpcap entrega tramas
// 802.11+Radiotap que nuestro conversor (pcap_convert.rs) ya entiende (DLT 105/127).
//
// wpcap.dll se carga en runtime con libloading (viene con Npcap, con o sin modo
// WinPcap-compat): sin .lib de linkado y con error claro si Npcap falta.
// La función usada es el API clásico de libpcap, estable desde hace décadas.
use serde::Serialize;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar};
use std::ptr;
use tauri::command;

#[repr(C)]
struct PcapPkthdr {
    ts_sec: i32, // long en Windows x64 = 32 bits
    ts_usec: i32,
    caplen: u32,
    len: u32,
}

type PcapOpenLive = unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> *mut c_void;
type PcapNextEx = unsafe extern "C" fn(*mut c_void, *mut *mut PcapPkthdr, *mut *const c_uchar) -> c_int;
type PcapDumpOpen = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type PcapDump = unsafe extern "C" fn(*mut c_void, *const PcapPkthdr, *const c_uchar);
type PcapDumpClose = unsafe extern "C" fn(*mut c_void);
type PcapClose = unsafe extern "C" fn(*mut c_void);
type PcapDatalink = unsafe extern "C" fn(*mut c_void) -> c_int;
type PcapGeterr = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type PcapSendpacket = unsafe extern "C" fn(*mut c_void, *const c_uchar, c_int) -> c_int;

struct Wpcap {
    _lib: libloading::Library, // se mantiene viva mientras se usan los símbolos
    open_live: PcapOpenLive,
    next_ex: PcapNextEx,
    dump_open: PcapDumpOpen,
    dump: PcapDump,
    dump_close: PcapDumpClose,
    close: PcapClose,
    datalink: PcapDatalink,
    geterr: PcapGeterr,
    /// Solo usada por el test de laboratorio `lab_inject_probe`.
    #[allow(dead_code)]
    sendpacket: PcapSendpacket,
}

impl Wpcap {
    unsafe fn load() -> Result<Self, String> {
        // 1) Npcap con Dot11  2) WinPcap-compat en System32  3) PATH
        let candidates = [
            r"C:\Windows\System32\Npcap\wpcap.dll",
            r"C:\Windows\System32\wpcap.dll",
            "wpcap.dll",
        ];
        let mut last_err = String::new();
        for c in candidates {
            match libloading::Library::new(c) {
                Ok(lib) => {
                    macro_rules! sym {
                        ($n:literal) => {
                            match lib.get($n.as_bytes()) {
                                Ok(s) => *s,
                                Err(e) => {
                                    return Err(format!("wpcap.dll sin símbolo {}: {}", $n, e));
                                }
                            }
                        };
                    }
                    return Ok(Wpcap {
                        open_live: sym!("pcap_open_live"),
                        next_ex: sym!("pcap_next_ex"),
                        dump_open: sym!("pcap_dump_open"),
                        dump: sym!("pcap_dump"),
                        dump_close: sym!("pcap_dump_close"),
                        close: sym!("pcap_close"),
                        datalink: sym!("pcap_datalink"),
                        geterr: sym!("pcap_geterr"),
                        sendpacket: sym!("pcap_sendpacket"),
                        _lib: lib,
                    });
                }
                Err(e) => last_err = format!("{}: {}", c, e),
            }
        }
        Err(format!(
            "X wpcap.dll no encontrada ({}).\nInstala Npcap desde https://npcap.com con 'Support raw 802.11 traffic'.",
            last_err
        ))
    }

    unsafe fn err_str(&self, handle: *mut c_void) -> String {
        let p = (self.geterr)(handle);
        if p.is_null() {
            return "error desconocido de wpcap".into();
        }
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NativeCaptureResult {
    pub success: bool,
    pub output_file: String,
    pub packets: u64,
    pub datalink: i32,
    pub datalink_name: String,
    pub message: String,
}

fn datalink_name(dlt: i32) -> &'static str {
    match dlt {
        1 => "EN10MB (Ethernet emulado: sin modo monitor)",
        105 => "IEEE802_11 (802.11 sin radiotap)",
        127 => "IEEE802_11_RADIO (802.11 + radiotap)",
        _ => "desconocido",
    }
}

/// Captura N segundos de tráfico 802.11 con Npcap y los guarda en .pcap clásico.
/// Requiere el adaptador en modo monitor (activate_monitor primero): si no lo
/// está, falla con mensaje claro en vez de grabar Ethernet emulado en silencio.
/// El .pcap resultante se convierte con pcap_to_22000 / pmkid_convert.
#[command]
pub async fn native_capture(
    iface_guid: String,
    duration_secs: Option<u64>,
    output_path: Option<String>,
) -> NativeCaptureResult {
    let fail = |msg: String| NativeCaptureResult {
        success: false,
        output_file: String::new(),
        packets: 0,
        datalink: -1,
        datalink_name: String::new(),
        message: msg,
    };
    let dur = duration_secs.unwrap_or(30).clamp(5, 600);

    // wpcap presente
    let w = match unsafe { Wpcap::load() } {
        Ok(w) => w,
        Err(e) => return fail(e),
    };

    // GUID limpio → nombre de dispositivo NPF
    let guid = {
        let s = iface_guid.trim();
        if let (Some(a), Some(b)) = (s.find('{'), s.find('}')) {
            if b > a {
                s[a + 1..b].to_string()
            } else {
                s.to_string()
            }
        } else {
            s.trim_start_matches("NPF_").to_string()
        }
    };
    if guid.is_empty() {
        return fail("GUID de interfaz vacío.".into());
    }
    // Npcap expone DOS dispositivos por adaptador WiFi (verificado en lab):
    //   \Device\NPF_{GUID}       → siempre Ethernet emulado (DLT 1)
    //   \Device\NPF_WIFI_{GUID}  → 802.11 crudo + Radiotap en modo monitor (DLT 127)
    let dev = format!(r"\Device\NPF_WIFI_{{{}}}", guid);
    let out = output_path.unwrap_or_else(|| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("npcap_capture_{}.pcap", ts)
    });

    unsafe {
        let dev_c = match CString::new(dev.clone()) {
            Ok(c) => c,
            Err(_) => return fail("GUID con bytes nulos.".into()),
        };
        let mut errbuf = [0 as c_char; 256];
        let handle = (w.open_live)(dev_c.as_ptr(), 65536, 1, 1000, errbuf.as_mut_ptr());
        if handle.is_null() {
            let e = CStr::from_ptr(errbuf.as_ptr()).to_string_lossy().into_owned();
            return fail(format!(
                "X No se pudo abrir {}: {}\n¿Npcap con Dot11Support? ¿GUID correcto (modal monitor)?",
                dev, e
            ));
        }
        let dlt = (w.datalink)(handle);
        // Sin modo monitor el driver entrega Ethernet emulado: inútil para PMKID/handshake.
        if dlt == 1 {
            let msg = format!(
                "X {} entrega Ethernet emulado (DLT 1): el adaptador NO está en modo monitor.\nActívalo primero con activate_monitor y reintenta.",
                dev
            );
            (w.close)(handle);
            return fail(msg);
        }
        let out_c = match CString::new(out.clone()) {
            Ok(c) => c,
            Err(_) => {
                (w.close)(handle);
                return fail("Ruta de salida con bytes nulos.".into());
            }
        };
        let dumper = (w.dump_open)(handle, out_c.as_ptr());
        if dumper.is_null() {
            let e = w.err_str(handle);
            (w.close)(handle);
            return fail(format!("X No se pudo crear {}: {}", out, e));
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(dur);
        let mut pkts: u64 = 0;
        let mut last_err = String::new();
        loop {
            if std::time::Instant::now() >= deadline {
                break;
            }
            let mut hdr: *mut PcapPkthdr = ptr::null_mut();
            let mut data: *const c_uchar = ptr::null();
            match (w.next_ex)(handle, &mut hdr, &mut data) {
                1 => {
                    if !hdr.is_null() && !data.is_null() {
                        (w.dump)(dumper, hdr as *const PcapPkthdr, data);
                        pkts += 1;
                    }
                }
                0 => continue, // timeout de 1s: revisar deadline
                _ => {
                    last_err = w.err_str(handle);
                    break;
                }
            }
        }
        (w.dump_close)(dumper);
        (w.close)(handle);
        let name = datalink_name(dlt);
        let mut message = format!(
            "[OK] {} paquetes en {}s → {}\nEnlace: {} ({})\nSiguiente: pcap_to_22000 para el .22000 y hashcat.",
            pkts, dur, out, dlt, name
        );
        if !last_err.is_empty() {
            message.push_str(&format!("\nAviso wpcap al cerrar: {}", last_err));
        }
        NativeCaptureResult {
            success: true,
            output_file: out,
            packets: pkts,
            datalink: dlt,
            datalink_name: name.into(),
            message,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InjectionCheckResult {
    pub supported: bool,
    pub message: String,
}

#[command]
pub async fn check_injection_capability(iface_guid: String) -> InjectionCheckResult {
    let guid = {
        let s = iface_guid.trim();
        if let (Some(a), Some(b)) = (s.find('{'), s.find('}')) {
            if b > a {
                s[a + 1..b].to_string()
            } else {
                s.to_string()
            }
        } else {
            s.trim_start_matches("NPF_").to_string()
        }
    };
    if guid.is_empty() {
        return InjectionCheckResult {
            supported: false,
            message: "GUID de interfaz vacío. Usa el modal «Activar modo monitor» para obtener el GUID NPF_{...} del adaptador.".into(),
        };
    }
    let w = match unsafe { Wpcap::load() } {
        Ok(w) => w,
        Err(e) => {
            return InjectionCheckResult {
                supported: false,
                message: format!("wpcap.dll no disponible: {}", e),
            }
        }
    };
    unsafe {
        let dev = format!(r"\Device\NPF_WIFI_{{{}}}", guid);
        let dev_c = match CString::new(dev.clone()) {
            Ok(c) => c,
            Err(_) => {
                return InjectionCheckResult {
                    supported: false,
                    message: "GUID con bytes nulos.".into(),
                }
            }
        };
        let mut errbuf = [0 as c_char; 256];
        let h = (w.open_live)(dev_c.as_ptr(), 65536, 1, 500, errbuf.as_mut_ptr());
        if h.is_null() {
            let e = CStr::from_ptr(errbuf.as_ptr()).to_string_lossy().into_owned();
            return InjectionCheckResult {
                supported: false,
                message: format!(
                    "X No se pudo abrir {}: {}\n¿Npcap con Dot11Support? ¿GUID correcto?",
                    dev, e
                ),
            };
        }
        let dlt = (w.datalink)(h);
        if dlt == 1 {
            let msg = format!(
                "X {} entrega Ethernet emulado (DLT 1): el adaptador NO está en modo monitor.\nActívalo primero con activate_monitor.",
                dev
            );
            (w.close)(h);
            return InjectionCheckResult {
                supported: false,
                message: msg,
            };
        }
        let macb: Vec<u8> = vec![0xFF; 6];
        let mut cts = vec![0xC4, 0x00, 0x00, 0x00];
        cts.extend_from_slice(&macb);
        let rc = (w.sendpacket)(h, cts.as_ptr(), cts.len() as c_int);
        (w.close)(h);
        if rc == 0 {
            return InjectionCheckResult {
                supported: true,
                message: format!(
                    "✅ Inyección soportada: pcap_sendpacket rc=0 en {} (DLT {}).",
                    dev, datalink_name(dlt)
                ),
            };
        }
        let e = w.err_str(h);
        InjectionCheckResult {
            supported: false,
            message: format!(
                "X Inyección NO soportada: pcap_sendpacket rc=-1 en {} (DLT {}).\nError: {}\n\nEste adaptador/driver no permite inyección 802.11 en Windows+Npcap.\nOpciones:\n  1) Usa un adaptador con driver Dot11 completo (RTL8812AU/AR9271).\n  2) Usa el motor WSL2/Kali (requiere usbipd + distro Kali).",
                dev, datalink_name(dlt), e
            ),
        }
    }
}

#[cfg(test)]
mod lab_capture_tests {
    //! Captura real con HW (ignorado por defecto).
    //! `cargo test --lib lab_native_capture -- --ignored --nocapture`
    use super::*;
    use crate::monitor_mode::{activate_monitor, monitor_status, restore_managed};

    #[tokio::test]
    #[ignore]
    async fn lab_dlt_matrix() {
        //! Matriz de nombres de dispositivo × promiscuo → DLT (diagnóstico).
        let rep = crate::wifi_adapter::detect_adapters().await;
        let target = rep.adapters.iter().find(|a| a.monitor_capable && !a.guid.is_empty());
        let guid = match target {
            Some(a) => a.guid.clone().trim_matches(|c| c == '{' || c == '}').to_string(),
            None => {
                println!("SKIP: sin adaptador");
                return;
            }
        };
        let act = activate_monitor(guid.clone(), None).await;
        println!("ACTIVATE ok={} mode={}", act.success, act.mode);
        let w = unsafe { Wpcap::load().expect("wpcap") };
        let devs = [
            format!(r"\Device\NPF_{{{}}}", guid),
            format!(r"\Device\NPF_WIFI_{{{}}}", guid),
            format!(r"\\.\Npcap\WIFI_{{{}}}", guid),
            format!(r"\\.\NPF_{{{}}}", guid),
        ];
        for dev in &devs {
            for promisc in [1, 0] {
                unsafe {
                    let dev_c = CString::new(dev.clone()).unwrap();
                    let mut errbuf = [0 as c_char; 256];
                    let h = (w.open_live)(dev_c.as_ptr(), 65536, promisc, 500, errbuf.as_mut_ptr());
                    if h.is_null() {
                        let e = CStr::from_ptr(errbuf.as_ptr()).to_string_lossy().into_owned();
                        println!("DEV={} promisc={} -> OPEN-FAIL {}", dev, promisc, e.lines().next().unwrap_or(""));
                    } else {
                        let dlt = (w.datalink)(h);
                        println!("DEV={} promisc={} -> DLT={} ({})", dev, promisc, dlt, datalink_name(dlt));
                        (w.close)(h);
                    }
                }
            }
        }
        let rs = restore_managed(guid.clone()).await;
        println!("RESTORE ok={}", rs.success);
    }

    #[tokio::test]
    #[ignore]
    async fn lab_inject_probe() {
        //! Sonda de inyección: 1 CTS-to-self (10B, duration 0, inofensivo) por
        //! pcap_sendpacket en el dispositivo WIFI_, con y sin cabecera radiotap.
        //! rc=0 solo dice "aceptado en buffer"; rc=-1 cierra la cuestión.
        //! `cargo test --lib lab_inject_probe -- --ignored --nocapture`
        use crate::monitor_mode::{activate_monitor, restore_managed};
        let rep = crate::wifi_adapter::detect_adapters().await;
        let target = rep.adapters.iter().find(|a| a.monitor_capable && !a.guid.is_empty());
        let (guid, mac) = match target {
            Some(a) => {
                let g = a.guid.clone().trim_matches(|c| c == '{' || c == '}').to_string();
                println!("TARGET {} {} mac={}", a.name, a.guid, a.mac);
                (g, a.mac.clone())
            }
            None => {
                println!("SKIP: sin adaptador");
                return;
            }
        };
        let macb: Vec<u8> = mac
            .split(':')
            .filter_map(|h| u8::from_str_radix(h, 16).ok())
            .collect();
        if macb.len() != 6 {
            println!("SKIP: MAC no parseable ({})", mac);
            return;
        }
        let act = activate_monitor(guid.clone(), None).await;
        println!("ACTIVATE ok={} mode={}", act.success, act.mode);
        // CTS-to-self: FC=0xC4, duration=0, RA=nuestra MAC
        let mut cts = vec![0xC4, 0x00, 0x00, 0x00];
        cts.extend_from_slice(&macb);
        let rt = vec![0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]; // radiotap mínimo
        let mut with_rt = rt.clone();
        with_rt.extend_from_slice(&cts);
        let w = unsafe { Wpcap::load().expect("wpcap") };
        unsafe {
            let dev = format!(r"\Device\NPF_WIFI_{{{}}}", guid);
            let dev_c = CString::new(dev.clone()).unwrap();
            let mut errbuf = [0 as c_char; 256];
            let h = (w.open_live)(dev_c.as_ptr(), 65536, 1, 500, errbuf.as_mut_ptr());
            if h.is_null() {
                println!("OPEN-FAIL");
            } else {
                for (tag, frame) in [("con-radiotap", &with_rt), ("sin-radiotap", &cts)] {
                    let rc = (w.sendpacket)(h, frame.as_ptr(), frame.len() as c_int);
                    let e = if rc == 0 { String::new() } else { w.err_str(h) };
                    println!("SEND {} len={} -> rc={} {}", tag, frame.len(), rc, e.lines().next().unwrap_or(""));
                }
                // Discriminador MaxFrameSize (NPF_Write): si MaxFrameSize>0,
                // un paquete de 6000B da error DISTINTO (INVALID_LENGTH); si
                // MaxFrameSize==0, el check de Npcap da el MISMO error 31.
                let mut big = rt.clone();
                big.extend_from_slice(&cts);
                big.resize(6000, 0);
                let rc = (w.sendpacket)(h, big.as_ptr(), big.len() as c_int);
                let e = if rc == 0 { String::new() } else { w.err_str(h) };
                println!("SEND OVERSIZE len={} -> rc={} {} (si error != 31, MaxFrameSize>0 y el envio SÍ llega al driver)", big.len(), rc, e.lines().next().unwrap_or(""));
                (w.close)(h);
            }
        }
        let rs = restore_managed(guid.clone()).await;
        println!("RESTORE ok={}", rs.success);
    }

    #[tokio::test]
    #[ignore]
    async fn lab_send_managed_ethernet() {
        //! Test discriminador TX: en modo MANAGED envía (1) un frame Ethernet
        //! por el LWF normal NPF_ y (2) el CTS-to-self por NPF_WIFI_.
        //! Si (1) da rc=-1 err31 también, el driver TX está roto en general.
        //! Si (1) pasa y (2) falla, el rechazo es del path 802.11 nativo NDIS.
        let rep = crate::wifi_adapter::detect_adapters().await;
        let a = rep.adapters.iter().find(|x| !x.guid.is_empty())
            .expect("sin adaptador");
        let guid = a.guid.clone().trim_matches(|c| c == '{' || c == '}').to_string();
        println!("TARGET {} mode-actual", a.name);
        let w = unsafe { Wpcap::load().expect("wpcap") };
        unsafe {
            // ARP broadcast falso (42 B) — inofensivo
            let mut eth = vec![0xff; 6];
            eth.extend_from_slice(&[0x00, 0xC0, 0xCA, 0x59, 0xA7, 0xD8]);
            eth.extend_from_slice(&[0x08, 0x06]);
            eth.extend_from_slice(&[0u8; 28]);
            for (tag, dev) in [
                ("NPF_ (ethernet)", format!(r"\Device\NPF_{{{}}}", guid)),
                ("NPF_WIFI_ (nativo)", format!(r"\Device\NPF_WIFI_{{{}}}", guid)),
            ] {
                let dev_c = CString::new(dev).unwrap();
                let mut errbuf = [0 as c_char; 256];
                let h = (w.open_live)(dev_c.as_ptr(), 65536, 1, 500, errbuf.as_mut_ptr());
                if h.is_null() {
                    println!("{} open-FAIL: {}", tag, CStr::from_ptr(errbuf.as_ptr()).to_string_lossy());
                    continue;
                }
                let rc = (w.sendpacket)(h, eth.as_ptr(), eth.len() as c_int);
                let e = if rc == 0 { String::new() } else { w.err_str(h) };
                println!("{} send len={} -> rc={} {}", tag, eth.len(), rc, e.lines().next().unwrap_or(""));
                (w.close)(h);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn lab_assoc_send_test() {
        //! Prueba decisiva: enviar frame raw por NPF_WIFI_ estando ASOCIADO a una
        //! red abierta (BSS válido). Si rc=0 asociado vs err31 sin asociar,
        //! la capa nativa 802.11 exige BSS y podríamos inyectar en modo asociado.
        //! `cargo test --lib lab_assoc_send_test -- --ignored --nocapture`
        use std::process::Command;
        let rep = crate::wifi_adapter::detect_adapters().await;
        let a = rep.adapters.iter().find(|x| !x.guid.is_empty()).expect("sin adaptador");
        let guid = a.guid.clone().trim_matches(|c| c == '{' || c == '}').to_string();
        let mac = a.mac.clone();
        println!("TARGET {} guid={} mac={}", a.name, guid, mac);

        // Red abierta conocida del entorno de lab (impresora HP, canal 3)
        let ssid = "HP-Print-34-ENVY 4500 series";
        println!("perfil+connect a red abierta: {}", ssid);
        let profile = format!(
            "<?xml version=\"1.0\"?><WLANProfile xmlns=\"http://www.microsoft.com/networking/WLAN/profile/v1\"><name>{}</name><SSIDConfig><SSID><name>{}</name></SSID></SSIDConfig><connectionType>ESS</connectionType><connectionMode>manual</connectionMode><MSM><security><authEncryption><authentication>open</authentication><encryption>none</encryption><useOneX>false</useOneX></authEncryption></security></MSM></WLANProfile>",
            ssid, ssid
        );
        let mut pf = std::env::temp_dir();
        pf.push("uifipill_open_profile.xml");
        std::fs::write(&pf, profile).unwrap();
        let _ = Command::new("netsh")
            .args([
                "wlan", "add", "profile",
                "filename=&".trim_end_matches("&"),
            ])
            .status();
        // netsh requiere filename= sin espacios problemáticos; usar cmd para pasar la ruta
        let add = Command::new("cmd")
            .args(["/C", &format!(
                "netsh wlan add profile filename=\"{}\" user=all",
                pf.to_string_lossy()
            )])
            .output()
            .unwrap();
        println!("add-profile: {}", String::from_utf8_lossy(&add.stdout).trim());
        let conn = Command::new("cmd")
            .args(["/C", &format!("netsh wlan connect name=\"{}\"", ssid)])
            .output()
            .unwrap();
        println!("connect: {}", String::from_utf8_lossy(&conn.stdout).trim());
        std::thread::sleep(std::time::Duration::from_secs(8));
        let st = Command::new("netsh").args(["wlan", "show", "interfaces"]).output().unwrap();
        let st_out = String::from_utf8_lossy(&st.stdout).to_string();
        for line in st_out.lines() {
            if line.contains("Estado") || line.contains("State") || line.contains("SSID") { println!("  {}", line.trim()); }
        }

        let macb: Vec<u8> = mac.split(':')
            .filter_map(|h| u8::from_str_radix(h, 16).ok()).collect();
        let w = unsafe { Wpcap::load().expect("wpcap") };
        unsafe {
            // CTS-to-self por NPF_WIFI_ (asociados)
            let mut cts = vec![0xC4, 0x00, 0x00, 0x00];
            cts.extend_from_slice(&macb);
            let rt = vec![0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00];
            let mut with_rt = rt.clone();
            with_rt.extend_from_slice(&cts);
            let dev_c = CString::new(format!(r"\Device\NPF_WIFI_{{{}}}", guid)).unwrap();
            let mut errbuf = [0 as c_char; 256];
            let h = (w.open_live)(dev_c.as_ptr(), 65536, 1, 500, errbuf.as_mut_ptr());
            if h.is_null() {
                println!("OPEN-FAIL");
            } else {
                for (tag, frame) in [("WIFI con-radiotap", &with_rt), ("WIFI sin-radiotap", &cts)] {
                    let rc = (w.sendpacket)(h, frame.as_ptr(), frame.len() as c_int);
                    let e = if rc == 0 { String::new() } else { w.err_str(h) };
                    println!("SEND-ASSOC {} len={} -> rc={} {}", tag, frame.len(), rc, e.lines().next().unwrap_or(""));
                }
                // Y un data-frame real hacia el AP (QoS-null func, 0x48: FC+dur+RA=BSSID+TA=mac)
                let mut qnull = vec![0x48, 0x01, 0x00, 0x00];
                qnull.extend_from_slice(&[0xf0, 0x92, 0x1c, 0xcf, 0x19, 0x34]); // RA=BSSID impresora
                qnull.extend_from_slice(&macb);                                  // TA=nosotros
                qnull.extend_from_slice(&[0x00, 0x53, 0x92]);                    // BSSID... 3B dummy
                let rc = (w.sendpacket)(h, qnull.as_ptr(), qnull.len() as c_int);
                let e = if rc == 0 { String::new() } else { w.err_str(h) };
                println!("SEND-ASSOC qos-null len={} -> rc={} {}", qnull.len(), rc, e.lines().next().unwrap_or(""));
                (w.close)(h);
            }
        }
        let _ = Command::new("netsh").args(["wlan", "disconnect"]).status();
        println!("disconnect enviado");
    }

    #[tokio::test]
    #[ignore]
    async fn lab_native_capture() {
        let rep = crate::wifi_adapter::detect_adapters().await;
        let target = rep.adapters.iter().find(|a| a.monitor_capable && !a.guid.is_empty());
        let guid = match target {
            Some(a) => {
                println!("TARGET {} {}", a.name, a.guid);
                a.guid.clone()
            }
            None => {
                println!("SKIP: sin adaptador");
                return;
            }
        };
        let act = activate_monitor(guid.clone(), None).await;
        println!("ACTIVATE ok={} mode={}", act.success, act.mode);
        assert!(act.success);
        let st = monitor_status(guid.clone()).await;
        println!("STATUS mode={} ch={:?}", st.mode, st.channel_set);
        let cap = native_capture(
            guid.clone(),
            Some(15),
            Some(r"C:\Users\INdaHouse\AppData\Local\Temp\opencode\lab_capture.pcap".into()),
        )
        .await;
        println!("CAPTURE ok={} pkts={} dlt={}({})\n{}", cap.success, cap.packets, cap.datalink, cap.datalink_name, cap.message.lines().next().unwrap_or(""));
        assert!(cap.success, "native_capture falló: {}", cap.message);
        assert!(std::path::Path::new(&cap.output_file).exists());
        // Conversión inmediata del mismo fichero (prueba punta a punta)
        let conv = crate::pcap_convert::convert_pcap_to_22000(
            cap.output_file.clone(),
            r"C:\Users\INdaHouse\AppData\Local\Temp\opencode\lab_capture.22000".into(),
        )
        .await;
        println!("CONVERT hashes={} pmkids={} hs={} {:?}", conv.hash_count, conv.pmkid_count, conv.handshake_count, conv.messages);
        let rs = restore_managed(guid.clone()).await;
        println!("RESTORE ok={}", rs.success);
        assert!(rs.success);
    }
}
