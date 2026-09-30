# e1_rebind_active.ps1 — Experimento E1 (memory.md §4, rev. 2)
# Requiere ADMIN. Fases:
#   1. Republicar netr28ux (pnputil /add-driver del DriverStore repo)
#   2. Rebind a netr28ux (la antena vuelve a ser WiFi normal)
#   3. Escaneo continuo con netsh (radio EN USO — el vendor no puede dormir el BBP)
#   4. hot_rebind a WinUSB EN CALIENTE (sin pausa)
#   5. Mensaje: correr rt3070_hotread INMEDIATAMENTE (<2 s)
# Uso: e1_rebind_active.ps1
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$hwid = 'USB\VID_148F&PID_3070'

function Show-Driver {
    $dev = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
    if (-not $dev) { Write-Output "Device NOT present."; return $null }
    $inf = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    Write-Output "Device: $($dev.FriendlyName) Status=$($dev.Status) INF=$inf"
    return $inf
}

"== FASE 1: republicar netr28ux en el store =="
$repo = Get-ChildItem "$env:SystemRoot\System32\DriverStore\FileRepository" -Filter 'netr28ux.inf_amd64_*' -Directory | Select-Object -First 1
if (-not $repo) { Write-Output "ERROR: no hay repo netr28ux"; exit 1 }
$infPath = Join-Path $repo.FullName 'netr28ux.inf'
Write-Output "INF: $infPath"
pnputil /add-driver $infPath /install | Out-String | Write-Output

"== FASE 2: rebind a netr28ux =="
# localizar el oem publicado recién
$enum = pnputil /enum-drivers | Out-String
$netrOem = $null
foreach ($b in ($enum -split '(?=Published Name|Nombre publicado)')) {
    if ($b -match 'netr28ux\.inf' -and $b -match '(oem\d+\.inf)') { $netrOem = $Matches[1]; break }
}
if (-not $netrOem) { Write-Output "ERROR: netr28ux no quedó publicado"; exit 1 }
Write-Output "netr28ux publicado como $netrOem"
& "$dir\hot_rebind.ps1" to-netr28ux

"== FASE 3: escaneo continuo (radio EN USO) =="
$scanJob = Start-Job -ScriptBlock {
    while ($true) { netsh wlan show networks mode=bssid | Out-Null; Start-Sleep -Milliseconds 300 }
}
Start-Sleep -Seconds 3
Write-Output "netsh escaneando en bucle (job $($scanJob.Id))"

"== FASE 4: rebind a WinUSB EN CALIENTE =="
& "$dir\hot_rebind.ps1" to-winusb

"== FASE 5: listo — correr INMEDIATAMENTE =="
Write-Output "AHORA: driver activo:"
Show-Driver | Out-Null
Write-Output ">>> Ejecutar YA: target\release\rt3070_hotread.exe (desde driver_re\usb_tx) <<<"
Write-Output "(el job de escaneo sigue activo; pararlo con: Get-Job | Stop-Job; Get-Job | Remove-Job)"
