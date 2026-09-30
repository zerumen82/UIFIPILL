// RX en la MISMA antena como Linux: aprovechar el init del driver vendor
// netr28ux (que SÍ despierta el BBP — capturas Npcap de 392 pkts lo prueban),
// hacer hot-rebind a WinUSB SIN power-cycle y leer el estado vivo + EP 0x81.
//
// REGLAS de este binario (romperlas = reset = BBP mudo otra vez):
//   1. NUNCA USB_DEVICE_MODE reset
//   2. NUNCA MAC_SYS_CTRL reset (bit0/bit1)
//   3. NUNCA recargar firmware (el MCU ya arrancó)
//   4. Solo LECTURAS salvo consentimiento explícito (--chan)
//
// Uso:
//   rt3070_hotread            → vuelca estado (MAC/BBP/RF/mailbox) y 5s de RX
//   rt3070_hotread --rx 15    → solo RX 15s por EP 0x81
//   rt3070_hotread --chan 6   → cambia canal ANTES de leer RX (escrituras RF)
mod common;

use common::*;
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rx_only = args.iter().any(|a| a == "--rx");
    let rx_secs: u64 = args.iter().position(|a| a == "--rx")
        .and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(10);
    let wake = args.iter().any(|a| a == "--wake");
    let chan: Option<u8> = args.iter().position(|a| a == "--chan")
        .and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok());

    let h = open_rt3070().unwrap_or_else(|e| {
        eprintln!("❌ abrir RT3070: {e} (¿hot-rebind a WinUSB hecho? ver hot_rebind.ps1)");
        std::process::exit(1);
    });

    if !rx_only {
        println!("== Estado VIVO del chip (sin reset, sin firmware) ==\n");
        let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0);
        println!("MAC_CSR0     = {csr0:#010x} (chip={:#06x} rev={:#04x})", csr0 >> 16, csr0 & 0xffff);
        println!("MAC_SYS_CTRL = {:#010x}", reg_read(&h, MAC_SYS_CTRL).unwrap_or(0));
        println!("USB_DMA_CFG  = {:#010x}", reg_read(&h, USB_DMA_CFG).unwrap_or(0));
        println!("BCN_TIME_CFG = {:#010x}", reg_read(&h, BCN_TIME_CFG).unwrap_or(0));
        println!("TX_PIN_CFG   = {:#010x}", reg_read(&h, TX_PIN_CFG).unwrap_or(0));
        println!("RX_FILTER_CFG= {:#010x}", reg_read(&h, RX_FILTER_CFG).unwrap_or(0));
        let mb = reg_read(&h, H2M_MAILBOX_CSR).unwrap_or(0);
        println!("H2M_MAILBOX  = {mb:#010x} (OWNER={})", (mb >> 24) & 0xff);

        println!("\n-- BBP vivo (0,1,4,62) --");
        for r in [0u8, 1, 4, 62] {
            let v = bbp_read(&h, r).unwrap_or(0xff);
            println!("bbp[{r}] = {v:#04x} {}", if v == 0x00 || v == 0xff { "❌ mudo" } else { "✅ VIVO" });
        }
        println!("\n-- RF vivo (0,3,7,30) --");
        for r in [0u8, 3, 7, 30] {
            println!("rfcsr[{r}] = {:#04x}", rfcsr_read(&h, r).unwrap_or(0xff));
        }
    }

    // Despertar el MCU/BBP: el vendor manda MCU_SLEEP al desengancharse y el
    // BBP se duerme (registros MAC/RF quedan igual, por eso parecía "vivo").
    // rt2800usb_set_state STATE_AWAKE: MCU_WAKEUP, token 0xff, arg0=0, arg1=2.
    if wake {
        println!("\n== MCU_WAKEUP (0x31, tok 0xff, arg 0,2) ==");
        match mcu_request_wait(&h, MCU_WAKEUP, 0xff, 0, 2, 1000) {
            Ok(true) => println!("✅ WAKEUP consumido"),
            Ok(false) => println!("⚠️ WAKEUP no consumido"),
            Err(e) => println!("❌ err {e}"),
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        for r in [0u8, 1, 4] {
            let v = bbp_read(&h, r).unwrap_or(0xff);
            println!("bbp[{r}] = {v:#04x} {}", if v == 0x00 || v == 0xff { "❌" } else { "✅ VIVO" });
        }
    }

    // Cambio de canal opcional (escrituras RF — solo con --chan)
    if let Some(ch) = chan {
        println!("\n== Cambio de canal a {ch} (sin reset) ==");
        match config_channel_rt3070(&h, ch) {
            Ok(_) => println!("✅ canal {ch} programado"),
            Err(e) => println!("⚠️ canal: {e}"),
        }
    }

    // RX por EP 0x81 — el mismo mecanismo de scan.rs
    println!("\n== RX {rx_secs}s por EP 0x81 (estado heredado del driver vendor) ==");
    // Habilitar RX si no lo está (lectura+OR, sin reset)
    let msc = reg_read(&h, MAC_SYS_CTRL).unwrap_or(0);
    if msc & 0x08 == 0 {
        println!("  (MAC_SYS_CTRL sin ENABLE_RX — poniéndolo)");
        let _ = reg_write(&h, MAC_SYS_CTRL, msc | 0x08);
    }
    let _ = rx_filter_monitor(&h);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(rx_secs);
    let mut nets: HashMap<[u8; 6], (String, u8, u8)> = HashMap::new();
    let mut frames = 0usize;
    let mut buf = vec![0u8; 8192];

    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, std::time::Duration::from_millis(100)) {
            Ok(n) if n >= 36 => {
                let mut off = 0usize;
                while off + 36 <= n {
                    let dma = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap()) as usize;
                    if dma < 36 || dma > 4096 { break; }
                    if off + dma > n { break; }
                    let wi = off + 4;
                    let fs = wi + 32;
                    let w0 = u32::from_le_bytes(buf[wi..wi + 4].try_into().unwrap());
                    let mpdu = ((w0 >> 16) & 0x0fff) as usize;
                    if mpdu < 26 || fs + mpdu > n { break; }
                    let frame = &buf[fs..fs + mpdu.saturating_sub(4)];
                    frames += 1;
                    if !rx_only {
                        // modo verbose: parsear beacons también
                        let w2 = u32::from_le_bytes(buf[wi + 8..wi + 12].try_into().unwrap());
                        parse_mgmt(frame, (w2 & 0xff) as u8, 11, &mut nets);
                    }
                    off += (dma + 3) & !3;
                }
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => { if !rx_only { println!("  read err {e:?}"); } std::thread::sleep(std::time::Duration::from_millis(20)); }
        }
    }

    println!("\n📡 {frames} frames en {rx_secs}s");
    if !nets.is_empty() {
        let mut list: Vec<_> = nets.iter().collect();
        list.sort_by_key(|(_, (_, _, r))| std::cmp::Reverse(*r));
        for (bssid, (ssid, ch, rssi)) in &list {
            let mac: Vec<String> = bssid.iter().map(|b| format!("{b:02X}")).collect();
            println!("AP|{}|{}|{}|{}", ssid, mac.join(":"), ch, rssi);
        }
    } else if frames == 0 {
        println!("(0 frames — ¿el estado del vendor driver se perdió en el rebind?)");
    }
}

