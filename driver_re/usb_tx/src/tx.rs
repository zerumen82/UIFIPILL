// TX de beacons manuales por USB bulk OUT (EP 0x01).
// Formato del frame: | TXINFO (4B) | TXWI (20B) | 802.11 beacon |
// Port de rt2800usb_write_tx_desc + TXWI de rt2800lib.
mod common;

use common::*;

// TXWI word0: OFDM 6Mbps (PHY mode 0, MCS 0), len en bits 16-27
fn txwi(wifi_len: u16) -> [u8; 20] {
    let mut w = [0u8; 20];
    // w0: MPDU Total Byte Count (bits 16-27), PHY=0 (CCK), MCS=0 (1Mbps)
    let w0: u32 = (wifi_len as u32) << 16 | 0x0000_0000;
    w[0..4].copy_from_slice(&w0.to_le_bytes());
    // w1: CF-END? no. ACK=0, TS=0, OFDM? manten CCK 1Mbps para máxima compat
    let w1: u32 = 0;
    w[4..8].copy_from_slice(&w1.to_le_bytes());
    // w2: IV sin cifrar, todo 0
    w
}

// TXINFO word0: USB_DMA_TX_PKT_LEN (TXWI+802.11), WIV=1, QSEL=2
fn txinfo(total: u16) -> [u8; 4] {
    let w0: u32 = ((total as u32 - 4) & 0xFFFF) | (1 << 30) | (2 << 26);
    w0.to_le_bytes()
}

fn build_beacon(ssid: &str, chan: u8, mac: [u8; 6]) -> Vec<u8> {
    let mut f = Vec::new();
    // 802.11 beacon: type 0, subtype 8
    f.extend_from_slice(&[0x80, 0x00]);
    f.extend_from_slice(&[0x00, 0x00]); // duration
    f.extend_from_slice(&mac); // DA (broadcast)
    f.extend_from_slice(&mac); // SA (BSSID)
    f.extend_from_slice(&mac); // BSSID
    f.extend_from_slice(&[0x00, 0x00]); // seq

    // Timestamp (8B, 0)
    f.extend_from_slice(&[0u8; 8]);
    f.extend_from_slice(&100u16.to_le_bytes()); // beacon interval 100 TU
    f.extend_from_slice(&0x0021u16.to_le_bytes()); // cap: ESS, short preamble

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

    // MAC/BSSID del transmisor
    let dw0: u32 = u32::from_le_bytes(mac[0..4].try_into().unwrap());
    let dw1: u32 = 0x0000_0000 | (mac[4] as u32) | ((mac[5] as u32) << 8);
    reg_write(&h, MAC_ADDR_DW0, dw0).unwrap();
    reg_write(&h, MAC_ADDR_DW1, dw1).unwrap();
    reg_write(&h, MAC_BSSID_DW0, dw0).unwrap();
    reg_write(&h, MAC_BSSID_DW1, dw1).unwrap();

    let beacon = build_beacon(&ssid, chan, mac);
    let txwi = txwi(beacon.len() as u16);

    let mut frame = Vec::new();
    frame.extend_from_slice(&txinfo((4 + 20 + beacon.len()) as u16));
    frame.extend_from_slice(&txwi);
    frame.extend_from_slice(&beacon);
    // USB end pad 4 bytes
    frame.extend_from_slice(&[0u8; 4]);

    println!("Beacon SSID=\"{ssid}\" chan={chan} len={}", beacon.len());
    println!("Frame TX total = {} bytes → EP 0x01", frame.len());

    // TX config previa (rt2800usb_start): USB_DMA_CFG con TX/RX bulk enable
    // bit29 TXbulk? En RT2800: TX_EN=bit29? Formal: RX_DMA_EN bit28?TX_DMA_EN bit29
    // valor clásico de driver: 0x0000007C | agg. Reutilizamos el de init.rs
    let _ = common::reg_write(&h, common::USB_DMA_CFG, 0x0000_009C | (1 << 29) | (1 << 28));
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Endpoint bulk OUT: el primer EP OUT bulk de la interfaz 0 (0x01 en RT2870)
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
    }
    println!("\n{ok}/{count} frames escritos al chip (último error: {:?})", last_err);
    if ok > 0 {
        println!("➡️  Verifica en la OTRA tarjeta: netsh wlan show networks — debe aparecer \"{ssid}\"");
        println!("   (TX status: lee TX_STA_FIFO 0x1718 con rt3070_probe para confirmar ACK/hora)");
    }
}
