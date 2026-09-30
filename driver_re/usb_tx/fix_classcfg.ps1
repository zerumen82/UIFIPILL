# fix_classcfg.ps1 - ensure NetSetupSvc actually runs, then restart device for class config.
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_classcfg_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== fix_classcfg ==="

L "1. remove device"
pnputil /remove-device 'USB\VID_148F&PID_3070\1.0' | Out-File -Append $log -Encoding utf8

L "2. pre-start NetSetupSvc"
sc.exe start NetSetupSvc | Out-File -Append $log -Encoding utf8
Start-Sleep -Seconds 5
$q = sc.exe query NetSetupSvc | Out-String
if ($q -match 'START_PENDING') {
    L "   still START_PENDING after 5 s - waiting 20 more"
    Start-Sleep -Seconds 20
    $q = sc.exe query NetSetupSvc | Out-String
}
L ($q -replace "`r`n", " | ")

L "3. start WlanSvc if stopped"
$q2 = sc.exe query WlanSvc | Out-String
if ($q2 -match 'STOPPED') { sc.exe start WlanSvc | Out-File -Append $log -Encoding utf8 }

L "4. rescan"
pnputil /scan-devices | Out-File -Append $log -Encoding utf8
L "   waiting 45 s for install + class config"
Start-Sleep -Seconds 45

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $pc = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    L ("RESULT: Status=$($d.Status)  Problem=$pc")
} else { L "RESULT: device not present" }
L "=== done ==="
