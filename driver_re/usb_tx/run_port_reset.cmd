@echo off
setlocal
set PS1=%~dp0port_reset.ps1
net session >nul 2>&1
if errorlevel 1 (
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b 0
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%PS1%"
pause
endlocal
