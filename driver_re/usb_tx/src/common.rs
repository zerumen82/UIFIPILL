// Constantes del protocolo RT2870/RT3070 — portadas del driver Linux
// rt2x00usb/rt2800usb (GPL-2.0, torvalds/linux). Solo uso lab interno.
use rusb::UsbContext;

pub const VID_RALINK: u16 = 0x148f;
pub const PID_RT3070: u16 = 0x3070;

// Vendor requests (bRequest) — rt2x00usb.h REAL (hallazgo 2026-09-22:
// la tabla que tenía era la de RT2500/RT73, el RT2800/RT3070 usa otra):
//   USB_DEVICE_MODE=1, SINGLE_WRITE=2, SINGLE_READ=3,
//   MULTI_WRITE=6, MULTI_READ=7, EEPROM_W=8, EEPROM_R=9, LED=10, RX=12
pub const USB_DEVICE_MODE: u8 = 0x01;
pub const USB_SINGLE_WRITE: u8 = 0x02;
pub const USB_SINGLE_READ: u8 = 0x03;
pub const USB_MULTI_WRITE: u8 = 0x06;
pub const USB_MULTI_READ: u8 = 0x07;
pub const USB_EEPROM_WRITE: u8 = 0x08;
pub const USB_EEPROM_READ: u8 = 0x09;
pub const USB_RX_CONTROL: u8 = 0x0C;

// wValue para USB_DEVICE_MODE — rt2800usb.c
pub const USB_MODE_RESET: u16 = 1;
pub const USB_MODE_AUTORUN: u16 = 0x11;
pub const USB_MODE_FIRMWARE: u16 = 2;

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

// Registros MAC clave (rt2800.h)
pub const MAC_SYS_CTRL: u16 = 0x0004;
pub const MAC_ADDR_DW0: u16 = 0x1004;
pub const MAC_ADDR_DW1: u16 = 0x1008;
pub const MAC_BSSID_DW0: u16 = 0x100C;
pub const MAC_BSSID_DW1: u16 = 0x1010;
pub const BCN_TIME_CFG: u16 = 0x1114;
pub const USB_DMA_CFG: u16 = 0x0250;
pub const PBF_SYS_CTRL: u16 = 0x0400;
pub const H2M_MAILBOX_CID: u16 = 0x0704;
pub const H2M_MAILBOX_STATUS: u16 = 0x0708;
pub const H2M_MAILBOX_CSR: u16 = 0x070C;
pub const TX_STA_FIFO: u16 = 0x1718;

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
    let mut h = match handle {
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
