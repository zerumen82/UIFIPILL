# ============================================================================
# prep_uefi.ps1 — Preparación ANTES de desactivar Secure Boot en la UEFI.
# 1. Estado de BitLocker; si C: está protegido, suspenderlo con RebootCount 2
#    (el reinicio a UEFI consume 1; el arranque posterior consume 2) para no
#    pedir la recovery key.
# 2. bcdedit /set testsigning on (con Secure Boot activo se ignora; al
#    desactivarlo debe estar ya a ON para cargar el driver parcheado).
# 3. Diagnóstico a prep_uefi_log.txt.
# Ejecutar COMO ADMINISTRADOR. Lab-only.
# ============================================================================
$ErrorActionPreference = 'Continue'
$log = Join-Path $PSScriptRoot 'prep_uefi_log.txt'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }

Write-Host '== 1/3 BitLocker =='
$bde = & manage-bde -status C: 2>&1 | Out-String
Write-Host $bde
if ($bde -match 'Protecci[óo]n activa|Protection On') {
  Write-Host '   BitLocker ACTIVO -> suspendiendo (RebootCount 2)...'
  & manage-bde -protectors -disable C: -RebootCount 2 2>&1 | Out-Host
} else {
  Write-Host '   BitLocker no activo en C: — nada que suspender.'
}

Write-Host '== 2/3 testsigning =='
& bcdedit /set testsigning on 2>&1 | Out-Host
Write-Host '   estado (bcdedit /enum):'
& cmd /c 'bcdedit /enum {current}' 2>&1 | Out-Host

Write-Host '== 3/3 Estado del adaptador (informativo) =='
& pnputil /enum-devices /instanceid 'USB\VID_148F&PID_3070\1.0' 2>&1 | Out-Host

Write-Host ''
Write-Host 'PREP HECHA. Ahora: REINICIA A LA UEFI y desactiva Secure Boot.'
Write-Host 'Al volver a Windows, ejecuta (o pide a Buffy) driver_re\verify_patch.ps1.'
