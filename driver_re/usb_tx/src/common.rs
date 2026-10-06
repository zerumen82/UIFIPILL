// Constantes del protocolo RT2870/RT3070 — portadas del driver Linux
// rt2x00usb/rt2800usb (GPL-2.0, torvalds/linux). Solo uso lab interno.
// Nota: los 6 binarios consumen subconjuntos distintos de este módulo;
// silenciar dead_code en vez de perseguir warnings por binario.
#![allow(dead_code)]
use rusb::UsbContext;
use std::collections::HashMap;

pub const VID_RALINK: u16 = 0x148f;
pub const PID_RT3070: u16 = 0x3070;

// Vendor requests (bRequest) — rt2x00usb.h REAL (hallazgo 2026-09-22:
// la tabla que tenía era la de RT2500/RT73, el RT2800/RT3070 usa otra):
//   USB_DEVICE_MODE=1, SINGLE_WRITE=2, SINGLE_READ=3,
//   MULTI_WRITE=6, MULTI_READ=7, EEPROM_W=8, EEPROM_R=9, LED=10, RX=12
#[allow(dead_code)] // USB_DEVICE_MODE lo usan init.rs vía REQ OUT/IN directo
pub const USB_DEVICE_MODE: u8 = 0x01;
#[allow(dead_code)]
pub const USB_SINGLE_WRITE: u8 = 0x02;
pub const USB_SINGLE_READ: u8 = 0x03;
pub const USB_MULTI_WRITE: u8 = 0x06;
pub const USB_MULTI_READ: u8 = 0x07;
#[allow(dead_code)]
pub const USB_EEPROM_WRITE: u8 = 0x08;
pub const USB_EEPROM_READ: u8 = 0x09;
pub const USB_RX_CONTROL: u8 = 0x0C;

// wValue para USB_DEVICE_MODE — rt2x00usb.h enum rt2x00usb_mode_offset REAL
// (BUG CRÍTICO 2026-09-24: teníamos FIRMWARE=2 que es UNPLUG; el modo además
// va en wValue, no en wIndex — ver rt2x00usb_vendor_request_sw en load_firmware)
pub const USB_MODE_RESET: u16 = 1;
pub const USB_MODE_UNPLUG: u16 = 2;
pub const USB_MODE_FIRMWARE: u16 = 8;
pub const USB_MODE_AUTORUN: u16 = 17;

/// USB_DMA_CFG armado EXACTO de rt2800usb_enable_radio (rt2800usb.c:299):
///   AGG_EN=0, AGG_TIMEOUT=128 (0x80),
///   AGG_LIMIT=(rx->limit(128) * DATA_FRAME_SIZE(2432) / 1024) - 3 = 301,
///   RX_BULK_EN | TX_BULK_EN. (Antes: AGG_EN=1 + 0x9C en el campo bajo — el
///   valor 0x9C mezclaba AGG_TIMEOUT con bits del AGG_LIMIT y COLGABA EL USB
///   del chip, desconexión del bus medida. Luego: sin AGG_LIMIT el chip
///   sobrevive pero no entrega URBs de RX — 0 frames medido.)
pub const USB_DMA_CFG_VALUE: u32 =
    0x0000_0080                  // RX_BULK_AGG_TIMEOUT=128 (sin AGG_EN)
    | (301 << 8)                 // RX_BULK_AGG_LIMIT=301 (0x12D << 8)
    | 0x0040_0000                // RX_BULK_EN
    | 0x0080_0000;               // TX_BULK_EN

// bmRequestType: vendor, out/in
pub const REQ_OUT: u8 = 0x40;
pub const REQ_IN: u8 = 0xC0;

// Timeout de registros (ms) — REGISTER_TIMEOUT=10, FIRMWARE=10x
pub const REGISTER_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
pub const FIRMWARE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

// FIRMWARE_IMAGE_BASE (rt2800.h)
pub const FIRMWARE_IMAGE_BASE: u16 = 0x0800;
// offset/len de la sección RT3070 dentro de rt2870.bin
pub const FW_OFFSET: usize = 0;
pub const FW_LENGTH: usize = 4096;

// Registros MAC clave (rt2800.h) — direcciones reales (2026-09-23: corregidas
// contra torvalds/linux rt2800.h; la lista anterior tenía MAC_ADDR en 0x1004/8
// y BSSID en 0x100C/10, pero el layout real es ADDR_DW0=0x1008, ADDR_DW1=0x100C,
// BSSID_DW0=0x1010, BSSID_DW1=0x1014 — mismo desplazamiento que MAC_CSR0=0x1000
// y MAC_SYS_CTRL=0x1004 ya usados con éxito).
// BUG corregido 2026-10-01: la constante era 0x0004 = E2PROM_CSR (rt2800.h:129)
// — cada "enable TX+RX" (0x04/0x0C) iba al registro de EEPROM y el MAC no se
// activaba. Dirección real: rt2800.h:729.
pub const MAC_SYS_CTRL: u16 = 0x1004;
pub const MAC_CSR0: u16 = 0x1000;       // ASIC version (rt2800.h:722)
pub const MAC_ADDR_DW0: u16 = 0x1008;
pub const MAC_ADDR_DW1: u16 = 0x100C;
pub const MAC_BSSID_DW0: u16 = 0x1010;
pub const MAC_BSSID_DW1: u16 = 0x1014;
pub const BCN_TIME_CFG: u16 = 0x1114;
pub const TX_PIN_CFG: u16 = 0x1328;   // PA_PE_G0_EN: potencia al aire
pub const USB_DMA_CFG: u16 = 0x02a0;  // (0x0250 es TX_BASE_PTR2 en PCI; USB es 0x02a0)
pub const PBF_SYS_CTRL: u16 = 0x0400;
pub const PWR_PIN_CFG: u16 = 0x1204;      // rt2800.h:1037 (init_registers: 0x00000003)
pub const H2M_MAILBOX_CID: u16 = 0x7014;   // (antes 0x0704 — ERRÓNEO, rt2800.h:2123)
pub const H2M_MAILBOX_STATUS: u16 = 0x701C; // (antes 0x0708 — ERRÓNEO, rt2800.h:2133)
pub const H2M_MAILBOX_CSR: u16 = 0x7010;   // (antes 0x070C — ERRÓNEO, rt2800.h:2112)
pub const H2M_BBP_AGENT: u16 = 0x7028;     // rt2800.h:2143
pub const H2M_INT_SRC: u16 = 0x7024;       // rt2800.h:2138
pub const HOST_CMD_CSR: u16 = 0x0404;      // puerta de comandos MCU (rt2800.h:575)

// Registros del bloque init_registers (rt2800.h, direcciones verificadas)
pub const WPDMA_GLO_CFG: u16 = 0x0208;
pub const US_CYC_CNT: u16 = 0x02a4;
pub const PBF_CFG: u16 = 0x0408;
pub const PBF_MAX_PCNT: u16 = 0x040c;
pub const MAX_LEN_CFG: u16 = 0x1018;
pub const LED_CFG: u16 = 0x102c;
pub const AMPDU_BA_WINSIZE: u16 = 0x1040;
pub const XIFS_TIME_CFG: u16 = 0x1100;
pub const BKOFF_SLOT_CFG: u16 = 0x1104;
pub const CH_TIME_CFG: u16 = 0x110c;
pub const INT_TIMER_CFG: u16 = 0x1128;
pub const MAC_STATUS_CFG: u16 = 0x1200;
pub const AUTOWAKEUP_CFG: u16 = 0x1208;   // rt2800.h:1044 (0 antes de cargar firmware)
pub const TX_SW_CFG0: u16 = 0x1330;
pub const TX_SW_CFG1: u16 = 0x1334;
pub const TX_SW_CFG2: u16 = 0x1338;
pub const TXOP_CTRL_CFG: u16 = 0x1340;
pub const TX_RTS_CFG: u16 = 0x1344;
pub const TX_TIMEOUT_CFG: u16 = 0x1348;
pub const TX_RTY_CFG: u16 = 0x134c;
pub const TX_LINK_CFG: u16 = 0x1350;
pub const HT_FBK_CFG0: u16 = 0x1354;
pub const HT_FBK_CFG1: u16 = 0x1358;
pub const LG_FBK_CFG0: u16 = 0x135c;
pub const LG_FBK_CFG1: u16 = 0x1360;
pub const CCK_PROT_CFG: u16 = 0x1364;
pub const OFDM_PROT_CFG: u16 = 0x1368;
pub const MM20_PROT_CFG: u16 = 0x136c;
pub const MM40_PROT_CFG: u16 = 0x1370;
pub const GF20_PROT_CFG: u16 = 0x1374;
pub const GF40_PROT_CFG: u16 = 0x1378;
pub const EXP_ACK_TIME: u16 = 0x1380;
pub const TXOP_HLDR_ET: u16 = 0x1608;
pub const RX_FILTER_CFG: u16 = 0x1400;
pub const AUTO_RSP_CFG: u16 = 0x1404;
pub const LEGACY_BASIC_RATE: u16 = 0x1408;
pub const HT_BASIC_RATE: u16 = 0x140c;

