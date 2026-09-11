
# Actualizar setup_tools.ps1 — SIN WSL2, solo binarios Windows nativos
# Aircrack-ng Suite + MSYS2 MinGW64 portable
# ================================================================

$ErrorActionPreference = "Continue"
$ProgressPreference    = "SilentlyContinue"

# Directorios
$ToolsDir   = "$env:APPDATA\UIFIPILL\tools"
$ToolsDir86 = "$env:APPDATA\UIFIPILL\tools-32bit"
$TmpDir     = "$env:TEMP\UIFIPILL_setup"
if (Test-Path $TmpDir) { Remove-Item -Recurse -Force $TmpDir -EA SilentlyContinue }
New-Item -ItemType Directory -Path [$ToolsDir,$ToolsDir86,$TmpDir] -Force | Out-Null

# ── Helpers ────────────────────────────────────────────────────────
function Download-File {
    param([string]$Url, [string]$Dest, [string]$Label)
    if (Test-Path $Dest) { Write-Host "  Ya existe, omitiendo." -Foreground Green; return $true }
    Write-Host "  [$Label] Descargando..." -NoNewline
    try {
        Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing -ErrorAction Stop
        Return $true
    } catch {
        Write-Host " ERROR" -Foreground Red; return $false
    }
}
function Extract-Zip {
    param([string]$Zip, [string]$Dest)
    if (!(Test-Path $Dest)) { New-Item -ItemType Directory -Path $Dest -Force | Out-Null }
    try { Expand-Archive -Path $Zip -DestinationPath $Dest -Force -EA Stop; return $true } catch { return $false }
}

