# fix_restart_dev.ps1 - simple device restart with result logging.
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_restart_dev_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== restart device ==="
L "remove + rescan (full re-install)"
pnputil /remove-device 'USB\VID_148F&PID_3070\1.0' | Out-File -Append $log -Encoding utf8
pnputil /scan-devices | Out-File -Append $log -Encoding utf8
L "waiting 40 s"
Start-Sleep -Seconds 40

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $pc = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    L ("RESULT: Status=$($d.Status)  Problem=$pc")
} else { L "RESULT: device not present" }
L "=== done ==="
