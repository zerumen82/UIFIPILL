# e1_run.ps1 — Experimento E1 completo, TODO al log. ADMIN. Sin interacción.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$log  = "$dir\e1_log.txt"
$hwid = 'USB\VID_148F&PID_3070'

function L($msg) { "$(Get-Date -Format HH:mm:ss.fff) $msg" | Out-File $log -Append }

Remove-Item $log -ErrorAction SilentlyContinue
L "=== E1 START ==="

# helper rebind (newdev.dll)
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvSwitch {
    [DllImport("newdev.dll", SetLastError=true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        [MarshalAs(UnmanagedType.LPWStr)] string HardwareId,
        [MarshalAs(UnmanagedType.LPWStr)] string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
}
"@

function Get-DriverInf {
    $dev = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
    if (-not $dev) { return "NONE" }
    return (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
}

function Rebind([string]$infPath) {
    $reboot = $false
    $ok = [DrvSwitch]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $infPath, 1, [ref]$reboot)
    if ($ok) { L "rebind OK ($infPath) reboot=$reboot" }
    else { L "rebind FAIL win32err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error()) ($infPath)" }
}

# localizar INFs
$netrInf  = Join-Path (Get-ChildItem "$env:SystemRoot\System32\DriverStore\FileRepository" -Filter 'netr28ux.inf_amd64_*' -Directory | Select-Object -First 1).FullName 'netr28ux.inf'
$winusbOem = $null
$enum = pnputil /enum-drivers | Out-String
foreach ($b in ($enum -split '(?=Published Name|Nombre publicado)')) {
    if ($b -match 'rt3070_winusb\.inf' -and $b -match '(oem\d+\.inf)') { $winusbOem = $Matches[1]; break }
}
$winusbInf = "$env:windir\INF\$winusbOem"
L "netrInf=$netrInf"
L "winusbInf=$winusbInf (oem=$winusbOem)"

# FASE A: rebind a netr28ux (radio del vendor viva)
L "=== FASE A: to netr28ux ==="
Rebind $netrInf
Start-Sleep -Seconds 4
L "driver ahora: $(Get-DriverInf)"

# FASE B: escaneo continuo (radio EN USO)
L "=== FASE B: escaneo activo 6 s ==="
$scan = Start-Job -ScriptBlock {
    $end = (Get-Date).AddSeconds(6)
    while ((Get-Date) -lt $end) { netsh wlan show networks mode=bssid | Out-Null; Start-Sleep -Milliseconds 250 }
}
Start-Sleep -Seconds 2

# FASE C: rebind a WinUSB EN CALIENTE (sin dejar pausa larga)
L "=== FASE C: to winusb EN CALIENTE ==="
Rebind $winusbInf
L "driver ahora: $(Get-DriverInf)"
Start-Job -ScriptBlock {} | Remove-Job -ErrorAction SilentlyContinue

L "=== FIN: ejecutar rt3070_hotread YA ==="
