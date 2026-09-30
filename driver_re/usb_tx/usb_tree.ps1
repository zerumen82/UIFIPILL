# usb_tree.ps1 - Show the USB parent chain of the RT3070 and all root hubs.
# Used to pick the right \\.USBPcapN control device for sniff_vendor.ps1.
$rt = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
if (-not $rt) { Write-Output 'RT3070 NOT present'; exit 1 }
Write-Output "RT3070: $($rt.InstanceId)  Status: $($rt.Status)"
$id = $rt.InstanceId
for ($i = 0; $i -lt 8 -and $id; $i++) {
    $parent = (Get-PnpDeviceProperty -InstanceId $id -KeyName 'DEVPKEY_Device_Parent' -ErrorAction SilentlyContinue).Data
    if (-not $parent) { break }
    $dev = Get-PnpDevice -InstanceId $parent -ErrorAction SilentlyContinue
    Write-Output ("parent{0}: {1}  [{2}]" -f $i, $dev.FriendlyName, $dev.InstanceId)
    $id = $parent
}
Write-Output '--- all root hubs / host controllers ---'
Get-PnpDevice -Class USB |
    Where-Object { $_.InstanceId -match 'ROOT_HUB' -or $_.FriendlyName -match 'Host Controller' } |
    ForEach-Object { "{0,-45} {1,-8} {2}" -f $_.FriendlyName, $_.Status, $_.InstanceId }
