# Launcher UAC para hotswap_tx1.ps1 (igual que launch_redeploy_elevated.ps1)
$ErrorActionPreference = 'Stop'
$dir = $PSScriptRoot
$proc = Start-Process -FilePath "$dir\run_hotswap_tx1.cmd" -Verb RunAs -PassThru -WorkingDirectory $dir
$proc.WaitForExit()
Write-Host "EXITCODE: $($proc.ExitCode)"
Write-Host "Log: $dir\hotswap_tx1_log.txt"
Get-Content "$dir\hotswap_tx1_log.txt"
