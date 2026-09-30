# to_netr28ux.ps1 - Remove WinUSB (oem395) binding so netr28ux takes over.
# Recipe verified in memory.md part 4 rev.2 (E1). Run AS ADMIN (auto-elevates + transcript).
$ErrorActionPreference = 'Continue'
Start-Transcript -Path "$PSScriptRoot\to_netr28ux_log.txt" -Force

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Stop-Transcript
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

$inst = 'USB\VID_148F&PID_3070\1.0'

Write-Output "=== removing device instance ==="
pnputil /remove-device "$inst" 2>&1 | Out-String

Write-Output "=== deleting WinUSB package oem395 ==="
pnputil /delete-driver oem395.inf /uninstall 2>&1 | Out-String
# NOTE: uninstall can take 3-4 minutes (known timeout) - do not kill.

Write-Output "=== rescan (netr28ux should bind) ==="
pnputil /scan-devices 2>&1 | Out-String
Write-Output "Waiting 20 s..."
Start-Sleep -Seconds 20

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $inf = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    Write-Output "RESULT: Status=$($d.Status)  INF=$inf"
} else {
    Write-Output "RESULT: device NOT present after rescan - re-plug it."
}
Stop-Transcript
