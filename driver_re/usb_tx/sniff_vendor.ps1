# sniff_vendor.ps1 - USBPcap capture of netr28ux command traffic (RX_PLAN.md rev.3).
#
# Goal: RX-WinUSB is blocked because the vendor's BBP access does not go through
# BBP_CSR_CFG (0x11C) visible to us. Static RE of netr28ux.sys is exhausted (all
# string xrefs are log wrappers). The ONLY remaining path: sniff what the vendor
# sends on its BULK command pipes during scan/init and replicate it via WinUSB.
#
# Steps automated here (see RX_PLAN.md rev.3 "PROCEDIMIENTO SNIFFER"):
#   1. Check USBPcap is installed (USBPcapCmd.exe present). If not, print install
#      hint (USBPcapSetup-1.5.4.0.exe + REBOOT) and exit.
#   2. Check the antenna is on netr28ux (restore_netr28ux.ps1 if not).
#   3. Start USBPcapCmd.exe capture on the USBPcap device hosting the RT3070.
#   4. Drive netsh scan loop 30-60 s so the vendor emits command traffic.
#   5. Stop capture -> vendor.pcap in this folder (gitignored).
#
# Run AS ADMIN. Lab-only.
# Usage: sniff_vendor.ps1 [seconds]     (default 45)
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$secs = if ($args.Count -gt 0) { [int]$args[0] } else { 45 }
$pcap = "$dir\vendor.pcap"

# --- 1. USBPcap installed? -----------------------------------------------------
$cmd = "$env:ProgramFiles\USBPcap\USBPcapCmd.exe"
if (-not (Test-Path $cmd)) {
    Write-Output "USBPcap NOT installed."
    Write-Output "Run: $dir\USBPcapSetup-1.5.4.0.exe  (next, next...)  THEN REBOOT."
    Write-Output "Re-run this script after reboot."
    exit 1
}
Write-Output "USBPcap found: $cmd"

# --- 2. Antenna present and on netr28ux? ---------------------------------------
$dev = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
if (-not $dev) {
    Write-Output "RT3070 NOT present on USB. Plug it in (power-cycle if degraded) and re-run."
    exit 1
}
$inf = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
Write-Output "Device: $($dev.FriendlyName)  Status: $($dev.Status)  INF: $inf.inf"
if ($inf -notmatch 'netr28ux') {
    Write-Output "Antenna is NOT on netr28ux (current: $inf.inf)."
    Write-Output "Run: powershell -File $dir\restore_netr28ux.ps1  then re-plug, then re-run."
    exit 1
}

# --- 3. Find the USBPcap control device for the RT3070 --------------------------
# USBPcapCmd lists root hubs/controllers; the RT3070 sits under one of them.
# We pick the USBPcap instance whose parent tree contains our device.
$rtLoc = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_LocationInfo' -ErrorAction SilentlyContinue).Data
Write-Output "RT3070 location: $rtLoc"
$pcapDevs = Get-PnpDevice -Class USB -FriendlyName 'USBPcap*' -ErrorAction SilentlyContinue
if (-not $pcapDevs) {
    # fallback: enumerate hub addresses via USBPcapCmd itself
    Write-Output "No USBPcap PnP device visible; enumerating with USBPcapCmd:"
    & $cmd 2>&1 | Out-String | Write-Output
    Write-Output "Pick the \\.USBPcapN of the controller the RT3070 hangs off and re-run with:"
    Write-Output "  sniff_vendor.ps1 <seconds>  (after setting `$pcapDev below)"
    exit 1
}
$pcapDev = ($pcapDevs | Select-Object -First 1).InstanceId
# USBPcapCmd wants the control device path like \\.\USBPcap0
$ctrl = "\\.\$($pcapDev.Split('\')[-1])"
Write-Output "Capturing on: $ctrl"

# --- 4+5. Capture while scanning -------------------------------------------------
if (Test-Path $pcap) { Remove-Item $pcap -Force }
Write-Output "Starting capture for $secs s while netsh scans..."
$p = Start-Process -FilePath $cmd -ArgumentList @("-d", $ctrl, "-o", $pcap) -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 2

$deadline = (Get-Date).AddSeconds($secs)
$n = 0
while ((Get-Date) -lt $deadline) {
    # scan loop: forces the vendor to emit beacons + command traffic on bulk pipes
    $null = netsh wlan show networks mode=bssid 2>$null
    $n++
    Start-Sleep -Seconds 2
}
if (-not $p.HasExited) { $p.Kill() }
Wait-Process -Id $p.Id -ErrorAction SilentlyContinue

if (Test-Path $pcap) {
    $sz = (Get-Item $pcap).Length
    Write-Output "OK: $pcap ($sz bytes, $n netsh scans)."
    Write-Output "Next (see RX_PLAN.md rev.3): open in Wireshark, filter bulk OUT of 148f:3070,"
    Write-Output "small transfers 32-64 B to EP 0x05/0x0d = command packets [cmd][reg][val]..."
} else {
    Write-Output "FAIL: no pcap written. Check USBPcap root enumeration."
    exit 1
}
