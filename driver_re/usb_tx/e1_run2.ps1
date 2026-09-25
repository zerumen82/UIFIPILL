# e1_run2.ps1 — E1 v2: mecanismo pnputil (que SÍ funcionó) en vez de newdev.dll.
# ADMIN. Todo al log.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$log  = "$dir\e1_log.txt"

function L($msg) { "$(Get-Date -Format HH:mm:ss.fff) $msg" | Out-File $log -Append -Encoding unicode }

Remove-Item $log -ErrorAction SilentlyContinue
L "=== E1 v2 START ==="

function Get-DriverInf {
    $dev = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
    if (-not $dev) { return "NONE" }
    return (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
}

$netrInf = Join-Path (Get-ChildItem "$env:SystemRoot\System32\DriverStore\FileRepository" -Filter 'netr28ux.inf_amd64_*' -Directory | Select-Object -First 1).FullName 'netr28ux.inf'
L "netrInf=$netrInf"

# FASE A: rebind a netr28ux vía pnputil /add-driver /install
L "=== FASE A: pnputil add netr28ux /install ==="
$out = pnputil /add-driver $netrInf /install 2>&1 | Out-String
L ($out -replace "`r`n", " | ")
Start-Sleep -Seconds 4
L "driver ahora: $(Get-DriverInf)"

# FASE B: escaneo continuo (radio EN USO)
L "=== FASE B: escaneo activo 6 s ==="
$job = Start-Job -ScriptBlock {
    $end = (Get-Date).AddSeconds(6)
    while ((Get-Date) -lt $end) { netsh wlan show networks mode=bssid | Out-Null; Start-Sleep -Milliseconds 250 }
}
Start-Sleep -Seconds 2

# FASE C: rebind a WinUSB EN CALIENTE vía pnputil (el INF ya está publicado como oem180)
L "=== FASE C: rebind a winusb vía pnputil (install del paquete oem180 sobre el hwid) ==="
# pnputil no tiene 'update driver for device' directo; usar devcon si existe, si no UpdateDriver con el INF de DriverStore del paquete oem180
$devcon = Get-ChildItem "$dir" -Filter 'devcon.exe' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
if ($devcon) {
    L "devcon: $($devcon.FullName)"
    $o = & $devcon.FullName update "$env:windir\INF\oem180.inf" 'USB\VID_148F&PID_3070' 2>&1 | Out-String
    L ($o -replace "`r`n", " | ")
} else {
    # fallback: newdev con el INF REAL del paquete oem180 (el propio INF del store del paquete)
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvSwitch2 {
    [DllImport("newdev.dll", SetLastError=true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        [MarshalAs(UnmanagedType.LPWStr)] string HardwareId,
        [MarshalAs(UnmanagedType.LPWStr)] string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
}
"@
    # ruta del FileRepository del paquete oem180 (rt3070_winusb)
    $repoWinusb = Get-ChildItem "$env:SystemRoot\System32\DriverStore\FileRepository" -Filter 'rt3070_winusb.inf_amd64_*' -Directory | Select-Object -First 1
    if ($repoWinusb) {
        $infReal = Join-Path $repoWinusb.FullName 'rt3070_winusb.inf'
        L "winusb INF real: $infReal"
        $reboot = $false
        $ok = [DrvSwitch2]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, 'USB\VID_148F&PID_3070', $infReal, 1, [ref]$reboot)
        if ($ok) { L "rebind winusb OK reboot=$reboot" }
        else { L "rebind winusb FAIL win32err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
    } else {
        L "no repo rt3070_winusb en FileRepository"
    }
}
Start-Sleep -Seconds 3
L "driver final: $(Get-DriverInf)"
L "=== FIN E1 v2: correr rt3070_hotread YA ==="
exit 0
