# UIFIPILL - WiFi Suite for Windows

Windows desktop WiFi suite: scan + monitor mode + PMKID capture/convert/crack + WPS PIN bruteforce.

## Requirements

- Windows 10/11
- Npcap (https://npcap.com/) - for monitor mode
- MSYS2 (https://www.msys2.org/) - for aircrack-ng suite (airodump-ng, aireplay-ng, airbase-ng)
- WSL2 - for hcxdumptool, hcxpcapngtool, bully, reaver, mdk3
- hashcat - for cracking

## Binaries

All attack tools must run without WSL2 where possible:
- airodump-ng, aireplay-ng, airbase-ng: MSYS2 MinGW64 (pacman -S mingw-w64-ucrt-x86_64-aircrack-ng)
- hashcat: native Windows .exe
- hcxdumptool, hcxpcapngtool, bully, reaver, mdk3: require WSL2

## Build

```bash
npm run lint
npx vite build
cd src-tauri && cargo build --release
npx tauri build --bundles nsis
```

## Vías de conexión (objetivo: conectarse a la red del lab)

| Vía | Estado en UIFIPILL | Dónde corre | Éxito real |
|---|---|---|---|
| Crack PMKID/handshake + `wifi_connect` | ✅ Inspector, estrategia (`-a 0/1/3/6/7`), conectar por perfil netsh | Windows | Alto si clave débil |
| Keygen Comtrend (Jazztel_/WLAN_) | ✅ Detect + 512/1 candidatos (MD5 verificado) | Windows | Alto si no cambiaron la clave |
| Portal Evil Twin verificado vs handshake | ✅ Kit Kali + `verify_candidate` local | Kali / Windows | Alto vs humanos |
| Downgrade transición WPA2+WPA3 | ✅ Audit + rogue .conf (confirmar MFP en Kali) | Kali | Medio |
| Wacker SAE online | ⚠️ Guía con plantilla (sin ejecución) | Kali | Bajo (solo claves débiles) |
| WPS Pixie Dust / PIN | ✅ Tarjetas (reaver.exe sin probar en Win) | Dudoso en Win | Instantáneo si chipset vulnerable |
| Keygen Thomson (ThomsonXXXXXX, Orange-…) | ❌ Backlog: exige diccionario OUI + SHA1 masivo | — | Medio en routers viejos |
| PIN WPS offline (ComputePIN/Arcadyan) | ❌ Backlog | — | Medio |
| Relay EAP enterprise | ❌ Backlog (hostapd-mana + wpa_sycophant) | Kali | Medio si sin validación de cert |
| KRACK / FragAttacks | ❌ Descartado: parcheado desde 2017/2021 | — | Nulo |

Sin WSL instalado en esta máquina y sin inyección NDIS en Windows, todo lo
que transmita 802.11 (deauth, rogue activo, Wacker) es Kali + adaptador
compatible. Lo que calcula (crack, keygen, verify, perfiles, kits) corre aquí.
