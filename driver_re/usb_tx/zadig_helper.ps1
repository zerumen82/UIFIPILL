# zadig_helper.ps1 — Abre Zadig y guía el rebind manual (2 clics).
# Zadig usa libwdi: instala WinUSB SIN pelear con el rank de netr28ux
# (elimina el devnode y re-crea con WinUSB directamente, a nivel PnP).
# Es LA vía oficial para esto y la única que ha funcionado siempre.
# Ejecutar COMO ADMIN (Zadig lo pide él solo, este script solo guía).

$dir = $PSScriptRoot
$zadig = Join-Path $dir 'zadig.exe'

if (-not (Test-Path $zadig)) { throw "No existe $zadig" }

Write-Host @"
====================================================================
 ZADIG — poner WinUSB en el RT3070 (2 clics)
====================================================================
 1. En Zadig: menu Options -> marca 'List All Devices'
 2. En el desplegable elige: '802.11n USB Wireless LAN Card'
    (debe mostrar USB\VID_148F&PID_3070)
 3. Driver destino (caja verde/derecha): selecciona 'WinUSB'
    (deberia ser el default)
 4. Pulsa 'Replace Driver' (o 'Install Driver')
 5. Espera el dialogo de exito (10-30 s)
====================================================================
 Abriendo Zadig...
====================================================================
"@

Start-Process $zadig -Verb RunAs -Wait

Write-Host ""
Write-Host "== Verificación posterior =="
$inst = 'USB\VID_148F&PID_3070'
$dev = Get-PnpDevice | Where-Object { $_.InstanceId -like "$inst*" }
$dev | Format-List FriendlyName, Status, InstanceId | Out-String
$svc = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
Write-Host "Service: $svc"
if ($svc -match 'WinUSB') {
    Write-Host "OK: WinUSB activo. Ahora puedes correr rt3070_probe / rt3070_init / rt3070_tx."
    Write-Host "REVERTIR cuando acabes: restore_netr28ux.ps1 + re-enchufar la antena."
} else {
    Write-Host "Sigue '$svc'. Si Zadig no lo cambio, revisa el paso 2 (device exacto)."
}
