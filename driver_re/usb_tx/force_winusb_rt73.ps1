# force_winusb_rt73.ps1 — Rebind RT73 (148F:2573) a WinUSB.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$inf  = "$dir\rt73_winusb.inf"
$hwid = 'USB\VID_148F&PID_2573'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class DrvUpd {
    [DllImport("newdev.dll", SetLastError=true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent, string HardwareId, string FullInfPath, uint InstallFlags, out bool bRebootRequired);
}
'@

"== 1/3 pnputil add-driver =="
pnputil /add-driver "$inf" /install 2>&1 | Out-String

"== 2/3 UpdateDriverForPlugAndPlayDevices (FORCE) =="
$reboot = $false
$ok = [DrvUpd]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $inf, 0x1, [ref]$reboot)
"  resultado: $ok (reboot: $reboot) LastError: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
Start-Sleep -Seconds 3

"== 3/3 Sonda =="
& "$dir\target\release\rt73_probe.exe" 2>&1 | Out-String
"Fin."
