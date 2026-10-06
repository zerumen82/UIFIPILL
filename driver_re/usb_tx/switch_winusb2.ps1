# switch_winusb2.ps1 — rebind RT3070 → WinUSB, atacando la causa raíz del err=2.
#
# Causa raíz (2026-10-01): C:\Windows\INF\oem180.inf está publicado SIN su
# catálogo (no existe oem180.cat) → UpdateDriverForPlugAndPlayDevices devuelve
# ERROR_FILE_NOT_FOUND(2) y DiInstallDriverW 203. Además el .PNF de netr28ux en
# C:\Windows\INF permite que el driver inbox vuelva a ganar el rescan.
#
# Secuencia (reversible con restore_netr28ux.ps1):
#   0. backups de netr28ux.inf / netr28ux.PNF (si existen)
#   1. delete-driver oem180.inf /force + add-driver local (inf+cat juntos) /install
#   2. UpdateDriverForPlugAndPlayDevices FORCE con el paquete recién publicado
#   3. fallback: DiInstallDriverW con el INF local
#   4. fallback: ocultar el .PNF de netr28ux + disable/enable + rescan
#   5. probe rt3070_probe
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\switch_winusb2_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -Value "elevating..."
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== switch_winusb2: causa raiz (cat faltante) ==="
$dir  = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx'
$inst = 'USB\VID_148F&PID_3070\1.0'
$hwid = 'USB\VID_148F&PID_3070'
$inf  = "$dir\rt3070_winusb.inf"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvSw3 {
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

function Get-ServiceNow {
    (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
}
function Show-State {
    $svc = Get-ServiceNow
    $st  = (Get-PnpDevice -InstanceId $inst -ErrorAction SilentlyContinue).Status
    L "   service=$svc status=$st"
    $svc
}

L "0. backups de netr28ux inbox (.inf y .PNF)"
foreach ($f in @("$env:SystemRoot\INF\netr28ux.inf", "$env:SystemRoot\INF\netr28ux.PNF")) {
    if (Test-Path $f) {
        $bak = "$f.uifipill-bak"
        if (-not (Test-Path $bak)) { Copy-Item $f $bak -Force; L "   backup: $bak" }
        else { L "   backup ya existe: $bak" }
    } else { L "   no existe: $f" }
}

L "1. republicar paquete WinUSB (el publicado no tiene .cat)"
pnputil /delete-driver oem180.inf /force 2>&1 | ForEach-Object { L "   del oem180: $_" }
pnputil /add-driver "$inf" /install 2>&1 | ForEach-Object { L "   add: $_" }

$enum = pnputil /enum-drivers | Out-String
$oem = $null
foreach ($b in ($enum -split '(?=Nombre publicado|Published Name)')) {
    if ($b -match 'rt3070_winusb\.inf' -and $b -match '(oem\d+\.inf)') { $oem = $Matches[1] }
}
$cat = if ($oem) { "$env:SystemRoot\INF\" + ($oem -replace '\.inf$', '.cat') } else { $null }
L "   publicado=$oem  cat existe=$(if ($cat) { Test-Path $cat } else { 'n/a' })"

if ($oem) {
    L "2. UpdateDriverForPlugAndPlayDevices FORCE con paquete publicado (con cat)"
    $reboot = $false
    $path = "$env:SystemRoot\INF\$oem"
    $ok = [DrvSw3]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $path, 1, [ref]$reboot)
    L "   ok=$ok err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error()) reboot=$reboot"
    Start-Sleep -Seconds 8
    $svc = Show-State
} else { $svc = $null }

if ($svc -notmatch 'WinUSB') {
    L "3. fallback DiInstallDriverW con INF local (inf+cat juntos)"
    $rb = $false
    $ok2 = [DrvSw3]::DiInstallDriverW([IntPtr]::Zero, $inf, 0, [ref]$rb)
    L "   ok2=$ok2 err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error()) reboot=$rb"
    Start-Sleep -Seconds 8
    $svc = Show-State
}

if ($svc -notmatch 'WinUSB') {
    L "4. fallback: ocultar PNF de netr28ux + disable/enable + rescan"
    $pnf = "$env:SystemRoot\INF\netr28ux.PNF"
    if (Test-Path $pnf) {
        takeown /F $pnf | Out-Null
        icacls $pnf /grant "Administradores:F" /grant "Administrators:F" 2>&1 | Out-Null
        Remove-Item $pnf -Force
        L "   netr28ux.PNF ocultado (backup .uifipill-bak)"
    }
    pnputil /disable-device $inst 2>&1 | ForEach-Object { L "   disable: $_" }
    Start-Sleep -Seconds 3
    pnputil /enable-device  $inst 2>&1 | ForEach-Object { L "   enable: $_" }
    pnputil /scan-devices   2>&1 | ForEach-Object { L "   rescan: $_" }
    Start-Sleep -Seconds 15
    $svc = Show-State
}

L "5. probe"
$probe = & "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String
L ("   " + ($probe -replace "`r`n", " | "))
$final = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_DriverDesc' -ErrorAction SilentlyContinue).Data
L "   FINAL: service=$svc desc=$final"
L "=== done ==="
