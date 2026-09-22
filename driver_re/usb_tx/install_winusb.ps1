# install_winusb.ps1 — Pone WinUSB en el RT3070 (reemplaza netr28ux temporalmente),
# ejecuta la sonda cruda y DEJA el device habilitado con WinUSB para init/tx.
# Ejecutar COMO ADMIN. Revertir: revert_winusb.ps1 (vuelve a netr28ux).
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$inst = 'USB\VID_148F&PID_3070\1.0'

"== 1/7 Estado inicial =="
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } |
    Format-List FriendlyName, Status, Problem | Out-String

"== 2/7 Deshabilitar device (liberar netr28ux) =="
pnputil /disable-device $inst 2>&1 | Out-String
Start-Sleep -Seconds 2

"== 3/7 Instalar paquete WinUSB en el store (cat firmado lab) =="
$add = pnputil /add-driver "$dir\rt3070_winusb.inf" /install 2>&1 | Out-String
$add

"== 4/7 Rebindear el device al driver WinUSB =="
# pnputil no puede hacer update-driver por instancia; usar Windows Update API
# via el objeto 'Win32_PnPEntity' no vale; la vía fiable scriptada es devcon.
# Alternativa: rmdir + rescan hace que Windows elija el mejor driver del store.
# Si netr28ux sigue ganando (mejor rank), hacemos un 'update' forzado vía
# DevCon si existe; si no, dejamos preparado y avisamos.
$devcon = Get-ChildItem "$dir\..\..","$dir","$env:TEMP" -Filter devcon.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
if ($devcon) {
    "Usando devcon: $($devcon.FullName)"
    & $devcon.FullName update "$dir\rt3070_winusb.inf" "USB\VID_148F&PID_3070" 2>&1 | Out-String
} else {
    "devcon no encontrado -> re-evaluacion por rescan (puede no rebindear si netr28ux tiene mejor rank)"
    pnputil /enable-device $inst 2>&1 | Out-String
    pnputil /scan-devices 2>&1 | Out-String
}
Start-Sleep -Seconds 3

"== 5/7 Estado tras rebind =="
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } |
    Format-List FriendlyName, Status, Problem | Out-String
$drv = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_DriverDescription' -ErrorAction SilentlyContinue).Data
"Driver activo: $drv"

"== 6/7 Sonda USB cruda =="
& "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String

"== 7/7 Resumen =="
if ($drv -match 'WinUSB') {
    "OK: WinUSB en sitio. Ejecutar rt3070_probe / rt3070_init / rt3070_tx directamente."
    "Revertir cuando quieras: revert_winusb.ps1 (restaura netr28ux)."
} else {
    "Pendiente: el device sigue con '$drv'. Si es netr28ux, hacer rebind manual con Zadig (zadig.exe) o devcon."
}
