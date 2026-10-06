# sniff_vendor_admin.ps1 - FASE 1 (plan RX-WinUSB): captura USBPcap del trafico
# del vendor netr28ux para replicar luego el boot por bulk desde WinUSB.
#
# Por que: bajo WinUSB el TX ya funciona y el RX no porque el BBP queda mudo.
# El vendor (netr28ux) SI despierta el BBP - por pipes bulk de comandos
# (RTUSBBulkOutPktCmd). Nadie lo ha capturado aun (no existia vendor.pcap).
#
# Que captura:
#   1. Ventana de RE-INIT: pnputil /restart-device -> el driver baja el
#      firmware e inicializa BBP/RF (ahi esta la secuencia que buscamos).
#   2. Ventana de SCAN: netsh en bucle -> comandos de sintonia/canal.
#
# Auto-eleva (UAC). Log en ruta ABSOLUTA hardcodeada (regla del proyecto:
# Start-Transcript no es fiable en este equipo). Lab-only.
# Usage: powershell -File sniff_vendor_admin.ps1 [segundos-scan]   (default 30)
$ErrorActionPreference = 'Continue'
$secScan = 30
if ($args.Count -gt 0) {
    $parsed = 0
    if ([int]::TryParse(("" + $args[0]).Trim(), [ref]$parsed) -and $parsed -gt 0) { $secScan = $parsed }
}

$LOG    = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\sniff_vendor_log.txt'
$PCAP   = 'D:\PROJECTS\UIFIPILL\driver_re\usb_tx\vendor.pcap'
$USBP   = "$env:ProgramFiles\USBPcap\USBPcapCmd.exe"
$CTRL   = '\\.\USBPcap1'   # verificado 2026-09-30: unico filtro, hub del RT3070
$HWID   = 'USB\VID_148F&PID_3070\1.0'

function Log([string]$m) {
    $line = "[{0}] {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'), $m
    try { Add-Content -LiteralPath $LOG -Value $line -Encoding UTF8 } catch {}
    Write-Output $line
}

# --- auto-elevacion ----------------------------------------------------------
$id  = [Security.Principal.WindowsIdentity]::GetCurrent()
$pr  = New-Object Security.Principal.WindowsPrincipal($id)
if (-not $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Log "No-admin: relanzando elevado (UAC). Acepta el prompt."
    $hostExe = (Get-Process -Id $PID).Path
    $arg = "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    if ($args.Count -gt 0) { $arg += ' ' + ($args -join ' ') }
    try {
        $p = Start-Process -FilePath $hostExe -ArgumentList $arg -Verb RunAs -PassThru -Wait
        Log "Elevado terminado exit=$($p.ExitCode). Ver $LOG"
    } catch {
        Log "ELEVACION FALLADA: $($_.Exception.Message)"
    }
    exit
}

Log "=== sniff_vendor_admin INICIO (scan=${secScan}s) ==="
Log "Host: $([Environment]::MachineName) user=$([Environment]::UserName)"

# --- 1/6 USBPcap instalado ---------------------------------------------------
if (-not (Test-Path -LiteralPath $USBP)) {
    Log "X USBPcap NO instalado ($USBP). Instalar USBPcapSetup-1.5.4.0.exe + REBOOT."
    exit 1
}
Log "1/6 USBPcap OK: $USBP"

# --- 2/6 Antena presente y en netr28ux ---------------------------------------
$dev = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F&PID_3070*' } | Select-Object -First 1
if (-not $dev) { Log "X RT3070 NO conectado. Conecta la antena y re-ejecuta."; exit 1 }
$inf = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
Log "2/6 Device: $($dev.FriendlyName) status=$($dev.Status) problem=$($dev.Problem) INF=$inf"
if ("$inf" -notmatch 'netr28ux') {
    Log "X La antena NO esta en netr28ux (esta en $inf). Ejecuta restore_netr28ux.ps1 y re-ejecuta."
    exit 1
}
$parent = (Get-PnpDeviceProperty -InstanceId $dev.InstanceId -KeyName 'DEVPKEY_Device_Parent' -ErrorAction SilentlyContinue).Data
Log "    parent hub: $parent   (USBPcap1 = ese hub, Port 6)"

