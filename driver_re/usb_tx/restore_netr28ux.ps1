# restore_netr28ux.ps1 — Devuelve la antena a netr28ux (revierte rebind_winusb.ps1).
# Ejecutar COMO ADMIN.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$inst = 'USB\VID_148F&PID_3070\1.0'

"== 1/4 Desinstalar paquete WinUSB (oem261.inf o el que sea) =="
$enum = pnputil /enum-drivers | Out-String
$blocks = $enum -split '(?=Nombre publicado)'
foreach ($b in $blocks) {
    if ($b -match 'rt3070_winusb\.inf' -and $b -match '(oem\d+\.inf)') {
        "  quitando $($Matches[1])"
        pnputil /delete-driver $Matches[1] /uninstall /force 2>&1 | Out-String
    }
}

"== 2/4 Reinstalar netr28ux desde el DriverStore =="
$infs = Get-ChildItem 'C:\Windows\System32\DriverStore\FileRepository' -Filter 'netr28ux.inf' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
if ($infs) {
    "  fuente: $($infs.FullName)"
    pnputil /add-driver "$($infs.FullName)" /install 2>&1 | Out-String
} else {
    "  AVISO: no hay netr28ux.inf en DriverStore (lo borro /uninstall?). Usar el INF original del fabricante."
}

"== 3/4 Restaurar INF inbox (si fue ocultado) y rescan =="
$inbox = "$env:SystemRoot\INF\netr28ux.inf"
$backup = "$inbox.uifipill-bak"
if ((-not (Test-Path $inbox)) -and (Test-Path $backup)) {
    Copy-Item $backup $inbox -Force
    "  INF inbox restaurado desde backup"
}
pnputil /scan-devices 2>&1 | Out-String
Start-Sleep -Seconds 5

"== 4/4 Verificación =="
$drv = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_DriverDescription' -ErrorAction SilentlyContinue).Data
"Driver activo: '$drv'"
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } | Format-List FriendlyName, Status, Problem | Out-String
if ($drv -match 'netr28|802\.11|Ralink|MediaTek|Wireless') { "OK: netr28ux restaurado." } else { "REVISAR: driver inesperado." }
