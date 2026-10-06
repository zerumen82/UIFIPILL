# fix56_services_restart.ps1 — Code 56 persistente: subir servicios de clase Net y reiniciar el device.
# (Diagnóstico 2026-09-30: driver bound OK vía Kernel-PnP 400, pero class config colgada;
#  NetSetupSvc/NetMan estaban Stopped durante el remove+rescan previo.)
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix56_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== fix 56: servicios clase Net + restart/remove-rescan ==="

L "sc start NetSetupSvc:"
(sc.exe start NetSetupSvc 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
L "sc start NetMan:"
(sc.exe start NetMan 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
Start-Sleep -Seconds 3
Get-Service NetSetupSvc,NetMan,WlanSvc | ForEach-Object { L ("svc {0} = {1}" -f $_.Name, $_.Status) }

$dev = 'USB\VID_148F&PID_3070\1.0'
L "pnputil /restart-device:"
(pnputil /restart-device $dev 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
L "waiting 30 s"
Start-Sleep -Seconds 30

function Get-State {
    $d = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
    if (-not $d) { return @{ present = $false } }
    $pc  = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    $cf  = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ConfigFlags' -ErrorAction SilentlyContinue).Data
    return @{ present = $true; status = $d.Status; problem = $pc; cfgflags = $cf }
}

$s = Get-State
L ("tras restart-device: " + ($s | Out-String).Trim())

if ($s.present -and $s.problem -eq 56) {
    L "sigue 56 → remove + rescan CON servicios de clase arriba"
    (pnputil /remove-device $dev 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
    (pnputil /scan-devices 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
    L "waiting 45 s"
    Start-Sleep -Seconds 45
    $s = Get-State
    L ("tras remove+rescan: " + ($s | Out-String).Trim())
}

L ("veredicto final: present=$($s.present) status=$($s.status) problem=$($s.problem) cfgflags=$($s.cfgflags)")
if ($s.present -and $s.problem -eq 0) {
    L "netsh:"; (netsh wlan show interfaces 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
}
L "=== done ==="
