// diag.rs — Diagnóstico fino del protocolo vendor RT2870.
// Prueba sistemáticamente qué requests responde el chip para entender
// por qué MULTI_READ (0x02) no contesta cuando DEVICE_MODE (0x01) sí.
mod common;

use common::*;

fn try_read(h: &rusb::DeviceHandle<rusb::Context>, label: &str, req: u8, val: u16, idx: u16, len: usize) {
    let mut buf = vec![0u8; len];
    match h.read_control(REQ_IN, req, val, idx, &mut buf, std::time::Duration::from_millis(1500)) {
        Ok(n) => {
            let hex: Vec<String> = buf[..n].iter().map(|b| format!("{b:02x}")).collect();
            println!("   {label}: OK {n} bytes [{}]", hex.join(" "));
        }
        Err(e) => println!("   {label}: {e:?}"),
    }
}

fn try_write(h: &rusb::DeviceHandle<rusb::Context>, label: &str, req: u8, val: u16, idx: u16, data: &[u8]) {
    match h.write_control(REQ_OUT, req, val, idx, data, std::time::Duration::from_millis(1500)) {
        Ok(_) => println!("   {label}: OK"),
        Err(e) => println!("   {label}: {e:?}"),
    }
}

fn main() {
    let h = open_rt3070().expect("abrir RT3070");
    println!("== DIAGNÓSTICO PROTOCOLO VENDOR RT3070 ==\n");

    // 1. DEVICE_MODE IN en sus variantes (lo que SÍ responde)
    try_read(&h, "DEVICE_MODE IN val=AUTORUN(0x11) 1B", USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, 1);
    try_read(&h, "DEVICE_MODE IN val=AUTORUN(0x11) 4B", USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, 4);
    try_read(&h, "DEVICE_MODE IN val=0 4B",           USB_DEVICE_MODE, 0, 0, 4);

    // 2. MULTI_READ a MAC_CSR0 (ASIC version, el registro que TODO driver lee primero)
    try_read(&h, "MULTI_READ 0x1000 (ASIC) 4B", USB_MULTI_READ, 0, 0x1000, 4);
    try_read(&h, "MULTI_READ 0x0004 (SYS) 4B",  USB_MULTI_READ, 0, 0x0004, 4);

    // 3. Variantes exoticas por si el ROM usa otra tabla:
    try_read(&h, "SINGLE_READ 0x1000 4B", USB_SINGLE_READ, 0, 0x1000, 4);
    // rt73usb-style: READ_MAC_CSR? mismo 0x02...
    try_read(&h, "MULTI_READ val=addr 4B", USB_MULTI_READ, 0x1000, 0, 4);

    // 4. Escrituras con la tabla CORRECTA (SINGLE_WRITE=2, MULTI_WRITE=6)
    try_write(&h, "MULTI_WRITE 0x1004 val=0 (req 6)", USB_MULTI_WRITE, 0, 0x1004, &[0u8; 4]);
    try_write(&h, "SINGLE_WRITE 0x1004 val=0 (req 2)", USB_SINGLE_WRITE, 0, 0x1004, &[0u8; 4]);

    // 5. MULTI_READ con bRequest=7 (el CORRECTO para RT2800usb)
    try_read(&h, "MULTI_READ 0x1000 (req 7)", USB_MULTI_READ, 0, 0x1000, 4);
    try_read(&h, "MULTI_READ 0x0004 (req 7)", USB_MULTI_READ, 0, 0x0004, 4);

    // 6. Forzar re-arranque del autoload: DEVICE_MODE OUT con USB_MODE_AUTORUN
    try_write(&h, "DEVICE_MODE OUT val=AUTORUN (re-autoload)", USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, &[]);
    std::thread::sleep(std::time::Duration::from_millis(2000));
    try_read(&h, "DEVICE_MODE IN tras re-autoload", USB_DEVICE_MODE, USB_MODE_AUTORUN, 0, 4);
    try_read(&h, "MULTI_READ 0x1000 tras re-autoload", USB_MULTI_READ, 0, 0x1000, 4);

    // 7. DEVICE_MODE OUT con USB_MODE_FIRMWARE (0x02): ¿arranca el MCU aunque no haya firmware?
    try_write(&h, "DEVICE_MODE OUT val=FIRMWARE(0x02)", USB_DEVICE_MODE, 0, USB_MODE_FIRMWARE, &[]);
    std::thread::sleep(std::time::Duration::from_millis(2000));
    try_read(&h, "MULTI_READ 0x1000 tras FIRMWARE-mode", USB_MULTI_READ, 0, 0x1000, 4);

    println!("\n== FIN: pegar este output completo al agente ==");
}
