# switch_winusb4.ps1 — rebind con el INF ARREGLADO (decoración NTamd64).
# Causa raíz de todos los intentos previos (err=259 con FORCE, err=2, netr28ux
# ganando siempre): [Standard.NT$ARCH$] no lo resuelve SetupAPI fuera del build
# WDK → el paquete no listaba ningún modelo para NTamd64 → nunca era candidato.
# Secuencia: borrar oem180 viejo → republicar INF+cat nuevos → DiInstallDriverW
# (newdev.dll) → UpdateDriver FORCE → si acaso, remove-device + re-evaluar.
# Reversible: restore_netr28ux.ps1
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\switch_winusb4_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -Value "elevating..."
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== switch_winusb4: INF NTamd64 + cat nuevo ==="
$dir  = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx'
$inst = 'USB\VID_148F&PID_3070\1.0'
$hwid = 'USB\VID_148F&PID_3070'
$inf  = "$dir\rt3070_winusb.inf"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvSw5 {
    [DllImport("newdev.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    public static extern bool DiInstallDriverW(IntPtr hwndParent, string InfPath, uint Flags, out bool NeedReboot);
    [DllImport("newdev.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(IntPtr hwndParent, string HardwareId, string FullInfPath, uint InstallFlags, out bool bRebootRequired);
}
"@

function Svc { (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data }
L "inf=$inf"
L "cat=$(Test-Path "$dir\rt3070_winusb.cat")  firma=$((Get-AuthenticodeSignature "$dir\rt3070_winusb.cat").Status)"

L "1. borrar paquete WinUSB viejo"
pnputil /delete-driver oem180.inf /uninstall /force 2>&1 | ForEach-Object { L "   $_" }
Start-Sleep -Seconds 2

L "2. republicar INF+cat nuevos"
pnputil /add-driver "$inf" /install 2>&1 | ForEach-Object { L "   $_" }
Start-Sleep -Seconds 5
L "   service tras add: $(Svc)"

L "3. DiInstallDriverW (newdev.dll, INF local)"
$rb = $false
$ok = [DrvSw5]::DiInstallDriverW([IntPtr]::Zero, $inf, 0, [ref]$rb)
L "   ok=$ok reboot=$rb err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
Start-Sleep -Seconds 8
$svc = Svc
L "   service=$svc"

if ($svc -notmatch 'WinUSB') {
    L "4. UpdateDriverForPlugAndPlayDevices INSTALLFLAG_FORCE"
    $rb = $false
    $ok = [DrvSw5]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $inf, 1, [ref]$rb)
    L "   ok=$ok reboot=$rb err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    Start-Sleep -Seconds 8
    $svc = Svc
    L "   service=$svc"
}

if ($svc -notmatch 'WinUSB') {
    L "5. remove-device + rescan (sin FORCE, ya no debe haber competidor)"
    pnputil /remove-device $inst 2>&1 | ForEach-Object { L "   $_" }
    Start-Sleep -Seconds 3
    pnputil /scan-devices 2>&1 | ForEach-Object { L "   $_" }
    Start-Sleep -Seconds 8
    $svc = Svc
    L "   service=$svc"
    if ($svc -notmatch 'WinUSB') {
        $rb = $false
        $ok = [DrvSw5]::DiInstallDriverW([IntPtr]::Zero, $inf, 0, [ref]$rb)
        L "   5b DiInstall ok=$ok reboot=$rb err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
        Start-Sleep -Seconds 8
        $svc = Svc
        L "   5b service=$svc"
    }
}

L "6. probe"
$probe = & "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String
L ("   " + ($probe -replace "`r`n", " | "))
$dev = Get-PnpDevice -InstanceId $inst -ErrorAction SilentlyContinue
L "   FINAL: service=$svc status=$($dev.Status) problem=$($dev.Problem)"
L "=== done ==="
