// RX por USB crudo (EP 0x81 bulk IN) — captura 802.11 con el MISMO chip WinUSB.
// Port del path RX de rt2x00usb: URBs bulk-in → cada paquete USB contiene
// 1..N frames con formato RT2870: [RXDMA len(4B)] [RXWI (varios, aquí 32B rxd
// compartido)] + 802.11. Para beacons/datos básicos basta parsear el primer
// descriptor: USB DMA length y RXWI wireless header de 32 bytes.
// Uso: rt3070_sniff <segundos> [salida.pcap] [canal]
// Requiere rt3070_init previo (firmware + radio ON + canal). Lab-only.
mod common;

use common::*;
use std::io::Write;
use std::process::exit;

const RXWI_USB_DESC: usize = 4;  // USB RXDMA: bytes = le32 (incluye propia cabecera)
// RT2870 RX packet: [USB_DMA_LEN u32 le][RXWI 32B cuando aggr no][802.11 frame]
// RXWI word0 (le): DATA_BYTE_CNT bits 16-27, TID etc; se usa para validar.

fn now_ts() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64).unwrap_or(0)
}

fn pcap_header() -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes()); // magic (microsegundos)
    h.extend_from_slice(&2u16.to_le_bytes());          // version major
    h.extend_from_slice(&4u16.to_le_bytes());          // version minor
    h.extend_from_slice(&0u32.to_le_bytes());          // thiszone
    h.extend_from_slice(&0u32.to_le_bytes());          // sigfigs
    h.extend_from_slice(&262144u32.to_le_bytes());     // snaplen
    h.extend_from_slice(&127u32.to_le_bytes());        // DLT 127 (IEEE802_11_RADIO)
    h
}

fn pcap_packet(ts_us: u64, data: &[u8]) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&((ts_us / 1_000_000) as u32).to_le_bytes());
    p.extend_from_slice(&((ts_us % 1_000_000) as u32).to_le_bytes());
    p.extend_from_slice(&(data.len() as u32).to_le_bytes());
    p.extend_from_slice(&(data.len() as u32).to_le_bytes());
    p.extend_from_slice(data);
    p
}

/// radiotap mínimo (8 B): header rev0 pad0 len=8, flags present=0x00000000
fn radiotap_min() -> Vec<u8> {
    let mut r = vec![0u8; 8];
    r[0] = 0; // rev
    r[1] = 0; // pad
    r[2..4].copy_from_slice(&8u16.to_le_bytes()); // header len
    // present = 0 → sin campos
    r
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.get(1).map(|s| s.parse().unwrap_or(10)).unwrap_or(10);
    let out_path = args.get(2).cloned().unwrap_or_else(|| format!("sniff_{}.pcap", now_ts()));
    let _chan: u8 = args.get(3).map(|s| s.parse().unwrap_or(11)).unwrap_or(11);

    let h = open_rt3070().expect("abrir RT3070 (usa rt3070_init antes)");

    // RX DMA enable (USB_DMA_CFG: RX bulk + agg). Valor de rt2800usb: 0x9C | TXEN|RXEN
    reg_write(&h, USB_DMA_CFG, 0x0000_009C | (1 << 29) | (1 << 28)).unwrap();
    // MAC_SYS_CTRL: enable RX (0x08) con TX también (0x0C) — radio ya ON tras init
    reg_write(&h, MAC_SYS_CTRL, 0x0C).unwrap();
    // BCN_TIME_CFG=0 (sin beacons propios)
    reg_write(&h, BCN_TIME_CFG, 0).unwrap();
    println!("✅ RX activado (USB_DMA_CFG + MAC_SYS_CTRL=0x0C) — escuchando {secs}s en EP 0x81…");

    let mut file = std::fs::File::create(&out_path).expect("crear pcap");
    file.write_all(&pcap_header()).unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let rt = radiotap_min();
    let mut pkt_count = 0usize;
    let mut buf = vec![0u8; 8192];

    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, std::time::Duration::from_millis(300)) {
            Ok(n) if n >= 4 => {
                let ts = now_ts();
                let mut off = 0usize;
                // Aggregación: múltiples paquetes por URB, cada uno alineado a 4 B
                while off + 8 <= n {
                    let dma = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap()) as usize;
                    if dma < 32 || dma > 4096 { break; } // sanidad (RXWI mín + frame)
                    // El paquete: RXWI(32) + 802.11(len = dma - 32 - 4crc)
                    if off + dma > n { break; }
                    let frame_start = off + 4 + 32;
                    let frame_len = dma.saturating_sub(4 + 32 + 4); // sin USB dma hdr, sin RXWI, sin FCS
                    if frame_len >= 24 && off + 4 + 32 + frame_len <= n {
                        let mut rec = rt.clone();
                        rec.extend_from_slice(&buf[frame_start..frame_start + frame_len]);
                        file.write_all(&pcap_packet(ts, &rec)).unwrap();
                        pkt_count += 1;
                        if pkt_count <= 5 {
                            let fc = u16::from_le_bytes(buf[frame_start..frame_start + 2].try_into().unwrap());
                            println!("  [{pkt_count}] {n} B urb, dma={dma}, FC={fc:#06x} type={} sub={}", fc & 3, (fc >> 4) & 0xF);
                        }
                    }
                    // cada paquete USB aggr va alineado a 4 bytes
                    off += (dma + 3) & !3;
                }
            }
            Ok(_) => {} // corto: ruido
            Err(rusb::Error::Timeout) => {} // normal: sin tráfico
            Err(e) => {
                eprintln!("⚠️ read_bulk: {e:?}");
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    file.flush().ok();
    println!("\n✅ Capturados {pkt_count} frames → {out_path}");
    println!("   Convertir: pcap_to_22000 (nativo) o hcxpcapngtool");
}
