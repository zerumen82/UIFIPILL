// Diagnóstico NO destructivo: ¿la carga de firmware PEGA de verdad?
//
// 1. Escribe fw (64 chunks MULTI_WRITE).
// 2. READBACK de cada chunk con MULTI_READ sobre FIRMWARE_IMAGE_BASE+off
//    y compara byte a byte.
// 3. NO hace kick → el chip no se degrada aunque todo falle.
//
// Si el readback NO pega: el MCU salta a basura en el kick → el USB muere en
// cada arranque (exactamente el síntoma medido). Si pega: el problema está
// en el kick/arranque en sí bajo WinUSB.
mod common;

use common::*;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fw_path = args.get(1).cloned().unwrap_or_else(|| "rt2870.bin".into());

    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[probe] MAC_CSR0 = {csr0:#010x}");
    if csr0 == 0xffff_ffff || csr0 == 0 {
        println!("❌ chip sordo — haz power-cycle antes de esto");
        std::process::exit(1);
    }

    let fw = std::fs::read(&fw_path).expect("leer rt2870.bin");
    let sec = &fw[FW_OFFSET..FW_OFFSET + FW_LENGTH];

    // ── 1. escribir ──
    let mut ok = 0;
    for (i, chunk) in sec.chunks(64).enumerate() {
        let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
        let (v, idx) = encode_reg_addr(addr);
        if h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, Duration::from_millis(500)).is_ok() { ok += 1; }
        std::thread::sleep(Duration::from_millis(2));
    }
    println!("[write] firmware {ok}/64 chunks");

    // ── 2. readback y comparar ──
    let mut match_ok = 0usize;
    let mut match_fail = 0usize;
    let mut read_fail = 0usize;
    for (i, chunk) in sec.chunks(64).enumerate() {
        let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
        let (v, idx) = encode_reg_addr(addr);
        let mut buf = [0u8; 64];
        match h.read_control(REQ_IN, USB_MULTI_READ, v, idx, &mut buf, Duration::from_millis(500)) {
            Ok(64) => {
                if buf == *chunk { match_ok += 1; }
                else {
                    match_fail += 1;
                    if match_fail <= 3 {
                        let diff: Vec<usize> = (0..64).filter(|&j| buf[j] != chunk[j]).collect();
                        println!("  ✗ chunk {i} (addr {addr:#06x}) difiere en {} bytes: {:?}",
                            diff.len(), &diff[..diff.len().min(8)]);
                        println!("    esperado: {:02x?}…", &chunk[..8.min(chunk.len())]);
                        println!("    leído:    {:02x?}…", &buf[..8]);
                    }
                }
            }
            Ok(n) => { read_fail += 1; if read_fail <= 3 { println!("  ✗ chunk {i}: respuesta corta ({n}B)"); } }
            Err(e) => { read_fail += 1; if read_fail <= 3 { println!("  ✗ chunk {i}: {e:?}"); } }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    println!("\n[readback] {match_ok} idénticos / {match_fail} difieren / {read_fail} sin leer (de 64)");
    if match_ok == 64 {
        println!("✅ FIRMWARE PEGA EN RAM — el problema está en el kick/arranque del MCU");
    } else if match_ok == 0 && read_fail == 0 {
        println!("❌ NADA PEGA: MULTI_READ lee otra cosa (¿zona no accesible a lectura?)");
    } else {
        println!("⚠️ pega parcial — ver arriba");
    }
    println!("\n(sin kick — chip intacto)");

    // ── 3. Tests de patrón para distinguir 'no pega' vs 'no legible' ──
    // (a) ¿el chip sigue vivo?  (b) ¿un patrón escrito en 0x800 lee igual?
    // (c) ¿y en zonas MAC normales?  (d) ¿SINGLE_WRITE vs MULTI_WRITE?
    println!("\n[pattern tests]");
    let csr = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("  MAC_CSR0 tras writes = {csr:#010x} {}", if csr == 0xffff_ffff || csr == 0 { "❌ sordo" } else { "✅ vivo" });

    // (a) patrón en MAC normal (0x1008 MAC_ADDR_DW0) — control positivo conocido
    let _ = reg_write(&h, 0x1008, 0xA5A5_5A5A);
    let rb = reg_read(&h, 0x1008).unwrap_or(0);
    println!("  MAC 0x1008 write/readback = {rb:#010x} {}", if rb == 0xA5A5_5A5A { "✅ PEGA" } else { "❌ NO pega" });

    // (b) patrón MULTI_WRITE en 0x800
    let pat: u32 = 0xDEAD_BEEF;
    let (v, idx) = encode_reg_addr(FIRMWARE_IMAGE_BASE);
    let w = h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, &pat.to_le_bytes(), Duration::from_millis(500)).is_ok();
    let mut buf4 = [0u8; 4];
    let r = h.read_control(REQ_IN, USB_MULTI_READ, v, idx, &mut buf4, Duration::from_millis(500));
    let rb32 = r.map(|_| u32::from_le_bytes(buf4)).unwrap_or(0);
    println!("  0x0800 MULTI write={w} readback = {rb32:#010x} {}", if rb32 == pat { "✅ PEGA Y LEE" } else if rb32 == 0 { "⚠️ lee 0 — write no pega O zona ilegible" } else { "❌ lee otra cosa" });

    // (c) SINGLE_WRITE en 0x800
    let w2 = h.write_control(REQ_OUT, USB_SINGLE_WRITE, v, idx, &0x1234_5678u32.to_le_bytes(), Duration::from_millis(500)).is_ok();
    let r2 = h.read_control(REQ_IN, USB_MULTI_READ, v, idx, &mut buf4, Duration::from_millis(500));
    let rb2 = r2.map(|_| u32::from_le_bytes(buf4)).unwrap_or(0);
    println!("  0x0800 SINGLE write={w2} readback = {rb2:#010x} {}", if rb2 == 0x1234_5678 { "✅ PEGA Y LEE" } else if rb2 == 0 { "⚠️ lee 0" } else { "❌ lee otra cosa" });

    // (d) ¿zonas vecinas? 0x07FC (bajo) y 0x0C00 (dentro de imagen 4096B: 0x800+0x400)
    for addr in [0x07FCu16, 0x0C00, 0x17FC] {
        let (vv, ii) = encode_reg_addr(addr);
        let _ = h.write_control(REQ_OUT, USB_MULTI_WRITE, vv, ii, &pat.to_le_bytes(), Duration::from_millis(500));
        let mut bb = [0u8; 4];
        let rr = h.read_control(REQ_IN, USB_MULTI_READ, vv, ii, &mut bb, Duration::from_millis(500));
        let rbv = rr.map(|_| u32::from_le_bytes(bb)).unwrap_or(0);
        println!("  {addr:#06x} readback = {rbv:#010x} {}", if rbv == pat { "✅ PEGA" } else if rbv == 0 { "⚠️ 0" } else { "❌ otro" });
    }

    println!("\n(sin kick — chip intacto)");
}
