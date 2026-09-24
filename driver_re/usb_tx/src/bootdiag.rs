// Diagnóstico fino del arranque MCU/BBP: muestra TODO el progreso de
// init_radio (los logs intermedios que init.rs pierde al fallar) + test
// explícito de consumo de comandos MCU (OWNER→0).
mod common;
use common::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fw_path = args.get(1).cloned().unwrap_or_else(|| "rt2870.bin".into());

    let h = open_rt3070().expect("abrir RT3070");
    println!("== ASIC ==");
    println!("MAC_CSR0 = {:#010x}", reg_read(&h, MAC_CSR0).unwrap_or(0xffff_ffff));

    println!("\n== Test MCU mailbox (pre-firmware: se espera que NO consuma) ==");
    match mcu_request_wait(&h, 0x72, 0, 0, 0, 300) {
        Ok(true) => println!("BOOT_SIGNAL consumido (inesperado sin firmware)"),
        Ok(false) => println!("no consumido (esperado sin firmware)"),
        Err(e) => println!("err {e}"),
    }

    println!("\n== init_radio paso a paso ==");
    // Reproducimos init_radio pero imprimiendo cada línea EN SU MOMENTO.
    match init_radio_verbose(&h, 11, &fw_path) {
        Ok(()) => println!("\n✅ init COMPLETO — BBP vivo"),
        Err(e) => println!("\n❌ init falló: {e}"),
    }

    println!("\n== BBP post-init ==");
    for reg in [0u8, 1, 4, 62] {
        println!("bbp[{reg}] = {:#04x}", bbp_read(&h, reg).unwrap_or(0xff));
    }
}

