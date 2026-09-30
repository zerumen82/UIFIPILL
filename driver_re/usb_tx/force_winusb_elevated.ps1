# force_winusb_elevated.ps1 — Rebind DIRECTO del RT3070 a WinUSB.
# Secuencia: ocultar inbox netr28ux → reinstalar paquete WinUSB →
# UPDATE-driver vía SetupAPI → si falla, desinstalar el DEVICE (devnode)
# y rescan para que PnP re-evalue drivers desde cero. Reversible con
# restore_netr28ux.ps1. Ejecutar COMO ADMIN. Lab-only.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$inst = 'USB\VID_148F&PID_3070\1.0'
$inf  = "$dir\rt3070_winusb.inf"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DrvUpd {
    [DllImport("newdev.dll", SetLastError=true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        string HardwareId,
        string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
    [DllImport("setupapi.dll", SetLastError=true)]
    public static extern bool DiInstallDriverW(
        IntPtr hwndParent,
        string InfPath,
        uint Flags,
        out bool NeedReboot);
}
"@

"== 0/4 Ocultar netr28ux.inf del inbox (para que no gane por rank) =="
$inbox  = "$env:SystemRoot\INF\netr28ux.inf"
$backup = "$inbox.uifipill-bak"
if (Test-Path $backup) { Remove-Item $backup -Force -ErrorAction SilentlyContinue }
if (Test-Path $inbox) {
    takeown /F $inbox | Out-Null
    $admin = New-Object Security.Principal.SecurityIdentifier('S-1-5-32-544')
    $adminName = $admin.Translate([Security.Principal.NTAccount]).Value
    icacls $inbox /grant "${adminName}:F" 2>&1 | Out-Null
    Move-Item $inbox $backup -Force
    "  netr28ux.inf → backup: $backup"
} else {
    "  inbox ya ocultado"
}

"== 0b/4 Reinstalar paquete WinUSB actualizado (INF limpio) =="
pnputil /add-driver "$inf" /install 2>&1 | Out-String

"== 1/5 UpdateDriverForPlugAndPlayDevices (INSTALLFLAG_FORCE) =="
$hwid = 'USB\VID_148F&PID_3070'
$reboot = $false
$ok = [DrvUpd]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $inf, 0x1, [ref]$reboot)
"  resultado: $ok (reboot: $reboot) LastError: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
Start-Sleep -Seconds 3

$svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
if ($svc -notmatch 'WinUSB') {
    "== 1b/5 DiInstallDriverW (instala INF en driver + re-evalua HW) =="
    $rb2 = $false
    $ok2 = [DrvUpd]::DiInstallDriverW([IntPtr]::Zero, $inf, 0, [ref]$rb2)
    "  resultado: $ok2 (reboot: $rb2) LastError: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    Start-Sleep -Seconds 3
    $svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
}

if ($svc -notmatch 'WinUSB') {
    "== 1c/5 Desinstalar DEVNODE + rescan (PnP re-evalua desde cero) =="
    "  (surprise-removal lógico del devnode)"
    Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } |
        Disable-PnpDevice -Confirm:$false -ErrorAction Continue 2>&1 | Out-String
    Start-Sleep -Seconds 2
    # remove devnode via pnputil /remove-device
    pnputil /remove-device $inst 2>&1 | Out-String
    Start-Sleep -Seconds 2
    pnputil /scan-devices 2>&1 | Out-String
    Start-Sleep -Seconds 5
    $svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
}

"== 2/5 Estado del device =="
"  Service: $svc"
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } | Format-List FriendlyName, Status, Problem | Out-String

"== 3/5 Sonda USB cruda =="
& "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String

"== 4/5 Veredicto =="
if ($svc -match 'WinUSB') {
    "OK: WinUSB activo. rt3070_init / rt3070_tx listos para usar."
    "REVERTIR: restore_netr28ux.ps1"
} else {
    "Service sigue en '$svc'. Próximo paso: desenchufar/re-enchufar la antena físicamente (rescan lógico no siempre re-evalua) o usar Zadig."
}
