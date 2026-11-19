@echo off
set LOG=D:\PROJECTS\UIFIPILL\e011_verify.log
echo === E0-11 verify start %DATE% %TIME% === > "%LOG%"
cd /d D:\PROJECTS\UIFIPILL\src-tauri
call cargo build --release >> "%LOG%" 2>&1
echo [src-tauri release] exit=%ERRORLEVEL% >> "%LOG%"
call cargo test --lib >> "%LOG%" 2>&1
echo [src-tauri test] exit=%ERRORLEVEL% >> "%LOG%"
cd /d D:\PROJECTS\UIFIPILL\driver_re\usb_tx
call cargo build --release >> "%LOG%" 2>&1
echo [usb_tx release] exit=%ERRORLEVEL% >> "%LOG%"
call cargo test >> "%LOG%" 2>&1
echo [usb_tx test] exit=%ERRORLEVEL% >> "%LOG%"
echo === E0-11 verify done === >> "%LOG%"