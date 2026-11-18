mod common;
use common::*;
fn main() {
    let h = open_rt73().unwrap();
    for i in (0..256).step_by(2) {
        let mut buf = [0u8;2];
        let off = i as u16;
        let _ = h.read_control(common::REQ_IN, common::USB_MULTI_READ, 0, off, &mut buf, common::FIRMWARE_TIMEOUT);
        print!("{:04x}: ", off);
        for b in buf { print!("{:02x} ", b); }
        if i % 16 == 14 { println!(); }
    }
    println!();
}
