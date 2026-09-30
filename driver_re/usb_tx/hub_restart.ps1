# hub_restart.ps1 - Restart the USB root hub hosting the RT3070 to force-load
# the USBPcap class filter WITHOUT rebooting. Also checks elevation.
# Run AS ADMIN. Lab-only.
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Write-Output "Elevated: $isAdmin"
if (-not $isAdmin) { Write-Output "NEEDS ADMIN - run from an elevated shell."; exit 1 }

$hubId = 'USB\ROOT_HUB30\4&12D3D8F7&0&0'
Write-Output "Restarting root hub: $hubId"
pnputil /restart-device "$hubId"
if ($LASTEXITCODE -ne 0) {
    Write-Output "restart-device failed (rc=$LASTEXITCODE), trying disable+enable..."
    pnputil /disable-device "$hubId"
    Start-Sleep -Seconds 3
    pnputil /enable-device "$hubId"
}
Start-Sleep -Seconds 6

Write-Output '--- USBPcap service ---'
sc.exe query USBPcap | Select-String 'STATE|ESTADO'
Write-Output '--- USBPcap PnP devices ---'
Get-PnpDevice | Where-Object { $_.InstanceId -like '*USBPcap*' } |
    ForEach-Object { "{0,-25} {1,-8} {2}" -f $_.FriendlyName, $_.Status, $_.InstanceId }
Write-Output '--- RT3070 back? ---'
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } |
    ForEach-Object { "{0,-40} {1,-8} {2}" -f $_.FriendlyName, $_.Status, $_.InstanceId }

# If a USBPcap control device exists, try enumerating it
$pc = Get-PnpDevice | Where-Object { $_.FriendlyName -like 'USBPcap*' } | Select-Object -First 1
if ($pc) {
    $ctrl = "\\.\USBPcap" + ($pc.FriendlyName -replace '[^0-9]', '')
    Write-Output "Control device candidate: $ctrl"
    & "$env:ProgramFiles\USBPcap\USBPcapCmd.exe" -d $ctrl 2>&1 | Select-Object -First 12
} else {
    Write-Output 'No USBPcap control device yet - filter did NOT attach; reboot may be required.'
}
