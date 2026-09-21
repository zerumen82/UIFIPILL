# ============================================================================
# hotswap_driver.ps1 — Reemplazo EN CALIENTE de netr28ux.sys sin reiniciar.
#
# Via A: disable PnP del adaptador -> descarga el driver -> copia -> enable.
# Via B (fallback): sc stop (falla con 1052 si el adaptador esta en uso).
#
# Contexto: PendingFileRenameOperations fallo 2 veces en este equipo (SMSS
# consume el valor sin ejecutar el rename). El driver es DEMAND_START y el
# adaptador es USB\VID_148F&PID_3070\1.0 — desactivarlo por PnP descarga el
# driver y libera netr28ux.sys.
# Ejecutar COMO ADMINISTRADOR. Lab-only. Rollback: restore_driver.ps1
# Sin cmdlets de modulos (reg/certutil/pnputil/sc.exe) — a prueba de
# PSModulePath roto en sesiones elevadas de este equipo.
# ============================================================================
$ErrorActionPreference = 'Continue'

$sys     = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$patched = Join-Path $PSScriptRoot 'netr28ux_patched_clean.sys'
$EXPECT_PATCHED = 'd2c7cf43e49a055ef156125080028e9c7662dfef86f06f5d1c5ad3f94e9cb590'
$EXPECT_ORIG    = 'ade38351a626dc0a6bcde1d09b214c94a65ba89fc9b0a69045019ecbf27bef59'
$INSTANCE = 'USB\VID_148F&PID_3070\1.0'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }
if (-not (Test-Path $patched)) { throw "No existe $patched" }

function Get-HashS($p) {
  certutil -hashfile $p SHA256 | Where-Object { $_ -match '^[0-9a-fA-F]{64}$' } | Select-Object -First 1
}

Write-Host '== 1/6 Estado actual =='
$h = Get-HashS $sys
Write-Host "   netr28ux.sys actual: $h"
if ($h -eq $EXPECT_PATCHED) { Write-Host '   YA ESTA PARCHEADO. No hay nada que hacer.'; exit 0 }
if ($h -ne $EXPECT_ORIG)    { Write-Host '   AVISO: hash no conocido (ni orig ni patched).' }

Write-Host '   testsigning:'
& bcdedit /enum '{current}' | Select-String -SimpleMatch 'testsigning' | ForEach-Object { Write-Host "     $_" }

Write-Host '== 2/6 Verificar hash del fichero parcheado =='
$hp = Get-HashS $patched
Write-Host "   patched: $hp"
if ($hp -ne $EXPECT_PATCHED) { throw "El fichero parcheado NO tiene el hash esperado ($hp)" }

Write-Host '== 3/6 Desactivar dispositivo PnP (descarga el driver) =='
& pnputil /disable-device "$INSTANCE" | Out-Host
$disabled = $false
foreach ($i in 1..15) {
  Start-Sleep -Seconds 1
  $q = & pnputil /enum-devices /instanceid "$INSTANCE" | Out-String
  # ES: 'Deshabilitado' / 'Desactivado'; EN: 'Disabled'
  if ($q -match 'Disabled|Deshabilitado|Desactivado') { $disabled = $true; break }
}
Write-Host ("   disabled: $disabled (tras $i s)")
if (-not $disabled) {
  Write-Host '   FALLO: no se pudo desactivar el dispositivo. Re-activando...'
  & pnputil /enable-device "$INSTANCE" | Out-Host
  exit 2
}
Start-Sleep -Seconds 2

Write-Host '== 4/6 Copiar driver parcheado =='
try {
  [System.IO.File]::Copy($patched, $sys, $true)
  Write-Host '   copia OK'
} catch {
  Write-Host "   FALLO copia: $($_.Exception.Message)"
  Write-Host '   Re-activando el dispositivo...'
  & pnputil /enable-device "$INSTANCE" | Out-Host
  Start-Sleep -Seconds 3
  exit 3
}

Write-Host '== 5/6 Re-activar dispositivo =='
& pnputil /enable-device "$INSTANCE" | Out-Host
Start-Sleep -Seconds 4
$q = & pnputil /enum-devices /instanceid "$INSTANCE" | Out-String
Write-Host $q

Write-Host '== 6/6 Verificacion =='
$h2 = Get-HashS $sys
Write-Host "   netr28ux.sys ahora: $h2"
if ($h2 -eq $EXPECT_PATCHED) { Write-Host '   OK: FICHERO PARCHEADO EN SITIO.' }
$run = (& sc.exe query netr28ux | Out-String) -match 'RUNNING'
Write-Host ("   servicio netr28ux RUNNING: $run")

Write-Host ''
Write-Host 'Siguiente paso: probar cambio de canal en modo monitor (UI: Escanear -> monitor).'
