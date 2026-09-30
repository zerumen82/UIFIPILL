# ============================================================================
# deploy_driver.ps1 — Instalar netr28ux_patched.sys (RT3070, canal en monitor)
# EJECUTAR COMO ADMINISTRADOR. Lab-only. Requiere reinicio.
# Rollback: restore_driver.ps1 (deja el driver original y testsigning off).
# ============================================================================
$ErrorActionPreference = 'Stop'

$sys = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$bak = "$env:SystemRoot\System32\drivers\netr28ux.sys.orig"
$patched = Join-Path $PSScriptRoot 'netr28ux_patched_clean.sys'
$cer     = Join-Path $PSScriptRoot 'labtest.cer'

# 0) comprobar elevación
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }
if (-not (Test-Path $patched)) { throw "No existe $patched" }

Write-Host '== 1/5 Copia de seguridad del driver original =='
if (-not (Test-Path $bak)) {
    Copy-Item $sys $bak
    Write-Host "   backup -> $bak"
} else {
    Write-Host "   ya existe $bak (no se sobreescribe)"
}

Write-Host '== 2/5 Confiar en el cert de laboratorio (TrustedPublisher + Root) =='
Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\LocalMachine\TrustedPublisher' | Out-Null
Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\LocalMachine\Root' | Out-Null
Write-Host '   OK'

Write-Host '== 3/5 Activar testsigning =='
bcdedit /set testsigning on | Out-Null
Write-Host '   OK (se aplica tras reiniciar)'

Write-Host '== 4/5 Instalar driver parcheado =='
# detener el servicio para liberar el .sys (si el PnP lo re-arranca, usar rename pendiente)
sc.exe stop netr28ux 2>$null | Out-Null
Start-Sleep -Seconds 1
$copied = $false
try {
    Copy-Item $patched $sys -Force
    $copied = $true
    Write-Host "   copiado $patched -> $sys"
} catch {
    Write-Host '   .sys bloqueado (driver en uso): agendando reemplazo en el próximo arranque'
    # El .sys parcheado se copia a un NUEVO fichero en el mismo directorio (no está
    # bloqueado) y el rename pendiente va de ese fichero a netr28ux.sys. Rutas NT
    # con prefijo \??\ (obligatorio en PendingFileRenameOperations; '??' a secas es
    # inválido y Session Manager descarta el par).
    $staged = "$env:SystemRoot\System32\drivers\netr28ux_patched_stage.sys"
    Copy-Item $patched $staged -Force
    $srcNt = '\??\' + $staged
    $dstNt = '\??\' + $sys
    $reg = 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager'
    $cur = (Get-ItemProperty -Path $reg -Name PendingFileRenameOperations -ErrorAction SilentlyContinue).PendingFileRenameOperations
    # par 1: staged -> netr28ux.sys ; par 2: borrar el staged tras el rename (src vacío = delete)
    $pairs = @($srcNt, $dstNt, '', $staged)
    if ($cur) { Set-ItemProperty -Path $reg -Name PendingFileRenameOperations -Value ($cur + $pairs) }
    else      { New-ItemProperty -Path $reg -Name PendingFileRenameOperations -PropertyType MultiString -Value $pairs | Out-Null }
    Write-Host "   staged: $staged"
    Write-Host '   OK: el fichero se sustituirá al arrancar (antes de cargar el driver)'
}

Write-Host '== 5/5 Re-scan PnP =='
pnputil /scan-devices | Out-Null
Write-Host ''
Write-Host 'HECHO. REINICIA el equipo para cargar el driver parcheado.'
Write-Host 'Tras reiniciar: verificar hash y probar cambio de canal en modo monitor.'
