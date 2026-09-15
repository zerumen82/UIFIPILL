# UIFIPILL - WiFi Suite for Windows

Suite WiFi de escritorio para Windows: scan + modo monitor + captura PMKID/handshake +
crack con hashcat + WPS PIN/Pixie Dust + keygen de router. **Solo laboratorio** (redes
propias o con autorización explícita).

- Estado técnico y bitácora de decisiones: `AGENTS.md`
- Motor dual WSL2/Kali (RF) con bloqueadores medidos: `ROADMAP_WSL2.md`

## Requirements

- Windows 10/11
- **Npcap 1.10+** con API WinPcap-compatible y soporte Dot11 — modo monitor y captura
  pasiva nativa vía `wpcap.dll` (sin binarios externos)
- **hashcat 7.x** + diccionario (en este repo: `tools/hashcat.exe`, `tools/rockyou.txt`)
- Opcional, para RF fuera de Windows: **WSL2 + Kali** (`wsl --install -d kali-linux`) con
  `usbipd-win`, o un adaptador con driver Dot11 completo (RTL8812AU / AR9271)
- Solo para recompilar herramientas nativas: MSYS2 + mingw-w64 (ver `tools/build/BUILD.md`)

## Binaries

| Binario | Origen | Notas |
|---|---|---|
| `reaver.exe`, `wash.exe` | port Windows propio (fuente + parche en `tools/build/`) | WPS PIN/Pixie Dust y wash; TX limitada por el driver |
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

## Qué corre dónde (mapa honesto, 2026-09-15)

| Área | Estado | Dónde |
|---|---|---|
| Scan de redes + perfilado del objetivo | ✅ | Windows |
| Modo monitor (OID) + captura pasiva + conversor `.22000` nativo | ✅ | Windows (Npcap Dot11) |
| Crack PMKID/handshake + `wifi_connect` | ✅ | Windows (hashcat + netsh) |
| Keygen Comtrend (MD5) y **Thomson** (SHA1, 21,8 M con threads) | ✅ | Windows |
| Auditoría WPA3 / rogue conf / kit Evil Twin / verificar 1 candidato | ✅ (generan y guían) | Windows (ejecución en Kali) |
| Inyección (deauth, ARP replay, rogue activo, beacon flood) y cambio de canal | ❌ bloqueado por driver/Npcap | Kali vía WSL2 |
| Recetas RF en Kali (hcxdumptool, airodump, wash, deauth, reaver) | ⚠️ código + UI listos, RF bloqueada | Kali-WSL2 (usbipd / VirtualHere) |
| WPS Pixie Dust / PIN con `reaver.exe` nativo | ⚠️ requiere adaptador con Dot11 y TX | Windows (limitado) |
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
| WPS Pixie Dust / PIN | ✅ Tarjetas (reaver.exe); TX no confirmada en Win | Dudoso en Win | Instantáneo si el chipset es vulnerable |
| PIN WPS offline (ComputePIN/Arcadyan) | ❌ Backlog | — | Medio |
| Relay EAP enterprise | ❌ Backlog (hostapd-mana + wpa_sycophant) | Kali | Medio si no valida certificado |
| KRACK / FragAttacks | ❌ Descartado: parcheado desde 2017/2021 | — | Nulo |

## Límites medidos en Windows (RT3070 + Npcap 1.10.5)

- Inyección: `pcap_sendpacket` sobre `\Device\NPF_WIFI_{GUID}` devuelve
  `ERROR_GEN_FAILURE` (31) → sin TX (con y sin radiotap).
- Cambio de canal: agotadas las 3 vías OID (canal, frecuencia y vendor Ralink); la radio
  se queda en el canal en el que esté. Todos los flujos de la UI heredan el canal del AP
  escaneado.
- `airodump-ng` / `aireplay-ng` de Windows responden `Adapter not supported` (esperan
  AirPcap): para RF real hay que ir a Kali con un chipset compatible.

Bloqueadores abiertos del motor dual (detalle y bitácora en `ROADMAP_WSL2.md`): RX muerta
en Kali sobre usbipd, VM WSL2 reciclada por el host cada 5–60 min y VirtualHere `USE` →
`API Timeout` con trial. Alternativas: licencia VH, cliente GUI bajo WSLg, depurar RX de
usbipd o Kali bare-metal.

Lo que calcula (crack, keygen, verificación, perfiles, kits) corre aquí; todo lo que
transmita 802.11 (deauth, rogue activo, Wacker) exige Kali + adaptador compatible.
