# fix_dev56b.ps1 - Clear CONFIGFLAG_FAILEDINSTALL (0x80000) on RT3070 and restart it.
# Run AS ADMIN (auto-elevates + transcript).
$ErrorActionPreference = 'Continue'
Start-Transcript -Path "$PSScriptRoot\fix_dev56b_log.txt" -Force

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Stop-Transcript
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

$inst = 'USB\VID_148F&PID_3070\1.0'
$enum = 'HKLM:\SYSTEM\CurrentControlSet\Enum\USB\VID_148F&PID_3070\1.0'

Write-Output "=== clearing ConfigFlags ==="
$k = Get-ItemProperty $enum -ErrorAction SilentlyContinue
if ($k) {
    Write-Output "  before: ConfigFlags=$($k.ConfigFlags)"
    Set-ItemProperty $enum -Name ConfigFlags -Value 0 -Type DWord
    Write-Output "  after:  ConfigFlags=$((Get-ItemProperty $enum).ConfigFlags)"
} else {
    Write-Output "  enum key NOT found (device removed?) - will rescan"
}

# Make sure the service is not disabled
sc.exe config netr28ux start= demand 2>&1 | Out-String

Write-Output "=== restart-device ==="
pnputil /restart-device "$inst" 2>&1 | Out-String

Write-Output "Waiting 15 s..."
Start-Sleep -Seconds 15

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $inf  = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    $code = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    Write-Output "RESULT: Status=$($d.Status)  INF=$inf  ProblemCode=$code"
} else {
    Write-Output "RESULT: device NOT present - re-plug."
}
Stop-Transcript
