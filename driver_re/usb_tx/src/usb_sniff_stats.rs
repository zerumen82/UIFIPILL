// usb_sniff_stats.rs - FASE 2 (plan RX-WinUSB): analiza vendor.pcap (USBPcap,
// DLT 249) y extrae el trafico de comandos del vendor netr28ux.
//
// Layout medido 2026-09-30 (no el del header.h upstream): headerLen=28,
//   [0..2] len  [2..10] irpId  [10..14] status  [14..16] function
//   [16] info (0=submission,1=completion)  [17..19] bus  [19..21] device
//   [21] endpoint  [22] transfer  [23..27] dataLen  [27] padding
// Control: en submission el body ES el setup de 8 bytes (+ payload si lo hay);
//          en completion el body ES el valor leido.
//
// Uso: cargo run --bin usb_sniff_stats -- vendor.pcap [--dbg]
mod common;
use std::collections::BTreeMap;
use std::fmt::Write as _;

struct Rec {
    t_us: u64,
    device: u16,
    endpoint: u8,
    transfer: u8,
    info: u8,
    status: i32,
    body: Vec<u8>,
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "vendor.pcap".into());
    let dbg = args.any(|a| a == "--dbg");

    let buf = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("X no se pudo leer {}: {}", path, e);
            std::process::exit(1);
        }
    };
    if buf.len() < 24 {
        eprintln!("X pcap demasiado corto ({} B)", buf.len());
        std::process::exit(1);
    }
    let magic = le32(&buf, 0);
    let swap = match magic {
        0xa1b2c3d4 | 0xa1b23c4d => false,
        0xd4c3b2a1 | 0x4d3cb2a1 => true,
        _ => {
            eprintln!("X magic desconocida {:#010x}", magic);
            std::process::exit(1);
        }
    };
    let rd = |b: &[u8], o: usize| -> u32 {
        let v = le32(b, o);
        if swap {
            v.swap_bytes()
        } else {
            v
        }
    };
    let linktype = rd(&buf, 20);
    println!(
        "== {} ({} B) linktype={}{} ==",
        path,
        buf.len(),
        linktype,
        if linktype == 249 { " (USBPCAP)" } else { " (NO USBPCAP)" }
    );

    let mut recs: Vec<Rec> = Vec::new();
    let mut off = 24usize;
    let mut bad = 0usize;
    while off + 16 <= buf.len() {
        let ts_sec = rd(&buf, off);
        let ts_usec = rd(&buf, off + 4);
        let incl = rd(&buf, off + 8) as usize;
        off += 16;
        if incl == 0 || off + incl > buf.len() {
            break;
        }
        let pkt = &buf[off..off + incl];
        off += incl;
        if pkt.len() < 28 {
            bad += 1;
            continue;
        }
        let hl = le16(pkt, 0) as usize;
        if hl < 27 || hl > pkt.len() {
            bad += 1;
            continue;
        }
        let info = pkt[16];
        let data_len = le32(&pkt, 23) as usize;
        let body_all = &pkt[hl..];
        let take = data_len.min(body_all.len());
        recs.push(Rec {
            t_us: (ts_sec as u64) * 1_000_000 + (ts_usec as u64),
            device: le16(&pkt, 19),
            endpoint: pkt[21],
            transfer: pkt[22],
            info,
            status: le32(&pkt, 10) as i32,
            body: body_all[..take].to_vec(),
        });
    }
    if recs.is_empty() {
        eprintln!("X 0 URBs validos (descartados={}) - captura vacia?", bad);
        std::process::exit(1);
    }
    let t0 = recs[0].t_us;
    let rel = |r: &Rec| (r.t_us - t0) as f64 / 1e6;
    println!(
        "URBs={}  descartados={}  ventana={:.1}s",
        recs.len(),
        bad,
        (recs[recs.len() - 1].t_us - t0) as f64 / 1e6
    );

    if dbg {
        println!("\n== DBG: 20 URBs crudos ==");
        for (i, r) in recs.iter().take(20).enumerate() {
            println!(
                "  #{} t=+{:.3}s dev={} ep=0x{:02x} tr={} info={} st={} body={}",
                i,
                rel(r),
                r.device,
                r.endpoint,
                r.transfer,
                r.info,
                r.status,
                hex(&r.body, 40)
            );
        }
    }

    // --- 1. Resumen por device/EP ------------------------------------------
    println!("\n== 1. Resumen por device/EP (0=isoc 1=int 2=ctrl 3=bulk) ==");
    let mut agg: BTreeMap<(u16, u8, u8, u8), (usize, usize)> = BTreeMap::new();
    for r in &recs {
        let d = if r.info == 0 { 0u8 } else { 1u8 };
        let e = agg.entry((r.device, r.endpoint, r.transfer, d)).or_insert((0, 0));
        e.0 += 1;
        e.1 += r.body.len();
    }
    for ((dev, ep, tr, d), (n, b)) in &agg {
        println!(
            "  dev={:<3} EP=0x{:02x} tr={} {}  pkts={:<6} bytes={}",
            dev,
            ep,
            tr,
            if *d == 0 { "SUB" } else { "CMP" },
            n,
            b
        );
    }

    // --- 2. Detectar el RT3070: vendor requests 0x40/0xC0 bRequest 2/3/6/7 --
    let is_vendor_setup = |r: &Rec| -> Option<(u8, u8, u16, u16, u16)> {
        if r.transfer != 2 || r.info != 0 || r.body.len() < 8 {
            return None;
        }
        let bm = r.body[0];
        if bm != 0x40 && bm != 0xC0 {
            return None;
        }
        let breq = r.body[1];
        if !matches!(breq, 2 | 3 | 6 | 7) {
            return None;
        }
        Some((
            bm,
            breq,
            le16(&r.body, 2),
            le16(&r.body, 4),
            le16(&r.body, 6),
        ))
    };
    let mut votes: BTreeMap<u16, usize> = BTreeMap::new();
    for r in &recs {
        if is_vendor_setup(r).is_some() {
            *votes.entry(r.device).or_insert(0) += 1;
        }
    }
    let rt = votes.iter().max_by_key(|(_, n)| **n).map(|(d, _)| *d);
    match rt {
        Some(d) => println!(
            "\n== 2. RT3070 = device {} ({} vendor requests) ==",
            d,
            votes[&d]
        ),
        None => println!("\n== 2. No se detecto RT3070 por vendor request =="),
    }
    if votes.len() > 1 {
        for (d, n) in &votes {
            println!("    (candidato dev={} n={})", d, n);
        }
    }
    let rt = rt.unwrap_or(0);

    // --- 3. Timeline de operaciones de registro del vendor ------------------
    // Pareamos cada SUB control IN con su primer CMP posterior (mismo EP): el
    // vendor es sincronono, así que el orden basta.
    println!("\n== 3. Timeline vendor (submission) para dev={} ==", rt);
    let mut ops: Vec<usize> = (0..recs.len())
        .filter(|&i| recs[i].device == rt && is_vendor_setup(&recs[i]).is_some())
        .collect();
    ops.sort_by_key(|&i| recs[i].t_us);
    let mut paired: BTreeMap<usize, usize> = BTreeMap::new();
    for &i in &ops {
        let r = &recs[i];
        if r.body.is_empty() || r.body[0] != 0xC0 {
            continue;
        }
        for (j, c) in recs.iter().enumerate().skip(i + 1) {
            if c.info == 1 && c.transfer == 2 && c.endpoint == r.endpoint && c.device == rt {
                paired.insert(i, j);
                break;
            }
        }
    }
    let mcu_name = |c: u8| -> &'static str {
        match c {
            0x30 => "MCU_SLEEP",
            0x31 => "MCU_WAKEUP",
            0x35 => "MCU_RADIO_OFF",
            0x36 => "MCU_CURRENT",
            0x50 => "MCU_LED",
            0x60 => "MCU_RADAR",
            0x72 => "MCU_BOOT_SIGNAL",
            0x73 => "MCU_ANT_SELECT",
            0x74 => "MCU_FREQ_OFFSET",
            0x80 => "MCU_BBP_SIGNAL",
            0x83 => "MCU_POWER_SAVE",
            0x91 => "MCU_BAND_SELECT",
            _ => "MCU_?",
        }
    };
    let mut agent_lo: Option<u16> = None;
    let mut agent_hi: Option<u16> = None;
    let mut mbox_lo: u16 = 0;
    let mut mbox_hi: u16 = 0;
    let mut bbp_ops: Vec<(bool, u8, u8, f64)> = Vec::new();
    let mut last_t = f64::MAX;
    let mut idx = 0usize;
    for &i in &ops {
        let r = &recs[i];
        let (bm, breq, wval, widx, wlen) = is_vendor_setup(r).unwrap();
        let t = rel(r);
        if last_t != f64::MAX && t - last_t > 0.30 {
            println!("  ---- pausa {:.2}s ----", t - last_t);
        }
        last_t = t;
        let rd_val = paired.get(&i).map(|&j| hex(&recs[j].body, recs[j].body.len()));
        let mut line = match (bm, breq) {
            (0x40, 2) => {
                if widx == 0x7028 {
                    agent_lo = Some(wval);
                }
                if widx == 0x702a {
                    agent_hi = Some(wval);
                }
                if widx == 0x7010 {
                    mbox_lo = wval;
                }
                if widx == 0x7012 {
                    mbox_hi = wval;
                }
                if widx == 0x0404 && wval == 0x0080 {
                    let lo = agent_lo.unwrap_or(0);
                    let hi = agent_hi.unwrap_or(0);
                    let word = ((hi as u32) << 16) | lo as u32;
                    let (val, reg, flags) = ((word & 0xff) as u8, ((word >> 8) & 0xff) as u8, (word >> 16) as u8);
                    bbp_ops.push((flags & 1 != 0, reg, val, t));
                    format!(
                        "W16 reg=0x{:04x} <= 0x{:04x}   CMD 0x80 MCU_BBP_SIGNAL  BBP {} reg={} val=0x{:02x} flags=0x{:02x}",
                        widx,
                        wval,
                        if flags & 1 != 0 { "READ " } else { "WRITE" },
                        reg,
                        val,
                        flags
                    )
                } else if widx == 0x0404 {
                    format!(
                        "W16 reg=0x{:04x} <= 0x{:04x}   CMD 0x{:02x} {}  mailbox=0x{:04x}{:04x} (arg1=0x{:02x} tok=0x{:02x} owner=0x{:02x} arg0=0x{:02x})",
                        widx, wval, wval as u8, mcu_name(wval as u8),
                        mbox_hi, mbox_lo,
                        (mbox_lo >> 8) as u8, mbox_hi as u8, (mbox_hi >> 8) as u8, mbox_lo as u8
                    )
                } else if widx == 0x7028 || widx == 0x702a {
                    format!("W16 reg=0x{:04x} <= 0x{:04x}   (H2M_BBP_AGENT)", widx, wval)
                } else {
                    format!("W16 reg=0x{:04x} <= 0x{:04x}", widx, wval)
                }
            }
            (0x40, 6) => format!("WM   reg=0x{:04x} len={} {}", widx, wlen, hex(&r.body[8..], 16)),
            (0xC0, 2 | 3 | 7) => format!("R    reg=0x{:04x}", widx),
            _ => format!("bReq={} wValue=0x{:04x} wIndex=0x{:04x}", breq, wval, widx),
        };
        if bm == 0xC0 {
            match &rd_val {
                Some(v) => {
                    let _ = write!(line, " -> {}", v);
                    if widx == 0x7010 {
                        if let Some(hexs) = rd_val.as_ref() {
                            let bytes: Vec<&str> = hexs.split(' ').collect();
                            if bytes.len() >= 4 {
                                let word = u32::from_le_bytes([
                                    u8::from_str_radix(bytes[0], 16).unwrap_or(0),
                                    u8::from_str_radix(bytes[1], 16).unwrap_or(0),
                                    u8::from_str_radix(bytes[2], 16).unwrap_or(0),
                                    u8::from_str_radix(bytes[3], 16).unwrap_or(0),
                                ]);
                                let _ = write!(
                                    line,
                                    "  [mailbox owner=0x{:02x} tok=0x{:02x} arg1=0x{:02x} arg0=0x{:02x}]",
                                    (word >> 24) & 0xff,
                                    (word >> 16) & 0xff,
                                    (word >> 8) & 0xff,
                                    word & 0xff
                                );
                            }
                        }
                    }
                }
                None => line.push_str(" -> ??"),
            }
        }
        println!("  #{:<4} t=+{:<8.3} {}", idx, t, line);
        idx += 1;
        if idx >= 600 {
            println!("  ... (corte en 600 de {})", ops.len());
            break;
        }
    }

    // --- 3b. Resumen de operaciones BBP vía MCU ------------------------------
    println!("\n== 3b. BBP via MCU_BBP_SIGNAL: {} ops ==", bbp_ops.len());
    let mut seen: Vec<(bool, u8, u8)> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    for (rd, reg, val, _) in &bbp_ops {
        match seen.iter().position(|s| s.0 == *rd && s.1 == *reg && s.2 == *val) {
            Some(k) => counts[k] += 1,
            None => {
                seen.push((*rd, *reg, *val));
                counts.push(1);
            }
        }
    }
    for (k, (rd, reg, val)) in seen.iter().enumerate() {
        println!(
            "  x{:<3} {} reg={:<4} val=0x{:02x}",
            counts[k],
            if *rd { "READ " } else { "WRITE" },
            reg,
            val
        );
    }

    // --- 4. Fases por densidad (boot vs estable) ----------------------------
    let mut buckets: BTreeMap<u64, usize> = BTreeMap::new();
    for &i in &ops {
        *buckets.entry((rel(&recs[i]) / 1.0) as u64).or_insert(0) += 1;
    }
    println!("\n== 4. densidad de vendor requests por segundo ==");
    for (s, n) in &buckets {
        println!("  t={:>4}s  {:>4} {}", s, n, "#".repeat((*n).min(80)));
    }

    // --- 5. Bulk del RT3070: la via RX --------------------------------------
    let bulk_in: Vec<&Rec> = recs
        .iter()
        .filter(|r| r.device == rt && r.transfer == 3 && r.endpoint & 0x80 != 0)
        .collect();
    let bulk_out: Vec<&Rec> = recs
        .iter()
        .filter(|r| r.device == rt && r.transfer == 3 && r.endpoint & 0x80 == 0)
        .collect();
    let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
    for r in &bulk_in {
        *sizes.entry(r.body.len()).or_insert(0) += 1;
    }
    println!(
        "\n== 5. bulk IN (RX)={} pkts/{} B   bulk OUT (TX)={} pkts/{} B ==",
        bulk_in.len(),
        bulk_in.iter().map(|r| r.body.len()).sum::<usize>(),
        bulk_out.len(),
        bulk_out.iter().map(|r| r.body.len()).sum::<usize>()
    );
    let mut sv: Vec<(usize, usize)> = sizes.into_iter().collect();
    sv.sort_by(|a, b| b.1.cmp(&a.1));
    println!(
        "  tamanios IN: {}",
        sv.iter()
            .take(12)
            .map(|(l, c)| format!("{}x{}", l, c))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!("  primeros 8 bulk IN (48 B):");
    for r in bulk_in.iter().take(8) {
        println!("    t=+{:.3}s {}", rel(r), hex(&r.body, 48));
    }
    // buscamos cabeceras 802.11 (FC tipo mgmt 0x00/0x08 y subtype beacon 8)
    let mut found_beacon = 0usize;
    for r in &bulk_in {
        for w in r.body.windows(2) {
            let fc = le16(w, 0);
            if fc & 0x00fc == 0x0080 && (fc & 0x0c00) == 0 {
                found_beacon += 1;
                break;
            }
        }
    }
    println!(
        "  paquetes IN que contienen un FC de beacon (0x80): {}",
        found_beacon
    );

    // --- 5b. Layout exacto de los bulk IN (prefix + agregacion) -------------
    // Pregunta: FC en que offset? hay mas de un frame por URB? que campo da
    // la longitud?  Esto fija el parser de scan/sniff/rx_monitor.
    println!("\n== 5b. layout bulk IN (dev={}) ==", rt);
    let plausible_fc = |b: &[u8], off: usize| -> bool {
        if off + 24 > b.len() {
            return false;
        }
        let fc = le16(b, off);
        let typ = (fc >> 2) & 3;
        let sub = (fc >> 4) & 0xf;
        if typ > 2 {
            return false;
        }
        if typ == 0 && sub > 13 {
            return false;
        }
        if typ == 1 && sub > 9 {
            return false;
        }
        if typ == 2 && sub > 7 {
            return false;
        }
        // direccion 1 tiene que parecer MAC
        !(b[off + 4] == 0 && b[off + 5] == 0 && b[off + 6] == 0)
    };
    let mut fc_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut rel_w0: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut multi = 0usize;
    let mut examples: Vec<String> = Vec::new();
    for r in &bulk_in {
        let b = &r.body;
        if b.len() < 24 {
            continue;
        }
        let offs: Vec<usize> = (0..=b.len() - 24).filter(|&o| plausible_fc(b, o)).collect();
        let mut uniq: Vec<usize> = Vec::new();
        for o in offs {
            if uniq.last().map_or(true, |p| o > *p + 4) {
                uniq.push(o);
            }
        }
        if uniq.len() > 1 {
            multi += 1;
        }
        if let Some(&o) = uniq.first() {
            *fc_hist.entry(o).or_insert(0) += 1;
        }
        let w0 = le32(b, 0) as usize;
        let n = b.len();
        for (name, ok) in [
            ("w0+8==n", w0 + 8 == n),
            ("w0+4==n", w0 + 4 == n),
            ("w0==n", w0 == n),
            ("w0+20==n", w0 + 20 == n),
        ] {
            if ok {
                *rel_w0.entry(name).or_insert(0) += 1;
            }
        }
        if examples.len() < 8 {
            examples.push(format!(
                "    n={:<5} w0={:#06x} fc@{:?}",
                n,
                w0,
                &uniq[..uniq.len().min(4)]
            ));
        }
    }
    println!("  URBs IN analizados={}", bulk_in.len());
    println!("  histograma offset del 1er FC: {:?}", fc_hist);
    println!("  URBs con >1 FC plausible (agregacion): {}", multi);
    println!("  relacion w0 vs longitud del URB: {:?}", rel_w0);
    println!("  ejemplos:");
    for e in &examples {
        println!("{}", e);
    }
    // deltas entre FC consecutivos cuando hay varios
    let mut deltas: BTreeMap<usize, usize> = BTreeMap::new();
    for r in bulk_in.iter().take(40) {
        let b = &r.body;
        if b.len() < 24 {
            continue;
        }
        let offs: Vec<usize> = (0..=b.len() - 24)
            .filter(|&o| plausible_fc(b, o))
            .collect();
        let mut uniq: Vec<usize> = Vec::new();
        for o in offs {
            if uniq.last().map_or(true, |p| o > *p + 4) {
                uniq.push(o);
            }
        }
        for w in uniq.windows(2) {
            *deltas.entry(w[1] - w[0]).or_insert(0) += 1;
        }
    }
    if !deltas.is_empty() {
        println!("  deltas entre FC consecutivos: {:?}", deltas);
    }
    // Estricto: FC de beacon EXACTO (0x0080) — para fijar el offset real del
    // 802.11 sin falsos positivos del heuristico de arriba.
    let mut beacon_off: BTreeMap<usize, usize> = BTreeMap::new();
    println!("  primeros 6 URBs (le32 en 0/4/8/12/16 + offset beacon):");
    for (i, r) in bulk_in.iter().take(6).enumerate() {
        let b = &r.body;
        if b.len() < 40 {
            continue;
        }
        let mut bo = None;
        for off in 0..=b.len() - 2 {
            if le16(b, off) == 0x0080 {
                bo = Some(off);
                break;
            }
        }
        if let Some(o) = bo {
            *beacon_off.entry(o).or_insert(0) += 1;
        }
        println!(
            "    #{} n={:<5} w@0={:#010x} w@4={:#010x} w@8={:#010x} w@12={:#010x} w@16={:#010x} beacon@{:?}",
            i,
            b.len(),
            le32(b, 0),
            le32(b, 4),
            le32(b, 8),
            le32(b, 12),
            le32(b, 16),
            bo
        );
    }
    let mut beacon_all: BTreeMap<usize, usize> = BTreeMap::new();
    for r in &bulk_in {
        let b = &r.body;
        for off in 0..=b.len().saturating_sub(2) {
            if le16(b, off) == 0x0080 {
                *beacon_all.entry(off).or_insert(0) += 1;
                break;
            }
        }
    }
    println!("  histograma beacon exacto 0x0080 (primeras 6): {:?}", beacon_off);
    println!("  histograma beacon exacto 0x0080 (todos): {:?}", beacon_all);

    // --- 5c. Validacion OFFLINE del parser (common::walk_rx) sobre la captura
    // Esto comprueba el parser SIN hardware: si aqui salen ~84 beacons, el
    // mismo codigo contara frames cuando el chip este vivo.
    println!("\n== 5c. parser walk_rx sobre la captura (validacion offline) ==");
    let mut pf = 0usize;
    let mut pbeacon = 0usize;
    let mut paps: BTreeMap<Vec<u8>, (String, u8)> = BTreeMap::new();
    for r in &bulk_in {
        for fr in common::walk_rx(&r.body) {
            pf += 1;
            if let Some((ssid, ch, bssid)) = common::parse_beacon(fr.data) {
                pbeacon += 1;
                paps.entry(bssid.to_vec()).or_insert((ssid, ch));
            }
        }
    }
    println!(
        "  frames={} beacons={} URBs={}  offset del 1er frame: se asume 20",
        pf,
        pbeacon,
        bulk_in.len()
    );
    for (bssid, (ssid, ch)) in paps.iter().take(15) {
        let mac = bssid.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":");
        println!("    AP|{ssid}|{mac}|ch{ch}");
    }

    // --- 6. Control IN (otras requests no-vendor) ---------------------------
    let others: Vec<&Rec> = recs
        .iter()
        .filter(|r| r.device == rt && r.transfer == 2 && r.info == 0)
        .filter(|r| is_vendor_setup(r).is_none())
        .collect();
    println!("\n== 6. control submissions NO-vendor para dev={} : {} ==", rt, others.len());
    let mut seen: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    for r in &others {
        *seen.entry(r.body.clone()).or_insert(0) += 1;
    }
    for (b, n) in seen.iter().take(20) {
        println!("  x{:<4} {}", n, hex(b, b.len()));
    }
    println!("\nOK. Siguiente: comparar con common::init_radio (Fase 3).");
}

fn hex(b: &[u8], n: usize) -> String {
    let n = n.min(b.len());
    let mut s = String::with_capacity(n * 3);
    for (i, x) in b[..n].iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{:02x}", x);
    }
    if n < b.len() {
        s.push_str(" ...");
    }
    s
}
