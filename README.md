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
