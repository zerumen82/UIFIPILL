// Busca el RT3070 en el bus USB tras la re-enumeración del firmware.
// El firmware Ralink al arrancar resetea el USB y puede volver con otro PID.
// Lista TODOS los devices Ralink/MediaTek (VID 0x148f) presentes.
mod common;
use rusb::UsbContext;

fn main() {
    let ctx = rusb::Context::new().unwrap();
    let devices = ctx.devices().unwrap();
    let mut found = false;
    for dev in devices.iter() {
        if let Ok(d) = dev.device_descriptor() {
            if d.vendor_id() == 0x148f {
                found = true;
                println!("Ralink/MediaTek: VID={:04x} PID={:04x} bus={:03} addr={:03}",
                    d.vendor_id(), d.product_id(), dev.bus_number(), dev.address());
            }
        }
    }
    if !found {
        println!("Ningún device 0x148f en el bus — el chip está en modo firmware (sin USB activo)");
        println!("o espera re-enumeración. Prueba: re-enchufar o pnputil /scan-devices como admin.");
    }
}
