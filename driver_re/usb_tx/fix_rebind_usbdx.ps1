# fix_rebind_winusb.ps1 - rebind RT3070 to WinUSB (class USB, no Net class-config hang).
# Self-elevates; logs DIRECTLY to file (transcript unreliable).
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_rebind_winusb_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -Value "elevating..."
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== rebind to WinUSB ==="
$dir  = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx'
$inf  = "$dir\rt3070_winusb.inf"
$inst = 'USB\VID_148F&PID_3070\1.0'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvUpd2 {
    [DllImport("newdev.dll", SetLastError=true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        string HardwareId,
        string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
}
"@

L "1. hide inbox netr28ux.inf (backup)"
$inbox  = "$env:SystemRoot\INF\netr28ux.inf"
$backup = "$inbox.uifipill-bak"
if (Test-Path $inbox) {
    takeown /F $inbox | Out-Null
    $admin = New-Object Security.Principal.SecurityIdentifier('S-1-5-32-544')
    $adminName = $admin.Translate([Security.Principal.NTAccount]).Value
    icacls $inbox /grant "${adminName}:F" | Out-Null
    if (Test-Path $backup) { Remove-Item $backup -Force -ErrorAction SilentlyContinue }
    Move-Item $inbox $backup -Force
    L "   hidden -> $backup"
} elseif (Test-Path $backup) {
    L "   already hidden"
} else {
    L "   inbox INF not found (ok, package inbox hidden earlier)"
}

L "2. add WinUSB package"
$add = pnputil /add-driver "$inf" /install 2>&1 | Out-String
L ($add -replace "`r`n", " | ")

L "3. UpdateDriverForPlugAndPlayDevices (FORCE)"
$reboot = $false
$ok = [DrvUpd2]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, 'USB\VID_148F&PID_3070', $inf, 0x1, [ref]$reboot)
L ("   ok=$ok reboot=$reboot err=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
Start-Sleep -Seconds 8

$svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
L ("   service now: $svc")

if ($svc -notmatch 'WinUSB') {
    L "4. fallback: remove device + rescan"
    pnputil /remove-device $inst | Out-Null
    pnputil /scan-devices | Out-Null
    Start-Sleep -Seconds 15
    $svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
    L ("   service after fallback: $svc")
}

L "5. probe"
$probe = & "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String
L ($probe -replace "`r`n", " | ")
L "=== done ==="
