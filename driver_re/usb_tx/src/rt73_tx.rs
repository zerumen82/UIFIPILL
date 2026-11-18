// TX probe RT73: lista endpoints y prueba bulk OUT 0x02.
mod common;
use common::*;

fn main() {
    let h = match open_rt73() {
        Ok(h) => h,
        Err(e) => { eprintln!("❌ RT73: {e:?}"); std::process::exit(1); }
    };
    let cfg = h.device().active_config_descriptor().unwrap();
    let mut out_ep = 0u8;
    for ifc in cfg.interfaces() {
        for d in ifc.descriptors() {
            for ep in d.endpoint_descriptors() {
                println!("   EP {:#04x} {:?} pkt={}", ep.address(), ep.transfer_type(), ep.max_packet_size());
                if ep.transfer_type() == rusb::TransferType::Bulk && ep.address() & 0x80 == 0 {
                    out_ep = ep.address();
                }
            }
        }
    }
    if out_ep == 0 { out_ep = 0x02; }
    println!("Usando EP OUT {:#04x}", out_ep);
    let buf = [0u8; 64];
    match h.write_bulk(out_ep, &buf, std::time::Duration::from_millis(1000)) {
        Ok(n) => println!("TX bulk OUT OK: {n} bytes"),
        Err(e) => println!("TX bulk OUT ERR: {e:?}"),
    }
}
