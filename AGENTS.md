# AGENTS.md — UIFIPILL Project

> **Reglas, objetivos, arquitectura y estado VIGENTE** (última revisión 2026-09-29).
> La historia vive en **`memory.md`**: §1 bitácora de sesiones (2026-09-13 → 2026-09-29),
> §2 roadmap motor dual WSL2/Kali, §3 RE de netr28ux.sys, §4 investigación RX WinUSB,
> §5 por qué no funciona igual que en Windows-vs-Linux.
> README.md = overview para usuario. Lab-only; no commit sin petición explícita.

## Objetivo (Goal)
Windows desktop WiFi suite: scan + modo monitor + PMKID capture/convert/crack +
WPS PIN bruteforce + keygen. **Solo laboratorio** (redes propias o con
autorización explícita).

## Reglas del proyecto
1. **Verificación obligatoria tras tocar código**: `npm run lint` + `npx vite build`
   + `cd src-tauri && cargo build --release` (**0 warnings**) + `cargo test --lib`
   (23 passed / 5-7 ignored — los `lab_*` exigen HW). Entorno: el runner corta a
   ~30 s y mata el árbol de procesos → builds largos con `schtasks /create + /run`
   y sondeo del log.
2. **Sin fakes/stubs**: ningún comando devuelve éxito simulado. Éxito = evidencia
   real (`.cracked` escrito por hashcat, PIN parseado, línea `TXDONE` del chip).
   Binario ausente → hint accionable (preflight en `run_bin`/`run_bin_bg`), nunca
   fallo críptico. Auditorías previas en memory §1 (rondas 2026-09-14/17/18).
3. **USB/RT3070 — reglas duras (MEDIDO, no reintentar)**:
   - **NUNCA kick `USB_DEVICE_MODE FIRMWARE(8)` bajo WinUSB** — mata el chip (5/5);
     solo el power-cycle físico (15 s) lo recupera.
   - Chip degradado (STALL, registros 0x00/0xff, timeout total) → power-cycle
     antes del siguiente test. Antena nueva (…D8:A7) se degrada más rápido.
   - Leer `MCU_CURRENT` antes de decidir cargar firmware; nunca kick con MCU vivo.
   - Vendor requests RT2800/RT3070: `SINGLE_WRITE=2, SINGLE_READ=3, MULTI_WRITE=6,
     MULTI_READ=7` (la tabla 2/3/4/5 es de RT2500/RT73).
   - `USB_DEVICE_MODE`: el modo va en **wValue** (wIndex=0); FIRMWARE=8, UNPLUG=2,
     RESET=1, AUTORUN=17. RESET antes de leer registros deja el chip sordo.
   - Constantes críticas: H2M_MAILBOX_CSR=0x7010, CID=0x7014, BBP_AGENT=0x7028,
     HOST_CMD_CSR=0x0404 (MCU_CMD va por aquí); `BBP_RW_MODE` (1<<19) en todo
     acceso BBP; USB_DMA_CFG=0x02a0 con AGG_LIMIT=**301**; MCU OWNER=0x01000000
     (no 0x80000000); AUTOWAKEUP_CFG (0x1208) antes del firmware.
   - **RX por USB crudo WinUSB: CERRADA** (BBP mudo en 2 antenas; el autoload de
     fábrica no deja el BBP vivo — memory §4/§5). **TX por USB crudo: CONFIRMADO
     al aire** (hito 2026-09-25). Única vía RX-WinUSB abierta: sniffer USBPcap.
