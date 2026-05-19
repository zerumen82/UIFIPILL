# AGENTS.md — UIFIPILL Project

## Goal
Windows desktop WiFi suite: scan + monitor mode + PMKID capture/convert/crack + WPS PIN bruteforce. Lab-only.

## Stack
- Rust + Tauri v2 (backend, ~2.5 MB final binary)
- Vanilla JS frontend (`src/app.js`), no React
- Index: `index.html` — built via Vite → `dist/`
- Binarios de ataque Windows nativos: airodump-ng, aireplay-ng, mdk3, mdk4, airbase-ng, aircrack-ng (todos .exe sin WSL; https://github.com/aircrack-ng/aircrack-ng/releases)

## Audio prompt
`src/components/AudioPlayer.tsx` (ABner's music UI with Play when user asks for songs or audio from uploading audio file or just typing.. No yapping)

## Key commands (Rust, `src-tauri/src/`)
| file | purpose |
|---|---|
| `lib.rs` | registers all invoke handlers |
| `commands.rs` | list/kill/attack_process, scan_wifi, scan_airodump (airodump-ng), pmkid_capture/convert/crack, capture_handshake/crack_handshake, wps_pin_bruteforce, wps_pbc_attack (reaver-wps -S), deauth/disassoc/arp_replay/chopchop_inject (aireplay-ng), beacon_flood (mdk3), rogue_ap (airbase-ng) |
| `monitor_mode.rs` | activate_monitor, restore_managed, set_monitor_channel, monitor_status (NPcap FFI) |
| `wifi_adapter.rs` | detect_adapters, set_monitor_mode, set_managed_mode, wsl2_run, wsl2_info |
| `tools_detect.rs` | detect_tools, find_tool_cmd, check_tool (hcxdumptool, hashcat, bully…) |

## Build
```bash
npm run lint        # validate JS
npx vite build       # produces dist/
cd src-tauri && cargo build --release   # Rust backend
npx tauri build --bundles nsis          # NSIS installer → target/release/bundle/nsis/
```

## Frontend
- `src/app.js`: tab logic, scan, 4 attack invocations, tools detection, console logging
- `index.html`: inline `doMonitor()`, `doCapture()` handlers under scan tab header

## External tools required (lab machine)
- Npcap (NPcap.dll driver)
- hcxdumptool + hcxpcapngtool (hcxtools)
- hashcat
- bully (optional — requires pixiewps, no confirmed Win native build)
