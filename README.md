# UIFIPILL - WiFi Suite for Windows

Suite WiFi de escritorio para Windows: scan + modo monitor + captura PMKID/handshake +
crack con hashcat + WPS PIN/Pixie Dust + keygen de router. **Solo laboratorio** (redes
propias o con autorización explícita).

- Reglas, objetivos y estado vigente: `AGENTS.md`
- Bitácora de sesiones e historia técnica: `memory.md` (§1 sesiones · §2 WSL2/Kali · §3 RE driver · §4 RX WinUSB · §5 Windows-vs-Linux)
- Especificación SDD (principios, arquitectura, requisitos, pendientes): `spec/`

## Requirements

- Windows 10/11
- **Npcap 1.10+** con API WinPcap-compatible y soporte Dot11 — modo monitor y captura
  pasiva nativa vía `wpcap.dll` (sin binarios externos)
- **hashcat 7.x** + diccionario (en este repo: `tools/hashcat.exe`, `tools/rockyou.txt`)
- Opcional, para RF fuera de Windows: **WSL2 + Kali** (`wsl --install -d kali-linux`) con
  `usbipd-win`, o un adaptador con driver Dot11 completo (RTL8812AU / AR9271)
- Opcional: MSYS2 + mingw-w64 solo para recompilar herramientas desde su upstream
  (el sistema de build de reaver se eliminó del repo; ver `AGENTS.md`)

## Binaries

