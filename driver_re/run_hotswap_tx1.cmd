@echo off
rem run_hotswap_tx1.cmd — ejecuta hotswap_tx1.ps1 y guarda log. Lo lanza el launcher UAC.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "hotswap_tx1.ps1" > hotswap_tx1_log.txt 2>&1
echo EXITCODE %ERRORLEVEL% >> hotswap_tx1_log.txt
exit /b %ERRORLEVEL%
