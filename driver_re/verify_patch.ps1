# ============================================================================
# verify_patch.ps1 — Ejecutar AL VOLVER de desactivar Secure Boot en la UEFI.
# 1. Comprueba SecureBoot (reg) y activa testsigning (ahora sí podrá).
# 2. Re-activa el dispositivo RT3070 (código 52) con ciclo disable/enable
#    para que Windows reintente cargar el driver parcheado.
# 3. Verifica: dispositivo OK + servicio RUNNING + hash del .sys.
# Ejecutar COMO ADMINISTRADOR. Lab-only.
# ============================================================================
$ErrorActionPreference = 'Continue'

$sys      = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$EXPECT_PATCHED = 'd2c7cf43e49a055ef156125080028e9c7662dfef86f06f5d1c5ad3f94e9cb590'
$INSTANCE = 'USB\VID_148F&PID_3070\1.0'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }

function Get-HashS($p) {
  certutil -hashfile $p SHA256 | Where-Object { $_ -match '^[0-9a-fA-F]{64}$' } | Select-Object -First 1
}

Write-Host '== 1/4 Secure Boot y testsigning =='
$sb = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State' -ErrorAction SilentlyContinue).UEFISecureBootEnabled
Write-Host "   UEFISecureBootEnabled: $sb (0 = desactivado, buscamos esto)"
if ($sb -ne 0) { Write-Host '   AVISO: Secure Boot SIGUE ACTIVO. El parche NO cargará. Revisa la UEFI.' }
& bcdedit /set testsigning on 2>&1 | Out-Host

Write-Host '== 2/4 Hash del driver en disco =='
$h = Get-HashS $sys
Write-Host "   netr28ux.sys: $h"
if ($h -ne $EXPECT_PATCHED) { Write-Host '   AVISO: NO es el hash parcheado esperado.' }

Write-Host '== 3/4 Ciclo PnP para recargar el driver =='
& pnputil /disable-device "$INSTANCE" 2>&1 | Out-Host
Start-Sleep -Seconds 3
& pnputil /enable-device "$INSTANCE" 2>&1 | Out-Host
Start-Sleep -Seconds 5

Write-Host '== 4/4 Veredicto =='
$dev = & pnputil /enum-devices /instanceid "$INSTANCE" | Out-String
Write-Host $dev
$run = (& sc.exe query netr28ux | Out-String) -match 'RUNNING'
Write-Host ("   servicio netr28ux RUNNING: $run")
if ($dev -match 'Problema|52') {
  Write-Host '   SIGUE EN PROBLEMA. Copiar el log y revisar CodeIntegrity.'
} elseif ($run) {
  Write-Host '   TODO OK: driver parcheado CARGADO. Probar cambio de canal en la UI.'
}
