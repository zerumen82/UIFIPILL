@echo off
rem Launcher del redeploy (re-agendar rename pendiente con rutas NT correctas)
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "& '.\redeploy_pending.ps1' *>&1 | Out-File '.\redeploy_log.txt' -Encoding UTF8"
echo EXITCODE %ERRORLEVEL% >> "%~dp0redeploy_log.txt"
