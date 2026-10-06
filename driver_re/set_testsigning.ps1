# set_testsigning.ps1 — activar testsigning (requiere REBOOT para aplicar).
# Contexto 2026-09-30: driver parcheado ya en sitio (d2c7cf43) pero Problem 52
# (test-signed + testsigning OFF) hasta reiniciar.
$log = 'D:\PROJECTS\UIFIPILL\driver_re\set_ts_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== testsigning ON ==="
L "bcdedit /set testsigning on:"
(bcdedit /set testsigning on 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
L "verificacion:"
(bcdedit /enum '{current}' 2>&1 | Out-String) -split "`r?`n" | Select-String '^testsigning' | ForEach-Object { L "  $_" }
L "Secure Boot (Confirm-SecureBootUEFI, puede fallar sin admin/UEFI):"
try { L ("  " + (Confirm-SecureBootUEFI)) } catch { L ("  no disponible: " + $_.Exception.Message) }
L "hash netr28ux.sys (debe seguir siendo el parcheado d2c7cf43...):"
L ("  " + (certutil -hashfile "$env:SystemRoot\System32\drivers\netr28ux.sys" SHA256 | Where-Object { $_ -match '^[0-9a-fA-F]{64}$' } | Select-Object -First 1))
L "=== HECHO: REINICIAR para aplicar ==="
