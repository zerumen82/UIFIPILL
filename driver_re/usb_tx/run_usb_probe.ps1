# Sonda USB cruda - liberar el RT3070 del driver y probar acceso directo
# Ejecutar COMO ADMIN (UAC). Reversible: al final se re-habilita el device.

$ErrorActionPreference = "Continue"
$inst = "USB\VID_148F&PID_3070\1.0"
$log = "$PSScriptRoot\usb_tx_log.txt"

"== 1/5 Estado del device ==" | Tee-Object $log
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } |
    Format-List FriendlyName, Status, Problem | Tee-Object $log -Append

"== 2/5 Deshabilitar device (descarga netr28ux) ==" | Tee-Object $log -Append
pnputil /disable-device $inst 2>&1 | Tee-Object $log -Append
Start-Sleep -Seconds 2

Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } |
    Format-List Status, Problem | Tee-Object $log -Append

"== 3/5 Sonda USB cruda (lectura registros, NO destructiva) ==" | Tee-Object $log -Append
& "$PSScriptRoot\target\release\rt3070_probe.exe" 2>&1 | Tee-Object $log -Append

"== 4/5 Re-habilitar device (restaurar) ==" | Tee-Object $log -Append
pnputil /enable-device $inst 2>&1 | Tee-Object $log -Append
Start-Sleep -Seconds 2

"== 5/5 Estado final ==" | Tee-Object $log -Append
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } |
    Format-List Status, Problem | Tee-Object $log -Append

"Log completo en $log" | Tee-Object $log -Append