/// Comando MCU completo (port de rt2800_mcu_request): mailbox con token/args
/// + HOST_CMD_CSR con el comando. La 0x70 que escribíamos antes al CSR era el
/// comando sin token ni puerta — el MCU lo ignoraba.
/// FIX 2026-09-24 (bbpdiag session): OWNER es FIELD32(0xff000000) con valor 1
/// → 0x01000000 (SET_FIELD = value << bit_offset). Escribíamos 0x80000000
/// (OWNER=0x80) — el MCU no reconocía el comando y NUNCA consumió el
/// BOOT_SIGNAL → el BBP se quedaba en reset (bbp0=0x00 siempre, medido).
pub fn mcu_request(h: &rusb::DeviceHandle<rusb::Context>, command: u8, token: u8, arg0: u8, arg1: u8) -> rusb::Result<()> {
    mcu_request_wait(h, command, token, arg0, arg1, 0).map(|_| ()).map_err(|_| rusb::Error::Other)
}

/// Igual que mcu_request pero espera a que el MCU consuma el comando (OWNER
/// vuelve a 0). `timeout_ms` = 0 → sin espera. Devuelve true si se consumió.
pub fn mcu_request_wait(h: &rusb::DeviceHandle<rusb::Context>, command: u8, token: u8, arg0: u8, arg1: u8, timeout_ms: u64) -> Result<bool, String> {
    let _ = wait_busy(h, H2M_MAILBOX_CSR, 0xff00_0000, 0).map_err(|e| e.to_string())?; // OWNER libre
    let mailbox: u32 = 0x0100_0000                              // OWNER=1 (¡no 0x80!)
        | ((token as u32) << 16) | ((arg0 as u32)) | ((arg1 as u32) << 8);
    reg_write(h, H2M_MAILBOX_CSR, mailbox).map_err(|e| e.to_string())?;
    reg_write(h, HOST_CMD_CSR, command as u32).map_err(|e| e.to_string())?;
    if timeout_ms == 0 {
        return Ok(true);
    }
    // El MCU limpia OWNER al tomar el comando (rt2x00 WAIT_FOR_MCU semantics)
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    while std::time::Instant::now() < deadline {
        if let Ok(v) = reg_read(h, H2M_MAILBOX_CSR) {
            if v & 0xff00_0000 == 0 {
                return Ok(true); // consumido
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Ok(false)
}

// Comandos MCU (rt2800.h:3005+)
// FIX 2026-09-30: MCU_CURRENT era 0x30 = ¡MCU_SLEEP! (rt2800.h:3005 es
// MCU_SLEEP=0x30; MCU_CURRENT=0x36 en rt2800.h:3008 y rt2800lib.c:10709).
// Nuestro init_radio mandaba MCU_SLEEP con arg0=0/arg1=0 → el firmware se
// metía en power-save justo al acabar el init (candidato a RX muda).
pub const MCU_WAKEUP: u8 = 0x31;
pub const MCU_BOOT_SIGNAL: u8 = 0x72;
pub const MCU_CURRENT: u8 = 0x36;
// MCU_BBP_SIGNAL (0x80): acceso a BBP a través del firmware. Es la ÚNICA vía
// que usa el driver vendor netr28ux (captura USBPcap 2026-09-30: 19 ops BBP,
// cero toques a BBP_CSR_CFG=0x101C).
pub const MCU_BBP_SIGNAL: u8 = 0x80;
pub const TX_STA_FIFO: u16 = 0x1718;

// Contadores RX (rt2800.h:1850-1866) — evidencia de si el MAC recibe aunque
// el bulk IN esté vacío: 0x1700 CRC/PHY, 0x1704 CCA/PLCP, 0x1708 dupli/overflow
pub const RX_STA_CNT0: u16 = 0x1700;
pub const RX_STA_CNT1: u16 = 0x1704;
pub const RX_STA_CNT2: u16 = 0x1708;

// TX_STA_FIFO (rt2800.h:1908) — FIFO de 16 entradas; cada read extrae la
// siguiente entrada. VALID=0 → no hay más resultados.
pub const TX_STA_FIFO_VALID: u32 = 0x0000_0001;
pub const TX_STA_FIFO_TX_SUCCESS: u32 = 0x0000_0020;
pub const TX_STA_FIFO_TX_ACK_REQUIRED: u32 = 0x0000_0080;
pub const TX_STA_FIFO_MCS: u32 = 0x007f_0000;
pub const TX_STA_FIFO_PHYMODE: u32 = 0xc000_0000;

// TXWI word0/word1 (rt2800.h:3078+). PHYMODE real del driver:
// RATE_MODE_CCK=1, RATE_MODE_OFDM=2, RATE_MODE_HT_MIX=0, RATE_MODE_HT_GF=3.
pub const RATE_MODE_HT_MIX: u32 = 0;
pub const RATE_MODE_CCK: u32 = 1;
pub const RATE_MODE_OFDM: u32 = 2;

// Registros de acceso indirecto RF/BBP (rt2800.h)
pub const RF_CSR_CFG: u16 = 0x0500;   // DATA bits0-7 | REGNUM bits8-13 | WRITE bit16 | BUSY bit17
pub const BBP_CSR_CFG: u16 = 0x101C;  // VALUE bits0-7 | REGNUM bits8-15 | READ bit16 | BUSY bit17
pub const TX_BAND_CFG: u16 = 0x132C;

// EFUSE (rt2800.h:655-664) — antenas sin EEPROM externa (config 0xffff por
// vendor EEPROM_READ pero chip ID válido = efuse). El driver usa
// rt2800_read_eeprom_efuse en ese caso.
pub const EFUSE_CTRL: u16 = 0x0580;
pub const EFUSE_CTRL_ADDRESS_IN: u32 = 0x03fe_0000; // bits 17-25
pub const EFUSE_CTRL_KICK: u32 = 0x4000_0000;
pub const EFUSE_CTRL_PRESENT: u32 = 0x8000_0000;
pub const EFUSE_DATA0: u16 = 0x0590;
pub const EFUSE_DATA1: u16 = 0x0594;
pub const EFUSE_DATA2: u16 = 0x0598;
pub const EFUSE_DATA3: u16 = 0x059C;

// Tabla de canales 2.4GHz para RF3070 (rt2800lib.c rf_vals_3x[] = Ralink
// FreqItems3020 — idéntica, verificado 2026-10-01):
// { canal, N (→RFCSR2), R (→RFCSR6[1:0]), K (→RFCSR3[3:0]) }
const RF_VALS_3X: [(u8, u8, u8, u8); 14] = [
    (1, 241, 2, 2), (2, 241, 2, 7), (3, 242, 2, 2), (4, 242, 2, 7),
    (5, 243, 2, 2), (6, 243, 2, 7), (7, 244, 2, 2), (8, 244, 2, 7),
    (9, 245, 2, 2), (10, 245, 2, 7), (11, 246, 2, 2), (12, 246, 2, 7),
    (13, 247, 2, 2), (14, 248, 2, 4),
];

/// Construye TXWI de 20 bytes (4 palabras) con el layout REAL del driver:
///   w0: MCS bits16-22 | PHYMODE bits30-31 (CCK=1)
///   w1: ACK bit0 | NSEQ bit1 | WCID bits8-15 | MPDU_TOTAL_BYTE_COUNT bits16-27
///       | PACKETID bits28-31 (≠0 → entra al TX_STA_FIFO, feedback TXDONE)
/// Antes (bug 2026-09-22): la longitud se ponía en w0 bits16-27 (que es MCS+BW
/// de la doc PCI) y w1 iba a 0 → el chip TX-eaba basura o nada.
pub fn txwi_bytes(wifi_len: u16, ack: bool, packet_id: u32) -> [u8; 20] {
    let w0: u32 = (0u32 << 16)                      // MCS=0 → 1 Mbps CCK
        | (RATE_MODE_CCK << 30);                     // PHYMODE=CCK
    let w1: u32 = (ack as u32)                       // ACK
        | (0u32 << 8)                                // WCID=0 (broadcast/MAC propio)
        | ((wifi_len as u32 & 0x0fff) << 16)         // MPDU_TOTAL_BYTE_COUNT
        | ((packet_id & 0xf) << 28);                 // PACKETID ≠0 → TX_STA_FIFO
    let mut txwi = [0u8; 20];
    txwi[0..4].copy_from_slice(&w0.to_le_bytes());
    txwi[4..8].copy_from_slice(&w1.to_le_bytes());
    // w2/w3 = IV/EIV: sin cifrado → 0
    txwi
}

// Registro endianness: el RT2870 multiplexa valor en las palabras altas
// del buffer del control (wValue/wIndex). Multi-read/write usa el offset
// como wIndex y el word se codifica: wValue = addr >> 16, wIndex = addr & 0xFFFF.
// (Ver rt2x00usb_register_read: usa USB_MULTI_READ con offset en wIndex.)

pub fn encode_reg_addr(addr: u16) -> (u16, u16) {
    // rt2x00usb: value = addr >> 16 (0), index = addr & 0xffff
    (0, addr)
}

pub fn open_rt3070() -> rusb::Result<rusb::DeviceHandle<rusb::Context>> {
    let ctx: rusb::Context = rusb::Context::new()?;
    let mut handle: Option<rusb::DeviceHandle<rusb::Context>> = None;
    let devices = ctx.devices()?;
    for dev in devices.iter() {
        let d = match dev.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if d.vendor_id() == VID_RALINK && d.product_id() == PID_RT3070 {
            handle = Some(dev.open()?);
            break;
        }
    }
    let h = match handle {
        Some(h) => h,
        None => return Err(rusb::Error::NotFound),
    };
    h.set_auto_detach_kernel_driver(true).ok();
    // CRÍTICO en Windows/WinUSB: asegurar que el device está CONFIGURADO.
    // Linux lo hace en enumeración; Windows no siempre → sin esto los requests
    // vendor con datos (MULTI_READ/WRITE) no se procesan (hallazgo 2026-09-22).
    h.set_active_configuration(1).ok();
    // La interfaz 0 lleva los endpoints bulk de datos.
    h.claim_interface(0)?;
    Ok(h)
}

/// Lectura de un registro MAC de 32 bits (USB_MULTI_READ de 4 bytes).
pub fn reg_read(h: &rusb::DeviceHandle<rusb::Context>, addr: u16) -> rusb::Result<u32> {
    let (val, idx) = encode_reg_addr(addr);
    let mut buf = [0u8; 4];
    let n = h.read_control(REQ_IN, USB_MULTI_READ, val, idx, &mut buf, FIRMWARE_TIMEOUT)?;
    if n != 4 {
        return Err(rusb::Error::Io);
    }
    Ok(u32::from_le_bytes(buf))
}

/// Escritura de un registro MAC de 32 bits (USB_MULTI_WRITE).
pub fn reg_write(h: &rusb::DeviceHandle<rusb::Context>, addr: u16, val: u32) -> rusb::Result<()> {
    let (v, idx) = encode_reg_addr(addr);
    h.write_control(REQ_OUT, USB_MULTI_WRITE, v, idx, &val.to_le_bytes(), REGISTER_TIMEOUT)?;
    Ok(())
}

/// Escritura de 16 bits con SINGLE_WRITE (bRequest 2, wValue=val, wIndex=addr,
/// sin stage de datos). Es la ÚNICA forma de escribir que usa el driver vendor
/// netr28ux: en la captura USBPcap las 258 escrituras OUT tienen setup de
/// 8 bytes y 0 payload (2064/258 = 8). Reservado para el canal MCU/BBP.
pub fn reg_write16(h: &rusb::DeviceHandle<rusb::Context>, addr: u16, val: u16) -> rusb::Result<()> {
    h.write_control(REQ_OUT, USB_SINGLE_WRITE, val, addr, &[], REGISTER_TIMEOUT)?;
    Ok(())
}

/// Espera a que un campo de un registro tome un valor (regbusy_read).
pub fn wait_busy(
    h: &rusb::DeviceHandle<rusb::Context>,
    addr: u16,
    mask: u32,
    want: u32,
) -> rusb::Result<bool> {
    for _ in 0..50 {
        let r = reg_read(h, addr)?;
        if (r & mask) == want {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    Ok(false)
}

/// Lee una entrada de TX_STA_FIFO (feedback TXDONE del chip).
/// Devuelve None si la FIFO está vacía (VALID=0).
/// (Mismo mecanismo que rt2800usb_tx_sta_fifo_read_worker: el chip devuelve
/// un resultado de TX por cada frame procesado si PACKETID≠0 en TXWI_W1.)
pub fn tx_sta_fifo_pop(h: &rusb::DeviceHandle<rusb::Context>) -> Option<u32> {
    let v = reg_read(h, TX_STA_FIFO).ok()?;
    if v & TX_STA_FIFO_VALID == 0 {
        return None;
    }
    Some(v)
}

/// Drena e interpreta hasta `max` entradas de TX_STA_FIFO. Devuelve líneas
/// listas para log: una por frame con éxito/fallo + MCS/PHYMODE usado.
pub fn drain_tx_status(h: &rusb::DeviceHandle<rusb::Context>, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..max {
        match tx_sta_fifo_pop(h) {
            None => break,
            Some(v) => {
                let success = v & TX_STA_FIFO_TX_SUCCESS != 0;
                let mcs = (v & TX_STA_FIFO_MCS) >> 16;
                let phymode = (v & TX_STA_FIFO_PHYMODE) >> 30;
                out.push(format!(
                    "TXDONE success={} mcs={} phymode={} (raw={v:#010x})",
                    success, mcs, phymode
                ));
            }
        }
    }
    out
}

// ── Acceso indirecto RF (RFCSR) y BBP ──────────────────────────────────────
// RFCSR: port de rt2800_rfcsr_write/read (RF_CSR_CFG, idéntico al vendor).
// BBP: vía MCU del vendor netr28ux (MCU_BBP_SIGNAL); la vía directa
// rt2800_bbp_write queda como *_direct solo para diagnóstico. ──

/// Escritura RFCSR: espera BUSY=0, luego DATA|REGNUM<<8|WRITE|BUSY.
pub fn rfcsr_write(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8) -> rusb::Result<()> {
    let _ = wait_busy(h, RF_CSR_CFG, 0x0002_0000, 0);
    let word: u32 = (val as u32) | ((reg as u32 & 0x3f) << 8) | (1 << 16) | (1 << 17);
    reg_write(h, RF_CSR_CFG, word)
}

/// Lectura RFCSR: REGNUM + BUSY (WRITE=0), sondeo hasta BUSY=0, leer DATA.
pub fn rfcsr_read(h: &rusb::DeviceHandle<rusb::Context>, reg: u8) -> rusb::Result<u8> {
    let word: u32 = ((reg as u32 & 0x3f) << 8) | (1 << 17);
    reg_write(h, RF_CSR_CFG, word)?;
    for _ in 0..50 {
        let v = reg_read(h, RF_CSR_CFG)?;
        if v & 0x0002_0000 == 0 {
            return Ok((v & 0xff) as u8);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    Err(rusb::Error::Io)
}

/// ── BBP vía MCU: el protocolo MEDIDO del driver vendor netr28ux ────────────
/// Captura USBPcap 2026-09-30 (vendor.pcap): las 19 operaciones BBP del vendor
/// van TODAS por H2M_BBP_AGENT (0x7028/0x702A) + H2M_MAILBOX_CSR (0x7010) con
/// TOKEN=0xff/OWNER=1 + HOST_CMD_CSR=MCU_BBP_SIGNAL(0x80). Ni un solo toque a
/// BBP_CSR_CFG (0x101C) en 380 vendor requests. La vía directa 0x101C bajo
/// WinUSB devuelve 0x00 siempre (medido, memory §4); la vía MCU devuelve
/// valores reales (BBP1=0x40, BBP49=0x8a vistos en la misma captura).
///
/// Word de H2M_BBP_AGENT (idéntico a BBP_CSR_CFG): VALUE[7:0] | REGNUM[15:8]
/// | flags<<16 donde flag bit0=READ, bit1=BUSY, bit3=BBP_RW_MODE
/// (0x0b = READ|BUSY|RW, 0x0a = WRITE|BUSY|RW; el MCU deja 0x09/0x08 al
/// terminar, con BUSY=0 → ahí se recoge el resultado).
const AGENT_BUSY: u32 = 0x0002_0000;
const AGENT_FLAG_READ: u16 = 0x000b;
const AGENT_FLAG_WRITE: u16 = 0x000a;

/// Espera a que H2M_BBP_AGENT tenga BUSY=0 (y, si se pide, con REGNUM
/// confirmado: el agente conserva el registro tras la operación, así que un
/// BUSY=0 con el reg equivocado = el MCU aún no lo ha cogido).
fn agent_wait(h: &rusb::DeviceHandle<rusb::Context>, want_reg: Option<u8>) -> rusb::Result<u32> {
    for _ in 0..100 {
        let w = reg_read(h, H2M_BBP_AGENT)?;
        if w & AGENT_BUSY == 0
            && want_reg.map_or(true, |r| ((w >> 8) & 0xff) as u8 == r)
        {
            return Ok(w);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    Err(rusb::Error::Timeout)
}

/// Operación BBP única a través del firmware (MCU_BBP_SIGNAL).
fn mcu_bbp(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8, read: bool) -> rusb::Result<u8> {
    // 1) agente libre (el vendor lee 0x7028 antes de cada operación)
    let _ = agent_wait(h, None)?;
    // 2) cargar el agente: [15:0] = VALUE|REGNUM<<8, [23:16] = flags
    let flags = if read { AGENT_FLAG_READ } else { AGENT_FLAG_WRITE };
    reg_write16(h, H2M_BBP_AGENT, (val as u16) | ((reg as u16) << 8))?;
    reg_write16(h, H2M_BBP_AGENT + 2, flags)?;
    // 3) mailbox: esperar OWNER=0 y escribir OWNER=1, TOKEN=0xff (sin status)
    for _ in 0..50 {
        let m = reg_read(h, H2M_MAILBOX_CSR)?;
        if m & 0xff00_0000 == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    reg_write16(h, H2M_MAILBOX_CSR, 0)?;
    reg_write16(h, H2M_MAILBOX_CSR + 2, 0x01ff)?;
    // 4) disparar el comando al 8051
    reg_write16(h, HOST_CMD_CSR, MCU_BBP_SIGNAL as u16)?;
    reg_write16(h, HOST_CMD_CSR + 2, 0)?;
    // 5) recoger: BUSY=0 + REGNUM confirmado → VALUE es el dato leído
    let w = agent_wait(h, Some(reg))?;
    Ok((w & 0xff) as u8)
}

/// Lectura BBP por la vía del vendor (MCU). Verifica REGNUM en la respuesta.
/// Si el transporte está conmutado a directo (bbp_set_via_mcu(false)), lee por
/// BBP_CSR_CFG — decisión tomada por sonda medida en bbpmcu.
pub fn bbp_read(h: &rusb::DeviceHandle<rusb::Context>, reg: u8) -> rusb::Result<u8> {
    if bbp_via_mcu() { mcu_bbp(h, reg, 0, true) } else { bbp_read_direct(h, reg) }
}

/// Escritura BBP por la vía del vendor (MCU). Devuelve el byte que había en
/// el agente al terminar (0x00 en las escrituras: el MCU no lo conserva).
/// Con transporte directo: BBP_CSR_CFG (rw_mode incluido, medido 2026-09-23).
pub fn bbp_write(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8) -> rusb::Result<()> {
    if bbp_via_mcu() { mcu_bbp(h, reg, val, false).map(|_| ()) } else { bbp_write_direct(h, reg, val) }
}

/// Transporte BBP efectivo. true = vía MCU (MCU_BBP_SIGNAL, la del vendor),
/// false = vía directa BBP_CSR_CFG. Se decide con una SONDA MEDIDA (no por
/// defecto): el bin bbpmcu compara ambas vías y llama a bbp_set_via_mcu().
/// Mantener true por defecto: los bins que cargan firmware (init/vendorradio)
/// siguen con la vía del vendor, que es la que el driver usa.
use std::sync::atomic::{AtomicBool, Ordering};
static BBP_VIA_MCU: AtomicBool = AtomicBool::new(true);

pub fn bbp_set_via_mcu(v: bool) { BBP_VIA_MCU.store(v, Ordering::SeqCst); }
pub fn bbp_via_mcu() -> bool { BBP_VIA_MCU.load(Ordering::SeqCst) }

/// Elige la vía BBP que de verdad responda (MCU_BBP_SIGNAL vs BBP_CSR_CFG
/// directo) — misma sonda que bbpmcu. Medido 2026-10-01 (runs 8/9): sin
/// firmware la vía MCU da timeout 0/5 y la directa lee vivo 2/2; con el
/// default MCU, init_bbp moría en "BBP no responde". Devuelve true=MCU.
pub fn bbp_probe_transport(h: &rusb::DeviceHandle<rusb::Context>) -> bool {
    let mut mcu_ok = 0;
    let mut dir_ok = 0;
    for r in [0u8, 1] {
        if let Ok(v) = mcu_bbp(h, r, 0, true) {
            if v != 0x00 && v != 0xff { mcu_ok += 1; }
        }
        if let Ok(v) = bbp_read_direct(h, r) {
            if v != 0x00 && v != 0xff { dir_ok += 1; }
        }
    }
    let via = mcu_ok > 0 && mcu_ok >= dir_ok;
    bbp_set_via_mcu(via);
    via
}

/// Vía DIRECTA por BBP_CSR_CFG (rt2800_bbp_read/write de rt2800lib.c).
/// Medido 2026-09-23/25 bajo WinUSB: devolvía 0x00 siempre. Medido 2026-10-01
/// (MCU corriendo, sin firmware): lee VIVO (BBP0=0x60, BBP1=0x40) mientras la
/// vía MCU da timeout — por eso existe el conmutador bbp_set_via_mcu().
pub fn bbp_read_direct(h: &rusb::DeviceHandle<rusb::Context>, reg: u8) -> rusb::Result<u8> {
    // Port exacto de rt2800_bbp_read: READ_CONTROL=1 (bit16), BUSY=1 (bit17),
    // BBP_RW_MODE=1 (bit19) — el driver lo pone SIEMPRE (rt2800lib.c:133).
    // REGISTER_USB_BUSY_COUNT=20 × 100us ≈ 2ms de espera BUSY (no 100ms).
    let _ = wait_busy(h, BBP_CSR_CFG, 1 << 17, 0)?;
    let word: u32 = ((reg as u32 & 0xff) << 8) | (1 << 16) | (1 << 17) | (1 << 19);
    reg_write(h, BBP_CSR_CFG, word)?;
    let _ = wait_busy(h, BBP_CSR_CFG, 1 << 17, 0);
    let v = reg_read(h, BBP_CSR_CFG)?;
    Ok((v & 0xff) as u8)
}

pub fn bbp_write_direct(h: &rusb::DeviceHandle<rusb::Context>, reg: u8, val: u8) -> rusb::Result<()> {
    // Port exacto de rt2800_bbp_write: READ_CONTROL=0, BUSY=1, BBP_RW_MODE=1.
    // (Antes faltaba RW_MODE — las escrituras BBP se ignoraban y el BBP
    // quedaba sin inicializar: RX muda, 0 frames medido 2026-09-23.)
    let _ = wait_busy(h, BBP_CSR_CFG, 0x0002_0000, 0);
    let word: u32 = (val as u32) | ((reg as u32 & 0xff) << 8) | (1 << 17) | (1 << 19);
    reg_write(h, BBP_CSR_CFG, word)
}

// Calibración de filtro RX BW20 (retorno de rx_filter_calibration) — la usa
// config_channel_rt3070 (RFCSR24/31 campo 0x7f). Default 0x09 = medido en
// vendor.pcap (reg24=reg31=0x09 tras el init del vendor) por si un bin
// llama a config sin init_rfcsr.
static CALIB_BW20: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0x09);

/// Valor calibrado BW20 para RFCSR24/31 (máx. 0x7f).
pub fn calib_bw20() -> u8 {
    CALIB_BW20.load(std::sync::atomic::Ordering::Relaxed)
}

/// Configuración de canal RT3070 — port de `RT30xx_ChipSwitchChannel`
/// (driver Ralink vendor, chips/rt30xx.c:590) = `rt2800_config_channel_rf3xxx`
/// (rt2800lib.c:2466). **Este chip usa el path 3xxx (N→RFCSR2), no rf53xx**:
/// en vendor.pcap #184 el vendor escribe N=0xF6 (canal 11) a RFCSR2, y
/// RFCSR8 resultó ser el ID de versión del RF (0x42 fijo en 1..14) — con el
/// path rf53xx (N→RFCSR8) el PLL nunca sintonizó → ΔCCA=0 (medido 2026-10-01).
///   RFCSR2=N, RFCSR3[3:0]=K, RFCSR6[1:0]=R, RFCSR12/13[4:0]=TX power,
///   RFCSR1=streams 1T1R (0xf1), RFCSR24/31=calib BW20, RFCSR7.RF_TUNING=1,
///   RFCSR30 bit7 pulso VCOCAL 1 ms, BBP 62/63/64/82/75/86, TX_BAND_CFG.
/// Después de esto el llamador debe encender los PA con enable_tx_pa().
pub fn config_channel_rt3070(h: &rusb::DeviceHandle<rusb::Context>, channel: u8) -> rusb::Result<()> {
    let idx = (channel.clamp(1, 14) - 1) as usize;
    let (_, n, r, k) = RF_VALS_3X[idx];

    // N del PLL → RFCSR2 (escritura directa, como el vendor #184)
    rfcsr_write(h, 2, n)?;

    // K → RFCSR3[3:0] (RFCSR3_K=0x0f); alto nibble = bias PA, se conserva
    // (vendor: lee 0x32, escribe 0x32 con K=2)
    let r3 = rfcsr_read(h, 3).unwrap_or(0);
    rfcsr_write(h, 3, (r3 & 0xF0) | (k & 0x0F))?;

    // R → RFCSR6[1:0] (RFCSR6_R1=0x03)
    let r6 = rfcsr_read(h, 6).unwrap_or(0);
    rfcsr_write(h, 6, (r6 & 0xFC) | (r & 0x03))?;

    // TX power RFCSR12/13 (campo 0x1f): vendor ch11 = 6/5. Por canal lo da
    // EEPROM (pendiente efuse_read); solo afecta a TX.
    let r12 = rfcsr_read(h, 12).unwrap_or(0);
    rfcsr_write(h, 12, (r12 & 0xE0) | 0x06)?;
    let r13 = rfcsr_read(h, 13).unwrap_or(0);
    rfcsr_write(h, 13, (r13 & 0xE0) | 0x05)?;

    // Streams 1T1R: bits7:4 = PD de TX1/TX2/RX1/RX2 (0xA0|0x50), bit0 =
    // RF_BLOCK_EN → 0xf1 como el vendor vivo. Bit1 (PLL_PD) se FUERZA a 0:
    // runs viejos con `r1 | 0x0f` lo dejaron en 1 (medido 0xf3, run8) y un
    // RMW que lo preserve mantiene el PLL en power-down.
    let r1 = rfcsr_read(h, 1).unwrap_or(0);
    rfcsr_write(h, 1, (r1 & 0x01) | 0xA0 | 0x50)?;

    // Frec. offset RFCSR23 = 0x09: el valor MEDIDO en vendor.pcap de ESTA
    // antena (vendor lee 0x09 = su RfFreqOffset desde EEPROM). Nuestro
    // silicio arranca en 0x00; con 0 el RX ya funciona (run9/scan1), pero el
    // LO queda exactamente donde lo pone el vendor. Bit7 (siempre 1 en el
    // vendor) se preserva.
    let r23 = rfcsr_read(h, 23).unwrap_or(0);
    rfcsr_write(h, 23, (r23 & 0x80) | 0x09)?;

    // Calibración filtro BW20 → RFCSR24 (TX_CALIB) y RFCSR31 (RX_CALIB),
    // campos 0x7f medidos en init_rfcsr (vendor: 0x09 en ambos). ANTES no se
    // tocaban en el cambio de canal → quedaban en estado BW40 del init.
    let cal = calib_bw20();
    let r24 = rfcsr_read(h, 24).unwrap_or(0);
    rfcsr_write(h, 24, (r24 & 0x80) | (cal & 0x7f))?;
    let r31 = rfcsr_read(h, 31).unwrap_or(0);
    rfcsr_write(h, 31, (r31 & 0x80) | (cal & 0x7f))?; // bit5 RX_H20M va en cal (0 → BW20)

    // RF tuning (RFCSR7 bit0) + pulso VCO calibration (RFCSR30 bit7, 1 ms)
    // = final de RT30xx_ChipSwitchChannel. ANTES hacíamos VCOCAL_EN en el
    // bit7 de RFCSR3 — ese bit es bias PA2 CCK, registro equivocado.
    let r7 = rfcsr_read(h, 7).unwrap_or(0);
    rfcsr_write(h, 7, r7 | 0x01)?;
    let r30 = rfcsr_read(h, 30).unwrap_or(0);
    rfcsr_write(h, 30, r30 | 0x80)?;
    std::thread::sleep(std::time::Duration::from_millis(1));
    let r30 = rfcsr_read(h, 30).unwrap_or(0);
    rfcsr_write(h, 30, r30 & 0x7F)?;

    // BBP tramo 2.4 GHz — rama external_lna_bg de rt2800_config_channel
    // (rt2800lib.c:4261-4264: 82=0x62, 75=0x46). ANTES llevábamos la rama
    // else (84/50, sin LNA externa) y el vendor de ESTE dispositivo usa la
    // de LNA externa (vendor.pcap §3b escribe 82=0x62 y 75=0x46): con 84/50
    // ΔCCA=0 en 1/6/11 (RF sordo, medido 2026-10-01).
    let _ = bbp_write(h, 62, 0x37);
    let _ = bbp_write(h, 63, 0x37);
    let _ = bbp_write(h, 64, 0x37);
    let _ = bbp_write(h, 82, 0x62);
    let _ = bbp_write(h, 75, 0x46);
    let _ = bbp_write(h, 86, 0x00);

    // TX_BAND_CFG: BG=1 (2.4GHz), A=0, HT40minus=0
    reg_write(h, TX_BAND_CFG, 0x0000_0004)?;

    Ok(())
}

/// Enciende los PA de transmisión (TX_PIN_CFG) — tramo PA del
/// rt2800_config_channel: PA_PE_G0 (2.4G), LNA PE, RFTR, TRSW.
pub fn enable_tx_pa(h: &rusb::DeviceHandle<rusb::Context>) -> rusb::Result<()> {
    let pin = reg_read(h, TX_PIN_CFG).unwrap_or(0);
    let on = 0x0000_0002   // PA_PE_G0_EN
        | 0x0000_0100      // LNA_PE_A0_EN
        | 0x0000_0200      // LNA_PE_G0_EN
        | 0x0001_0000      // RFTR_EN
        | 0x0004_0000;     // TRSW_EN
    reg_write(h, TX_PIN_CFG, pin | on)
}

// ── EFUSE — port de rt2800_efuse_detect + rt2800_efuse_read (rt2800lib.c) ──

/// rt2800_efuse_detect: bit PRESENT del EFUSE_CTRL.
pub fn efuse_present(h: &rusb::DeviceHandle<rusb::Context>) -> bool {
    reg_read(h, EFUSE_CTRL).map(|v| v & EFUSE_CTRL_PRESENT != 0).unwrap_or(false)
}

/// rt2800_efuse_read: un KICK lee 8 words (16 bytes) de golpe; el bloque
/// arranca en el word `i` (ADDRESS_IN). Data read "end to start":
/// DATA3→word i, DATA2→i+1, DATA1→i+2, DATA0→i+3.
pub fn efuse_read_block(h: &rusb::DeviceHandle<rusb::Context>, i: u16) -> rusb::Result<[u16; 8]> {
    let mut reg = reg_read(h, EFUSE_CTRL)?;
    reg = (reg & !EFUSE_CTRL_ADDRESS_IN)
        | (((i as u32) << 17) & EFUSE_CTRL_ADDRESS_IN)
        | EFUSE_CTRL_KICK; // MODE=0
    reg_write(h, EFUSE_CTRL, reg)?;
    wait_busy(h, EFUSE_CTRL, EFUSE_CTRL_KICK, 0)?;
    let d0 = reg_read(h, EFUSE_DATA0)?;
    let d1 = reg_read(h, EFUSE_DATA1)?;
    let d2 = reg_read(h, EFUSE_DATA2)?;
    let d3 = reg_read(h, EFUSE_DATA3)?;
    Ok([
        ((d3) & 0xffff) as u16,
        ((d3 >> 16) & 0xffff) as u16,
        ((d2) & 0xffff) as u16,
        ((d2 >> 16) & 0xffff) as u16,
        ((d1) & 0xffff) as u16,
        ((d1 >> 16) & 0xffff) as u16,
        ((d0) & 0xffff) as u16,
        ((d0 >> 16) & 0xffff) as u16,
    ])
}

/// Vuelca los primeros `words` words del efuse (por bloques de 8).
pub fn efuse_dump(h: &rusb::DeviceHandle<rusb::Context>, words: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0u16;
    while (i as usize) < words {
        match efuse_read_block(h, i) {
            Ok(blk) => {
                for (k, w) in blk.iter().enumerate() {
                    let off = (i as usize + k) * 2;
                    out.push(format!("efuse[@{off:#04x}] = {w:#06x}"));
                }
            }
            Err(e) => {
                out.push(format!("efuse[@{:#04x}..] err {e:?}", i * 2));
                break;
            }
        }
        i += 8;
    }
    out
}

// ── init_registers RT3070 — port de rt2800_init_registers +
// rt2800usb_init_registers (el hook drv_init_registers del USB). CRÍTICO:
// en el driver real esto se ejecuta DESPUÉS de cargar firmware y ANTES de
// wait_bbp_ready, e incluye el SEGUNDO reset MAC+BBP — es el candidato a
// despertar el BBP (nuestro reset anterior iba ANTES del firmware).
pub fn init_registers_rt3070(h: &rusb::DeviceHandle<rusb::Context>) -> Result<(), String> {
    // disable_wpdma (PCI-only, por si acaso): enables a 0, burst=3
    let wp = reg_read(h, WPDMA_GLO_CFG).unwrap_or(0) & !0x0000_004f;
    reg_write(h, WPDMA_GLO_CFG, wp | 0x30).map_err(|e| e.to_string())?;

    // ── drv_init_registers (rt2800usb_init_registers, rt2800usb.c:270) ──
    // wait_csr_ready: MAC_CSR0 válido
    for _ in 0..100 {
        if reg_read(h, MAC_CSR0).unwrap_or(0xffff_ffff) != 0xffff_ffff { break; }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    // PBF_SYS_CTRL &= ~0x2000
    reg_write(h, PBF_SYS_CTRL, reg_read(h, PBF_SYS_CTRL).unwrap_or(0) & !0x0000_2000)
        .map_err(|e| e.to_string())?;
    // SEGUNDO RESET MAC+BBP (DESPUÉS del firmware, como el driver)
    reg_write(h, MAC_SYS_CTRL, 0x0000_0003).map_err(|e| e.to_string())?;
    let _ = h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_RESET, 0, &[], REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_millis(10));
    reg_write(h, MAC_SYS_CTRL, 0x0000_0000).map_err(|e| e.to_string())?;

    // ── rt2800_init_registers cuerpo (valores calculados de las FIELD32) ──
    reg_write(h, LEGACY_BASIC_RATE, 0x0000_013f).map_err(|e| e.to_string())?;
    reg_write(h, HT_BASIC_RATE, 0x0000_8003).map_err(|e| e.to_string())?;
    reg_write(h, MAC_SYS_CTRL, 0).map_err(|e| e.to_string())?;
    reg_write(h, BCN_TIME_CFG, 0x0000_0640).map_err(|e| e.to_string())?; // interval=1600
    // config_filter(FIF_ALLMULTI): no monitor aún; NOT_TO_ME=1, control DROP
    reg_write(h, RX_FILTER_CFG, 0x0001_bf97).map_err(|e| e.to_string())?;
    reg_write(h, BKOFF_SLOT_CFG, 0x0000_0209).map_err(|e| e.to_string())?;
    // RT3070 rev>=F: TX_SW_CFG0=0x400, TX_SW_CFG1=0x00080606, TX_SW_CFG2=0
    reg_write(h, TX_SW_CFG0, 0x0000_0400).map_err(|e| e.to_string())?;
    reg_write(h, TX_SW_CFG1, 0x0008_0606).map_err(|e| e.to_string())?;
    reg_write(h, TX_SW_CFG2, 0).map_err(|e| e.to_string())?;
    reg_write(h, TX_LINK_CFG, 0x0000_1020).map_err(|e| e.to_string())?;
    reg_write(h, TX_TIMEOUT_CFG, 0x000a_2090).map_err(|e| e.to_string())?;
    // MAX_LEN_CFG: MAX_MPDU=3840(AGGREGATION_SIZE), MAX_PSDU=3, MIN 10
    reg_write(h, MAX_LEN_CFG, 0x000c_bf00).map_err(|e| e.to_string())?;
    reg_write(h, LED_CFG, 0x7f03_1e46).map_err(|e| e.to_string())?;
    reg_write(h, PBF_MAX_PCNT, 0x1f3f_bf9f).map_err(|e| e.to_string())?;
    reg_write(h, TX_RTY_CFG, 0x47d0_0202).map_err(|e| e.to_string())?;
    reg_write(h, AUTO_RSP_CFG, 0x0000_0007).map_err(|e| e.to_string())?;
    reg_write(h, CCK_PROT_CFG, 0x0574_0003).map_err(|e| e.to_string())?;
    reg_write(h, OFDM_PROT_CFG, 0x0574_0003).map_err(|e| e.to_string())?;
    reg_write(h, MM20_PROT_CFG, 0x0164_4004).map_err(|e| e.to_string())?;
    reg_write(h, MM40_PROT_CFG, 0x03e5_4084).map_err(|e| e.to_string())?;
    reg_write(h, GF20_PROT_CFG, 0x0164_4004).map_err(|e| e.to_string())?;
    reg_write(h, GF40_PROT_CFG, 0x03e5_4084).map_err(|e| e.to_string())?;
    reg_write(h, PBF_CFG, 0x00f4_0006).map_err(|e| e.to_string())?;
    reg_write(h, TXOP_CTRL_CFG, 0x0000_583f).map_err(|e| e.to_string())?;
    reg_write(h, TXOP_HLDR_ET, 0x0000_0002).map_err(|e| e.to_string())?;
    reg_write(h, TX_RTS_CFG, 0x0109_2b07).map_err(|e| e.to_string())?;
    reg_write(h, EXP_ACK_TIME, 0x0024_00ca).map_err(|e| e.to_string())?;
    reg_write(h, XIFS_TIME_CFG, 0x33a4_1010).map_err(|e| e.to_string())?;
    reg_write(h, HT_FBK_CFG0, 0x6543_2100).map_err(|e| e.to_string())?;
    reg_write(h, HT_FBK_CFG1, 0xedcb_9888).map_err(|e| e.to_string())?;
    reg_write(h, LG_FBK_CFG0, 0xedcb_a988).map_err(|e| e.to_string())?;
    reg_write(h, LG_FBK_CFG1, 0x0000_2100).map_err(|e| e.to_string())?;
    reg_write(h, INT_TIMER_CFG, 0x0000_0060).map_err(|e| e.to_string())?; // PRE_TBTT=6
    reg_write(h, CH_TIME_CFG, 0x0000_001f).map_err(|e| e.to_string())?;
    reg_write(h, PWR_PIN_CFG, 0x0000_0003).map_err(|e| e.to_string())?;
    // US_CYC_CNT: clock cycle=30 (USB)
    let cyc = reg_read(h, US_CYC_CNT).unwrap_or(0) & !0x0000_00ff;
    reg_write(h, US_CYC_CNT, cyc | 30).map_err(|e| e.to_string())?;
    Ok(())
}

/// Filtro RX en modo monitor/promiscuo: acepta control + PSPOLL + not-to-me
/// (equivalente a config_filter con FIF_CONTROL|FIF_PSPOLL|ALLMULTI + monitor).
pub fn rx_filter_monitor(h: &rusb::DeviceHandle<rusb::Context>) -> rusb::Result<()> {
    // DROP: CRC(1)|PHY(2)|VER(0x10)|DUP(0x80); todo lo demás aceptado
    reg_write(h, RX_FILTER_CFG, 0x0000_0093)
}

// ── RX bulk IN — layout MEDIDO (captura vendor.pcap 2026-09-30, 111/111 URBs) ─
//   [0..4]  longitud del registro ⇒ n = longitud + 8     (111/111 URBs)
//   [4..20] RXWI (W0@4 MPDU_TOTAL_BYTE_COUNT bits16-27, W1@8 secuencia,
//                 W2@12 RSSI0 bits0-7, W3@16 SNR)
//   [20..n] 802.11 — FC de beacon EXACTO en el byte 20 (84/111 URBs con beacon)
// El parser anterior asumía [4 dma][32 rxwi] → FC@36 → 0 frames CONTADOS
// aunque el chip entregara datos (falso negativo medido; por eso "0 frames").

/// Un frame 802.11 dentro de un URB bulk IN.
pub struct RxFrameRef<'a> {
    /// offset del FC dentro del buffer
    pub off: usize,
    /// fin del 802.11 (incluye FCS)
    pub end: usize,
    /// RSSI0 (RXWI_W2 bits 0-7)
    pub rssi: u8,
    /// 802.11 completo (sin radiotap), desde el FC
    pub data: &'a [u8],
}

/// Recorre los registros de un URB bulk IN con el layout medido y devuelve
/// sus frames. Tolera un URB truncado (entrega el frame parcial).
pub fn walk_rx(buf: &[u8]) -> Vec<RxFrameRef<'_>> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + 24 <= buf.len() {
        let ln = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]) as usize;
        if ln < 24 || ln > 8192 {
            break;
        }
        let full = off + ln + 8;
        let end = full.min(buf.len());
        let foff = off + 20;
        if end.saturating_sub(foff) < 24 {
            break;
        }
        let rssi = u32::from_le_bytes([
            buf[off + 12], buf[off + 13], buf[off + 14], buf[off + 15],
        ]) as u8;
        out.push(RxFrameRef { off: foff, end, rssi, data: &buf[foff..end] });
        if full > buf.len() {
            break; // URB truncado
        }
        off = full;
    }
    out
}

/// ¿Es un beacon? (tipo mgmt + subtipo 8) con al menos la cabecera fija.
pub fn is_beacon(frame: &[u8]) -> bool {
    frame.len() > 36 && (frame[0] & 0x0c) == 0 && (frame[0] >> 4) == 8
}

/// Extrae (SSID, canal, BSSID) de un beacon. El SSID vacío se devuelve como "".
pub fn parse_beacon(frame: &[u8]) -> Option<(String, u8, [u8; 6])> {
    if !is_beacon(frame) {
        return None;
    }
    // cabecera mgmt: FC(2) dur(2) addr1(6) addr2(6) addr3(6) seq(2) = 24 B
    // BSSID = addr3 (frame[16..22]); en un beacon addr2==addr3. (Antes usaba
    // addr1 = DA broadcast → salía FF:FF:FF:FF:FF:FF como BSSID.)
    let bssid: [u8; 6] = frame[16..22].try_into().unwrap();
    // la cabecera mgmt del beacon es 24 B + fixed params 12 B = 36
    let mut i = 36usize;
    let mut ssid = String::new();
    let mut ch = 0u8;
    // sin FCS (los IEs terminan antes de los últimos 4 bytes)
    let end = frame.len().saturating_sub(4).max(36);
    while i + 2 <= end {
        let t = frame[i];
        let l = frame[i + 1] as usize;
        if i + 2 + l > end { break; }
        if t == 0 && l > 0 && l <= 32 {
            ssid = String::from_utf8_lossy(&frame[i + 2..i + 2 + l]).to_string();
        }
        if t == 3 && l >= 1 { ch = frame[i + 2]; }
        i += 2 + l;
    }
    Some((ssid, ch, bssid))
}

#[derive(Default)]
pub struct RxStats {
    pub urbs: usize,
    pub bytes: usize,
    pub frames: usize,
    /// histograma (offset del FC → nº de frames)
    pub offsets: Vec<(usize, usize)>,
    /// (SSID, BSSID, canal) de los beacons vistos
    pub aps: Vec<(String, [u8; 6], u8)>,
}

/// Lee el EP 0x81 durante `secs` segundos con el parser del layout medido.
pub fn rx_monitor(h: &rusb::DeviceHandle<rusb::Context>, secs: u64) -> RxStats {
    let mut st = RxStats::default();
    let mut hist: HashMap<usize, usize> = HashMap::new();
    let mut nets: HashMap<[u8; 6], (String, u8)> = HashMap::new();
    let mut buf = vec![0u8; 8192];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        match h.read_bulk(0x81, &mut buf, std::time::Duration::from_millis(200)) {
            Ok(n) if n > 0 => {
                st.urbs += 1;
                st.bytes += n;
                for f in walk_rx(&buf[..n]) {
                    st.frames += 1;
                    *hist.entry(f.off).or_insert(0) += 1;
                    if let Some((ssid, ch, bssid)) = parse_beacon(f.data) {
                        nets.entry(bssid).or_insert((ssid, ch));
                    }
                }
            }
            Ok(_) => {}
            Err(rusb::Error::Timeout) => {}
            Err(_) => break,
        }
    }
    let mut offs: Vec<(usize, usize)> = hist.into_iter().collect();
    offs.sort();
    st.offsets = offs;
    let mut aps: Vec<(String, [u8; 6], u8)> = nets
        .into_iter()
        .map(|(b, (s, c))| (s, b, c))
        .collect();
    aps.sort_by(|a, b| a.1.cmp(&b.1));
    st.aps = aps;
    st
}

/// Imprime el resumen de una ventana RX (evidencia comparable con vendor.pcap).
pub fn print_rx(st: &RxStats) {
    println!("\n  URBs={} bytes={} frames={}", st.urbs, st.bytes, st.frames);
    println!("  histograma offset FC: {:?}", st.offsets);
    if !st.aps.is_empty() {
        println!("  APs (beacons):");
        for (ssid, bssid, ch) in st.aps.iter().take(20) {
            let mac = bssid.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":");
            println!("    AP|{ssid}|{mac}|ch{ch}");
        }
    }
    println!(
        "\n📡 {} frames en la ventana — {}",
        st.frames,
        if st.frames > 0 { "🎉 RX EN LA MISMA ANTENA" } else { "0 frames" }
    );
}

// ── Init BBP+RF — port de rt2800_init_bbp_30xx + rt2800_init_rfcsr_30xx ────
// (rt2800lib.c). SIN ESTO LA RADIO NO DEMODULA: el firmware sube el MCU pero
// los registros BBP quedan por defecto (RX muda). Medido 2026-09-23: 0 frames
// en 20s con chip sano, canal OK y DMA correcto → faltaba este bloque.

/// wait_bbp_ready + rt2800_init_bbp_30xx (RT3070 rev F+: BBP103=0xc0).
pub fn init_bbp_rt3070(h: &rusb::DeviceHandle<rusb::Context>) -> Result<(), String> {
    // wait_bbp_ready: H2M_BBP_AGENT=0, MAILBOX=0, sondear BBP0 != 0x00/0xff
    let _ = reg_write(h, H2M_BBP_AGENT, 0);
    let _ = reg_write(h, H2M_MAILBOX_CSR, 0);
    std::thread::sleep(std::time::Duration::from_millis(1));
    let mut bbp_ok = false;
    for _ in 0..100 {
        let v = bbp_read(h, 0).unwrap_or(0);
        if v != 0x00 && v != 0xff { bbp_ok = true; break; }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    if !bbp_ok {
        // Diagnóstico en el error: MAC_STATUS_CFG + valor leído
        let mac_st = reg_read(h, 0x1200).unwrap_or(0xffff_ffff);
        return Err(format!(
            "BBP no responde (reg0=0x00/0xff) — MAC_STATUS_CFG={mac_st:#010x}"
        ));
    }
    for (reg, val) in [
        (65u8, 0x2cu8), (66, 0x38), (69, 0x12), (73, 0x10), (70, 0x0a),
        (79, 0x13), (80, 0x05), (81, 0x33), (82, 0x62), (83, 0x6a),
        (84, 0x99), (86, 0x00), (91, 0x04), (92, 0x00), (103, 0xc0),
        (105, 0x05), (106, 0x35),
    ] {
        bbp_write(h, reg, val).map_err(|e| format!("bbp{reg}: {e}"))?;
    }
    // Overrides BBP de EEPROM de ESTE dispositivo: el vendor netr28ux los
    // lee del array EEPROM_BBP_START al final de rt2800_init_bbp y escribe
    // reg1=0x40, reg3=0x00 y reg66=0x1c (vendor.pcap §3b: ventana de 19 ops
    // BBP = write1×4 + write3×2 + write66 + config 62/63/64/82/75 + read49).
    // BBP66=0x1c = 0x1c + 2*lna_gain con lna_gain=0 (mismo valor que el AGC
    // init de rt2800lib.c para 2.4GHz); sin esto la radio queda con 66=0x38.
    // Vía raíz pendiente: parsear el EFUSE (efuse_read_block) y aplicar el
    // array real; aquí van los valores MEDIDOS del propio vendor.
    for (reg, val) in [(1u8, 0x40u8), (3, 0x00), (66, 0x1c)] {
        bbp_write(h, reg, val).map_err(|e| format!("bbp{reg}(eeprom): {e}"))?;
    }
    Ok(())
}

/// rt2800_init_rfcsr_30xx para RT3070 rev F (port del driver 6.6, verificado
/// línea a línea contra rt2800lib.c:7601). SIN LDO_CFG0/GPIO_SWITCH/31=0x14
/// (todo eso es RT3071/3090 o rev <F). Con rx_filter_calibration REAL.
pub fn init_rfcsr_rt3070(h: &rusb::DeviceHandle<rusb::Context>) -> Result<(), String> {
    // rf_init_calibration(30): pulso VCOCAL (bit7) en RFCSR30 (rt2800lib.c:7368)
    let r30 = rfcsr_read(h, 30).unwrap_or(0);
    rfcsr_write(h, 30, r30 | 0x80).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(1));
    rfcsr_write(h, 30, r30 & !0x80).map_err(|e| e.to_string())?;

    // Tabla de rt2800_init_rfcsr_30xx (rt2800lib.c:7610-7629, RT3070)
    for (reg, val) in [
        (4u8, 0x40u8), (5, 0x03), (6, 0x02), (7, 0x60), (9, 0x0f),
        (10, 0x41), (11, 0x21), (12, 0x7b), (14, 0x90), (15, 0x58),
        (16, 0xb3), (17, 0x92), (18, 0x2c), (19, 0x02), (20, 0xba),
        (21, 0xdb), (24, 0x16), (25, 0x03), (29, 0x1f),
    ] {
        rfcsr_write(h, reg, val).map_err(|e| format!("rfcsr{reg}: {e}"))?;
    }
    // rev >= RT3070F: el driver NO toca LDO_CFG0 ni RFCSR31 aquí. Corregido:
    // antes escribíamos LDO en 0x0504 (dirección errónea, real 0x05d4) y
    // rfcsr31=0x14 (solo RT3071/3090) — ambos sobrantes de otra revisión.

    // rx_filter_calibration (rt2800lib.c:7381): calibración del filtro RX con
    // loopback BBP y tono de test. filter_target BW20=0x16, BW40=0x19 (RT3070).
    // ANTES: solo rfcsr24=0x07 — sin calibración el filtro RX queda sin ajustar.
    let _f20 = rx_filter_calibration(h, false, 0x16)?;
    // El valor BW20 manda en RFCSR24/31 al cambiar de canal (config_channel)
    CALIB_BW20.store(_f20, std::sync::atomic::Ordering::Relaxed);
    let _f40 = rx_filter_calibration(h, true, 0x19)?;
    // Estado inicial de vuelta (final de rx_filter_calibration)
    bbp_write(h, 24, 0).map_err(|e| e.to_string())?;
    let r22 = rfcsr_read(h, 22).unwrap_or(0);
    rfcsr_write(h, 22, r22 & !0x01).map_err(|e| e.to_string())?;
    let b4 = bbp_read(h, 4).unwrap_or(0);
    bbp_write(h, 4, b4 & !0x20).map_err(|e| e.to_string())?; // BBP4_BANDWIDTH=0 (BW20)

    // rev < RT3070F: rfcsr27=0x03. Rev F NO lo escribe aquí (solo más abajo en
    // normal_mode_setup con R1=0).

    // led_open_drain_enable (rt2800lib.c:7288): OPT_14_CSR bit0=1
    let _ = reg_write(h, 0x0114, reg_read(h, 0x0114).unwrap_or(0) | 1);

    // normal_mode_setup_3xxx (rt2800lib.c:7427) — parte RT3070:
    // RFCSR17: TX_LO1_EN=0 (bit3). RFCSR1_R (bit5) SOLO si NO hay LNA
    // externo (rt2800lib.c:7440-7447: if (!external_lna_bg) set R=1);
    // este dispositivo SÍ tiene LNA externo (vendor.pcap: rama 82=0x62/75=0x46)
    // → NO setear bit5 (antes lo forzábamos siempre: 0x92→0xb2, ¡MAL!).
    let r17 = rfcsr_read(h, 17).unwrap_or(0);
    rfcsr_write(h, 17, r17 & !0x08).map_err(|e| e.to_string())?;
    // RFCSR27 (RT3070 rev>=F): R1=0,R2=0,R3=0,R4=0 (limpiar bits de ganancia)
    let r27 = rfcsr_read(h, 27).unwrap_or(0);
    rfcsr_write(h, 27, r27 & !0x77).map_err(|e| e.to_string())?;
    // (la rama RT3071/3090 de normal_mode_setup toca RFCSR1/15/20/21 — no aplica)

    Ok(())
}

/// rt2800_init_rx_filter (rt2800lib.c:7302): calibración del filtro RX con
/// loopback. Devuelve rfcsr24 calibrado. bw40=false → BW20.
fn rx_filter_calibration(h: &rusb::DeviceHandle<rusb::Context>, bw40: bool, filter_target: u8) -> Result<u8, String> {
    let mut rfcsr24: u8 = if bw40 { 0x27 } else { 0x07 };
    rfcsr_write(h, 24, rfcsr24).map_err(|e| e.to_string())?;

    let b4 = bbp_read(h, 4).unwrap_or(0);
    let b4 = (b4 & !0x20) | if bw40 { 0x20 } else { 0x00 }; // BBP4_BANDWIDTH=2*bw40 (bits5-6)
    bbp_write(h, 4, b4).map_err(|e| e.to_string())?;

    let r31 = rfcsr_read(h, 31).unwrap_or(0);
    let r31 = (r31 & !0x20) | if bw40 { 0x20 } else { 0x00 }; // RFCSR31_RX_H20M
    rfcsr_write(h, 31, r31).map_err(|e| e.to_string())?;

    let r22 = rfcsr_read(h, 22).unwrap_or(0);
    rfcsr_write(h, 22, r22 | 0x01).map_err(|e| e.to_string())?; // BASEBAND_LOOPBACK

    // Tono passband: bbp24=0, iterar bbp25=0x90 hasta bbp55 != 0 (max 100)
    bbp_write(h, 24, 0).map_err(|e| e.to_string())?;
    let mut passband = 0u8;
    for _ in 0..100 {
        bbp_write(h, 25, 0x90).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(1));
        passband = bbp_read(h, 55).unwrap_or(0);
        if passband != 0 { break; }
    }

    // Tono stopband: bbp24=0x06, iterar hasta (passband-stopband) <= target
    bbp_write(h, 24, 0x06).map_err(|e| e.to_string())?;
    let mut overtuned = 0u8;
    for _ in 0..100 {
        bbp_write(h, 25, 0x90).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(1));
        let stopband = bbp_read(h, 55).unwrap_or(0);
        if passband.saturating_sub(stopband) <= filter_target {
            rfcsr24 = rfcsr24.wrapping_add(1);
            if passband.saturating_sub(stopband) == filter_target { overtuned += 1; }
        } else { break; }
        rfcsr_write(h, 24, rfcsr24).map_err(|e| e.to_string())?;
    }
    if overtuned > 0 { rfcsr24 -= 1; }
    rfcsr_write(h, 24, rfcsr24).map_err(|e| e.to_string())?;
    Ok(rfcsr24)
}

