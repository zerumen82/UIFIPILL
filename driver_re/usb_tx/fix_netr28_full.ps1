# fix_netr28_full.ps1 - clean reinstall of netr28ux driver package.
# Fixes problem 31 (post-install poisoned state after HVCI reboot).
# Run AS ADMIN (auto-elevates + transcript). Delete/uninstall can take 3-4 min.
$ErrorActionPreference = 'Continue'
Start-Transcript -Path "$PSScriptRoot\fix_netr28_full_log.txt" -Force

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Stop-Transcript
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

$inst  = 'USB\VID_148F&PID_3070\1.0'
$store = 'C:\Windows\System32\DriverStore\FileRepository\netr28ux.inf_amd64_2613a90929adebda'
$backup = "$env:TEMP\netr28ux_backup"

Write-Output "=== 1. backup INF package to $backup ==="
if (-not (Test-Path $backup)) {
    Copy-Item -Recurse -Force $store $backup
    Write-Output "backed up."
} else {
    Write-Output "backup already exists."
}

Write-Output "=== 2. remove device instance ==="
pnputil /remove-device $inst 2>&1 | Out-String

Write-Output "=== 3. find and delete netr28ux package (may take minutes) ==="
$enum = pnputil /enum-drivers | Out-String
# parse blocks: OEM name + original inf name
$oem = $null
$lines = $enum -split "`r?`n"
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^Published Name:\s+(oem\d+\.inf)') {
        $cur = $Matches[1]
        for ($j = $i+1; $j -lt [Math]::Min($i+10, $lines.Count); $j++) {
            if ($lines[$j] -match 'Original Name:\s+netr28ux\.inf') { $oem = $cur; break }
            if ($lines[$j] -match '^Published Name:') { break }
        }
    }
    if ($oem) { break }
}
if ($oem) {
    Write-Output "deleting package $oem ..."
    pnputil /delete-driver $oem /uninstall /force 2>&1 | Out-String
} else {
    Write-Output "no oem package found for netr28ux.inf (may be inbox) - continuing."
}

Write-Output "=== 4. delete stale service key ==="
& sc.exe stop netr28ux 2>&1 | Out-String
& sc.exe delete netr28ux 2>&1 | Out-String
Start-Sleep -Seconds 2

Write-Output "=== 5. re-add driver package from backup ==="
pnputil /add-driver "$backup\netr28ux.inf" /install 2>&1 | Out-String

Write-Output "=== 6. rescan ==="
pnputil /scan-devices 2>&1 | Out-String
Write-Output "Waiting 25 s..."
Start-Sleep -Seconds 25

$d = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $inf = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    $pc  = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    Write-Output "RESULT: Status=$($d.Status)  INF=$inf  Problem=$pc"
} else {
    Write-Output "RESULT: device NOT present after rescan - re-plug it."
}
Stop-Transcript