fn parse_mgmt(frame: &[u8], rssi: u8, rx_ch: u8, nets: &mut HashMap<[u8; 6], (String, u8, u8)>) {
    if frame.len() < 24 { return; }
    let fc = u16::from_le_bytes([frame[0], frame[1]]);
    if fc & 0x03 != 0 { return; }
    let st = (fc >> 4) & 0xF;
    if st != 8 && st != 5 { return; }
    let bssid = [frame[10], frame[11], frame[12], frame[13], frame[14], frame[15]];
    let ie_off = 36;
    if frame.len() < ie_off { return; }
    let mut ssid: Option<String> = None;
    let mut chan: u8 = rx_ch;
    let mut i = ie_off;
    while i + 2 <= frame.len() {
        let tag = frame[i];
        let len = frame[i + 1] as usize;
        if i + 2 + len > frame.len() { break; }
        if tag == 0 && len > 0 {
            if let Ok(s) = std::str::from_utf8(&frame[i + 2..i + 2 + len]) {
                if s.chars().all(|c| !c.is_control()) { ssid = Some(s.to_string()); }
            }
        } else if tag == 3 && len >= 1 {
            let c = frame[i + 2];
            if (1..=14).contains(&c) { chan = c; }
        }
        i += 2 + len;
    }
    let e = nets.entry(bssid).or_insert_with(|| (String::new(), chan, 0));
    if let Some(s) = ssid { if e.0.is_empty() { e.0 = s; } }
    if chan >= 1 && chan <= 14 { e.1 = chan; }
    if rssi > e.2 { e.2 = rssi; }
}