# --- 3/6 Estado base (para comparar despues) ---------------------------------
$base = netsh wlan show interfaces 2>&1 | Out-String
Log ("3/6 netsh base: " + (($base -split "`n" | Where-Object { $_ -match 'Nombre|Estado|Estado de radio' }) -join ' / ').Trim())

# --- 4/6 Arrancar captura ----------------------------------------------------
if (Test-Path -LiteralPath $PCAP) { Remove-Item -LiteralPath $PCAP -Force }
$capOut = "$env:TEMP\uifipill_usbp_cap_out.txt"
$capErr = "$env:TEMP\uifipill_usbp_cap_err.txt"
Log "4/6 Capturando en $CTRL -A -> $PCAP"
# -A obligatorio: sin el, USBPcapCmd aborta con "Selected capture options
# result in empty capture" (medido 2026-09-30).
$cap = Start-Process -FilePath $USBP -ArgumentList @('-d', $CTRL, '-A', '-o', $PCAP) `
        -PassThru -WindowStyle Hidden -RedirectStandardOutput $capOut -RedirectStandardError $capErr
Start-Sleep -Seconds 3
if ($cap.HasExited) {
    $code = $null
    try { $code = $cap.ExitCode } catch {}
    Log "X USBPcapCmd salio ya exit=$code stdout=$((Get-Content $capOut -Raw -ErrorAction SilentlyContinue)) stderr=$((Get-Content $capErr -Raw -ErrorAction SilentlyContinue))"
    exit 1
}
Log "    captura viva pid=$($cap.Id)"

# --- 5/6 Re-init (boot del vendor) + scan ------------------------------------
Log "5/6 RE-INIT del driver (pnputil /restart-device) -> firmware + BBP/RF init"
$rd = pnputil /restart-device $HWID 2>&1 | Out-String
Log ("    restart-device: " + ($rd.Trim() -replace '\s+', ' '))
Start-Sleep -Seconds 6

$dev2 = Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_148F*PID_3070*' } | Select-Object -First 1
Log ("    device tras restart: status=$($dev2.Status) problem=$($dev2.Problem)")
if ($dev2.Status -ne 'OK') {
    Log "X El device no volvio OK - intenta fix56_inf_restore.ps1 / restore_netr28ux.ps1"
}

Log "    scan loop ${secScan}s (netsh)..."
$deadline = (Get-Date).AddSeconds($secScan)
$n = 0
while ((Get-Date) -lt $deadline) {
    $null = netsh wlan show networks mode=bssid 2>&1
    $n++
    Start-Sleep -Seconds 2
}
Log "    $n scans emitidos"

# --- 6/6 Parar y comprobar ---------------------------------------------------
if (-not $cap.HasExited) { $cap.Kill() }
Wait-Process -Id $cap.Id -ErrorAction SilentlyContinue
if (-not $cap.HasExited) { try { Stop-Process -Id $cap.Id -Force -ErrorAction SilentlyContinue } catch {} }

if (Test-Path -LiteralPath $PCAP) {
    $sz = (Get-Item -LiteralPath $PCAP).Length
    Log "6/6 OK: $PCAP ($sz bytes)"
} else {
    Log "X No se escribio el pcap. stdout=$((Get-Content $capOut -Raw -ErrorAction SilentlyContinue)) stderr=$((Get-Content $capErr -Raw -ErrorAction SilentlyContinue))"
}

$after = netsh wlan show interfaces 2>&1 | Out-String
$okNet = ($after -match '802.11n USB Wireless LAN Card')
Log ("    netsh tras captura: " + $(if ($okNet) { 'interfaz VISIBLE (OK)' } else { 'interfaz NO VISIBLE - RESTAURAR con restore_netr28ux.ps1' }))
Log "=== FIN. Siguiente: usb_sniff_stats (parsear bulk OUT de 148f:3070) ==="
