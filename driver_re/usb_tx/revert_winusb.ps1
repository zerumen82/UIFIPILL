# revert_winusb.ps1 — Devuelve netr28ux al RT3070 (deshace install_winusb.ps1).
# Ejecutar COMO ADMIN.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$log  = "$dir\winusb_revert_log.txt"
$inst = 'USB\VID_148F&PID_3070\1.0'

function Log($m) { $m | Tee-Object -FilePath $log -Append }

Remove-Item $log -ErrorAction SilentlyContinue

Log "== 1/4 Localizar paquete oem del INF WinUSB =="
$oems = pnputil /enum-drivers 2>&1 | Out-String
# parse simple: bloques "Published Name: oemXX.inf" + "Original Name: rt3070_winusb.inf"
$blocks = $oems -split "(?=Published Name)"
$target = $blocks | Where-Object { $_ -match 'rt3070_winusb\.inf' } |
    ForEach-Object { if ($_ -match '(oem\d+\.inf)') { $Matches[1] } }
if ($target) {
    foreach ($o in $target) {
        Log "Encontrado: $o — eliminando (con uninstall para rebindear netr28ux)..."
        pnputil /delete-driver $o /uninstall /force 2>&1 | Out-String | ForEach-Object { Log $_ }
    }
} else {
    Log "No se encontró el paquete rt3070_winusb.inf en el store (¿ya eliminado?)."
}

Log "== 2/4 Re-escanear / re-habilitar device =="
pnputil /enable-device $inst 2>&1 | Out-String | ForEach-Object { Log $_ }
Start-Sleep -Seconds 2
pnputil /scan-devices 2>&1 | Out-String | ForEach-Object { Log $_ }
Start-Sleep -Seconds 3

Log "== 3/4 Estado final =="
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } |
    Format-List FriendlyName, Status, Problem | Out-String | ForEach-Object { Log $_ }
$drv = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_DriverDescription' -ErrorAction SilentlyContinue).Data
Log "Driver activo: $drv"

Log "== 4/4 Nota =="
if ($drv -match 'netr28|802\.11|Ralink|MediaTek') {
    Log "OK: netr28ux restaurado. La antena vuelve a ser WiFi normal (netsh)."
} else {
    Log "AVISO: revisa el driver en el Administrador de dispositivos si no es netr28ux."
}
Log "Log completo en $log"
