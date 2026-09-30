# fix_dev56c.ps1 - Start NetSetupSvc (needed for NDIS class config post-install)
# then restart the RT3070 device. Run AS ADMIN (auto-elevates + transcript).
$ErrorActionPreference = 'Continue'
Start-Transcript -Path "$PSScriptRoot\fix_dev56c_log.txt" -Force

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Stop-Transcript
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Write-Output "=== starting network services ==="
foreach ($svc in @('NetMan','NetSetupSvc')) {
    Start-Service $svc -ErrorAction Continue
    Start-Sleep 2
    Write-Output "  $svc : $((Get-Service $svc).Status)"
}

$inst = 'USB\VID_148F&PID_3070\1.0'
Write-Output "=== restart-device ==="
pnputil /restart-device "$inst" 2>&1 | Out-String

Write-Output "Waiting 20 s for class config..."
Start-Sleep -Seconds 20

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $code = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    Write-Output "RESULT: Status=$($d.Status)  INF=$((Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data)  ProblemCode=$code"
} else {
    Write-Output "RESULT: device NOT present."
}

Write-Output "=== service state after ==="
Get-Service NetSetupSvc, NetMan | Select-Object Name, Status | Format-Table | Out-String

# Recent SCM errors about NetSetupSvc
Write-Output "=== recent System log (Service Control Manager, last 10 min) ==="
Get-WinEvent -FilterHashtable @{LogName='System'; ProviderName='Service Control Manager'; StartTime=(Get-Date).AddMinutes(-10)} -MaxEvents 10 -ErrorAction SilentlyContinue |
    Select-Object TimeCreated, Id, Message | Format-List | Out-String
Stop-Transcript
