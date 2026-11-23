// helper centralizado: los comandos Windows (hashcat, netsh, powershell, bins
// usb_tx) son de subsistema consola y, sin esta flag, abren una ventana negra
// cada vez que la app los lanza. Aplica CREATE_NO_WINDOW en Windows.
pub fn hidden_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}