# =========================================================================
# 1. HASHCAT
# =========================================================================
$hz = "$ToolsDir\hashcat.exe"
if (Test-Path $hz) {
    Write-Host "hashcat: OK ($hz)"
} else {
    $zip = "$TmpDir\hashcat-7.1.2.7z"
    if (Download-File "https://github.com/hashcat/hashcat/releases/download/v7.1.2/hashcat-7.1.2.7z" $zip "hashcat") {
        # extraer.7z con 7z o tar
        $has7z = Get-Command "7z" -EA SilentlyContinue
        if ($has7z) { & 7z x $zip -o"$ToolsDir\" -y 2>$null }
        else { tar -xf $zip -C "$ToolsDir\" 2>$null }
        $exe = Get-ChildItem $ToolsDir -Recurse -Filter "hashcat.exe" -EA SilentlyContinue | Select -First 1
        if ($exe) { Copy-Item $exe.FullName $hz -Force; Write-Host "hashcat: OK → $hz" }
        else { Write-Host "hashcat: extrae manualmente desde $zip" }
    }
}

# =========================================================================
# 2. CYGWIN + AIRCRACK-NG (aCRACK-NG Suite nativa Windows)
# =========================================================================
Write-Host ""
Write-Host "=== AIRCRACK-NG SUITE v1.7 (Cygwin) ==="

$acZip = "$TmpDir\aircrack-ng-1.7-win.zip"
if (!(Test-Path $acZip)) {
    Download-File "https://download.aircrack-ng.org/aircrack-ng-1.7-win.zip" $acZip "aircrack-ng-1.7-win.zip"
}

# Si ya tenemos el zip extraido, reusarlo
$acExtracted = "C:\Users\INdahouse\AppData\Local\Temp\kilo\ac_new"
if (!(Test-Path $acExtracted) -or !(Test-Path "$acExtracted\aircrack-ng-1.7-win\bin")) {
    if ((Test-Path $acZip)) { Extract-Zip $acZip $acExtracted }
}

$acBin = "$acExtracted\aircrack-ng-1.7-win\aircrack-ng-1.7-win\bin"
$filesCopied = 0
if (Test-Path $acBin) {
    Get-ChildItem $acBin -File | ForEach-Object {
        Copy-Item $_.FullName "$ToolsDir\$($_.Name)" -Force -EA SilentlyContinue
        Copy-Item $_.FullName "$ToolsDir86\$($_.Name)" -Force -EA SilentlyContinue
        $filesCopied++
    }
    Write-Host "aircrack-ng-1.7: $filesCopied archivos copiados a tools+ y tools-32"
} else {
    Write-Host "aircrack-ng-1.7: extrae $acZip manualmente y copia bin\a* a la carpeta de herramientas"
}

# =========================================================================
# 3. MSYS2 / MinGW64 — airodump-ng / aireplay-ng / airbase-ng NATIVOS
#    (sin Cygwin, sin airpcap.dll — puro Windows)
# =========================================================================
Write-Host ""
Write-Host "=== MSYS2 MinGW64 (binarios .exe nativos) ==="

# Ver si MSYS2 esta instalado
$msys2 = $null
$msys2_pacman = Get-ChildItem "C:\msys64" -EA SilentlyContinue
if ($msys2_pacman) { $msys2 = "C:\msys64" }

if ($msys2) {
    Write-Host "MSYS2 detectado en $msys2"
    # Usar pacman de MSYS2 para instalar aircrack-ng MinGW64
    $env:MSYSTEM = "UCRT64"
    & "$msys2\usr\bin\bash.exe" -lc "pacman -Sy --noconfirm mingw-w64-ucrt-x86_64-aircrack-ng 2>&1" | Out-Host
    # Copiar binarios a ToolsDir
    $msys_bin = "$msys2\ucrt64\bin"
    if (Test-Path $msys_bin) {
        Get-ChildItem $msys_bin -Filter "*.exe" | ForEach-Object {
            Copy-Item $_.FullName "$ToolsDir\" -Force -EA SilentlyContinue
            Copy-Item $_.FullName "$ToolsDir86\" -Force -EA SilentlyContinue
        }
        Write-Host "Binarios copiados a $ToolsDir"
    }
} else {
    Write-Host ""
    Write-Host "MSYS2 no esta instalado. Instala MSYS2 para binarios nativos de AIRCRACK-NG:"
    Write-Host ""
    Write-Host "  1. Descarga: https://www.msys2.org/"
    Write-Host "  2. Ejecuta msys2-x86_64-latest.exe"
    Write-Host "  3. En el terminal UCRT64:"
    Write-Host '     pacman -S base-devel mingw-w64-ucrt-x86_64-gcc'
    Write-Host '     pacman -S mingw-w64-ucrt-x86_64-aircrack-ng'
    Write-Host '     pacman -S mingw-w64-ucrt-x86_64-libpcap'
    Write-Host ""
    Write-Host "  Los binarios quedan en: C:\msys64\ucrt64\bin\"
    Write-Host "  (airodump-ng.exe, aireplay-ng.exe, airbase-ng.exe, etc.)"
    Write-Host ""
}

# =========================================================================
# 4. HCXTOOLS — NO hay .exe nativo, usar WSL2 o Cygwin
# =========================================================================
Write-Host ""
Write-Host "=== HCXTOOLS (WSL2) ==="
foreach ($t in @("hcxdumptool","hcxpcapngtool")) {
    $f = "$ToolsDir\$t.exe"
    if (Test-Path $f) { Write-Host "  $t: OK ($ToolsDir)" }
    else {
        Write-Host "  $t: FALTA (no hay .exe nativo)"
        Write-Host "  -> WSL2 Ubuntu: wsl sudo apt install -y hcxtools"
    }
}

# =========================================================================
# 5. BULLY / REAVER — WSL2
# =========================================================================
Write-Host ""
Write-Host "=== BULLY + REAVER (WSL2) ==="
foreach ($t in @("bully","reaver-wps-fork-t6x")) {
    $f = "$ToolsDir\$t.exe"
    if (Test-Path $f) { Write-Host "  $t: OK" }
    else {
        Write-Host "  $t: FALTA (no hay .exe nativo)"
        Write-Host "  -> WSL2 Ubuntu: wsl sudo apt install -y bully reaver"
    }
}

# =========================================================================
# 6. MDK3 / MDK4 — WSL2
# =========================================================================
Write-Host ""
Write-Host "=== MDK3 / MDK4 (WSL2) ==="
foreach ($t in @("mdk3.exe","mdk4.exe")) {
    $f = "$ToolsDir\$t"
    if (Test-Path $f) { Write-Host "  $t: OK" }
    else { Write-Host "  $t: FALTA -> WSL2 Ubuntu: wsl sudo apt install -y mdk3" }
}

# =========================================================================
# 7. ROCKYOU.TXT
# =========================================================================
Write-Host ""
Write-Host "=== WORDLISTS ==="
$ry = "$ToolsDir\rockyou.txt"
if (Test-Path $ry) {
    $mb = [math]::Round((Get-Item $ry).Length / 1MB, 1)
    Write-Host "  rockyou.txt: OK ($mb MB)"
} else {
    Write-Host "  rockyou.txt: FALTA (~145 MB)"
    Write-Host "  Descargala: https://weakpass.com/download"
}

# =========================================================================
# RESUMEN
# =========================================================================
Write-Host ""
Write-Host "===== RESUMEN ====="

$summary = [ordered]@{}
foreach ($t in @("hashcat","airodump-ng","aireplay-ng","airbase-ng",
                 "hcxdumptool","hcxpcapngtool","bully","reaver-wps-fork-t6x",
                 "mdk3","rockyou.txt")) {
    $found = Test-Path "$ToolsDir\$t"
    $src   = if ($t -in @("hcxdumptool","hcxpcapngtool","bully","reaver-wps-fork-t6x","mdk3")) { "WSL2" } else { "Windows" }
    if ($found) {
        $status = if ($src -eq "WSL2") { "[OK-WSL ]" } else { "[WIN  OK]" }
        Write-Host ("{0} {1}  ({2})" -f $status, $t, $src) -Foreground Green
    } else {
        $status = if ($src -eq "WSL2") { "[WSL FAL]" } else { "[WIN FAL]" }
        Write-Host ("{0} {1}  (instala en {2})" -f $status, $t, $src) -Foreground Yellow
    }
}

Write-Host ""
Write-Host "Herramientas instaladas en: $ToolsDir"
Get-ChildItem $ToolsDir -EA SilentlyContinue | ForEach-Object { Write-Host "  $($_.Name)" }

Remove-Item -Recurse -Force $TmpDir -EA SilentlyContinue
