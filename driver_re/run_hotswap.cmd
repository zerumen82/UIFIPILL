@echo off
rem run_hotswap.cmd — ejecuta hotswap_driver.ps1 y guarda log. Lo lanza el launcher UAC.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "hotswap_driver.ps1" > hotswap_log.txt 2>&1
echo EXITCODE %ERRORLEVEL% >> hotswap_log.txt
exit /b %ERRORLEVEL%
