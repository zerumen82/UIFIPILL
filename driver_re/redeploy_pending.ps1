# ============================================================================
# redeploy_pending.ps1 — Re-agendar el reemplazo de netr28ux.sys al arranque.
# Ejecutar COMO ADMINISTRADOR. Lab-only. Requiere reinicio.
# (El deploy anterior falló: el origen del rename era '??ruta' — inválido;
#  Session Manager lo descartó y el driver arrancó con el .sys original.)
# Rollback: restore_driver.ps1
# ============================================================================
$ErrorActionPreference = 'Stop'

$sys     = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$patched = Join-Path $PSScriptRoot 'netr28ux_patched_clean.sys'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Ejecuta este script COMO ADMINISTRADOR.' }
if (-not (Test-Path $patched)) { throw "No existe $patched" }

Write-Host '== 1/3 Estado actual =='
$hash = (Get-FileHash $sys -Algorithm SHA256).Hash
Write-Host "   netr28ux.sys actual: $hash"
Write-Host "   testsigning: $(bcdedit | Select-String 'testsigning')"

Write-Host '== 2/3 Staging del driver parcheado =='
$staged = "$env:SystemRoot\System32\drivers\netr28ux_patched_stage.sys"
Copy-Item $patched $staged -Force
Write-Host "   staged -> $staged"

Write-Host '== 3/3 PendingFileRenameOperations (rutas NT \??\) =='
$srcNt = '\??\' + $staged
$dstNt = '\??\' + $sys
$reg = 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager'
# par 1: staged -> netr28ux.sys (rename al arrancar, antes de cargar el driver)
# par 2: source vacío = borrar el staged tras el rename
$pairs = @($srcNt, $dstNt, '', $staged)
$cur = (Get-ItemProperty -Path $reg -Name PendingFileRenameOperations -ErrorAction SilentlyContinue).PendingFileRenameOperations
if ($cur) { Set-ItemProperty -Path $reg -Name PendingFileRenameOperations -Value ($cur + $pairs) }
else      { New-ItemProperty -Path $reg -Name PendingFileRenameOperations -PropertyType MultiString -Value $pairs | Out-Null }
Write-Host '   registrado:'
(Get-ItemProperty -Path $reg -Name PendingFileRenameOperations).PendingFileRenameOperations |
    ForEach-Object { Write-Host "     [$_]" }

Write-Host ''
Write-Host 'HECHO. REINICIA el equipo.'
Write-Host 'Tras reiniciar: (Get-FileHash netr28ux.sys).Hash debe ser d2c7cf43e49a055ef156125080028e9c7662dfef86f06f5d1c5ad3f94e9cb590'
