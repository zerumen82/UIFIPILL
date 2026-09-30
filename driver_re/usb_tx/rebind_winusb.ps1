# rebind_winusb.ps1 — Fuerza el rebind del RT3070 a WinUSB.
# Truco: delete-driver /uninstall del paquete netr28ux (oemXXX.inf) + rescan
# hace que Windows instale el siguiente mejor driver en el store para el HW
# (nuestro rt3070_winusb.inf / WinUSB). Revertir: restore_netr28ux.ps1
# (delete oem261 + re-add netr28ux desde su DriverStore o el INF original).
# Ejecutar COMO ADMIN. Lab-only.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$inst = 'USB\VID_148F&PID_3070\1.0'

"== 1/5 Localizar paquete netr28ux en el store =="
$enum = pnputil /enum-drivers | Out-String
$blocks = $enum -split '(?=Nombre publicado)'
$netr = @()
foreach ($b in $blocks) {
    if ($b -match 'netr28ux\.inf' -and $b -match '(oem\d+\.inf)') { $netr += $Matches[1] }
}
if (-not $netr) { "No hay paquete netr28ux en el store (¿ya desinstalado?)."; }
else { $netr | ForEach-Object { "  netr28ux = $_" } }

"== 2/5 Deshabilitar device =="
pnputil /disable-device $inst 2>&1 | Out-String
Start-Sleep -Seconds 2

"== 3/5 Desinstalar paquete netr28ux del store + renombrar INF inbox =="
foreach ($o in $netr) {
    pnputil /delete-driver $o /uninstall /force 2>&1 | Out-String
}
# CAUSA-RAIZ: el rebind seguía eligiendo netr28ux porque C:\Windows\INF\netr28ux.inf
# (driver inbox) sigue visible para PnP aunque el paquete oem se desinstale.
# Lo renombramos temporalmente (requiere TrustedInstaller; via takeown+icacls). La copia
# de seguridad queda como netr28ux.inf.uifipill-bak.
$inbox = "$env:SystemRoot\INF\netr28ux.inf"
$backup = "$inbox.uifipill-bak"
if (Test-Path $inbox) {
    if (Test-Path $backup) { Remove-Item $backup -Force }
    takeown /F $inbox /A | Out-Null
    icacls $inbox /grant "Administradores:F" /grant "Administrators:F" 2>&1 | Out-Null
    Copy-Item $inbox $backup -Force
    Remove-Item $inbox -Force
    "  INF inbox renombrado/ocultado (backup en $backup)"
} elseif (Test-Path $backup) {
    "  INF inbox ya ocultado antes (backup existe)"
}

"== 4/5 Re-scan: Windows debe elegir WinUSB (rt3070_winusb.inf oemXXX) =="
pnputil /enable-device $inst 2>&1 | Out-String
pnputil /scan-devices 2>&1 | Out-String
Start-Sleep -Seconds 5

"== 5/5 Verificación =="
$drv = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_DriverDescription' -ErrorAction SilentlyContinue).Data
$prob = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
"Driver activo: '$drv' (problem: $prob)"
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } | Format-List FriendlyName, Status, Problem | Out-String

& "$dir\target\release\rt3070_probe.exe" 2>&1 | Out-String

if ($drv -match 'WinUSB') {
    "RESULTADO: WinUSB activo. rt3070_probe/init/tx listos."
    "REVERTIR (cuando acabes): restore_netr28ux.ps1"
} else {
    "RESULTADO: sigue '$drv'. Si sale Code 28 (sin driver) o netr28ux, revisar."
    "Siguiente intento: desenchufa y re-enchufa la antena y mira rebind_log.txt"
}
