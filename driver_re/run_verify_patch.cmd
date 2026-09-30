@echo off
rem run_verify_patch.cmd — ejecuta verify_patch.ps1 y guarda log. Lo lanza el launcher UAC.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "verify_patch.ps1" > verify_patch_log.txt 2>&1
echo EXITCODE %ERRORLEVEL% >> verify_patch_log.txt
exit /b %ERRORLEVEL%
