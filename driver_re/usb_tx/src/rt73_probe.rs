// Sonda RT73 (148F:2573): verifica que el protocolo vendor rt2x00usb responde.
mod common;
use common::*;

fn main() {
    let h = match open_rt73() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("❌ No se pudo abrir RT73 ({e:?})");
            std::process::exit(1);
        }
    };
    println!("✅ RT73 abierto, interfaz 0 reclamada");
    let desc = h.device().device_descriptor().unwrap();
    println!("   USB {}:{}, speed={:?}", desc.vendor_id(), desc.product_id(), h.device().speed());
    let cfg = h.device().active_config_descriptor().unwrap();
    for ifc in cfg.interfaces() {
        for d in ifc.descriptors() {
            for ep in d.endpoint_descriptors() {
                println!("   EP {:#04x} {:?} pkt={}", ep.address(), ep.transfer_type(), ep.max_packet_size());
            }
        }
    }
    for addr in [0x0000u16, 0x0004, 0x0008, 0x000c, 0x0010] {
        let (v, idx) = (0u16, addr);
        let mut buf = [0u8; 4];
        match h.read_control(REQ_IN, USB_MULTI_READ, v, idx, &mut buf, FIRMWARE_TIMEOUT) {
            Ok(4) => println!("   MULTI_READ 0x{addr:04x} = {:#010x}", u32::from_le_bytes(buf)),
            Ok(n) => println!("   MULTI_READ 0x{addr:04x}: corto {n}"),
            Err(e) => println!("   MULTI_READ 0x{addr:04x}: ERR {e:?}"),
        }
    }
}
