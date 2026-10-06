# restore_run2.ps1 — restore de netr28ux SIN Start-Transcript (el transcript
# de PS5.1 en contexto elevado causó deadlock con pnputil, medido 2026-10-01).
# pnputil va por Start-Process con redirect a fichero + watchdog. Log ABSOLUTO.
$ErrorActionPreference = 'Continue'
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\restore_run.log'
function L($m) { Add-Content -LiteralPath $log -Value ("{0} {1}" -f (Get-Date -Format s), $m) }
function Run-Pnp([string[]]$argz, [int]$tmoSec = 60) {
    $of = [System.IO.Path]::GetTempFileName(); $ef = [System.IO.Path]::GetTempFileName()
    $p = Start-Process -FilePath 'pnputil.exe' -ArgumentList $argz -NoNewWindow -PassThru `
        -RedirectStandardOutput $of -RedirectStandardError $ef
    if (-not $p.WaitForExit($tmoSec * 1000)) {
        try { $p.Kill() } catch {}
        L ("  TIMEOUT {0}s: pnputil {1}" -f $tmoSec, ($argz -join ' '))
    } else { L ("  pnputil exit={0}: {1}" -f $p.ExitCode, ($argz -join ' ')) }
    L ("  out: " + ((Get-Content $of -Raw -EA SilentlyContinue) -replace '\s+', ' ').Trim())
    L ("  err: " + ((Get-Content $ef -Raw -EA SilentlyContinue) -replace '\s+', ' ').Trim())
    Remove-Item $of,$ef -Force -EA SilentlyContinue
}
L "=== restore_run2 START pid=$PID ==="

# 0/4 matar el arbol colgado de run1 (pnputil 17996 / inner 10628 / wrapper 18796)
foreach ($id in 17996, 10628, 18796) {
    if (Get-Process -Id $id -EA SilentlyContinue) {
        try { Stop-Process -Id $id -Force -EA Stop; L "killed $id" } catch { L "kill $id fallo: $($_.Exception.Message)" }
    } else { L "pid $id ya no existe" }
}
Start-Sleep -Seconds 1

# 1/4 desinstalar paquete WinUSB (oem180.inf medido 2026-10-01)
L "== 1/4 delete WinUSB pkg =="
Run-Pnp @('/delete-driver','oem180.inf','/uninstall','/force') 90

# 2/4 reinstalar netr28ux desde DriverStore
L "== 2/4 add+install netr28ux =="
$infs = Get-ChildItem 'C:\Windows\System32\DriverStore\FileRepository' -Filter 'netr28ux.inf' -Recurse -EA SilentlyContinue | Select-Object -First 1
if ($infs) { L "  fuente: $($infs.FullName)"; Run-Pnp @('/add-driver', $infs.FullName, '/install') 90 }
else { L "  AVISO: NO hay netr28ux.inf en FileRepository" }

# 3/4 INF inbox + rescan
L "== 3/4 inbox INF + rescan =="
$inbox = "$env:SystemRoot\INF\netr28ux.inf"; $backup = "$inbox.uifipill-bak"
if ((-not (Test-Path $inbox)) -and (Test-Path $backup)) { Copy-Item $backup $inbox -Force; L "  INF inbox restaurado desde backup" }
else { L "  inbox INF: $(if (Test-Path $inbox) { 'presente' } else { 'AUSENTE sin backup' })" }
Run-Pnp @('/scan-devices') 60
Start-Sleep -Seconds 5

# 4/4 verificacion
L "== 4/4 verify =="
$inst = 'USB\VID_148F&PID_3070\1.0'
$drv = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_DriverDescription' -EA SilentlyContinue).Data
L "Driver activo: '$drv'"
Get-PnpDevice -EA SilentlyContinue | Where-Object { $_.InstanceId -like '*VID_148F*' } | ForEach-Object {
    L ("  device: '{0}' Status={1} Problem={2}" -f $_.FriendlyName, $_.Status, $_.Problem)
}
if ($drv -match 'netr28|802\.11|Ralink|MediaTek|Wireless') { L "RESULTADO: OK netr28ux restaurado." }
else { L "RESULTADO: REVISAR driver inesperado '$drv'." }
L "=== restore_run2 DONE ==="
