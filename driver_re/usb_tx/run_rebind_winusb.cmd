@echo off
rem Lanzador UAC: fuerza el rebind del RT3070 a WinUSB (log capturado)
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','& \"\"%~dp0rebind_winusb.ps1\"\" *>&1 | Tee-Object -FilePath \"%~dp0rebind_log.txt\"; Read-Host \"Enter para cerrar\"'"
echo.
echo --- LOG ---
type "%~dp0rebind_log.txt" 2>nul
echo.
pause
