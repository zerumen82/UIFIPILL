@echo off
rem Launcher del deploy: una sola capa de quoting, log a fichero
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "& '.\deploy_driver.ps1' *>&1 | Out-File '.\deploy_log.txt' -Encoding UTF8"
echo EXITCODE %ERRORLEVEL% >> "%~dp0deploy_log.txt"
