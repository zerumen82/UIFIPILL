# Launcher UAC para usb_tx/run_usb_probe.ps1.
# NOTA: este launcher se llama desde run_usb_probe.cmd (que YA eleva con UAC).
# NO volver a elevar aqui: ejecutar la sonda directamente para evitar un bucle
# UAC infinito (bug corregido 2026-09-22: antes hacia Start-Process RunAs sobre
# el propio run_usb_probe.cmd externo, que re-lanzaba este launcher).
$ErrorActionPreference = 'Continue'
$dir = $PSScriptRoot

& "$dir\usb_tx\run_usb_probe.ps1"
Write-Host ""
Write-Host "EXITCODE: $LASTEXITCODE"
Write-Host "Log: $dir\usb_tx\usb_tx_log.txt"
if (Test-Path "$dir\usb_tx\usb_tx_log.txt") { Get-Content "$dir\usb_tx\usb_tx_log.txt" }
Read-Host 'Enter para cerrar'
