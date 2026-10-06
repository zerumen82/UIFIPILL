// EXPERIMENTO Fase 3 — «el vendor despierta el BBP por otra puerta» (memory:562).
//
// Hipótesis a validar (captura USBPcap 2026-09-30, vendor.pcap):
//  (A) el BBP se accede por MCU (H2M_BBP_AGENT + MCU_BBP_SIGNAL=0x80), no por
//      BBP_CSR_CFG=0x101C → bbp_read(0) debe devolver != 0x00/0xff;
//  (B) MCU_CURRENT es 0x36 (no 0x30 = MCU_SLEEP): el init anterior mandaba un
//      sleep al 8051 justo al acabar;
//  (C) el layout RX bulk IN es RXWI de 20 bytes (FC en offset 20), no
//      [4 dma][32 rxwi] → antes ni contábamos los frames que llegaban.
//
// Fases: (1) sondeo sin tocar la radio  (2) radio SIN kick FIRMWARE(8) (prohibido
// bajo WinUSB, AGENTS.md)  (3) RX con el parser correcto. Evidencia en crudo
// para contrastar con vendor.pcap.
mod common;

use common::*;
use std::time::Duration;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn probe(h: &rusb::DeviceHandle<rusb::Context>) -> bool {
    println!("== FASE 1: sondeo (sin escrituras de init) ==");
    let csr0 = reg_read(h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!(
        "  MAC_CSR0={csr0:#010x} {}",
        if csr0 == 0 || csr0 == 0xffff_ffff { "❌ sordo" } else { "✅" }
    );
    println!("  MAC_SYS_CTRL={:#010x} MAC_STATUS_CFG={:#010x} (busy bits={})",
        reg_read(h, MAC_SYS_CTRL).unwrap_or(0),
        reg_read(h, MAC_STATUS_CFG).unwrap_or(0xffff_ffff),
        reg_read(h, MAC_STATUS_CFG).unwrap_or(0xffff_ffff) & 3);
    println!("  USB_DMA_CFG={:#010x}  BBP_CSR_CFG(0x101C)={:#010x}",
        reg_read(h, USB_DMA_CFG).unwrap_or(0),
        reg_read(h, BBP_CSR_CFG).unwrap_or(0));

    // MCU vivo: MCU_CURRENT (0x36) con TOKEN=0xff — el 8051 debe bajar OWNER.
    print!("  MCU: MCU_CURRENT(0x36) ");
    let mcu_alive = match mcu_request_wait(h, MCU_CURRENT, 0xff, 0, 0, 1200) {
        Ok(true) => { println!("consumido ✅ (8051 corriendo)"); true }
        Ok(false) => { println!("NO consumido ⚠️ (MCU parado o sin fw)"); false }
        Err(e) => { println!("err {e}"); false }
    };

    println!("  -- BBP vía MCU (MCU_BBP_SIGNAL=0x80, la del vendor) --");
    let mut mcu_ok = 0;
    for r in [0u8, 1, 4, 49, 62] {
        match bbp_read(h, r) {
            Ok(v) => {
                let live = v != 0x00 && v != 0xff;
                if live { mcu_ok += 1; }
                println!("    bbp_mcu[{r:2}] = {v:#04x} {}", if live { "✅ VIVO" } else { "❌ 0x00/0xff" });
            }
            Err(e) => println!("    bbp_mcu[{r:2}] err {e:?}"),
        }
    }
    println!("  -- BBP directo (BBP_CSR_CFG 0x101C, la vía Linux) --");
    let mut dir_ok = 0;
    for r in [0u8, 1] {
        match bbp_read_direct(h, r) {
            Ok(v) => {
                let live = v != 0x00 && v != 0xff;
                if live { dir_ok += 1; }
                println!("    bbp_dir[{r}] = {v:#04x} {}", if live { "✅ VIVO" } else { "❌ 0x00/0xff" });
            }
            Err(e) => println!("    bbp_dir[{r}] err {e:?}"),
        }
    }
    println!("  veredicto Fase 1: vía-MCU vivos={mcu_ok}/5  directo vivos={dir_ok}/2");
    println!("  (hipótesis A cumplida si MCU>0 y directo=0)");
    // Transporte BBP elegido por Sonda, no por defecto: si la vía del vendor
    // (MCU_BBP_SIGNAL) no contesta y la directa lee vivo, se usa la directa y
    // se imprime — nada de éxito simulado.
    let via_mcu = mcu_ok > 0;
    bbp_set_via_mcu(via_mcu);
    println!(
        "  transporte BBP para las fases 2/3: {}{}",
        if via_mcu { "MCU (MCU_BBP_SIGNAL)" } else { "DIRECTO (BBP_CSR_CFG)" },
        if via_mcu { "" } else { " — la vía MCU no contestó en la sonda" }
    );
    mcu_alive
}

fn radio(h: &rusb::DeviceHandle<rusb::Context>, chan: u8) -> Result<(), String> {
    println!("\n== FASE 2: radio SIN firmware ni kick FIRMWARE(8) ==");
    // Misma secuencia que rt2800usb_enable_radio / vendorradio (MCU ya vivo):
    // AWAKE → USB_DMA_CFG → init_registers → wait_bbp_rf_ready → H2M clear →
    // BOOT_SIGNAL → init_bbp (vía MCU) → init_rfcsr → MCU_CURRENT → TX/RX → canal.
    match mcu_request_wait(h, MCU_WAKEUP, 0xff, 0, 2, 1000)? {
        true => println!("  MCU_WAKEUP consumido ✅"),
        false => println!("  ⚠️ MCU_WAKEUP sin confirmación"),
    }
    std::thread::sleep(Duration::from_millis(1));

    reg_write(h, USB_DMA_CFG, USB_DMA_CFG_VALUE).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(10));
    println!("  USB_DMA_CFG={:#010x}", USB_DMA_CFG_VALUE);

    init_registers_rt3070(h)?;
    println!("  init_registers completos");

    let _ = wait_busy(h, MAC_STATUS_CFG, 0x0000_0003, 0);
    reg_write(h, H2M_BBP_AGENT, 0).map_err(|e| e.to_string())?;
    reg_write(h, H2M_MAILBOX_CSR, 0).map_err(|e| e.to_string())?;
    reg_write(h, H2M_INT_SRC, 0).map_err(|e| e.to_string())?;
    match mcu_request_wait(h, MCU_BOOT_SIGNAL, 0, 0, 0, 800)? {
        true => println!("  BOOT_SIGNAL consumido ✅"),
        false => println!("  ⚠️ BOOT_SIGNAL sin consumo"),
    }
    std::thread::sleep(Duration::from_millis(1));

    init_bbp_rt3070(h)?;
    println!("  ✅ init_bbp (wait_bbp_ready + 17 regs) vía MCU");
    init_rfcsr_rt3070(h)?;
    println!("  ✅ init_rfcsr (19 regs + calibración, BW20={:#04x})", calib_bw20());

    let _ = mcu_request(h, MCU_CURRENT, 0, 0, 0);
    std::thread::sleep(Duration::from_millis(1));
    reg_write(h, MAC_SYS_CTRL, 0x04).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(1));
    reg_write(h, MAC_SYS_CTRL, 0x0C).map_err(|e| e.to_string())?;
    println!("  MAC enable TX+RX");

    config_channel_rt3070(h, chan).map_err(|e| format!("canal {chan}: {e}"))?;
    enable_tx_pa(h).map_err(|e| format!("PA: {e}"))?;
    reg_write(h, BCN_TIME_CFG, 0).map_err(|e| e.to_string())?;
    rx_filter_monitor(h).map_err(|e| e.to_string())?;
    println!("  canal {chan} + PA + filtro monitor");

    for r in [0u8, 4, 62] {
        match bbp_read(h, r) {
            Ok(v) => println!("  post-init bbp_mcu[{r:2}] = {v:#04x} {}", if v != 0 && v != 0xff { "✅" } else { "❌" }),
            Err(e) => println!("  post-init bbp_mcu[{r:2}] err {e:?}"),
        }
    }
    Ok(())
}

