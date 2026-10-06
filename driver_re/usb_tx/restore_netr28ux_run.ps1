# restore_netr28ux_run.ps1 — lanza restore_netr28ux.ps1 y loguea TODO a ruta
# absoluta (Start-Transcript no fiable bajo elección UAC — regla 5). Auto-elevado.
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\restore_run.log'
$ErrorActionPreference = 'Continue'
"=== $(Get-Date -Format s) restore start (pid=$PID) ===" | Out-File -Append -FilePath $log -Encoding utf8
$src = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\restore_netr28ux.ps1'
$out = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $src 2>&1
$out | ForEach-Object { "$_" } | Out-File -Append -FilePath $log -Encoding utf8
"=== $(Get-Date -Format s) restore done exit=$LASTEXITCODE ===" | Out-File -Append -FilePath $log -Encoding utf8