| Binario | Origen | Notas |
|---|---|---|
| `reaver.exe`, `wash.exe` | port Windows propio (binarios en `tools/`; fuentes solo locales en `tools/build/`, **fuera del repo**) | WPS PIN/Pixie Dust y wash; wash ✅ survey en Windows; reaver no asocia (Npcap #85) → PIN real en Kali |
| `hashcat.exe`, `rockyou.txt` | oficiales (7.1.2) | crack `-m 22000` en Windows |
| aircrack-ng 1.7 (airodump/aireplay/airbase) | Cygwin (`tools/aircrack-ng-win/`) | **no soportan Npcap** (esperan AirPcap HW): en Windows la RF es pasiva |
| hcxdumptool, hcxpcapngtool, bully, mdk3, pixiewps | sin port Windows | se ejecutan en Kali, o se sustituyen por lo nativo (captura + conversor en Rust) |

## Build

```bash
npm run lint                          # valida src/app.js
npx vite build                        # dist/
cd src-tauri && cargo build --release # backend Rust
npx tauri build --bundles nsis        # instalador → src-tauri/target/release/bundle/nsis/
```

## Qué corre dónde (mapa honesto, 2026-10-06)

| Área | Estado | Dónde |
|---|---|---|
| Scan de redes + perfilado del objetivo | ✅ | Windows (netsh + `rt3070_scan` crudo) |
| Modo monitor (OID) + captura pasiva + conversor `.22000` nativo | ✅ | Windows (Npcap Dot11) |
| Cambio de canal en monitor | ✅ (driver netr28ux **parcheado**, testsigning) | Windows |
| RX y TX al aire por USB crudo (WinUSB) | ✅ (524 frames/15 s; beacons/deauth verificados) | Windows |
| Crack PMKID/handshake + `wifi_connect` | ✅ | Windows (hashcat + netsh) |
| Keygen Comtrend (MD5) y **Thomson** (SHA1, 21,8 M con threads) | ✅ | Windows |
| wash (WPS survey) | ✅ E2E en modo monitor (iface `NPF_WIFI_` + `-F`) | Windows |
| Auditoría WPA3 / rogue conf / kit Evil Twin / verificar 1 candidato | ✅ (generan y guían) | Windows (ejecución en Kali) |
| Inyección vía Npcap (`pcap_sendpacket`, aireplay-ng, reaver TX) | ❌ err 31/203 (Npcap #85, cerrado en RE) | — |
| Recetas RF Kali (hcxdumptool, airodump, deauth, reaver/PIN real) | ✅ código + UI listos | **Kali live USB** (plan A; usbipd→WSL2 y VirtualHere bloqueados, medido) |
| WPS PIN/Pixie nativo (asociación + TX) | ⚠️ survey y parsing OK; TX no puede Windows → E2E real en Kali | Windows (limitado) / Kali |
| Wacker SAE online / relay EAP enterprise | ❌ backlog (solo guía) | Kali |
| KRACK / FragAttacks | ❌ descartado (parcheado 2017/2021) | — |

## Vías de conexión (objetivo: entrar en la red del lab)

| Vía | Estado en UIFIPILL | Dónde corre | Éxito real |
|---|---|---|---|
| Crack PMKID/handshake + `wifi_connect` | ✅ Inspector, estrategia (`-a 0/1/3/6/7`), conectar por perfil netsh | Windows | Alto si la clave es débil |
| Keygen Comtrend (Jazztel_/WLAN_) | ✅ Detect + 512/1 candidatos (MD5 verificado) | Windows | Alto si no cambiaron la clave |
| Keygen Thomson (ThomsonXXXXXX, Orange-…) | ✅ `thomson_run` (SHA1 "CP"+YY+WW+hex3, vector SpeedTouchF8A3D0 verificado) | Windows | Medio en routers viejos |
| Portal Evil Twin verificado vs handshake | ✅ Kit Kali + `verify_candidate` local | Kali / Windows | Alto vs humanos |
| Downgrade transición WPA2+WPA3 | ✅ Audit + rogue `.conf` (confirmar MFP en Kali) | Kali | Medio |
| Wacker SAE online | ⚠️ Guía con plantilla (sin ejecución) | Kali | Bajo (solo claves débiles) |
| WPS Pixie Dust / PIN | ✅ Tarjetas (reaver.exe: survey/parseo OK); asociación+TX falla por Npcap #85 → E2E real en Kali live USB | Kali (Windows limitado) | Instantáneo si el chipset es vulnerable |
| PIN WPS offline (ComputePIN/Arcadyan) | ❌ Backlog | — | Medio |
| Relay EAP enterprise | ❌ Backlog (hostapd-mana + wpa_sycophant) | Kali | Medio si no valida certificado |
| KRACK / FragAttacks | ❌ Descartado: parcheado desde 2017/2021 | — | Nulo |

## Límites medidos en Windows (RT3070; revisado 2026-10-06)

- **Inyección por Npcap**: `pcap_sendpacket` sobre `\Device\NPF_WIFI_{GUID}`
  devuelve `ERROR_GEN_FAILURE` (31) o err 203 → sin TX vía Npcap (Npcap #85,
  cerrado en RE: el bloqueo está en el LWF/path NPC, no en el driver).
- **Cambio de canal**: con el driver **netr28ux parcheado** (testsigning) la radio
  SÍ cambia en monitor (verificado 2026-09-21); con el stock no (3 vías OID
  rechazadas). Todos los flujos de la UI heredan el canal del AP escaneado.
- `airodump-ng` / `aireplay-ng` de Windows responden `Adapter not supported`
  (esperan AirPcap): RF interactiva nativa solo por la vía USB crudo (WinUSB) o
  Kali.
- **Con WinUSB activo la antena NO es WiFi para netsh** → revertir con
  `driver_re/usb_tx/restore_netr28ux.ps1`. Nunca kick `FIRMWARE(8)` bajo WinUSB
  (mata el chip; solo power-cycle físico de 15 s lo recupera).

## Auditoría de stubs/fakes (2026-09-17)

Revisión completa buscando código que simule éxito o ignore entradas. Corregidos:

- **Capturas hcxdumptool**: `pmkid_capture(_bg)` y `capture_handshake(_bg)` lanzaban
  `-i` sin interfaz (el flag se tragaba `-t`). Ahora aceptan el campo Interfaz de la
  tarjeta y, si falta, avisan con el GUID NPF a pegar (igual que el resto de comandos).
- **Crack sin falsos positivos**: el éxito de `pmkid_crack`/`crack_handshake` se
  confirma leyendo el `.cracked` que escribe hashcat, no parseando su stdout
  (lleno de líneas con `:` que antes daban "PASSWORD CRACKEADA" en falso).
- **Conversor nativo**: `pcap_to_22000` trata bien las capturas DLT 105 (802.11 sin
  radiotap); antes las corrompía saltando 4 bytes inexistentes de cabecera.
- `list_attack_processes` ya no devuelve éxito si powershell falla.

Todo lo demás se revisó y es real: OIDs Npcap, captura wpcap.dll, parser .22000,
keygen con vectores de test, validadores anti-inyección en WSL y veredictos
medidos (no simulados) de inyección/canal.

Bloqueadores abiertos del motor dual (detalle y bitácora en `memory.md` §2): RX muerta
en Kali sobre usbipd, VM WSL2 reciclada por el host cada 5–60 min y VirtualHere `USE` →
`API Timeout` con trial. Alternativas: licencia VH, cliente GUI bajo WSLg, depurar RX de
usbipd o Kali bare-metal.

Lo que calcula (crack, keygen, verificación, perfiles, kits) corre aquí; todo lo que
transmita 802.11 (deauth, rogue activo, Wacker) exige Kali + adaptador compatible.
