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

#[allow(dead_code)]
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

    // RX: la MISMA secuencia probada por rt3070_scan. init_radio ya configuró
    // USB_DMA_CFG (AGG_LIMIT=301) y MAC enable TX+RX — NO pisarlos aquí (el
    // overwrite previo con 0x9C|(1<<29)|(1<<28) daba ~3 beacons/10s; medido).
    let _ = rx_filter_monitor(&h);
    println!("✅ RX armado por init_radio + filtro monitor — escuchando {secs}s en EP 0x81…");

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
                // Layout MEDIDO (vendor.pcap): [4 len][RXWI 16][802.11 @20],
                // stride = len+8, FCS = últimos 4 bytes. El parser anterior
                // ([4][32 rxwi]) escribía basura en el pcap.
                for f in walk_rx(&buf[..n]) {
                    let frame_len = f.data.len().saturating_sub(4); // sin FCS
                    if frame_len >= 24 {
                        let mut rec = rt.clone();
                        rec.extend_from_slice(&f.data[..frame_len]);
                        file.write_all(&pcap_packet(ts, &rec)).unwrap();
                        pkt_count += 1;
                        if pkt_count <= 5 {
                            let fc = u16::from_le_bytes([f.data[0], f.data[1]]);
                            println!(
                                "  [{pkt_count}] {n} B urb, FC={fc:#06x} type={} sub={} rssi={}",
                                fc & 3,
                                (fc >> 4) & 0xF,
                                f.rssi
                            );
                        }
                    }
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
    if pkt_count == 0 {
        // Éxito = evidencia real (AGENTS.md regla 2): 0 frames NO es éxito.
        // Causas medidas: radio sin init (rt3070_init previo) o canal sin tráfico.
        eprintln!("⚠️ 0 frames en {secs}s: RX no armado (¿rt3070_init previo?) o canal sin tráfico");
        std::process::exit(2);
    }
    println!("   Convertir: pcap_to_22000 (nativo) o hcxpcapngtool");
}
