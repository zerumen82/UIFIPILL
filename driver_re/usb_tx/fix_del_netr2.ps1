# fix_del_netr2.ps1 - delete netr28ux package by published name + rescan -> WinUSB binds.
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_del_netr2_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -Value "elevating..."
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== delete netr28ux by published name ==="

$inst = 'USB\VID_148F&PID_3070\1.0'

L "1. remove device"
pnputil /remove-device $inst | Out-File -Append $log -Encoding utf8

L "2. stop + delete service"
sc.exe stop netr28ux | Out-File -Append $log -Encoding utf8
sc.exe delete netr28ux | Out-File -Append $log -Encoding utf8

L "3. delete package by published name (can take 3-4 min, DO NOT KILL)"
$del = pnputil /delete-driver netr28ux.inf /uninstall /force 2>&1 | Out-String
L ($del -replace "`r`n", " | ")
Start-Sleep -Seconds 10

L "4. rescan (WinUSB oem180 should be the only match now)"
pnputil /scan-devices | Out-File -Append $log -Encoding utf8
Start-Sleep -Seconds 25

$svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
$st  = (Get-PnpDevice -InstanceId $inst -ErrorAction SilentlyContinue).Status
$pc  = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
L ("   service: $svc  status: $st  problem: $pc")

L "5. probe"
$probe = & 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\target\release\rt3070_probe.exe' 2>&1 | Out-String
L ($probe -replace "`r`n", " | ")
L "=== done ==="
