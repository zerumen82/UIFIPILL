# e1_step1_publish.ps1 — republicar netr28ux (paso 1 del E1). ADMIN.
$ErrorActionPreference = 'Continue'
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\pnputil_log.txt'
$repo = Get-ChildItem "$env:SystemRoot\System32\DriverStore\FileRepository" -Filter 'netr28ux.inf_amd64_*' -Directory | Select-Object -First 1
if (-not $repo) { "ERROR: no repo" | Out-File $log; exit 1 }
$inf = Join-Path $repo.FullName 'netr28ux.inf'
"INF: $inf" | Out-File $log
pnputil /add-driver $inf /install 2>&1 | Out-String | Out-File $log -Append
"--- enum tras add ---" | Out-File $log -Append
pnputil /enum-drivers 2>&1 | Out-String | Out-File $log -Append
"DONE" | Out-File $log -Append
