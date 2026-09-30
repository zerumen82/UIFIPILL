// EXPERIMENTO DEFINITIVO: boot frío real y NO TOCAR NADA.
//
// Secuencia razonada (todo lo demás ya fue descartado medido):
//  1. UNPLUG(2) → el chip se desconecta y re-enumeración EN FRÍO (medido:
//     funciona; tras UNPLUG el chip vuelve con autoload de fábrica ARRANCANDO).
//  2. Esperar re-enum + estabilización (2 s por si el autoload está subiendo).
//  3. SIN kick, SIN carga de fw, SIN init_registers, SIN resets:
//     SOLO configurar las piezas host-side que Linux configura y el autoload
//     no puede conocer (USB_DMA_CFG con RX_BULK_EN, RX_FILTER monitor,
//     ENABLE_RX) y LEER EP 0x81.
//
// Si el autoload deja el BBP vivo (como debe ser para que el chip pueda
// enchufarse como WiFi en Windows sin driver), los beacons llegarán solos.
mod common;

use common::*;
use rusb::UsbContext;
use std::time::Duration;
use std::collections::HashMap;

fn find_addr() -> Option<u8> {
    let ctx = rusb::Context::new().ok()?;
    for dev in ctx.devices().ok()?.iter() {
        if let Ok(d) = dev.device_descriptor() {
            if d.vendor_id() == VID_RALINK && d.product_id() == PID_RT3070 {
                return Some(dev.address());
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(25);

    println!("=== BOOT FRÍO (UNPLUG) + radio host-side mínima ===\n");
    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    let addr_before = find_addr().unwrap_or(0);
    println!("[pre] MAC_CSR0={csr0:#010x} addr={addr_before}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo antes de empezar — power-cycle");
        std::process::exit(1);
    }

    // ── UNPLUG ──
    println!("[unplug] DEVICE_MODE OUT UNPLUG(2)…");
    match h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_UNPLUG, 0, &[], Duration::from_millis(1000)) {
        Ok(_) => println!("  enviado"),
        Err(e) => println!("  err {e:?} (normal)"),
    }
    drop(h);

    // ── esperar re-enum ──
    println!("[wait] re-enum…");
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    let mut h2 = None;
    while std::time::Instant::now() < deadline {
        match find_addr() {
            Some(a) if a != addr_before => {
                println!("  nueva addr {a} — esperando estabilización 2 s…");
                std::thread::sleep(Duration::from_millis(2000));
                if let Ok(h) = open_rt3070() { h2 = Some(h); }
                break;
            }
            None => { println!("  · fuera del bus"); }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    let h = match h2 {
        Some(h) => h,
        None => { println!("❌ no volvió"); std::process::exit(2); }
    };

    // ── estado del chip frío (solo lectura) ──
    std::thread::sleep(Duration::from_millis(500));
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    let msc = reg_read(&h, MAC_SYS_CTRL).unwrap_or(0xffff_ffff);
    let dma = reg_read(&h, USB_DMA_CFG).unwrap_or(0xffff_ffff);
    println!("[frío] MAC_CSR0={csr0:#010x} MAC_SYS_CTRL={msc:#010x} USB_DMA_CFG={dma:#010x}");
    for r in [0u8, 1, 4] {
        let v = bbp_read(&h, r).unwrap_or(0xff);
        println!("[frío] bbp[{r}] = {v:#04x} {}", if v == 0x00 || v == 0xff { "❌" } else { "✅ VIVO" });
    }
    let b0 = bbp_read(&h, 0).unwrap_or(0xff);
    let bbp_vivo = b0 != 0x00 && b0 != 0xff;

    // ── radio host-side mínima (NADA de init del chip) ──
    if dma == 0 || dma == 0xffff_ffff {
        reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE).ok();
        println!("[set] USB_DMA_CFG");
    }
    if msc != 0xffff_ffff && (msc & 0x08) == 0 {
        reg_write(&h, MAC_SYS_CTRL, msc | 0x08).ok();
        println!("[set] ENABLE_RX");
    }
    let _ = rx_filter_monitor(&h);

    // ── RX ──
    println!("\n== RX {secs}s ==");
    let mut frames = 0usize;
    let mut nets: HashMap<[u8; 6], (String, u8, i32)> = HashMap::new();
    let mut buf = vec![0u8; 8192];
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, Duration::from_millis(100)) {
            Ok(n) if n >= 36 => {
                let mut off = 0usize;
                while off + 36 <= n {
                    let dmalen = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap()) as usize;
                    if dmalen < 36 || dmalen > 4096 { break; }
                    if off + dmalen > n { break; }
                    let wi = off + 4;
                    let fs = wi + 32;
                    let w0 = u32::from_le_bytes(buf[wi..wi + 4].try_into().unwrap());
                    let mpdu = ((w0 >> 16) & 0x0fff) as usize;
                    if mpdu >= 26 && fs + mpdu <= n {
                        frames += 1;
                        let frame = &buf[fs..fs + mpdu.saturating_sub(4)];
                        if frame.len() > 24 && (frame[0] & 0x0f) == 0x80 {
                            let mut i = 24usize;
                            let mut ssid = String::new();
                            let mut ch = 0u8;
                            while i + 2 <= frame.len() {
                                let t = frame[i]; let l = frame[i+1] as usize;
                                if i + 2 + l > frame.len() { break; }
                                if t == 0 && l > 0 && l <= 32 {
                                    ssid = frame[i+2..i+2+l].iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' }).collect();
                                }
                                if t == 3 && l >= 1 { ch = frame[i+2]; }
                                i += 2 + l;
                            }
                            let bssid: [u8; 6] = frame[4..10].try_into().unwrap();
                            let w2 = u32::from_le_bytes(buf[wi+8..wi+12].try_into().unwrap());
                            let rssi = (w2 & 0xff) as i32;
                            nets.entry(bssid).or_insert((ssid, ch, rssi));
                        }
                    }
                    off += (dmalen + 3) & !3;
                }
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => { println!("  read err {e:?}"); break; }
        }
    }

    println!("\n📡 {frames} frames — {}", if frames > 0 {
        "🎉 ¡RX FUNCIONA EN BOOT FRÍO!"
    } else {
        "0 frames"
    });
    if !nets.is_empty() {
        println!("\nAPs:");
        let mut list: Vec<_> = nets.iter().collect();
        list.sort_by_key(|(_, (_, _, r))| std::cmp::Reverse(*r));
        for (bssid, (ssid, ch, rssi)) in list.iter().take(15) {
            let mac: Vec<String> = bssid.iter().map(|b| format!("{b:02X}")).collect();
            println!("AP|{}|{}|ch{}|rssi{}", ssid, mac.join(":"), ch, rssi);
        }
    }
    let _ = bbp_vivo;
}
