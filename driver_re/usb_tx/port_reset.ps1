# port_reset.ps1 - Restart del device USB para que el firmware en marcha
# re-presente el device tras el boot del MCU (equivalente a port reset).
# Uso: port_reset.ps1  (como admin)
$dev = Get-PnpDevice | Where-Object { $_.InstanceId -match 'VID_148F' } | Select-Object -First 1
if (-not $dev) { Write-Output "Device 148F no presente en PnP (ni siquiera phantom)"; exit 1 }
Write-Output "Restart de: $($dev.FriendlyName) [$($dev.InstanceId)]"
# Disable/enable del device = port reset del hub
Disable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false -ErrorAction Continue
Start-Sleep -Seconds 2
Enable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false -ErrorAction Continue
Start-Sleep -Seconds 5
$after = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -match '148F' }
if ($after) { Write-Output "RE-ENUMERADO: $($after.FriendlyName) [$($after.InstanceId)]" }
else { Write-Output "NO volvio a enumerar (el firmware no presenta USB sin power-cycle)" }
