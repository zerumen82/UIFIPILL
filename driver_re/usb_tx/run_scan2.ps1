$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\scan_run2.txt'
$exe = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\target\release\rt3070_scan.exe'
Remove-Item $log -ErrorAction SilentlyContinue
$out = & $exe 30 2>&1
$out | Out-File -FilePath $log -Encoding utf8
exit $LASTEXITCODE