fn rx(h: &rusb::DeviceHandle<rusb::Context>, secs: u64) {
    println!("\n== FASE 3: RX {secs}s (parser RXWI=20B, FC@20) ==");
    // Evidencia de estado: si RX_STA_CNT* sube con el bulk IN vacío, el MAC
    // recibe y el fallo es de USB/DMA; si no sube, es RF/filtro (no RX DMA).
    let regs = |tag: &str| {
        let msc = reg_read(h, MAC_SYS_CTRL).unwrap_or(0xffff_ffff);
        let rxf = reg_read(h, RX_FILTER_CFG).unwrap_or(0xffff_ffff);
        let dma = reg_read(h, USB_DMA_CFG).unwrap_or(0xffff_ffff);
        let c0 = reg_read(h, RX_STA_CNT0).unwrap_or(0xffff_ffff);
        let c1 = reg_read(h, RX_STA_CNT1).unwrap_or(0xffff_ffff);
        let c2 = reg_read(h, RX_STA_CNT2).unwrap_or(0xffff_ffff);
        println!("  [{tag}] MAC_SYS_CTRL={msc:#010x} RX_FILTER_CFG={rxf:#010x} USB_DMA_CFG={dma:#010x}");
        println!("  [{tag}] RX_CNT0={c0:#010x} RX_CNT1={c1:#010x} RX_CNT2={c2:#010x}");
    };
    regs("antes");
    // volcado crudo de los primeros URBs (contrastar con vendor.pcap)
    let mut dumps = 0usize;
    let mut buf = vec![0u8; 8192];
    let t0 = std::time::Instant::now();
    while dumps < 4 && t0.elapsed() < Duration::from_secs(2) {
        match h.read_bulk(0x81, &mut buf, Duration::from_millis(200)) {
            Ok(n) if n >= 48 => {
                dumps += 1;
                println!("  URB#{dumps} n={n}: {}", hex(&buf[..48]));
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(e) => { println!("  read err {e:?}"); break; }
        }
    }
    print_rx(&rx_monitor(h, secs));
    regs("final");
}

/// Fase 4 — diagnóstico RF/canal: lee de vuelta lo programado (RFCSR/BBP) y
/// barre 1/6/11 midiendo si el contador CCA (RX_STA_CNT1) se mueve en 4 s.
/// Devuelve un canal con actividad si lo hay. Nada de éxito simulado: solo
/// lecturas de registros reales del chip.
fn fase4(h: &rusb::DeviceHandle<rusb::Context>, chan: u8) -> Option<u8> {
    println!("\n== FASE 4: diagnóstico RF/canal ==");
    let txpin = reg_read(h, TX_PIN_CFG).unwrap_or(0xffff_ffff);
    let band = reg_read(h, TX_BAND_CFG).unwrap_or(0xffff_ffff);
    let msc = reg_read(h, MAC_SYS_CTRL).unwrap_or(0xffff_ffff);
    println!("  TX_PIN_CFG={txpin:#010x} TX_BAND_CFG={band:#010x} MAC_SYS_CTRL={msc:#010x}");

    // Path 3xxx: N→RFCSR2, K→RFCSR3[3:0], R→RFCSR6[1:0]. RFCSR8 solo como
    // evidencia del ID de versión RF (0x42 fijo — con rf53xx el write de N
    // caía aquí y nunca aterrizaba en el PLL).
    for r in [2u8, 3, 6, 7, 8, 12, 14, 17, 23, 24, 30, 31] {
        match rfcsr_read(h, r) {
            Ok(v) => println!("  RFCSR{r} = {v:#04x}"),
            Err(e) => println!("  RFCSR{r} err {e:?}"),
        }
    }
    match rfcsr_read(h, 1) {
        Ok(v) => println!("  RFCSR1 = {v:#04x} (vendor vivo=0xf1)"),
        Err(e) => println!("  RFCSR1 err {e:?}"),
    }
    // A/B RFCSR2: ¿escribe N y se queda? (path correcto del vendor #184:
    // N=0xF6 para canal 11). Lectura RAW del word para ver REGNUM/DATA/BUSY.
    let _ = rfcsr_write(h, 2, 0xF6);
    std::thread::sleep(Duration::from_millis(10));
    let raw1 = reg_read(h, RF_CSR_CFG).unwrap_or(0xffff_ffff);
    let _ = reg_write(h, RF_CSR_CFG, (2u32 << 8) | (1 << 17));
    std::thread::sleep(Duration::from_millis(5));
    let raw2 = reg_read(h, RF_CSR_CFG).unwrap_or(0xffff_ffff);
    let t1 = rfcsr_read(h, 2).unwrap_or(0xff);
    std::thread::sleep(Duration::from_millis(100));
    let t2 = rfcsr_read(h, 2).unwrap_or(0xff);
    println!("  RFCSR2 A/B: after-write raw={raw1:#010x} readreq-raw={raw2:#010x} → {t1:#04x} (10ms) {t2:#04x} (100ms)");
    let r3ab = rfcsr_read(h, 3).unwrap_or(0xff);
    let r6ab = rfcsr_read(h, 6).unwrap_or(0xff);
    println!("  RFCSR3/6 tras A/B: {r3ab:#04x}/{r6ab:#04x} (esperado 0x3?/0x02 con K/R de ch11)");
    for r in [1u8, 4, 14, 15, 62, 63, 64, 75] {
        match bbp_read(h, r) {
            Ok(v) => println!("  BBP{r:2} = {v:#04x}"),
            Err(e) => println!("  BBP{r:2} err {e:?}"),
        }
    }

    let mut activo = None;
    for c in 1u8..=14 {
        if let Err(e) = config_channel_rt3070(h, c) {
            println!("  canal {c}: config err {e}");
            continue;
        }
        let _ = enable_tx_pa(h);
        // RX_STA_CNT* son clear-on-read: la lectura inicial limpia el acumulado
        let a0 = reg_read(h, RX_STA_CNT1).unwrap_or(0) & 0xffff;
        let b0 = reg_read(h, RX_STA_CNT0).unwrap_or(0) & 0xffff;
        std::thread::sleep(Duration::from_secs(2));
        let a1 = reg_read(h, RX_STA_CNT1).unwrap_or(0) & 0xffff;
        let b1 = reg_read(h, RX_STA_CNT0).unwrap_or(0) & 0xffff;
        let dcca = a1.wrapping_sub(a0);
        let dcrc = b1.wrapping_sub(b0);
        let rf2 = rfcsr_read(h, 2).unwrap_or(0xff);
        let rf3 = rfcsr_read(h, 3).unwrap_or(0xff);
        let rf6 = rfcsr_read(h, 6).unwrap_or(0xff);
        println!("  canal {c}: ΔCCA={dcca} ΔCRC={dcrc}  RFCSR2/3/6={rf2:#04x}/{rf3:#04x}/{rf6:#04x}");
        if dcca > 0 && activo.is_none() {
            activo = Some(c);
        }
    }
    let _ = config_channel_rt3070(h, chan);
    let _ = enable_tx_pa(h);
    match activo {
        Some(c) => println!("  veredicto: RF OYE en el canal {c} → repetir RX ahí"),
        None => println!("  veredicto: ΔCCA=0 en 1..14 → RF sordo (más init no lo arregla)"),
    }
    activo
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let chan: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(11);
    let noinit = args.iter().any(|a| a == "--noinit");

    println!("=== FASE 3: BBP por MCU (protocolo netr28ux) + RX ===");
    println!("  uso: rt3070_bbpmcu [secs] [canal] [--noinit]\n");

    let h = match open_rt3070() {
        Ok(h) => h,
        Err(e) => {
            println!("❌ no se pudo abrir el RT3070 ({e:?}) — ¿está en WinUSB?");
            std::process::exit(1);
        }
    };
    let mcu_alive = probe(&h);
    if noinit {
        println!("\n(--noinit: solo sondeo)");
        return;
    }
    if !mcu_alive {
        println!("\n⚠️ MCU sin confirmar: sin firmware no hay MCU_BBP_SIGNAL posible.");
        println!("   NO se hace kick FIRMWARE(8) (prohibido bajo WinUSB — AGENTS.md).");
        println!("   Plan: power-cycle físico del chip y repetir, o --noinit para solo sondeo.");
    }
    if let Err(e) = radio(&h, chan) {
        println!("❌ radio: {e}");
        std::process::exit(4);
    }
    rx(&h, secs);
    fase4(&h, chan);
}
