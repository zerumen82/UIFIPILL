Write-Host "=== Dispositivos USB presentes (enumerados AHORA) ==="
Get-CimInstance Win32_USBHub | ForEach-Object { $_.DeviceID } | Sort-Object
Write-Host ""
Write-Host "=== RT3070 (VID_148F) ==="
Get-PnpDevice | Where-Object InstanceId -match 'VID_148F' | Format-Table FriendlyName,Status,Problem,InstanceId -AutoSize | Out-String -Width 140
Write-Host "=== Hubs USB y puertos ==="
Get-CimInstance Win32_USBHub | Where-Object DeviceID -match 'ROOT_HUB' | Format-Table DeviceID -AutoSize
Write-Host "=== Ultimos eventos Kernel-PnP del RT3070 (2h) ==="
try {
  Get-WinEvent -FilterHashtable @{LogName='Microsoft-Windows-Kernel-PnP/Configuration'; StartTime=(Get-Date).AddHours(-2)} -MaxEvents 200 -ErrorAction Stop |
    Where-Object { $_.Message -match '148F|3070|WLAN' } |
    Select-Object -First 12 TimeCreated, Id, Message | Format-Table -Wrap | Out-String -Width 160
} catch { Write-Host "(sin eventos o log no accesible: $_)" }
