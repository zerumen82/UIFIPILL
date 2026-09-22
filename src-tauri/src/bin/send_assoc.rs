//! Sonda TX asociado: envía CTS-to-self y QoS-Null por NPF_WIFI_ mientras la
//! interfaz está asociada a la red abierta (BSS válido). Uso:
//!   cargo run --bin send_assoc --release
use std::ffi::{c_char, c_int, c_void, CString};
use std::os::windows::ffi::OsStrExt;

fn err_str(handle: *mut c_void, wpcap: &libloading::Library) -> String {
    unsafe {
        let f: libloading::Symbol<unsafe extern "system" fn(*mut c_void) -> *const c_char> =
            wpcap.get(b"pcap_geterr").unwrap();
        let p = f(handle);
        if p.is_null() { return String::new(); }
        std::ffi::CStr::from_ptr(p).to_string_lossy().to_string()
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn main() {
    let guid = std::env::args().nth(1).expect("uso: send_assoc <GUID-sin-llaves> <MAC>");
    let mac = std::env::args().nth(2).expect("uso: send_assoc <GUID> <MAC>");
    let macb: Vec<u8> = mac.split(':')
        .filter_map(|h| u8::from_str_radix(h, 16).ok()).collect();
    assert_eq!(macb.len(), 6);

    unsafe {
        let wpcap: libloading::Library = libloading::Library::new("wpcap.dll").expect("wpcap.dll");
        let open_live: libloading::Symbol<
            unsafe extern "system" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> *mut c_void,
        > = wpcap.get(b"pcap_open_live").unwrap();
        let sendpacket: libloading::Symbol<
            unsafe extern "system" fn(*mut c_void, *const u8, c_int) -> c_int,
        > = wpcap.get(b"pcap_sendpacket").unwrap();
        let close: libloading::Symbol<unsafe extern "system" fn(*mut c_void)> =
            wpcap.get(b"pcap_close").unwrap();

        let dev = format!(r"\Device\NPF_WIFI_{{{}}}", guid);
        let dev_w = to_wide(&dev);
        // pcap_open_live es ANSI; usar versión wide no existe → CString
        let dev_c = CString::new(dev).unwrap();
        let mut errbuf = [0 as c_char; 256];
        let h = (open_live)(dev_c.as_ptr(), 65536, 1, 500, errbuf.as_mut_ptr());
        if h.is_null() {
            println!("OPEN-FAIL");
            return;
        }

        let frames: Vec<(&str, Vec<u8>)> = vec![
            ("deauth-AP-nosotros", {
                // Deauth fingiendo ser el AP: RA=nuestra MAC, TA=BSSID impresora.
                // Si sale por la radio, NUESTRO Windows se desconecta (observable
                // garantizado: netsh pasará a desconectado).
                let mut f = vec![0xC0, 0x00, 0x00, 0x00];
                f.extend_from_slice(&macb);                                  // RA = nosotros
                f.extend_from_slice(&[0xf0, 0x92, 0x1c, 0xcf, 0x19, 0x34]); // TA = AP
                f.extend_from_slice(&[0xf0, 0x92, 0x1c, 0xcf, 0x19, 0x34]); // BSSID
                f.extend_from_slice(&[0x03, 0x00]);                          // reason=3
                f
            }),
        ];
        for (tag, frame) in &frames {
            let _ = &dev_w; // silenciar unused
            let rc = (sendpacket)(h, frame.as_ptr(), frame.len() as c_int);
            let e = if rc == 0 { String::new() } else { err_str(h, &wpcap) };
            println!("SEND-ASSOC {} len={} -> rc={} {}", tag, frame.len(), rc, e);
        }
        (close)(h);
    }
}
