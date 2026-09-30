# hot_rebind.ps1 - netr28ux <-> WinUSB driver switch WITHOUT power-cycle.
#
# Goal: RX on the SAME antenna like Linux. Vendor driver netr28ux DOES wake
# the BBP (Npcap captures of 392 pkts prove it). Plan:
#   1. Start with netr28ux (radio alive, BBP initialized by vendor)
#   2. Switch to WinUSB WITHOUT electrical reset (USB re-enumeration only)
#   3. rt3070_hotread: inherits the live state (BBP/RF/MCU already running)
#
# MECHANISM: UpdateDriverForPlugAndPlayDevices changes the driver WITHOUT
# disabling the device or touching USB power. Re-enumeration is an interface
# reconfiguration, not a chip reset.
#
# Run AS ADMIN. Lab-only.
# Usage: hot_rebind.ps1 [to-winusb | to-netr28ux | status]
$ErrorActionPreference = 'Continue'
$dir  = $PSScriptRoot
$hwid = 'USB\VID_148F&PID_3070'

function Get-Driver([string]$infName) {
    $enum = pnputil /enum-drivers | Out-String
    $blocks = $enum -split '(?=Published Name|Nombre publicado)'
    foreach ($b in $blocks) {
        if ($b -match [regex]::Escape($infName) -and $b -match '(oem\d+\.inf)') { return $Matches[1] }
    }
    return $null
}

function Show-Status {
    $dev = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
    if (-not $dev) { Write-Output "Device NOT present."; return }
    $drv = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverDescription' -ErrorAction SilentlyContinue).Data
    $inf = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    Write-Output "Device: $($dev.FriendlyName)  Status: $($dev.Status)"
    Write-Output "Driver: $drv  (INF: $inf.inf)"
}

function Switch-Driver([string]$infPath) {
    $src = @"
using System;
using System.Runtime.InteropServices;
public static class DrvSwitch {
    [DllImport("newdev.dll", SetLastError=true)]
    public static extern bool UpdateDriverForPlugAndPlayDevices(
        IntPtr hwndParent,
        [MarshalAs(UnmanagedType.LPWStr)] string HardwareId,
        [MarshalAs(UnmanagedType.LPWStr)] string FullInfPath,
        uint InstallFlags,
        out bool bRebootRequired);
}
"@
    Add-Type -TypeDefinition $src
    $reboot = $false
    # INSTALLFLAG_FORCE = 0x00000001
    $ok = [DrvSwitch]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $infPath, 1, [ref]$reboot)
    if ($ok) { Write-Output "OK UpdateDriver (reboot needed: $reboot)" }
    else {
        $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        Write-Output "FAIL UpdateDriver win32err=$err"
    }
}

$mode = if ($args.Count -gt 0) { $args[0] } else { 'status' }

switch ($mode) {

'status' {
    Show-Status
}

'to-winusb' {
    Write-Output "== Initial state =="
    Show-Status
    Write-Output ""

    $winusbInf = Get-Driver 'rt3070_winusb.inf'
    if (-not $winusbInf) {
        Write-Output "rt3070_winusb.inf NOT in store - trying local INF file..."
        $infPath = "$dir\rt3070_winusb.inf"
        if (-not (Test-Path $infPath)) { $infPath = "$dir\..\rt3070_winusb.inf" }
        if (-not (Test-Path $infPath)) { Write-Output "ERROR: rt3070_winusb.inf not found"; exit 1 }
        $infPath = (Resolve-Path $infPath).Path
    } else {
        $infPath = "$env:windir\INF\$winusbInf"
    }
    Write-Output "WinUSB INF: $infPath"
    Switch-Driver $infPath

    Start-Sleep -Seconds 3
    Write-Output "== Final state =="
    Show-Status
}

'to-netr28ux' {
    Write-Output "== Initial state =="
    Show-Status
    Write-Output ""
    $netr = Get-Driver 'netr28ux.inf'
    if ($netr) {
        $infPath = "$env:windir\INF\$netr"
    } else {
        Write-Output "netr28ux not in store as oem*.inf - trying DriverStore repo..."
        $repo = Get-ChildItem "$env:SystemRoot\System32\DriverStore\FileRepository" -Filter 'netr28ux.inf_amd64_*' -Directory | Select-Object -First 1
        if ($repo) {
            $infPath = Join-Path $repo.FullName 'netr28ux.inf'
        } else {
            $inbox = "$env:SystemRoot\INF\netr28ux.inf"
            if (-not (Test-Path $inbox)) { $inbox = "$inbox.uifipill-bak" }
            if (-not (Test-Path $inbox)) { Write-Output "ERROR: no store, no repo, no inbox - use restore_netr28ux.ps1"; exit 1 }
            $infPath = $inbox
        }
    }
    Write-Output "netr28ux INF: $infPath"
    Switch-Driver $infPath

    Start-Sleep -Seconds 3
    Write-Output "== Final state =="
    Show-Status
}
}