/// Copia de init_radio con printing inmediato (para ver dónde muere).
fn init_radio_verbose(h: &rusb::DeviceHandle<rusb::Context>, channel: u8, fw_path: &str) -> Result<(), String> {
    let _ = reg_write(h, PBF_SYS_CTRL, reg_read(h, PBF_SYS_CTRL).unwrap_or(0) & !0x0000_2000);
    let _ = reg_write(h, MAC_SYS_CTRL, 0x3);
    let _ = h.write_control(REQ_OUT, USB_DEVICE_MODE, 0, USB_MODE_RESET, &[], REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = reg_write(h, MAC_SYS_CTRL, 0x0);
    println!("  [1] reset MAC/BBP ok");

    let fw = std::fs::read(fw_path).map_err(|e| format!("leer {fw_path}: {e}"))?;
    reg_write(h, AUTOWAKEUP_CFG, 0).map_err(|e| e.to_string())?;
    println!("  [2] AUTOWAKEUP_CFG=0");

    let mut buf4 = [0u8; 4];
    let autorun = h.read_control(REQ_IN, USB_DEVICE_MODE, 0, USB_MODE_AUTORUN, &mut buf4, FIRMWARE_TIMEOUT)
        .map(|_| u32::from_le_bytes(buf4) & 3 == 2)
        .unwrap_or(false);
    if autorun {
        println!("  [3] AutoRun: sin firmware");
    } else {
        let mut fw_ok = 0;
        for (i, chunk) in fw[FW_OFFSET..FW_OFFSET + FW_LENGTH].chunks(64).enumerate() {
            let addr = FIRMWARE_IMAGE_BASE + (i * 64) as u16;
            let (v, idx) = encode_reg_addr(addr);
            match h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, chunk, std::time::Duration::from_millis(500)) {
                Ok(_) => fw_ok += 1,
                Err(_) if i == 0 => return Err("chunk 0 rechazado".into()),
                Err(_) => {}
            }
        }
        println!("  [3] firmware {fw_ok}/64 chunks");
        reg_write(h, H2M_MAILBOX_CID, !0u32).map_err(|e| e.to_string())?;
        reg_write(h, H2M_MAILBOX_STATUS, !0u32).map_err(|e| e.to_string())?;
        h.write_control(REQ_OUT, USB_DEVICE_MODE, 0, USB_MODE_FIRMWARE, &[], FIRMWARE_TIMEOUT)
            .map_err(|e| format!("DEVICE_MODE FIRMWARE: {e}"))?;
        println!("  [4] DEVICE_MODE FIRMWARE enviado; esperando MCU…");
        let mut mcu_up = false;
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if reg_read(h, MAC_SYS_CTRL).is_ok() { mcu_up = true; break; }
        }
        if !mcu_up { return Err("MCU no responde tras firmware".into()); }
        println!("  [5] registros responden");
        reg_write(h, H2M_MAILBOX_CSR, 0).map_err(|e| e.to_string())?;
        match mcu_request_wait(h, MCU_BOOT_SIGNAL, 0, 0, 0, 1000) {
            Ok(true) => println!("  [6] BOOT_SIGNAL #1 CONSUMIDO ✅"),
            Ok(false) => println!("  [6] BOOT_SIGNAL #1 NO consumido ❌"),
            Err(e) => return Err(format!("BOOT_SIGNAL: {e}")),
        }
    }

    reg_write(h, USB_DMA_CFG, USB_DMA_CFG_VALUE).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(10));
    println!("  [7] USB_DMA_CFG");

    init_registers_rt3070(h)?;
    println!("  [8] init_registers completos");

    let _ = wait_busy(h, MAC_STATUS_CFG, 0x0000_0003, 0);
    reg_write(h, H2M_BBP_AGENT, 0).map_err(|e| e.to_string())?;
    reg_write(h, H2M_MAILBOX_CSR, 0).map_err(|e| e.to_string())?;
    reg_write(h, H2M_INT_SRC, 0).map_err(|e| e.to_string())?;
    match mcu_request_wait(h, MCU_BOOT_SIGNAL, 0, 0, 0, 1000) {
        Ok(true) => println!("  [9] BOOT_SIGNAL #2 CONSUMIDO ✅"),
        Ok(false) => println!("  [9] BOOT_SIGNAL #2 NO consumido ❌"),
        Err(e) => return Err(format!("BOOT_SIGNAL 2: {e}")),
    }
    std::thread::sleep(std::time::Duration::from_millis(1));

    // wait_bbp_ready con volcado de intentos
    println!("  [10] wait_bbp_ready (sondeo bbp0)…");
    let _ = reg_write(h, H2M_BBP_AGENT, 0);
    let _ = reg_write(h, H2M_MAILBOX_CSR, 0);
    std::thread::sleep(std::time::Duration::from_millis(1));
    let mut bbp_ok = false;
    for i in 0..100 {
        let v = bbp_read(h, 0).unwrap_or(0);
        if i < 5 || i % 20 == 0 {
            println!("      intento {i}: bbp0={v:#04x}");
        }
        if v != 0x00 && v != 0xff { bbp_ok = true; break; }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    if !bbp_ok {
        let mac_st = reg_read(h, 0x1200).unwrap_or(0xffff_ffff);
        return Err(format!("BBP mudo tras 100 intentos — MAC_STATUS_CFG={mac_st:#010x}"));
    }
    println!("  [11] BBP VIVO ✅");

    init_bbp_regs(h)?;
    println!("  [12] BBP 17 regs escritos");
    init_rfcsr_rt3070(h)?;
    println!("  [13] RFCSR init + calibración");
    let _ = mcu_request(h, MCU_CURRENT, 0, 0, 0);
    reg_write(h, MAC_SYS_CTRL, 0x04).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(1));
    reg_write(h, MAC_SYS_CTRL, 0x0C).map_err(|e| e.to_string())?;
    config_channel_rt3070(h, channel).map_err(|e| format!("canal: {e}"))?;
    enable_tx_pa(h).map_err(|e| format!("PA: {e}"))?;
    reg_write(h, BCN_TIME_CFG, 0).map_err(|e| e.to_string())?;
    println!("  [14] canal {channel} + PA + RX/TX ON");
    Ok(())
}

fn init_bbp_regs(h: &rusb::DeviceHandle<rusb::Context>) -> Result<(), String> {
    for (reg, val) in [
        (65u8, 0x2cu8), (66, 0x38), (69, 0x12), (73, 0x10), (70, 0x0a),
        (79, 0x13), (80, 0x05), (81, 0x33), (82, 0x62), (83, 0x6a),
        (84, 0x99), (86, 0x00), (91, 0x04), (92, 0x00), (103, 0xc0),
        (105, 0x05), (106, 0x35),
    ] {
        bbp_write(h, reg, val).map_err(|e| format!("bbp{reg}: {e}"))?;
    }
    Ok(())
}
