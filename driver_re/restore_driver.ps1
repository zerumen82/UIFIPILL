# ============================================================================
# restore_driver.ps1 — Rollback completo del parche de netr28ux.sys
# EJECUTAR COMO ADMINISTRADOR. Requiere reinicio al final.
# ============================================================================
$ErrorActionPreference = 'Stop'

$sys = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$bak = "$env:SystemRoot\System32\drivers\netr28ux.sys.orig"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }
if (-not (Test-Path $bak)) { throw "No hay backup en $bak — nada que restaurar." }

Write-Host '== 1/3 Restaurar driver original =='
Stop-Service netr28ux -ErrorAction SilentlyContinue
sc.exe stop netr28ux 2>$null | Out-Null
Copy-Item $bak $sys -Force
Write-Host "   restaurado $bak -> $sys"

Write-Host '== 2/3 Desactivar testsigning =='
bcdedit /set testsigning off | Out-Null

Write-Host '== 3/3 Re-scan PnP =='
pnputil /scan-devices | Out-Null
Write-Host ''
Write-Host 'HECHO. REINICIA para volver al driver original firmado.'
Write-Host '(Opcional: borrar labtest.cer de TrustedPublisher/Root y desinstalar el cert.)'
