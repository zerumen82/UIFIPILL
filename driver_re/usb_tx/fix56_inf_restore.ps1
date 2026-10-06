# fix56_inf_restore.ps1 — Code 56 raíz: NetSetup no encuentra C:\Windows\INF\netr28ux.inf
# (NetworkInterfaceInstallResult = 0x80070002 ERROR_FILE_NOT_FOUND en clase {4d36e972}\0006).
# Fix: restaurar INF+PNF desde DriverStore, limpiar ConfigFlags 0x80000, remove+rescan.
$log = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\fix56_inf_log.txt'
function L($m) { Add-Content -Path $log -Value ("{0}  {1}" -f (Get-Date -Format 'HH:mm:ss'), $m) }

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath) -Verb RunAs
    exit
}

Set-Content -Path $log -Value "=== fix 56: restaurar INF de clase Net + clear ConfigFlags + rescan ==="

$repo  = 'C:\Windows\System32\DriverStore\FileRepository\netr28ux.inf_amd64_2613a90929adebda'
$srcI  = Join-Path $repo 'netr28ux.inf'
$srcP  = Join-Path $repo 'netr28ux.PNF'
$dstI  = 'C:\Windows\INF\netr28ux.inf'
$dstP  = 'C:\Windows\INF\netr28ux.PNF'
$enum  = 'HKLM:\SYSTEM\CurrentControlSet\Enum\USB\VID_148F&PID_3070\1.0'
$class = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\0006'

function Get-DevState {
    $d = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*' } | Select-Object -First 1
    if (-not $d) { return 'present=False' }
    $pc = (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName 'DEVPKEY_Device_ProblemCode' -ErrorAction SilentlyContinue).Data
    $nid = (Get-ItemProperty $class -Name NetCfgInstanceId -ErrorAction SilentlyContinue).NetCfgInstanceId
    $res = (Get-ItemProperty $class -Name NetworkInterfaceInstallResult -ErrorAction SilentlyContinue).NetworkInterfaceInstallResult
    "present=True status=$($d.Status) problem=$pc NetCfgInstanceId=$nid NiiResult=$res"
}
L ("ANTES: " + (Get-DevState))

L "paso 1: restaurar INF+PNF desde DriverStore"
foreach ($pair in @(@($srcI,$dstI), @($srcP,$dstP))) {
    $s = $pair[0]; $d2 = $pair[1]
    try {
        if (Test-Path $s) {
            Copy-Item -Path $s -Destination $d2 -Force -ErrorAction Stop
            L ("  OK  {0} -> {1} ({2} B)" -f $s, $d2, (Get-Item $d2).Length)
        } else { L ("  FALTA fuente {0}" -f $s) }
    } catch { L ("  ERROR copia {0}: {1}" -f $d2, $_.Exception.Message) }
}
L ("verif: C:\Windows\INF\netr28ux.inf existe = " + (Test-Path $dstI))

L "paso 2: limpiar ConfigFlags (0x80000 FAILEDINSTALL)"
try { Set-ItemProperty -Path $enum -Name ConfigFlags -Value 0 -Type DWord -ErrorAction Stop; L ("  ConfigFlags ahora = " + (Get-ItemProperty $enum -Name ConfigFlags).ConfigFlags) }
catch { L ("  ERROR ConfigFlags: " + $_.Exception.Message) }

L "paso 3: remove + rescan"
(pnputil /remove-device 'USB\VID_148F&PID_3070\1.0' 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
(pnputil /scan-devices 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }

L "esperando 45 s a la class config..."
Start-Sleep -Seconds 45
L ("DESPUES: " + (Get-DevState))

L "netsh:"; (netsh wlan show interfaces 2>&1 | Out-String).Trim() | ForEach-Object { L "  $_" }
L "=== done ==="
