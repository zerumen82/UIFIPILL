# launch_redeploy_elevated.ps1 — lanza run_redeploy.cmd elevado (UAC) y escribe marca.
$ErrorActionPreference = 'Stop'
$mark = Join-Path $PSScriptRoot 'uac_launch_marker.txt'
$cmd  = Join-Path $PSScriptRoot 'run_redeploy.cmd'
try {
    $p = Start-Process -FilePath 'cmd.exe' -ArgumentList "/c `"$cmd`"" -Verb RunAs -PassThru
    Set-Content -Path $mark -Value ("LAUNCHED pid=" + $p.Id + " at " + (Get-Date -Format o))
    Write-Host ("LAUNCH OK pid=" + $p.Id)
} catch {
    Set-Content -Path $mark -Value ("LAUNCH FAIL: " + $_.Exception.Message)
    Write-Host ("LAUNCH FAIL: " + $_.Exception.Message)
    exit 1
}
