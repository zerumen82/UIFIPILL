# fix_dev56.ps1 - Repair CM_PROB_REGISTRY (56) on RT3070 after reboot.
# Removes the stale device instance and rescans so netr28ux reinstalls cleanly
# from the DriverStore. Run AS ADMIN (auto-elevates).
# Lab-only.
$ErrorActionPreference = 'Continue'
Start-Transcript -Path "$PSScriptRoot\fix_dev56_log.txt" -Force

# --- auto-elevate ---------------------------------------------------------------
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Write-Output "=== fix_dev56 ==="
$dev = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
if (-not $dev) {
    Write-Output "RT3070 not present. Plug it in and re-run."
    exit 1
}
$inst = $dev.InstanceId
Write-Output "Device: $inst  Status: $($dev.Status)"

# Remove the stale instance (this is what clears CM_PROB_REGISTRY)
Write-Output "pnputil /remove-device ..."
$p1 = pnputil /remove-device "$inst" 2>&1
$p1 | ForEach-Object { Write-Output $_ }

Start-Sleep -Seconds 2
Write-Output "pnputil /scan-devices ..."
$p2 = pnputil /scan-devices 2>&1
$p2 | ForEach-Object { Write-Output $_ }

Write-Output "Waiting 12 s for reinstall..."
Start-Sleep -Seconds 12

$d2 = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
if ($d2) {
    $inf = (Get-PnpDeviceProperty -InstanceId $d2.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    $code = (Get-PnpDeviceProperty -InstanceId $d2.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    Write-Output "RESULT: Status=$($d2.Status)  INF=$inf  ProblemCode=$code"
} else {
    Write-Output "RESULT: device not present after rescan (re-plug manually)."
}
Stop-Transcript
