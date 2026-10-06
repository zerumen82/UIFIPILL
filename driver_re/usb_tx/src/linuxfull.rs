// Port completo de la secuencia rt2800usb (Linux 6.6) a Windows/WinUSB.
// Modos:
//   rt3070_linuxfull [chan] [fw]            — flujo completo (fw + kick + re-enum + radio)
//   rt3070_linuxfull --resume [chan]        — port reset de software; si el CSR responde,
//                                             el firmware ya corre → directo a la radio.
mod common;

use common::*;
use std::time::Duration;

fn reopen_after_reenum(secs: u64) -> Option<rusb::DeviceHandle<rusb::Context>> {
    // FASE 1: esperar a que el device DESAPAREZCA (el MCU resetea su USB).
    // Sin esto la reapertura instantánea es un falso positivo (medido 2026-09-25).
    // FASE 2: al reaparecer, si el control pipe está sordo (normal tras re-enum
    // en Windows), port reset de software (h.reset()) hasta que hable.
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    let mut disappeared = false;
    while std::time::Instant::now() < deadline {
        match open_rt3070() {
            Ok(h) => {
                if !disappeared { drop(h); }
                else {
                    // ¿vivo? leer MAC_CSR0 con timeout corto
                    let mut buf = [0u8; 4];
                    match h.read_control(REQ_IN, USB_MULTI_READ, 0, MAC_CSR0, &mut buf, Duration::from_millis(700)) {
                        Ok(4) => {
                            println!("  ✅ device de vuelta y RESPONDE (MAC_CSR0={:#010x})", u32::from_le_bytes(buf));
                            return Some(h);
                        }
                        _ => {
                            // sordo: port reset de software y reintentar
                            let _ = h.reset();
                            drop(h);
                            std::thread::sleep(Duration::from_millis(2000));
                        }
                    }
                }
            }
            Err(_) => { disappeared = true; }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    None
}

// vendor_request_sw con reintentos (rt2x00usb_vendor_request loop): reintenta
// la escritura hasta agotar el presupuesto total en ms.
fn vendor_sw_retry(h: &rusb::DeviceHandle<rusb::Context>, req: u8, value: u16, total_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_millis(total_ms);
    while std::time::Instant::now() < deadline {
        if h.write_control(REQ_OUT, req, value, 0, &[], Duration::from_millis(200)).is_ok() {
            return true;
        }
    }
    false
}

fn wait_csr_ready(h: &rusb::DeviceHandle<rusb::Context>, tries: usize, ms: u64) -> Option<u32> {
    for i in 0..tries {
        let v = reg_read(h, MAC_CSR0).unwrap_or(0);
        if v != 0 && v != 0xffff_ffff {
            println!("  csr_ready (i={i}) = {v:#010x}");
            return Some(v);
        }
        std::thread::sleep(Duration::from_millis(ms));
    }
    None
}

// ══ Cola común: drv_init_registers → enable_radio → RX TEST ══
// Requiere CSR ya ready (firmware corriendo).
fn after_csr_ready(h: rusb::DeviceHandle<rusb::Context>, chan: u8) {
    println!("\n[drv_init_registers]");
    // PBF_SYS_CTRL &= ~0x2000
    let _ = reg_write(&h, PBF_SYS_CTRL, reg_read(&h, PBF_SYS_CTRL).unwrap_or(0) & !0x0000_2000);
    // MAC_SYS_CTRL = RESET_CSR|RESET_BBP (0x3) — AHORA es legal: el MCU corre
    let _ = reg_write(&h, MAC_SYS_CTRL, 0x0000_0003);
    // USB_DEVICE_MODE reset (wValue=1, con reintentos)
    let _ = vendor_sw_retry(&h, USB_DEVICE_MODE, USB_MODE_RESET, 200);
    std::thread::sleep(Duration::from_millis(10));
    let _ = reg_write(&h, MAC_SYS_CTRL, 0x0000_0000);
    println!("  segundo reset MAC+BBP hecho (como rt2800usb_init_registers)");

    // ══ rt2800usb_set_device_state(STATE_RADIO_ON): MCU_WAKEUP ANTES de la radio ══
    println!("\n[set_state AWAKE]");
    match mcu_request_wait(&h, MCU_WAKEUP, 0xff, 0, 2, 1000) {
        Ok(true) => println!("  MCU_WAKEUP consumido ✅"),
        Ok(false) => println!("  ⚠️ MCU_WAKEUP sin confirmación"),
        Err(e) => println!("  ⚠️ MCU_WAKEUP err: {e}"),
    }
    std::thread::sleep(Duration::from_millis(1));

    // ══ rt2800usb_enable_radio → USB_DMA_CFG + rt2800_enable_radio ══
    println!("\n[enable_radio]");
    reg_write(&h, USB_DMA_CFG, USB_DMA_CFG_VALUE).expect("USB_DMA_CFG");
    std::thread::sleep(Duration::from_millis(10));
    println!("  USB_DMA_CFG");
    init_registers_rt3070(&h).expect("init_registers");
    println!("  init_registers completos");
    let _ = wait_busy(&h, MAC_STATUS_CFG, 0x0000_0003, 0);
    reg_write(&h, H2M_BBP_AGENT, 0).unwrap();
    reg_write(&h, H2M_MAILBOX_CSR, 0).unwrap();
    reg_write(&h, H2M_INT_SRC, 0).unwrap();
    let _ = mcu_request(&h, MCU_BOOT_SIGNAL, 0, 0, 0);
    std::thread::sleep(Duration::from_millis(1));
    println!("  BOOT_SIGNAL (enable_radio)");
    match init_bbp_rt3070(&h) {
        Ok(_) => println!("  ✅ BBP init OK — ¡wait_bbp_ready pasó!"),
        Err(e) => { println!("  ❌ BBP: {e}"); std::process::exit(4); }
    }
    match init_rfcsr_rt3070(&h) {
        Ok(_) => println!("  RFCSR init OK"),
        Err(e) => { println!("  ❌ RFCSR: {e}"); std::process::exit(4); }
    }
    let _ = mcu_request(&h, MCU_CURRENT, 0, 0, 0);
    // enable TX luego RX
    reg_write(&h, MAC_SYS_CTRL, 0x04).unwrap();
    std::thread::sleep(Duration::from_millis(1));
    reg_write(&h, MAC_SYS_CTRL, 0x0C).unwrap();
    println!("  MAC enable TX+RX");
    // canal + PA (config_channel tail)
    config_channel_rt3070(&h, chan).expect("canal");
    enable_tx_pa(&h).unwrap();
    reg_write(&h, BCN_TIME_CFG, 0).unwrap();
    let _ = rx_filter_monitor(&h);
    println!("  canal {chan} + PA + RX monitor");

    // ══ RX TEST (parser MEDIDO: RXWI 20B, FC@20 — el anterior asumía [4][32]
    // y contaba 0 frames aunque el chip entregara datos) ══
    println!("\n== RX 15s EP 0x81 ==");
    print_rx(&rx_monitor(&h, 15));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let resume = args.iter().any(|a| a == "--resume");
    let chan: u8 = args.get(1).and_then(|s| s.parse().ok())
        .or_else(|| args.get(2).and_then(|s| s.parse().ok()))
        .unwrap_or(11);
    let fw_path = args.iter().skip(1)
        .find(|s| !s.starts_with("--") && s.parse::<u8>().is_err())
        .cloned().unwrap_or_else(|| "rt2870.bin".into());

    println!("=== rt2800usb FULL PORT (Linux 6.6) ===\n");

    if resume {
        // ── MODO RESUME: port reset de software; si el CSR responde, el firmware
        // ya corre (Linux se saltaría la carga vía autorun_detect) → directo a radio.
        println!("[resume] port reset de software + reabrir…");
        let h = open_rt3070().expect("abrir RT3070");
        let _ = h.reset();
        drop(h);
        std::thread::sleep(Duration::from_millis(1500));
        let h = match reopen_after_reenum(60) {
            Some(h) => h,
            None => { println!("❌ device no volvió tras el port reset"); std::process::exit(2); }
        };
        println!("  ✅ reabierto tras port reset");
        match wait_csr_ready(&h, 500, 2) {
            Some(csr0) => {
                println!("  ✅ CSR ready ({csr0:#010x}) — firmware corriendo, saltamos carga");
                after_csr_ready(h, chan);
            }
            None => {
                println!("❌ CSR no responde tras port reset — chip sordo.");
                println!("   Requiere power-cycle físico y luego rt3070_linuxfull completo.");
                std::process::exit(3);
            }
        }
        return;
    }

    // ── FLUJO COMPLETO ──
    // ══ rt2x00lib_probe_hw → rt2800_probe_rt ══
    // (sin nada: el kernel solo lee MAC_CSR0 para identificar; el reset NO va aquí)
    let h = open_rt3070().expect("abrir RT3070");
    let csr0 = reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff);
    println!("[probe] MAC_CSR0 = {csr0:#010x}");
    if csr0 != 0xffff_ffff && csr0 != 0 {
        // CSR ready = ASIC vivo (NO implica firmware: Linux lee esto en probe
        // ANTES de load_firmware). La decisión de cargar va por autorun_detect.
        let mut ab = [0u8; 4];
        match h.read_control(REQ_IN, USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, &mut ab, FIRMWARE_TIMEOUT) {
            Ok(_) => {
                let raw = u32::from_le_bytes(ab);
                println!("  autorun_detect = {raw:#010x} → modo {}", if raw & 3 == 2 { "AUTORUN (no cargar fw)" } else { "NORMAL (cargar fw)" });
                if raw & 3 == 2 {
                    println!("  ✅ AutoRun — saltamos la carga, directo a la radio");
                    after_csr_ready(h, chan);
                    return;
                }
                drop(h);
                // seguir al flujo de carga con el chip fresco (reabrir)
                let h2 = open_rt3070().expect("reabrir");
                full_boot(h2, &fw_path, chan);
                return;
            }
            Err(e) => {
                println!("  autorun_detect err: {e:?} — seguimos flujo completo");
                drop(h);
                let h2 = open_rt3070().expect("reabrir");
                full_boot(h2, &fw_path, chan);
                return;
            }
        }
    }
    println!("  (chip sin arrancar — normal antes del firmware; sigue)");
    full_boot(h, &fw_path, chan);
}

// ══ Flujo de CARGA de firmware (rt2x00lib_load_firmware → rt2800_load_firmware
// → drv_write_firmware=rt2800usb_write_firmware) + init posterior ══
fn full_boot(h: rusb::DeviceHandle<rusb::Context>, fw_path: &str, chan: u8) {
    // ══ rt2x00lib_start → rt2x00lib_load_firmware → rt2800_load_firmware ══
    println!("\n[load_firmware]");
    // AUTOWAKEUP_CFG=0 (rt2800_load_firmware head)
    let _ = reg_write(&h, AUTOWAKEUP_CFG, 0);
    let mut buf4 = [0u8; 4];
    let autorun = h.read_control(REQ_IN, USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, &mut buf4, FIRMWARE_TIMEOUT)
        .map(|_| u32::from_le_bytes(buf4) & 3 == 2).unwrap_or(false);
    if !autorun {
        // multiwrite FIRMWARE_IMAGE_BASE: register_multiwrite = UNA request por
        // chunk de 64B sin pausas (CSR_CACHE_SIZE=64)
        let fw = std::fs::read(&fw_path).expect("leer rt2870.bin");
        let sec = &fw[FW_OFFSET..FW_OFFSET + FW_LENGTH];
        let mut ok = 0;
        for (i, chunk) in sec.chunks(64).enumerate() {
            let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
            let (v, idx) = encode_reg_addr(addr);
            if h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, Duration::from_millis(500)).is_ok() { ok += 1; }
        }
        println!("  firmware {ok}/64 chunks");
    } else {
        println!("  AutoRun: firmware no requerido");
    }
    // H2M_MAILBOX_CID=~0, H2M_MAILBOX_STATUS=~0
    let _ = reg_write(&h, H2M_MAILBOX_CID, !0u32);
    let _ = reg_write(&h, H2M_MAILBOX_STATUS, !0u32);
    // vendor_request_sw(USB_DEVICE_MODE, offset=0, value=FIRMWARE(8), 1000ms)
    // con reintentos hasta 1000ms (rt2x00usb_vendor_request loop)
    let kick_ok = vendor_sw_retry(&h, USB_DEVICE_MODE, USB_MODE_FIRMWARE, 1000);
    println!("  kick FIRMWARE(8): {}", if kick_ok { "enviado" } else { "sin confirmación (el MCU puede haber cortado el USB: OK)" });

    // ══ EL PASO QUE NOS FALTABA: el MCU resetea el USB al arrancar.
    // Esperar re-enumeración REAL (desaparición del device) y REABRIR.
    // (Linux: el kernel USB lo hace solo.)
    drop(h);
    println!("  esperando re-enumeración (el MCU resetea su USB al arrancar)…");
    let h = match reopen_after_reenum(60) {
        Some(h) => h,
        None => {
            println!("❌ el device no volvió en 30 s. Re-enchúfalo y reintenta.");
            std::process::exit(2);
        }
    };
    println!("  ✅ device RE-ENUMERADO y reabierto");
    std::thread::sleep(Duration::from_millis(10));
    let _ = reg_write(&h, H2M_MAILBOX_CSR, 0);
    println!("  MAILBOX_CSR=0 (rt2800usb_write_firmware tail)");

    // wait_csr_ready — si no responde, probablemente la re-enum fue falsa
    // (el device nunca desapareció) y el control pipe quedó colgado.
    match wait_csr_ready(&h, 100, 1) {
        Some(_) => after_csr_ready(h, chan),
        None => {
            println!("  ❌ CSR no ready tras re-enum.");
            println!("   💡 Prueba: rt3070_linuxfull --resume {} (port reset + saltar carga)", chan);
            std::process::exit(3);
        }
    }
}
