# finish_rebind_elevated.ps1 — Limpia el devnode problemático (CM_PROB 56) y
# fuerza la re-instalación con WinUSB en una sola sesión admin.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$inst = 'USB\VID_148F&PID_3070\1.0'
$inf  = "$dir\rt3070_winusb.inf"
$hwid = 'USB\VID_148F&PID_3070'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvUpd2 {
    [DllImport("newdev.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    public static extern bool UpdateDriverForPlugAndPlayDevicesW(
        IntPtr hwndParent,
        string HardwareId,
        string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
}
"@

"== 1/5 Quitar devnode roto =="
pnputil /remove-device $inst 2>&1 | Out-String

"== 2/5 UpdateDriverForPlugAndPlayDevicesW (sin FORCE: instala en vacío) =="
$reboot = $false
$ok = [DrvUpd2]::UpdateDriverForPlugAndPlayDevicesW([IntPtr]::Zero, $hwid, $inf, 0, [ref]$reboot)
"  resultado: $ok (reboot: $reboot) LastError: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"

"== 3/5 Update con INSTALLFLAG_FORCE (por si ya hay devnode) =="
$ok = [DrvUpd2]::UpdateDriverForPlugAndPlayDevicesW([IntPtr]::Zero, $hwid, $inf, 0x1, [ref]$reboot)
"  resultado: $ok (reboot: $reboot) LastError: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"

"== 4/5 Rescan =="
pnputil /scan-devices 2>&1 | Out-String
Start-Sleep -Seconds 6

"== 5/5 Estado =="
$svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
"  Service: $svc"
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } | Format-List FriendlyName, Status, Problem | Out-String
& "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String
if ($svc -match 'WinUSB') { "OK: WinUSB ACTIVO." } else { "Sigue: '$svc'." }
