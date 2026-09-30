@echo off
rem Lanzador UAC: finish rebind (remove-device + UpdateDriver + rescan en una sesión)
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','& \"\"%~dp0finish_rebind_elevated.ps1\"\" *>&1 | Tee-Object -FilePath \"%~dp0finish_log.txt\"; Read-Host \"Enter para cerrar\"'"
echo.
echo --- LOG ---
type "%~dp0finish_log.txt" 2>nul
echo.
pause
