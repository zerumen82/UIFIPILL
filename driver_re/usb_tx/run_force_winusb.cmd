@echo off
rem Lanzador UAC: rebind directo a WinUSB via SetupAPI (log capturado)
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','& \"\"%~dp0force_winusb_elevated.ps1\"\" *>&1 | Tee-Object -FilePath \"%~dp0force_log.txt\"; Read-Host \"Enter para cerrar\"'"
echo.
echo --- LOG ---
type "%~dp0force_log.txt" 2>nul
echo.
pause
