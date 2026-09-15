# build_all.ps1 — Compila herramientas WiFi nativas Windows desde fuente
# Requiere: MSYS2 + mingw-w64-x86_64-{gcc,pkg-config,libpcap,openssl}
# Ejecutar como: .\tools\build\build_all.ps1
# O desde MSYS2:  powershell -File build_all.ps1

$ScriptRoot = $PSScriptRoot
$ProjectRoot = Resolve-Path "$ScriptRoot\..\.."
$ToolsDir = "$ProjectRoot\tools"

$env:CHERE_INVOKING = 'yes'
$env:MSYSTEM = 'MINGW64'

# Sin capas de entrecomillado: los flags viven en build_reaver.sh (rutas Unix).
Write-Host "=== Building reaver (WPS PIN bruteforce) ===" -ForegroundColor Cyan
$sh = ($ScriptRoot + "/build_reaver.sh") -replace '\\','/'
& "C:\msys64\usr\bin\bash.exe" --login $sh 2>&1 | ForEach-Object { Write-Host $_ }

if ($LASTEXITCODE -eq 0) {
    Write-Host "  reaver: OK" -ForegroundColor Green
    Copy-Item "$ProjectRoot\tools\build\reaver\src\reaver.exe" "$ToolsDir\reaver.exe" -Force
    # wash es el mismo binario (en MSYS `ln -sf` no genera wash.exe): duplicar.
    Copy-Item "$ProjectRoot\tools\build\reaver\src\reaver.exe" "$ToolsDir\wash.exe" -Force
} else {
    Write-Host "  reaver: FAILED" -ForegroundColor Red
}

Write-Host "`n=== Build complete ===" -ForegroundColor Cyan
Write-Host "Binarios en: $ToolsDir"
Get-ChildItem "$ToolsDir\*.exe" | Select-Object Name, Length
