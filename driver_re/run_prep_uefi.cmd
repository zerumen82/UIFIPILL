@echo off
rem run_prep_uefi.cmd — ejecuta prep_uefi.ps1 y guarda log. Lo lanza el launcher UAC.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "prep_uefi.ps1" > prep_uefi_log.txt 2>&1
echo EXITCODE %ERRORLEVEL% >> prep_uefi_log.txt
exit /b %ERRORLEVEL%
