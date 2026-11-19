@echo off
REM E0-11 E2E via schtasks con consola propia (la via que funciona — leccion 2026-10-06:
REM los binarios WinUSB con timeouts bulk pueden fallar lanzados desde el shell del agente).
set LOG=D:\PROJECTS\UIFIPILL\driver_re\usb_tx\e011_e2e_log.txt
echo === E0-11 E2E start %DATE% %TIME% === > "%LOG%"
cd /d D:\PROJECTS\UIFIPILL\driver_re\usb_tx
echo [probe] >> "%LOG%"
target\release\rt3070_probe.exe >> "%LOG%" 2>&1
echo [init ch11] >> "%LOG%"
target\release\rt3070_init.exe 11 >> "%LOG%" 2>&1
echo [scan 20s] >> "%LOG%"
target\release\rt3070_scan.exe 20 >> "%LOG%" 2>&1
echo [tx] >> "%LOG%"
target\release\rt3070_tx.exe >> "%LOG%" 2>&1
echo [sniff 30s] >> "%LOG%"
target\release\rt3070_sniff.exe 30 >> "%LOG%" 2>&1
echo === E0-11 E2E done === >> "%LOG%"
