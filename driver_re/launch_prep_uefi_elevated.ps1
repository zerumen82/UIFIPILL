# launch_prep_uefi_elevated.ps1 — lanza run_prep_uefi.cmd elevado (UAC) y escribe marca.
$ErrorActionPreference = 'Stop'
$mark = Join-Path $PSScriptRoot 'uac_prep_marker.txt'
$cmd  = Join-Path $PSScriptRoot 'run_prep_uefi.cmd'
try {
    $p = Start-Process -FilePath 'cmd.exe' -ArgumentList "/c `"$cmd`"" -Verb RunAs -PassThru
    Set-Content -Path $mark -Value ("LAUNCHED pid=" + $p.Id + " at " + (Get-Date -Format o))
    Write-Host ("LAUNCH OK pid=" + $p.Id)
} catch {
    Set-Content -Path $mark -Value ("LAUNCH FAIL: " + $_.Exception.Message)
    Write-Host ("LAUNCH FAIL: " + $_.Exception.Message)
    exit 1
}
