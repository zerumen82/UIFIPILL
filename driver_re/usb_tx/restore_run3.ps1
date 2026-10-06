# restore_run3.ps1 — UN SOLO UAC: restore completo netr28ux. Secuencia fusionada
# de restore_netr28ux + fix56_inf_restore (la que sanó el device 2026-09-30).
# SIN transcript (deadlock transcript+pnputil medido 2026-10-01). pnputil por
# Start-Process con redirect a fichero + watchdog. Log a ruta absoluta.
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
    } else { L ("  exit={0}: pnputil {1}" -f $p.ExitCode, ($argz -join ' ')) }
    L ("  out: " + (((Get-Content $of -Raw -EA SilentlyContinue) -replace '\s+', ' ').Trim()))
    L ("  err: " + (((Get-Content $ef -Raw -EA SilentlyContinue) -replace '\s+', ' ').Trim()))
    Remove-Item $of,$ef -Force -EA SilentlyContinue
}
L "=== restore_run3 START pid=$PID ==="
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
L "admin=$isAdmin"
if (-not $isAdmin) { L "NO ADMIN - abort"; exit 1 }

$devId = 'USB\VID_148F&PID_3070\1.0'
$repo  = 'C:\Windows\System32\DriverStore\FileRepository\netr28ux.inf_amd64_2613a90929adebda'
$enum  = "HKLM:\SYSTEM\CurrentControlSet\Enum\$devId"

L "== A: INF+PNF a C:\Windows\INF (raiz fix56: NetSetup 0x80070002) =="
foreach ($pair in @(@("$repo\netr28ux.inf", 'C:\Windows\INF\netr28ux.inf'), @("$repo\netr28ux.PNF", 'C:\Windows\INF\netr28ux.PNF'))) {
    try {
        if (Test-Path $pair[0]) { Copy-Item $pair[0] $pair[1] -Force -ErrorAction Stop; L ("  OK {0} ({1} B)" -f $pair[1], (Get-Item $pair[1]).Length) }
        else { L ("  FALTA fuente {0}" -f $pair[0]) }
    } catch { L ("  ERROR copia {0}: {1}" -f $pair[1], $_.Exception.Message) }
}
L "== B: limpiar ConfigFlags 0x80000 =="
try { Set-ItemProperty -Path $enum -Name ConfigFlags -Value 0 -Type DWord -ErrorAction Stop; L ("  ConfigFlags=" + (Get-ItemProperty $enum -Name ConfigFlags).ConfigFlags) }
catch { L ("  ERROR ConfigFlags: " + $_.Exception.Message) }
L "== C: remove-device =="
Run-Pnp @('/remove-device', $devId) 45
L "== D: delete paquete WinUSB oem180 =="
Run-Pnp @('/delete-driver', 'oem180.inf', '/uninstall', '/force') 90
L "== D2: verif paquete WinUSB =="
$of = [System.IO.Path]::GetTempFileName(); $ef = [System.IO.Path]::GetTempFileName()
$p = Start-Process -FilePath 'pnputil.exe' -ArgumentList '/enum-drivers' -NoNewWindow -PassThru -RedirectStandardOutput $of -RedirectStandardError $ef
if ($p.WaitForExit(60000)) {
    if (Select-String -Path $of -Pattern 'rt3070_winusb' -Quiet -EA SilentlyContinue) { L "  WINUSB AUN PUBLICADO" }
    else { L "  WinUSB pkg ya no esta publicado OK" }
} else { try { $p.Kill() } catch {}; L "  TIMEOUT enum-drivers" }
Remove-Item $of,$ef -Force -EA SilentlyContinue
L "== E: add+install netr28ux =="
Run-Pnp @('/add-driver', "$repo\netr28ux.inf", '/install') 90
L "== F: scan-devices =="
Run-Pnp @('/scan-devices') 60
L "esperando 45 s (class config - leccion fix56)..."
Start-Sleep -Seconds 45
L "== G: verify =="
$d = Get-PnpDevice -EA SilentlyContinue | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
$dd = $null
if ($d) {
    $pc = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -EA SilentlyContinue).Data
    $dd = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_DriverDescription' -EA SilentlyContinue).Data
    L ("  device='{0}' Status={1} ProblemCode={2} driver='{3}'" -f $d.FriendlyName, $d.Status, $pc, $dd)
} else { L "  device NO PRESENTE" }
L ("  ConfigFlags=" + ((Get-ItemProperty $enum -Name ConfigFlags -EA SilentlyContinue).ConfigFlags))
L "  netsh:"
(netsh wlan show interfaces 2>&1 | Out-String) -split "`n" | ForEach-Object { if ($_.Trim()) { L ("    " + $_.Trim()) } }
if ($d -and $d.Status -eq 'OK' -and $dd -match 'netr28|802\.11|Ralink|MediaTek|Wireless') { L "RESULTADO: OK netr28ux restaurado y device OK" }
else { L "RESULTADO: REVISAR (seguir con fix56 o redeploy)" }
L "=== restore_run3 DONE ==="
