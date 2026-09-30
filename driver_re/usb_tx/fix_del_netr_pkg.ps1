# fix_del_netr_pkg.ps1 - delete netr28ux package from DriverStore + rescan
# so WinUSB (oem180) becomes the only match (E1 recipe, verified working).
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix_del_netr_pkg_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Set-Content -Path $log -Value "elevating..."
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== delete netr28ux package (E1 recipe) ==="

L "1. find oem package for netr28ux.inf"
$enum = pnputil /enum-drivers | Out-String
$blocks = $enum -split "(?=Published Name:)"
$oem = $null
foreach ($b in $blocks) {
    if ($b -match '(?i)Original Name:\s*netr28ux\.inf') {
        if ($b -match '(?i)Published Name:\s*(oem\d+\.inf)') { $oem = $Matches[1]; break }
    }
}
if (-not $oem) {
    # fallback: linear scan
    $lines = pnputil /enum-drivers
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '(?i)netr28ux\.inf') {
            for ($j = $i; $j -ge [Math]::Max(0, $i-8); $j--) {
                if ($lines[$j] -match '(?i)(oem\d+\.inf)') { $oem = $Matches[1]; break }
            }
        }
        if ($oem) { break }
    }
}
L ("   package: " + $(if ($oem) { $oem } else { 'NONE' }))

if ($oem) {
    L "2. delete $oem /uninstall /force (can take 3-4 min, DO NOT KILL)"
    $del = pnputil /delete-driver $oem /uninstall /force 2>&1 | Out-String
    L ($del -replace "`r`n", " | ")
    Start-Sleep -Seconds 10
} else {
    L "2. no oem package - check DriverStore dirs"
    $dirs = Get-ChildItem 'C:\Windows\System32\DriverStore\FileRepository' -Directory -Filter 'netr28ux*' -ErrorAction SilentlyContinue
    L ("   store dirs: " + (($dirs | ForEach-Object Name) -join ', '))
}

L "3. rescan (WinUSB oem180 should bind now)"
pnputil /scan-devices | Out-File -Append $log -Encoding utf8
Start-Sleep -Seconds 20

$inst = 'USB\VID_148F&PID_3070\1.0'
$svc = (Get-PnpDeviceProperty -InstanceId $inst -KeyName 'DEVPKEY_Device_Service' -ErrorAction SilentlyContinue).Data
$st  = (Get-PnpDevice -InstanceId $inst -ErrorAction SilentlyContinue).Status
L ("   service: $svc  status: $st")

L "4. probe"
$probe = & 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\target\release\rt3070_probe.exe' 2>&1 | Out-String
L ($probe -replace "`r`n", " | ")
L "=== done ==="
