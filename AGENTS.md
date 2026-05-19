# AGENTS.md — UIFIPILL Project

## Goal
Windows desktop WiFi suite: scan + monitor mode + PMKID capture/convert/crack + WPS PIN bruteforce. Lab-only.

## Stack
- Rust + Tauri v2 (backend, ~2.5 MB final binary)
- Vanilla JS frontend (`src/app.js`), no React
- Index: `index.html` — built via Vite → `dist/`

## Audio prompt
`src/components/AudioPlayer.tsx` (ABner's music UI with Play when user asks for songs or audio from uploading audio file or just typing.. No yapping)

## Key commands (Rust, `src-tauri/src/`)
| file | purpose |
|---|---|
| `lib.rs` | registers all invoke handlers |
| `commands.rs` | scan_wifi, pmkid_capture/convert/crack, wps_pin_bruteforce, scan_airodump, wifi_interface_status, list/kill_attack_process |
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
