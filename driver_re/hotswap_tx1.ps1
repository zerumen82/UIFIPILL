# ============================================================================
# hotswap_tx1.ps1 — Desplegar la variante TX-1 (validador TX en 0x140015d18
# parcheado: xor edi,edi) EN CALIENTE via ciclo PnP, sin reiniciar.
# Ejecutar COMO ADMINISTRADOR. Lab-only. Rollback: re-ejecutar hotswap_driver.ps1
# (vuelve a netr28ux_patched_clean.sys) o restore_driver.ps1 (original).
# ============================================================================
$ErrorActionPreference = 'Continue'

$sys      = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$tx1      = Join-Path $PSScriptRoot 'netr28ux_tx3.sys'
$EXPECT_TX1  = '30080e89c6141529237cd77c999c275b48d38850a2931cd440ef75ef6d055579'
$EXPECT_PREV = 'ad3e2d41908d878edb2d20992bcbb8f04a2a1f5bd88db7158fe4bb68016277a2'
$INSTANCE = 'USB\VID_148F&PID_3070\1.0'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }
if (-not (Test-Path $tx1)) { throw "No existe $tx1" }

function Get-HashS($p) {
  certutil -hashfile $p SHA256 | Where-Object { $_ -match '^[0-9a-fA-F]{64}$' } | Select-Object -First 1
}

Write-Host '== 1/6 Estado actual =='
$h = Get-HashS $sys
Write-Host "   netr28ux.sys actual: $h"
if ($h -eq $EXPECT_TX1)  { Write-Host '   YA ESTA TX-3. No hay nada que hacer.'; exit 0 }
if ($h -ne $EXPECT_PREV) { Write-Host '   AVISO: hash actual no es TX-2 (ad3e2d41...). Continuando igualmente.' }

Write-Host '   testsigning:'
& bcdedit /enum '{current}' | Select-String -SimpleMatch 'testsigning' | ForEach-Object { Write-Host "     $_" }

Write-Host '== 2/6 Verificar hash TX-3 =='
$hp = Get-HashS $tx1
Write-Host "   tx3: $hp"
if ($hp -ne $EXPECT_TX1) { throw "netr28ux_tx3.sys NO tiene el hash esperado ($hp)" }

Write-Host '== 3/6 Desactivar dispositivo PnP (descarga el driver) =='
& pnputil /disable-device "$INSTANCE" | Out-Host
$disabled = $false
foreach ($i in 1..15) {
  Start-Sleep -Seconds 1
  $q = & pnputil /enum-devices /instanceid "$INSTANCE" | Out-String
  if ($q -match 'Disabled|Deshabilitado|Desactivado') { $disabled = $true; break }
}
Write-Host ("   disabled: $disabled (tras $i s)")
if (-not $disabled) {
  Write-Host '   FALLO: no se pudo desactivar. Re-activando...'
  & pnputil /enable-device "$INSTANCE" | Out-Host
  exit 2
}
Start-Sleep -Seconds 2

Write-Host '== 4/6 Copiar TX-1 =='
try {
  [System.IO.File]::Copy($tx1, $sys, $true)
  Write-Host '   copia OK'
} catch {
  Write-Host "   FALLO copia: $($_.Exception.Message)"
  & pnputil /enable-device "$INSTANCE" | Out-Host
  Start-Sleep -Seconds 3
  exit 3
}

Write-Host '== 5/6 Re-activar dispositivo =='
& pnputil /enable-device "$INSTANCE" | Out-Host
Start-Sleep -Seconds 4

Write-Host '== 6/6 Verificación =='
$h2 = Get-HashS $sys
Write-Host "   netr28ux.sys ahora: $h2"
if ($h2 -eq $EXPECT_TX1) { Write-Host '   OK: TX-3 EN SITIO (9 kills raw parcheados).' }
$run = (& sc.exe query netr28ux | Out-String) -match 'RUNNING'
Write-Host ("   servicio netr28ux RUNNING: $run")

Write-Host ''
Write-Host 'Siguiente paso: cargo test --lib lab_inject_probe -- --ignored --nocapture'
