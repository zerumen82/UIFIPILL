# run_usb_probe.cmd — ejecuta run_usb_probe.ps1 y guarda log. Lo lanza el launcher UAC.
cd /d "%~dp0usb_tx"
powershell -NoProfile -ExecutionPolicy Bypass -File "run_usb_probe.ps1" > usb_tx_log.txt 2>&1
echo EXITCODE %ERRORLEVEL% >> usb_tx_log.txt
exit /b %ERRORLEVEL%
