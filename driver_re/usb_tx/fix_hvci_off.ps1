# fix_hvci_off.ps1 - Disable HVCI (Memory Integrity) so the legacy netr28ux driver
# can load again. Same effect as Windows Security > Core isolation toggle.
# REBOOT REQUIRED. Run AS ADMIN (auto-elevates + transcript).
$ErrorActionPreference = 'Continue'
Start-Transcript -Path "$PSScriptRoot\fix_hvci_off_log.txt" -Force

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Stop-Transcript
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

$hg  = 'HKLM:\SYSTEM\CurrentControlSet\Control\DeviceGuard'
$hv  = "$hg\Scenarios\HypervisorEnforcedCodeIntegrity"

Write-Output "=== before ==="
Write-Output "HVCI Enabled: $((Get-ItemProperty $hv -ErrorAction SilentlyContinue).Enabled)"
Write-Output "VBS EnableVirtualizationBasedSecurity: $((Get-ItemProperty $hg -ErrorAction SilentlyContinue).EnableVirtualizationBasedSecurity)"

Write-Output "=== disabling HVCI ==="
if (-not (Test-Path $hv)) { New-Item -Path $hv -Force | Out-Null }
Set-ItemProperty $hv -Name Enabled -Value 0 -Type DWord
# Locked=0 lets the UI toggle change it later if desired
Set-ItemProperty $hv -Name Locked -Value 0 -Type DWord -ErrorAction SilentlyContinue
# Make sure VBS itself is not mandated (audit mode off, no mandation)
Set-ItemProperty $hg -Name EnableVirtualizationBasedSecurity -Value 0 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty $hg -Name RequirePlatformSecurityFeatures -Value 0 -Type DWord -ErrorAction SilentlyContinue

Write-Output "=== after (persistent config) ==="
Write-Output "HVCI Enabled: $((Get-ItemProperty $hv).Enabled)"
Write-Output "VBS EnableVirtualizationBasedSecurity: $((Get-ItemProperty $hg -ErrorAction SilentlyContinue).EnableVirtualizationBasedSecurity)"

Write-Output ""
Write-Output "REBOOT REQUIRED. After reboot, re-run the antenna checks."
Stop-Transcript
