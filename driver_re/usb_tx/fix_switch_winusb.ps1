# fix_switch_winusb.ps1 - in-place driver switch to WinUSB (hot_rebind recipe, Spanish-aware).
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_switch_winusb_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -Value "elevating..."
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== switch to WinUSB in place (hot_rebind recipe) ==="
$hwid = 'USB\VID_148F&PID_3070'
$inst = 'USB\VID_148F&PID_3070\1.0'

L "1. find published name of rt3070_winusb.inf (ES/EN aware)"
$lines = pnputil /enum-drivers
$oem = $null
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '(?i)rt3070_winusb\.inf') {
        for ($j = $i; $j -ge [Math]::Max(0, $i-8); $j--) {
            if ($lines[$j] -match '(?i)(oem\d+\.inf)') { $oem = $Matches[1]; break }
        }
    }
    if ($oem) { break }
}
L ("   published: " + $(if ($oem) { $oem } else { 'NONE' }))

$infPath = if ($oem) { "$env:windir\INF\$oem" } else { 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\rt3070_winusb.inf' }
L ("   INF path: $infPath  exists: " + (Test-Path $infPath))

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvSw2 {
    [DllImport("newdev.dll", SetLastError=true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        [MarshalAs(UnmanagedType.LPWStr)] string HardwareId,
        [MarshalAs(UnmanagedType.LPWStr)] string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
    [DllImport("setupapi.dll", SetLastError=true)]
    public static extern bool DiInstallDriverW(
        IntPtr hwndParent,
        [MarshalAs(UnmanagedType.LPWStr)] string InfPath,
        uint Flags,
        out bool NeedReboot);
}
"@

L "2. UpdateDriverForPlugAndPlayDevices FORCE (device present, in place)"
$reboot = $false
$ok = [DrvSw2]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $infPath, 1, [ref]$reboot)
L ("   ok=$ok reboot=$reboot err=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
Start-Sleep -Seconds 8

$svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
L ("   service now: $svc")

if ($svc -notmatch 'WinUSB') {
    L "3. fallback DiInstallDriverW"
    $rb2 = $false
    $ok2 = [DrvSw2]::DiInstallDriverW([IntPtr]::Zero, $infPath, 0, [ref]$rb2)
    L ("   ok2=$ok2 err=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
    Start-Sleep -Seconds 10
    $svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
    L ("   service now: $svc")
}

if ($svc -notmatch 'WinUSB') {
    L "4. fallback remove+rescan (WinUSB may be only match if netr28ux pkg uninstalled)"
    pnputil /remove-device $inst | Out-Null
    pnputil /scan-devices | Out-Null
    Start-Sleep -Seconds 20
    $svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
    L ("   service now: $svc")
}

$st = (Get-PnpDevice -InstanceId $inst -ErrorAction SilentlyContinue).Status
L ("   FINAL: service=$svc status=$st")

L "5. probe"
$probe = & 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\target\release\rt3070_probe.exe' 2>&1 | Out-String
L ($probe -replace "`r`n", " | ")
L "=== done ==="
