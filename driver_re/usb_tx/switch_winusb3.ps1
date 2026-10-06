# switch_winusb3.ps1 — intento programático con la P/Invoke CORRECTA.
# Bug de los intentos anteriores: DiInstallDriverW se importó de setupapi.dll;
# vive en newdev.dll → EntryPointNotFound (el "err=203" era ese).
# Secuencia: DiInstallDriverW(local inf+cat) → si no, UpdateDriver(FORCE) →
# si no, remove-device + DiInstallDriverW de nuevo. Reversible: restore_netr28ux.ps1
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\switch_winusb3_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -Value "elevating..."
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== switch_winusb3: newdev.dll correcto ==="
$dir  = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx'
$inst = 'USB\VID_148F&PID_3070\1.0'
$hwid = 'USB\VID_148F&PID_3070'
$inf  = "$dir\rt3070_winusb.inf"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvSw4 {
    [DllImport("newdev.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    public static extern bool DiInstallDriverW(IntPtr hwndParent, string InfPath, uint Flags, out bool NeedReboot);
    [DllImport("newdev.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(IntPtr hwndParent, string HardwareId, string FullInfPath, uint InstallFlags, out bool bRebootRequired);
}
"@

function Svc { (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data }
function Step([string]$name, [scriptblock]$b) {
    L $name
    $rb = $false
    try { $ok = & $b ([ref]$rb); L "   ok=$ok reboot=$rb err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
    catch { L "   EXC: $($_.Exception.Message)" }
    Start-Sleep -Seconds 8
    $svc = Svc
    L "   service=$svc"
    $svc
}

L "inf local=$inf cat=$(Test-Path ($inf -replace '\.inf$','.cat'))"
L "paquete publicado: $((pnputil /enum-drivers | Select-String 'rt3070_winusb').Count) entrada(s)"

$svc = Step "1. DiInstallDriverW (newdev.dll) con INF local" { param($r) [DrvSw4]::DiInstallDriverW([IntPtr]::Zero, $inf, 0, $r) }

if ($svc -notmatch 'WinUSB') {
    $svc = Step "2. UpdateDriverForPlugAndPlayDevices FORCE (newdev.dll, INF local)" { param($r) [DrvSw4]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $inf, 1, $r) }
}

if ($svc -notmatch 'WinUSB') {
    L "3. remove-device + rescan + DiInstallDriverW otra vez"
    pnputil /remove-device $inst 2>&1 | ForEach-Object { L "   $_" }
    Start-Sleep -Seconds 3
    pnputil /scan-devices 2>&1 | ForEach-Object { L "   $_" }
    Start-Sleep -Seconds 8
    $svc = Step "3b. DiInstallDriverW tras re-enumeración" { param($r) [DrvSw4]::DiInstallDriverW([IntPtr]::Zero, $inf, 0, $r) }
}

L "4. probe"
$probe = & "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String
L ("   " + ($probe -replace "`r`n", " | "))
$dev = Get-PnpDevice -InstanceId $inst -ErrorAction SilentlyContinue
L "   FINAL: service=$svc status=$($dev.Status) problem=$($dev.Problem)"
L "=== done ==="
