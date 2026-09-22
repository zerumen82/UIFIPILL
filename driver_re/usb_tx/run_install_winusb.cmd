@echo off
rem Lanzador UAC con log: ejecuta install_winusb.ps1 capturando TODO output.
rem La ventana elevada queda abierta (pause) para ver el resultado.
rem NOTA: este cmd vive EN usb_tx\, %~dp0 ya es la carpeta del script (no duplicar usb_tx).
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','& \"\"%~dp0install_winusb.ps1\"\" *>&1 | Tee-Object -FilePath \"%~dp0winusb_install_log.txt\"; Read-Host \"Enter para cerrar\"'"
echo.
echo --- LOG (winusb_install_log.txt) ---
type "%~dp0winusb_install_log.txt" 2>nul
echo.
pause