4. **Windows/driver**:
   - Driver netr28ux **parcheado** (testsigning, 2026-09-21) → el canal en monitor
     SÍ cambia; el stock rechazaba (3 vías OID). TX por Npcap sigue cerrada
     (Npcap #85, err 31) — capa independiente, cerrada en RE (memory §3).
   - Con WinUSB activo la antena NO es WiFi para netsh → revertir con
     `driver_re/usb_tx/restore_netr28ux.ps1`.
   - Instalador NSIS aplana `tools/` en `<exe>\_up_\tools\` → `install_dirs`/
     `resolve_bin_abs` lo cubren + DLLs (libpcap/libwinpthread…) en `resources`.
5. **UAC / entorno no-admin**: el shell NO es admin. Scripts auto-elevados →
   logging **directo a fichero con ruta absoluta hardcodeada** (Start-Transcript
   no fiable: el transcript del lanzador sobreescribe al elevado;
   `Get-AuthenticodeSignature` roto en el PS 5.1 de este equipo).
   USBPcap 1.5.4.0 instalado (`C:\Program Files\USBPcap\USBPcapCmd.exe`).
6. **Cableado UI↔backend**: Tauri v2 convierte args a camelCase (el frontend
   habla camelCase: `durationSeconds`, `ifaceGuid`, `targetBssid`…); respuestas
   Tauri en snake_case donde aplica. Ningún `invoke` sin handler; ningún
   `onclick`/id huérfano (auditar tras tocar index.html/app.js).
7. **Rendimiento**: backend emite `attack-progress` chunked (4-8 KB, techo de
   eventos, coalescencia); frontend pinta por frame y **cero DOM con el tab
   oculto**. Nunca un evento de stdout por línea.
8. Canales: `selectedChannel()` hereda el canal del AP escaneado en todos los
   flujos. Npcap exige Dot11Support (activado en este equipo).

## Estado actual (2026-09-29)
- **PENDIENTE (siguiente sesión)**: sanear el device netr28ux (ProblemCode 31/56
  tras el boot con HVCI; HVCI ya OFF, stack WLAN reparado, servicio recreado):
  ejecutar `driver_re/usb_tx/fix_restart_dev.ps1` en **consola admin manual**
  (el UAC automatizado no llega); si persiste en 56 → **reboot** (el boot
  envenenado fue el único con HVCI activo). Verificación final: device Status OK
  + `netsh wlan show interfaces` muestra la antena + (opcional) `activate_monitor`.
  Detalle: memory §1, sesión 2026-09-29.
- Una vez device OK → `hot_rebind.ps1 to-winusb` (admin) → power-cycle 15 s →
  App → tab Ataque → paso A: Estado chip → Init → TX beacons → TX al aire.
- App instalada en `%LOCALAPPDATA%\UIFIPILL\uifipill.exe` (rev. 2026-09-18 con
  instalador NSIS de 6,8 MB; master con commits sin push — re-verificar tras el
  próximo build).
- Motor dual WSL2/VirtualHere: código completo; RF bloqueada (usbipd RX muerta
  en 3 mediciones, VirtualHere = licencia de pago). **Plan A de RF = Kali live
  USB** + TX crudo Windows (decisión cerrada 2026-09-25).

## Límites hardware medidos (resumen; detalle memory §2/§5)
- **RT3070 + Windows/Npcap**: captura pasiva SÍ (392 pkts reales), crack/keygen/
  scan SÍ; inyección NO (err 31, Npcap #85). Canal en monitor: SÍ con el driver
  parcheado; el stock y las 3 vías OID fallaban.
- **RT3070 + WinUSB crudo (usb_tx)**: TX al aire SÍ (beacons "TXTEST" vistos desde
  otro device, TXDONE confirmado); scan hopping SÍ; **RX NO** (BBP lee 0x00 —
  cerrado, no es código).
- **usbipd → WSL2/Kali**: driver perfecto pero RX muerta por el transporte.
  **VirtualHere**: USE rechazado con trial (licencia). **Kali live USB**: RX
  completa — vía probada.

## Hitos clave (detalle en memory §1)
- 2026-09-14: captura Npcap nativa + conversor `.22000`; monitor↔managed OK.
- 2026-09-21: driver netr28ux parcheado → **la radio cambia de canal en monitor**.
- 2026-09-22: **TX por USB crudo** (WinUSB) pipeline end-to-end; RE TX del driver
  medida y cerrada (Npcap #85).
- 2026-09-23: TXWI/TXINFO reales + feedback TX_STA_FIFO + scan hopping crudo.
- 2026-09-25: **TX AL AIRE confirmado**; RX-WinUSB cerrada (coldrx); 2ª antena
  comparada (BBP mudo igual → es del boot, no del chip).
- 2026-09-28/29: reset PC → HVCI bloqueó netr28ux → HVCI off; device en
  saneamiento (31/56).

## Stack
- Rust + Tauri v2 (backend; `uifipill.exe` ~14 MB release, instalador NSIS ~7 MB)
- Vanilla JS frontend (`src/app.js`), no React
- Index: `index.html` — built via Vite → `dist/`
- Binarios de ataque Windows nativos: airodump-ng, aireplay-ng, mdk3, airbase-ng,
  aircrack-ng (todos .exe sin WSL; https://github.com/aircrack-ng/aircrack-ng/releases)
- Módulo USB crudo: `driver_re/usb_tx/` (Rust + rusb 0.9, binarios rt3070_probe/
  init/tx/scan/deauth/…, INF WinUSB firmado con cert lab)

## Build
```bash
npm run lint        # validate JS
npx vite build       # produces dist/
cd src-tauri && cargo build --release   # Rust backend
npx tauri build --bundles nsis          # NSIS installer → target/release/bundle/nsis/
```

## Key commands (Rust, `src-tauri/src/`)
| file | purpose |
|---|---|
| `lib.rs` | registers all invoke handlers (**95 comandos** registrados) |
| `commands.rs` | scan_wifi, scan_airodump (airodump-ng), pmkid_capture/convert/crack, capture_handshake/crack_handshake, wps_pin_bruteforce, wps_pbc_attack (reaver-wps -S), wps_pixiedust (reaver-wps -K 1), deauth/disassoc/fakeauth/arp_replay/chopchop/cafe-latte/interactive/fragment_inject (aireplay-ng), beacon_flood (mdk3), rogue_ap (airbase-ng), injection_test (aireplay-ng), list/kill/cancel_attack, pmkid_capture_bg/capture_handshake_bg/scan_airodump_bg/wps_pin_bruteforce_bg |
| `monitor_mode.rs` | activate_monitor, restore_managed, set_monitor_channel, monitor_status (NPcap FFI) |
| `wifi_adapter.rs` | detect_adapters, set_monitor_mode, set_managed_mode |
| `tools_detect.rs` | detect_tools, find_tool_cmd, check_tool (hcxdumptool, hashcat, bully…) |
| `capture.rs` | native_capture (captura pasiva vía wpcap.dll + libloading, `\Device\NPF_WIFI_{GUID}`) |
| `pcap_convert.rs` | pcap_to_22000 (parser nativo PMKID/EAPOL, sin hcxpcapngtool) |
| `wsl.rs` | motor dual Kali-WSL2: wsl_exec/attach/detach/from_win/to_win, recetas RF (pmkid, airodump, wash, deauth, reaver, kill, *_stream) y VirtualHere (vh_* ×8) |
| `usb_raw.rs` | puente a los binarios usb_tx (init/scan/tx/deauth, usb_raw_scan JSON) |
| `crack.rs` / `keygen.rs` / `connect.rs` | estrategia hashcat, keygen Comtrend+Thomson, wifi_connect netsh |
| `profiler.rs` / `wpa3.rs` / `eviltwin.rs` | perfilado de objetivo, auditoría WPA3, kit Evil Twin |

## Frontend
- `src/app.js`: tab logic, scan, ~25 attack invocations + auto-attack 9 pasos /
  quick-attack, tools detection, console logging, `genKaliKit()`,
  `applyHardwareGates()` (solo informativo, sin gating)
- `index.html`: inline `doMonitor()` (modal `detect_adapters` → `activate_monitor`),
  `doCapture()` (`pmkid_capture` 30s, exige BSSID) + wizard `.wiz-step` (pasos
  A-D y secciones 1-8) y resto de tabs (attack/console)

## Motor dual WSL2/Kali (resumen; fases y bitácora en memory §2)
- `src-tauri/src/wsl.rs` (20+ comandos): `wsl_exec` (sin shell intermedia,
  5–600 s), `wsl_attach`/`wsl_detach` (usbipd, busid `^[\d-]+$`),
  `wsl_from_win`/`wsl_to_win` (`D:\` ↔ `/mnt/d`, distro validada anti-inyección);
  recetas `wsl_pmkid_capture`, `wsl_airodump`, `wsl_wash`, `wsl_deauth` (fija
  canal con `iw`), `wsl_reaver`, `wsl_kill` — con `sudo -n` (whitelist
  `/etc/sudoers.d/uifipill-lab`) + `timeout -s INT` DENTRO de la VM y staging a
  `%TEMP%`; `wsl_stream_run` (streaming en vivo) y `wsl_health` (veredicto por
  paso: distro→wlan0→monitor→canal→probe RX).
- VirtualHere: `vh_server_check/provision/daemon/hub_add/list/use/stop/rf_check`.
- Bloqueadores MEDIDOS: usbipd RX muerta (0 pkts donde Windows ve 370/15 s),
  VM WSL2 reciclada cada 5–60 min, VirtualHere `USE` → `API Timeout` (licencia).
- UI: tarjetas «WSL2/Kali — puente RF» y «VirtualHere — USB a Kali» + wizard
  paso A («Kit Kali para este objetivo»).

## External tools required (lab machine)
- Npcap (NPcap.dll driver, Dot11Support)
- hashcat (+ `tools/rockyou.txt`); reaver/wash (port Win propio)
- hcxdumptool + hcxpcapngtool, bully, mdk3 (sin port Win → Kali)
- USBPcap 1.5.4.0 (sniffer del vendor para la investigación RX)
- Nota: el sistema de build de reaver (`tools/build/`) se eliminó del repo —
  recompilar con upstream + parche portable fuera de este repo.

## Documentación
- **`memory.md`** — §1 bitácora completa · §2 roadmap WSL2/Kali · §3 RE de
  netr28ux.sys (parche canal, TX cerrada) · §4 investigación RX WinUSB (plan y
  veredictos) · §5 explicación canónica Windows-vs-Linux + mapa funcional.
- `README.md` — overview, requisitos y mapa «qué corre dónde» para el usuario.
