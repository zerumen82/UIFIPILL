# ============================================================================
# redeploy_pending.ps1 — Re-agendar el reemplazo de netr28ux.sys al arranque.
# Ejecutar COMO ADMINISTRADOR. Lab-only. Requiere reinicio.
# (El deploy anterior falló: el origen del rename era '??ruta' — inválido;
#  Session Manager lo descartó y el driver arrancó con el .sys original.)
# Rollback: restore_driver.ps1
# NOTA: sin cmdlets de módulos (solo reg.exe/copy/certutil/bcdedit) — a prueba
# de PSModulePath roto (Get-FileHash falló en la sesión elevada de este equipo).
# ============================================================================
$ErrorActionPreference = 'Stop'

$sys     = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$patched = Join-Path $PSScriptRoot 'netr28ux_patched_clean.sys'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }
if (-not (Test-Path $patched)) { throw "No existe $patched" }

Write-Host '== 1/3 Estado actual =='
# certutil en vez de Get-FileHash (módulo no carga en la sesión elevada)
$hash = (certutil -hashfile $sys SHA256 | Where-Object { $_ -match '^[0-9a-fA-F]{64}' } | Select-Object -First 1)
Write-Host "   netr28ux.sys actual: $hash"
Write-Host '   testsigning:'
& bcdedit /enum '{current}' | Select-String -SimpleMatch 'testsigning' | ForEach-Object { Write-Host "     $_" }

Write-Host '== 2/3 Staging del driver parcheado =='
$staged = "$env:SystemRoot\System32\drivers\netr28ux_patched_stage.sys"
# .NET directo: 'copy' en PS es alias de Copy-Item (no acepta sintaxis cmd)
if (Test-Path $staged) { Remove-Item $staged -Force }
[System.IO.File]::Copy($patched, $staged, $true)
Write-Host "   staged -> $staged"

Write-Host '== 3/3 PendingFileRenameOperations (rutas NT \??\) =='
$srcNt = '\??\' + $staged
$dstNt = '\??\' + $sys
$regKey = 'HKLM\SYSTEM\CurrentControlSet\Control\Session Manager'
# par 1: staged -> netr28ux.sys (rename al arrancar, antes de cargar el driver)
# par 2: source vacío = borrar el staged tras el rename
# Vía .NET (siempre disponible, tolera PSModulePath roto; append seguro MultiString)
$k  = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey('SYSTEM\CurrentControlSet\Control\Session Manager', $true)
$cur = $k.GetValue('PendingFileRenameOperations', $null)
# SetValue(MultiString) exige string[] estricto (object[] => 'tipo no coincidía')
$list = New-Object 'System.Collections.Generic.List[string]'
if ($cur -ne $null) { foreach ($v in @($cur)) { if ($v -ne $null) { $null = $list.Add([string]$v) } } }
$null = $list.Add($srcNt)
$null = $list.Add($dstNt)
$null = $list.Add('')
$null = $list.Add($staged)
$k.SetValue('PendingFileRenameOperations', $list.ToArray(), [Microsoft.Win32.RegistryValueKind]::MultiString)
$k.Close()
$verify = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey('SYSTEM\CurrentControlSet\Control\Session Manager').GetValue('PendingFileRenameOperations')
Write-Host '   registrado (verificado):'
$verify | ForEach-Object { Write-Host "     [$_]" }

Write-Host ''
Write-Host 'HECHO. REINICIA el equipo.'
Write-Host 'Tras reiniciar: certutil -hashfile C:\Windows\System32\drivers\netr28ux.sys SHA256'
Write-Host 'debe dar d2c7cf43e49a055ef156125080028e9c7662dfef86f06f5d1c5ad3f94e9cb590'
