$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\bbpmcu_run5.txt'
$exe = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\target\release\rt3070_bbpmcu.exe'
Remove-Item $log -ErrorAction SilentlyContinue
$out = & $exe 15 11 2>&1
$out | Out-File -FilePath $log -Encoding utf8
exit $LASTEXITCODE
