# e1_run3.ps1 — E1 v3: mecanismo real del hot-rebind (delete-driver + rescan).
# ADMIN. Todo al log.
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$log  = "$dir\e1_log.txt"
$inst = 'USB\VID_148F&PID_3070\1.0'

function L($msg) { "$(Get-Date -Format HH:mm:ss.fff) $msg" | Out-File $log -Append -Encoding unicode }

Remove-Item $log -ErrorAction SilentlyContinue
L "=== E1 v3 START ==="

function Get-DriverInf {
    $dev = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
    if (-not $dev) { return "NONE" }
    return (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
}

# FASE A: asegurar que netr28ux gana y se instala (publicar + disable/enable)
L "=== FASE A: publicar netr28ux + rescan ==="
$netrInf = Join-Path (Get-ChildItem "$env:SystemRoot\System32\DriverStore\FileRepository" -Filter 'netr28ux.inf_amd64_*' -Directory | Select-Object -First 1).FullName 'netr28ux.inf'
pnputil /add-driver $netrInf 2>&1 | Out-String | ForEach-Object { L $_ }
# ocultar el winusb de Zadig (oem395) para que netr28ux gane el rank
pnputil /delete-driver oem395.inf /uninstall /force 2>&1 | Out-String | ForEach-Object { L $_ }
pnputil /scan-devices 2>&1 | Out-String | ForEach-Object { L $_ }
Start-Sleep -Seconds 5
L "driver ahora: $(Get-DriverInf)"

# FASE B: escaneo continuo (radio EN USO — netsh sobre la antena)
L "=== FASE B: escaneo activo 8 s ==="
$job = Start-Job -ScriptBlock {
    $end = (Get-Date).AddSeconds(8)
    while ((Get-Date) -lt $end) { netsh wlan show networks mode=bssid | Out-Null; Start-Sleep -Milliseconds 250 }
}
Start-Sleep -Seconds 3

# FASE C: hot-rebind a WinUSB EN CALIENTE: desinstalar netr28ux del store + rescan
L "=== FASE C: delete netr28ux + rescan (WinUSB toma el relevo con BBP caliente) ==="
$enum = pnputil /enum-drivers | Out-String
foreach ($b in ($enum -split '(?=Nombre publicado)')) {
    if ($b -match 'netr28ux\.inf' -and $b -match '(oem\d+\.inf)') {
        L "delete-driver $($Matches[1]) /uninstall"
        pnputil /delete-driver $Matches[1] /uninstall /force 2>&1 | Out-String | ForEach-Object { L $_ }
    }
}
pnputil /scan-devices 2>&1 | Out-String | ForEach-Object { L $_ }
Start-Sleep -Seconds 4
L "driver final: $(Get-DriverInf)"
L "=== FIN E1 v3: correr rt3070_hotread INMEDIATAMENTE ==="
exit 0