// ── Init completo reutilizable (port del main de rt3070_init) ──────────────
// Usado por rt3070_init y rt3070_scan (el scan necesita chip vivo y radio ON
// antes de saltar canales). Devuelve mensajes de progreso como líneas.
// SIN kick FIRMWARE(8): exigido por AGENTS.md (mata el chip 5/5 bajo WinUSB).
pub fn init_radio(h: &rusb::DeviceHandle<rusb::Context>, channel: u8, _fw_path: &str) -> Result<Vec<String>, String> {
    let mut log: Vec<String> = Vec::new();

    // 1. Reset MAC+BBP (tolerante). NOTA: el modo va en wValue (rt2x00usb
    // vendor_request_sw: value=mode, offset/index=0).
    let _ = reg_write(h, PBF_SYS_CTRL, reg_read(h, PBF_SYS_CTRL).unwrap_or(0) & !0x0000_2000);
    let _ = reg_write(h, MAC_SYS_CTRL, 0x3);
    let _ = h.write_control(REQ_OUT, USB_DEVICE_MODE, USB_MODE_RESET, 0, &[], REGISTER_TIMEOUT);
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = reg_write(h, MAC_SYS_CTRL, 0x0);
    log.push("reset MAC/BBP ok".into());

    // 2. Firmware — PROHIBIDO el kick USB_DEVICE_MODE FIRMWARE(8) bajo WinUSB
    // (AGENTS.md: mata el chip 5/5; solo el power-cycle físico lo recupera).
    // Si el MCU ya está vivo (autoload/arranque previo — medido en runs 8/9:
    // MCU_CURRENT consumido SIN cargar rt2870.bin) se salta la carga, como
    // hace Linux con autorun_detect. Si NO está vivo: error accionable, kick NUNCA.
    let mut buf4 = [0u8; 4];
    let autorun = h.read_control(REQ_IN, USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, &mut buf4, FIRMWARE_TIMEOUT)
        .map(|_| u32::from_le_bytes(buf4) & 3 == 2)
        .unwrap_or(false);
    let mcu_alive = mcu_request_wait(h, MCU_CURRENT, 0xff, 0, 0, 800).unwrap_or(false);
    if mcu_alive {
        log.push(format!(
            "MCU vivo{} — sin kick FIRMWARE(8) (prohibido bajo WinUSB)",
            if autorun { " (AutoRun)" } else { "" }
        ));
    } else {
        return Err(
            "MCU muerto: cargar firmware exigiría kick FIRMWARE(8) = chip muerto \
             (5/5 medido bajo WinUSB). Power-cycle físico (desenchufar 15 s) y reintentar."
                .into(),
        );
    }

    // 2b. Transporte BBP: elegir la vía viva (MCU_BBP_SIGNAL vs directa).
    // Medido runs 8/9: sin firmware la MCU-BBP da timeout y la directa vive;
    // con el default MCU, init_bbp fallaba con "BBP no responde".
    let via = bbp_probe_transport(h);
    log.push(format!(
        "transporte BBP: {}",
        if via { "MCU (MCU_BBP_SIGNAL)" } else { "directo (BBP_CSR_CFG)" }
    ));

    // 3. USB DMA (rt2800usb_enable_radio) — el DMA se configura ANTES de la
    // radio; la activación RX/TX real va al final (rt2800_enable_radio tail).
    reg_write(h, USB_DMA_CFG, USB_DMA_CFG_VALUE).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(10));

    // 3a. ORDEN REAL de rt2800_enable_radio (rt2800lib.c:10670): init_registers
    // COMPLETA (incluye el SEGUNDO reset MAC+BBP DESPUÉS del firmware y el
    // bloque TX_SW_CFG/RX_FILTER/XIFS) → wait_bbp_rf_ready → BOOT_SIGNAL →
    // wait_bbp_ready → init_bbp → init_rfcsr. Antes solo hacíamos "parcial".
    init_registers_rt3070(h)?;
    log.push("init_registers completos (2º reset MAC+BBP tras firmware)".into());

    // wait_bbp_rf_ready: MAC_STATUS_CFG bits BBP/RF BUSY deben estar a 0
    let _ = wait_busy(h, MAC_STATUS_CFG, 0x0000_0003, 0);

    // H2M_BBP_AGENT=0 + MAILBOX=0 + INT_SRC=0 + BOOT_SIGNAL (rt2800lib.c:10686)
    reg_write(h, H2M_BBP_AGENT, 0).map_err(|e| e.to_string())?;
    reg_write(h, H2M_MAILBOX_CSR, 0).map_err(|e| e.to_string())?;
    reg_write(h, H2M_INT_SRC, 0).map_err(|e| e.to_string())?;
    match mcu_request_wait(h, MCU_BOOT_SIGNAL, 0, 0, 0, 500) {
        Ok(true) => log.push("radio ON (BOOT_SIGNAL consumido)".into()),
        Ok(false) => log.push("⚠️ BOOT_SIGNAL (radio) no consumido".into()),
        Err(e) => return Err(format!("BOOT_SIGNAL radio: {e}")),
    }
    std::thread::sleep(std::time::Duration::from_millis(1));

    // BBP init (incluye wait_bbp_ready: sondeo bbp0 != 0x00/0xff)
    match init_bbp_rt3070(h) {
        Ok(_) => log.push("BBP init (17 regs, bbp0 vivo)".into()),
        Err(e) => return Err(format!("BBP: {e}")),
    }
    if let Err(e) = init_rfcsr_rt3070(h) { return Err(format!("RFCSR: {e}")); }
    log.push("RFCSR init (19 regs + calibración RX filter)".into());

    // MCU_CURRENT tras init BBP/RF en USB+RT3070 (rt2800lib.c:10704)
    let _ = mcu_request(h, MCU_CURRENT, 0, 0, 0);
    std::thread::sleep(std::time::Duration::from_millis(1));

    // Enable TX primero, luego RX (rt2800lib.c:10712-10730)
    reg_write(h, MAC_SYS_CTRL, 0x04).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(1));
    reg_write(h, MAC_SYS_CTRL, 0x0C).map_err(|e| e.to_string())?;
    log.push("MAC enable TX+RX".into());

    // 4. Canal + PA
    config_channel_rt3070(h, channel).map_err(|e| format!("canal {channel}: {e}"))?;
    enable_tx_pa(h).map_err(|e| format!("PA: {e}"))?;
    reg_write(h, BCN_TIME_CFG, 0).map_err(|e| e.to_string())?;
    log.push(format!("canal {channel} + PA"));

    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// URB sintético con el layout MEDIDO en vendor.pcap:
    /// [4 len][RXWI 16][802.11 @20] y len+8 == n (111/111 URBs reales).
    fn synthetic_beacon() -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend_from_slice(&0x0080u16.to_le_bytes());      // FC beacon
        frame.extend_from_slice(&0u16.to_le_bytes());           // duration
        frame.extend_from_slice(&[0xff; 6]);                    // addr1 = DA
        frame.extend_from_slice(&[0x48, 0x22, 0x54, 0x88, 0x29, 0xd6]); // addr2
        frame.extend_from_slice(&[0x48, 0x22, 0x54, 0x88, 0x29, 0xd6]); // addr3 = BSSID
        frame.extend_from_slice(&0u16.to_le_bytes());           // seq ctl
        frame.extend_from_slice(&[0u8; 8]);                     // timestamp
        frame.extend_from_slice(&100u16.to_le_bytes());         // beacon interval
        frame.extend_from_slice(&0x0411u16.to_le_bytes());      // capabilities
        frame.extend_from_slice(&[0x00, 0x07]);                 // IE SSID
        frame.extend_from_slice(b"LabSSID");
        frame.extend_from_slice(&[0x03, 0x01, 0x06]);           // IE DS: canal 6
        frame.extend_from_slice(&[0u8; 4]);                     // FCS
        let n = 20 + frame.len();
        let ln = n - 8;                                         // len = n - 8
        let mut urb = Vec::new();
        urb.extend_from_slice(&(ln as u32).to_le_bytes());      // [0..4]
        urb.extend_from_slice(&0u32.to_le_bytes());             // RXWI W0
        urb.extend_from_slice(&0u32.to_le_bytes());             // RXWI W1
        urb.extend_from_slice(&0x3au32.to_le_bytes());          // RXWI W2 RSSI0
        urb.extend_from_slice(&0u32.to_le_bytes());             // RXWI W3
        urb.extend_from_slice(&frame);                          // 802.11 @20
        assert_eq!(urb.len(), ln + 8);
        urb
    }

    #[test]
    fn walk_rx_measured_layout() {
        let urb = synthetic_beacon();
        let fr = walk_rx(&urb);
        assert_eq!(fr.len(), 1, "un frame por URB (111/111 en la captura)");
        assert_eq!(fr[0].off, 20, "el 802.11 arranca en el byte 20");
        assert_eq!(fr[0].rssi, 0x3a, "RSSI = RXWI_W2 bits 0-7");
        let (ssid, ch, bssid) = parse_beacon(fr[0].data).expect("parse beacon");
        assert_eq!(ssid, "LabSSID");
        assert_eq!(ch, 6);
        assert_eq!(bssid, [0x48, 0x22, 0x54, 0x88, 0x29, 0xd6], "BSSID = addr3");
    }

    #[test]
    fn walk_rx_rejects_garbage() {
        let junk = vec![0u8; 64];
        assert!(walk_rx(&junk).is_empty(), "len<24 → sin frames");
    }

    #[test]
    fn mcu_constants_regression() {
        // Regresión del bug medido 2026-09-30: 0x30 es MCU_SLEEP, no CURRENT.
        assert_eq!(MCU_CURRENT, 0x36);
        assert_eq!(MCU_WAKEUP, 0x31);
        assert_eq!(MCU_BOOT_SIGNAL, 0x72);
        assert_eq!(MCU_BBP_SIGNAL, 0x80);
    }
}
