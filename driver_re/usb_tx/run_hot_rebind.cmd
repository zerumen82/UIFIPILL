@echo off
REM run_hot_rebind.cmd - auto-eleva hot_rebind.ps1 (arg %1) y deja log
setlocal
set LOG=%~dp0hot_rebind_log.txt
set PS1=%~dp0hot_rebind.ps1
set MODE=%~1
if "%MODE%"=="" set MODE=status

net session >nul 2>&1
if errorlevel 1 (
    echo Pidiendo elevacion UAC...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -ArgumentList '%MODE%' -Verb RunAs"
    exit /b 0
)

echo === hot_rebind %MODE% ===> "%LOG%"
powershell -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %MODE% >> "%LOG%" 2>&1
echo === fin ===>> "%LOG%"
endlocal
