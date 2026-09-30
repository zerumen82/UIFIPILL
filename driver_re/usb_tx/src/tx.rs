// TX de beacons manuales por USB bulk OUT (EP 0x01).
// Formato del frame USB: | TXINFO (4B) | TXWI (20B) | 802.11 beacon | pad |
// Port EXACTO de rt2800usb_write_tx_desc + TXWI de rt2800lib (rev. 2026-09-23:
// el TXWI anterior ponía la longitud en w0 bits16-27 —que son MCS/BW— y el
// TXINFO activaba NEXT_VALID/SW_USE_LAST_ROUND en lugar de WIV/QSEL).
mod common;

use common::*;

// TXINFO word0 (rt2800usb.h): PKT_LEN bits0-15 = TXWI+802.11, WIV bit24,
// QSEL bits25-26 (=2, EDCA), TX_BURST bit31.
fn txinfo(total: u16) -> [u8; 4] {
    let w0: u32 = ((total as u32) & 0xFFFF)        // USB_DMA_TX_PKT_LEN: TXWI+frame
        | (1 << 24)                                 // WIV=1 (IV ya en TXWI, sin cifrar)
        | (2 << 25);                                // QSEL=2 (EDCA queue)
    w0.to_le_bytes()
}

fn build_beacon(ssid: &str, chan: u8, mac: [u8; 6]) -> Vec<u8> {
    let mut f = Vec::new();
    // 802.11 beacon: type 0, subtype 8
    f.extend_from_slice(&[0x80, 0x00]);
    f.extend_from_slice(&[0x00, 0x00]); // duration
    f.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); // DA broadcast
    f.extend_from_slice(&mac); // SA
    f.extend_from_slice(&mac); // BSSID
    f.extend_from_slice(&[0x00, 0x00]); // seq

    // Timestamp (8B, 0) + interval 100 TU + caps ESS/short-preamble
    f.extend_from_slice(&[0u8; 8]);
    f.extend_from_slice(&100u16.to_le_bytes());
    f.extend_from_slice(&0x0021u16.to_le_bytes());

    // IEs
    f.push(0); f.push(ssid.len() as u8); f.extend_from_slice(ssid.as_bytes());
    f.extend_from_slice(&[1, 4, 0x82, 0x84, 0x0B, 0x16]); // rates 1,2,5.5,11
    f.push(3); f.push(1); f.push(chan); // DS parameter set
    f
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ssid = args.get(1).map(|s| s.clone()).unwrap_or_else(|| "UIFIPILL-TX-TEST".into());
    let chan: u8 = args.get(2).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);
    let count: u32 = args.get(3).map(|s| s.parse().unwrap_or(100)).unwrap_or(100);
    let mac = [0xDE, 0xAD, 0xBE, 0xEF, 0x13, 0x37];

    let h = open_rt3070().expect("abrir RT3070 (usa rt3070_init antes)");

    // MAC/BSSID del transmisor (direcciones corregidas 0x1008/0x100C/0x1010/0x1014)
    let dw0: u32 = u32::from_le_bytes(mac[0..4].try_into().unwrap());
    let dw1: u32 = 0x0000_0000 | (mac[4] as u32) | ((mac[5] as u32) << 8);
    reg_write(&h, MAC_ADDR_DW0, dw0).unwrap();
    reg_write(&h, MAC_ADDR_DW1, dw1).unwrap();
    reg_write(&h, MAC_BSSID_DW0, dw0).unwrap();
    reg_write(&h, MAC_BSSID_DW1, dw1).unwrap();

    // Reconfigurar canal (RF real) + PA, por si init fue a otro canal
    if let Err(e) = config_channel_rt3070(&h, chan) { println!("⚠️ canal: {e:?}"); }
    let _ = enable_tx_pa(&h);

    // Frame: TXINFO + TXWI + beacon + pad a 4 bytes
    let beacon = build_beacon(&ssid, chan, mac);
    // PACKETID=1 → el chip reporta el resultado en TX_STA_FIFO (feedback real)
    let txwi = txwi_bytes(beacon.len() as u16, false, 1);
    let total = (4 + 20 + beacon.len()) as u16;

    let mut frame = Vec::new();
    frame.extend_from_slice(&txinfo(total));
    frame.extend_from_slice(&txwi);
    frame.extend_from_slice(&beacon);
    // USB end pad a múltiplo de 4
    while frame.len() % 4 != 0 { frame.push(0); }

    println!("Beacon SSID=\"{ssid}\" chan={chan} len={}", beacon.len());
    println!("Frame TX total = {} bytes → EP 0x01", frame.len());

    // USB DMA TX/RX enable (valor de constantes reales del driver)
    let _ = reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE);
    std::thread::sleep(std::time::Duration::from_millis(10));

    const EP_TX: u8 = 0x01;
    let mut ok = 0;
    let mut last_err: Option<String> = None;
    for i in 0..count {
        match h.write_bulk(EP_TX, &frame, std::time::Duration::from_millis(1000)) {
            Ok(n) => {
                ok += 1;
                if i < 3 || i % 10 == 0 {
                    println!("  [{i}] escrito {n} bytes ✅");
                }
            }
            Err(e) => {
                last_err = Some(format!("{e:?}"));
                if i < 5 {
                    println!("  [{i}] ERR {e:?}");
                }
                // Un timeout aquí puede ser solo backpressure: reintentar en vez de romper
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        // Feedback TXDONE: PACKETID=1 hace que cada frame procesado deje una
        // entrada en TX_STA_FIFO (VALID=1, TX_SUCCESS). Drenar para no llenarla.
        if i % 5 == 4 {
            for line in drain_tx_status(&h, 8) {
                println!("  [status] {line}");
            }
        }
    }
    // Drenado final
    for line in drain_tx_status(&h, 16) {
        println!("  [status] {line}");
    }
    println!("\n{ok}/{count} frames escritos al chip (último error: {:?})", last_err);
    if ok > 0 {
        println!("➡️  Verifica en la OTRA tarjeta: netsh wlan show networks — debe aparecer \"{ssid}\"");
        println!("   (Las líneas [status] TXDONE success=true confirman salida real al aire)");
    }
}
