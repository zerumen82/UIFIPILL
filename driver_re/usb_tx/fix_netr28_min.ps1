# fix_netr28_min.ps1 - minimal clean reinstall, logs to file directly (no transcript).
$ErrorActionPreference = 'Continue'
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_netr28_min_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -value "not admin - elevating"
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== fix_netr28_min started as admin ==="
$inst  = 'USB\VID_148F&PID_3070\1.0'
$store = 'C:\Windows\System32\DriverStore\FileRepository\netr28ux.inf_amd64_2613a90929adebda'
$backup = 'C:\Windows\Temp\netr28ux_backup'

L "1. backup"
if (-not (Test-Path $backup)) { Copy-Item -Recurse -Force $store $backup }
L ("   backup done: " + (Test-Path "$backup\netr28ux.inf"))

L "2. remove device"
pnputil /remove-device $inst | Out-File -Append $log -Encoding utf8

L "3. find oem package for netr28ux.inf"
$oem = $null
$lines = pnputil /enum-drivers
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match 'netr28ux\.inf') {
        for ($j = $i; $j -ge [Math]::Max(0, $i-6); $j--) {
            if ($lines[$j] -match '(oem\d+\.inf)') { $oem = $Matches[1]; break }
        }
    }
    if ($oem) { break }
}
L ("   oem package: " + $(if ($oem) { $oem } else { 'NONE (inbox?)' }))

if ($oem) {
    L "4. delete package $oem (can take minutes, do not kill)"
    pnputil /delete-driver $oem /uninstall /force | Out-File -Append $log -Encoding utf8
    L "   delete done"
}

L "5. delete service"
sc.exe stop netr28ux | Out-File -Append $log -Encoding utf8
sc.exe delete netr28ux | Out-File -Append $log -Encoding utf8
Start-Sleep -Seconds 2

L "6. re-add package from backup"
pnputil /add-driver "$backup\netr28ux.inf" /install | Out-File -Append $log -Encoding utf8

L "7. rescan + wait"
pnputil /scan-devices | Out-File -Append $log -Encoding utf8
Start-Sleep -Seconds 25

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $inf = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    $pc  = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    L ("RESULT: Status=$($d.Status)  INF=$inf  Problem=$pc")
} else {
    L "RESULT: device NOT present - re-plug it."
}
L "=== done ==="
