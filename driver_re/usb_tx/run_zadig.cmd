@echo off
rem Zadig + verificación (Zadig pide su propio UAC al instalar)
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0zadig_helper.ps1"
pause
