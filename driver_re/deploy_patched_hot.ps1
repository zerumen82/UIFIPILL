# deploy_patched_hot.ps1 — Poner el driver PARCHEADO (canal en monitor) en caliente.
# Flujo: testsigning check -> (si off: activar + pedir reboot) -> hotswap_driver.ps1 -> verificación.
# Log directo a fichero (ruta absoluta; Start-Transcript no fiable en este equipo).
$log = 'D:\PROJECTS\UIFIPILL\driver_re\deploy_hot_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== deploy parcheado (canal en monitor) ==="
$sys = "$env:SystemRoot\System32\drivers\netr28ux.sys"
$EXPECT_PATCHED = 'd2c7cf43e49a055ef156125080028e9c7662dfef86f06f5d1c5ad3f94e9cb590'

L ("hash actual: " + (certutil -hashfile $sys SHA256 | Where-Object { $_ -match '^[0-9a-fA-F]{64}$' } | Select-Object -First 1))

$ts = (bcdedit /enum '{current}' 2>&1 | Out-String)
$tsLine = ($ts -split "`r?`n" | Select-String 'testsigning' | Select-Object -First 1)
L ("bcdedit testsigning: " + $tsLine)

# Ojo: -match es case-insensitive; 'S[ií]' casaba con "si" de "testsigning" (bug 2026-09-30).
# Regex anclada a la línea: solo "testsigning <Yes>" cuenta como ON.
if ($tsLine -notmatch '(?m)^testsigning\s+Yes') {
    L "testsigning OFF -> activando (se aplica tras REBOOT)"
    (bcdedit /set testsigning on 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
    L "=== STOP: REINICIAR y volver a lanzar este script ==="
    exit 0
}

L "testsigning ON -> hotswap_driver.ps1 (disable PnP -> copia -> enable)"
$hot = 'D:\PROJECTS\UIFIPILL\driver_re\hotswap_driver.ps1'
(& powershell -NoProfile -ExecutionPolicy Bypass -File $hot 2>&1) | ForEach-Object { L ("  hot: $_") }

Start-Sleep -Seconds 3
$h2 = (certutil -hashfile $sys SHA256 | Where-Object { $_ -match '^[0-9a-fA-F]{64}$' } | Select-Object -First 1)
L ("hash final: $h2  patched=" + ($h2 -eq $EXPECT_PATCHED))

$d = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
if ($d) {
    $pc = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    L ("device: status=$($d.Status) problem=$pc")
} else { L "device: NO PRESENTE" }
L "netsh:"; (netsh wlan show interfaces 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
L "=== done ==="
