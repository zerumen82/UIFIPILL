@echo off
REM Lanzador elevado UAC para la sonda USB cruda (patrón hotswap probado)
powershell -NoProfile -Command "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','%~dp0launch_usb_probe_elevated.ps1'"
