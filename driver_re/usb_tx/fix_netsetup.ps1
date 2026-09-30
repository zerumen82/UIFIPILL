# fix_netsetup.ps1 - start NetSetupSvc/nlasvc and restart the device.
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_netsetup_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== fix_netsetup started ==="
L ("NetSetupSvc start type: " + (Get-Service NetSetupSvc).StartType)
L ("nlasvc start type: " + (Get-Service nlasvc).StartType)

# ensure start types are sane
sc.exe config NetSetupSvc start= delayed-auto | Out-File -Append $log -Encoding utf8
sc.exe config nlasvc start= auto | Out-File -Append $log -Encoding utf8

L "starting NetSetupSvc"
sc.exe start NetSetupSvc | Out-File -Append $log -Encoding utf8
L "starting nlasvc"
sc.exe start nlasvc | Out-File -Append $log -Encoding utf8
Start-Sleep -Seconds 3

L "restarting device"
pnputil /restart-device 'USB\VID_148F&PID_3070\1.0' | Out-File -Append $log -Encoding utf8
L "waiting 30 s"
Start-Sleep -Seconds 30

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $pc = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    L ("RESULT: Status=$($d.Status)  Problem=$pc")
} else { L "RESULT: device not present" }
L "=== done ==="
