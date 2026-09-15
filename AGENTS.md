# AGENTS.md — UIFIPILL Project

## Goal
Windows desktop WiFi suite: scan + monitor mode + PMKID capture/convert/crack + WPS PIN bruteforce. Lab-only.

## Stack
- Rust + Tauri v2 (backend; `uifipill.exe` ~14 MB release, instalador NSIS ~7 MB)
- Vanilla JS frontend (`src/app.js`), no React
- Index: `index.html` — built via Vite → `dist/`
- Binarios de ataque Windows nativos: airodump-ng, aireplay-ng, mdk3, airbase-ng, aircrack-ng (todos .exe sin WSL; https://github.com/aircrack-ng/aircrack-ng/releases)

## Key commands (Rust, `src-tauri/src/`)
| file | purpose |
|---|---|
| `lib.rs` | registers all invoke handlers (**81 comandos** registrados) |
| `commands.rs` | scan_wifi, scan_airodump (airodump-ng), pmkid_capture/convert/crack, capture_handshake/crack_handshake, wps_pin_bruteforce, wps_pbc_attack (reaver-wps -S), wps_pixiedust (reaver-wps -K 1), deauth/disassoc/fakeauth/arp_replay/chopchop/cafe-latte/interactive/fragment_inject (aireplay-ng), beacon_flood (mdk3), rogue_ap (airbase-ng), injection_test (aireplay-ng), list/kill/cancel_attack, pmkid_capture_bg/capture_handshake_bg/scan_airodump_bg/wps_pin_bruteforce_bg |
| `monitor_mode.rs` | activate_monitor, restore_managed, set_monitor_channel, monitor_status (NPcap FFI) |
| `wifi_adapter.rs` | detect_adapters, set_monitor_mode, set_managed_mode |
| `tools_detect.rs` | detect_tools, find_tool_cmd, check_tool (hcxdumptool, hashcat, bully…) |
| `capture.rs` | native_capture (captura pasiva vía wpcap.dll + libloading, `\Device\NPF_WIFI_{GUID}`) |
| `pcap_convert.rs` | pcap_to_22000 (parser nativo PMKID/EAPOL, sin hcxpcapngtool) |
| `wsl.rs` | motor dual Kali-WSL2: wsl_exec/attach/detach/from_win/to_win, recetas RF (pmkid, airodump, wash, deauth, reaver, kill) y VirtualHere (vh_* ×8) |
| `crack.rs` / `keygen.rs` / `connect.rs` | estrategia hashcat, keygen Comtrend+Thomson, wifi_connect netsh |
| `profiler.rs` / `wpa3.rs` / `eviltwin.rs` | perfilado de objetivo, auditoría WPA3, kit Evil Twin |

## Build
```bash
npm run lint        # validate JS
npx vite build       # produces dist/
cd src-tauri && cargo build --release   # Rust backend
npx tauri build --bundles nsis          # NSIS installer → target/release/bundle/nsis/
```

## Frontend
- `src/app.js`: tab logic, scan, ~25 attack invocations + auto-attack 9 pasos / quick-attack, tools detection, console logging
- `index.html`: inline `doMonitor()` (modal `detect_adapters` → `activate_monitor`), `doCapture()` (`pmkid_capture` 30s, exige BSSID seleccionado) + resto de tabs (attack/console)

## Motor dual WSL2/Kali — Fases 5/6/6b (bitácora viva en `ROADMAP_WSL2.md`)
- `src-tauri/src/wsl.rs` (nuevo, 20 comandos): puente `wsl_exec` (sin shell intermedia,
  5–600 s), `wsl_attach`/`wsl_detach` (usbipd, busid validado `^[\d-]+$`),
  `wsl_from_win`/`wsl_to_win` (`D:\` ↔ `/mnt/d`, distro validada anti-inyección);
  recetas `wsl_pmkid_capture` (hcxdumptool `--rds=1`), `wsl_airodump`, `wsl_wash`,
  `wsl_deauth` (fija canal con `iw`), `wsl_reaver`, `wsl_kill` — todas con `sudo -n`
  + `timeout -s INT` DENTRO de la VM (el timeout del cliente `wsl` no mata procesos)
  y staging a `%TEMP%`.
- VirtualHere (transporte USB alternativo): `vh_server_check` (TCP 7575 sin WSL),
  `vh_provision`, `vh_daemon`, `vh_hub_add`, `vh_list`, `vh_use` (hint de licencia si
  `API Timeout`), `vh_stop`, `vh_rf_check` (iw + ip up + tcpdump + firmware → VIVA/MUERTA).
- UI: tarjetas «WSL2/Kali — puente RF» y «VirtualHere — USB a Kali» en el tab de ataque.
- Bloqueadores MEDIDOS (no de código): RX muerta en Kali sobre usbipd (0 pkts donde
  Windows captura 370 pkts/15 s), VM WSL2 reciclada cada 5–60 min, VirtualHere USE →
  `API Timeout` con trial (exige licencia de pago). Decisión pendiente en
  `ROADMAP_WSL2.md` §ESTADO ACTUAL.

## Estado real (rev. 2026-09-13, verificado con `cargo test` + `cargo build` sin warnings)
## Lab Npcap — verificado con HW real 2026-09-14 (RT3070 USB, Npcap 1.10.5, sin admin)
- `detect_adapters`: RT3070 (VID 148F:PID 3070, añadido a CHIPS) → capable=true, MAC/GUID/estado reales.
- `monitor_status`/`activate_monitor`/`restore_managed`: ciclo monitor↔managed VERIFICADO por OID
  (incluye fix: el GET exigía `Length` en PACKET_OID_DATA; sin eso todo era `unknown`).
- Límite del driver: `set_monitor_channel`/`set_monitor_freq` NO mueven la radio del RT3070
  (canal se queda en 11; el WlanHelper oficial falla igual con 0xc0010017). El código ahora lo
  VEREDICTO ADMIN 2026-09-14: ni como administrador mueve el canal (CH pedido 6/1,
  radio fija en 11; freq OID rechazado 0xE0010017). Límite del driver RT3070, no de
  permisos. Estrategia: operar en el canal donde ya esté el AP (CH11 aquí) o adaptador
  con driver con Dot11 completo (RTL8812AU/AR9271).
- Vía OID privado Ralink (0xFF0100C8, `lab_vendor_channel`): TAMBIÉN rechazado (0xE0010017).
  Agotadas las 3 vías Windows (canal/freq/vendor). En Linux `rt2800usb` sí cambia (nl80211).
- Compensación implementada: `selectedChannel()` en `app.js` — todos los flujos (PMKID,
  handshake, WPS/PBC/Pixie, auto 5 y 9 pasos, quick) heredan el canal del AP desde el scan.

## Cableado UI↔backend — auditado 2026-09-14 (todo OK tras 3 fixes)
- Tauri v2 convierte args a camelCase por defecto: el frontend ya habla camelCase
  (`durationSeconds`, `ifaceGuid`, `attackId`, `targetBssid`…) — verificado en macro.
- Fixes: `arp_replay_inject` iba en snake (`target_bssid`) → roto, ahora `targetBssid`;
  `thomson_run` sin UI → tarjeta keygen deriva a Thomson con input Máx (1-50);
  beacon/rogue del auto-9 usaban CH1 fijo → ahora `autoCh9 || 1`.
- Todos los `onclick`/`doCmd` existen en `app.js` o inline; todos los `$()` tienen su
  `id=` (solo falso positivo: `monitor-modal-overlay`, creado dinámico).

## Correcciones fakes/stubs — ronda 2026-09-14- `scan_airodump(_bg)` y `beacon_flood` ignoraban la interfaz (`wlan0mon` hardcodeado;
  la UI ya tenía `airodump-iface`/`beacon-iface` sin leer): ahora param `iface` real.
- `resolve_iface()` + aviso NPF en los 12 comandos de inyección/captura: sin interfaz,
  avisan en vez de fallar críptico. `pin_channel_best_effort` avisa igual en los 9 WPS.
- Auto-attack 5/9 pasos: capturas en `_bg` + `currentAttackId` por paso (cancelable con ■),
  `window._autoStop` detiene la secuencia, progress visible y oculto al final.
- `ToolsReport.total` añadido (el frontend ya lo leía con fallback).

## Captura nativa Npcap — HITO 2026-09-14 (cierra el loop en Windows)
- airodump-ng/aireplay-ng Windows NO soportan Npcap (solo AirPcap HW) y Npcap no
  inyecta: la RF en Windows solo puede ser pasiva. Verificado `Adapter not supported`.
- Nuevo `capture::native_capture` (libloading + wpcap.dll, sin binarios): abre
  `\Device\NPF_WIFI_{GUID}` (¡el `NPF_` a secas da Ethernet! — hallazgo `lab_dlt_matrix`),
  exige monitor previo, vuelca .pcap clásico, registra comando + botón verde en la
  tarjeta PMKID (rellena el convertidor con el fichero). Respuestas Tauri en snake_case.
- Prueba real: 392 paquetes 802.11 (270 beacons) en 15s, DLT 127, convertidos a .22000
  (0 PMKID/handshakes: sin clientes activos que generar EAPOL; para PMKID hace falta
  tráfico contra el AP objetivo en su canal).
- Mapa funcional honesto: ✅ scan/monitor/captura/convert/crack/keygen/auditoría/kits;
  ❌ inyección/deauth y cambio de canal en RT3070 (límites driver/Npcap, no código).
- Plan motor dual en `ROADMAP_WSL2.md` (documento vivo con bitácora): UI Windows +
  ejecución RF en Kali-WSL2 vía usbipd. Fase actual y siguiente paso, siempre ahí.
- Inyección MEDIDA (`lab_inject_probe`, CTS-to-self por `pcap_sendpacket` en WIFI_):
  rc=-1, error 31 (ERROR_GEN_FAILURE) con y sin radiotap. Npcap #85 sigue abierto;
  reaver tampoco puede transmitir en Windows+Npcap aunque el port sea correcto.
- Tests: `lab_detect_and_status` (solo lectura, corre siempre) + `lab_monitor_cycle` (#[ignore],
  ciclo completo; `cargo test --lib lab_monitor_cycle -- --ignored --nocapture`). Comandos nuevos:
  `set_monitor_freq` (OID +54, kHz). UI: input CH en scan → `activate_monitor(guid, channel).
- Npcap requerido: WinPcap-compat + Dot11Support (este equipo ya los tiene: Dot11Support=1).
Inventario backend (`src-tauri/src/lib.rs`, ~40 invokes): scan_wifi, pmkid_capture(_bg),
pmkid_convert, pmkid_crack, wps_pin_bruteforce(_bg), wps_pbc_attack(_bg), wps_pixiedust(_bg),
wps_bruteforce_reaver(_bg), wash_scan(_bg), scan_airodump(_bg), capture_handshake(_bg),
crack_handshake, deauth/disassoc/fakeauth/arp_replay/chopchop/cafe-latte/interactive/fragment_inject
(aireplay-ng), beacon_flood (mdk3), rogue_ap (airbase-ng), injection_test, list/kill/cancel_attack,
detect_tools/find_tool_cmd/check_tool, detect_adapters/set_monitor_mode/set_managed_mode,
activate_monitor/restore_managed/set_monitor_channel/monitor_status (NPcap FFI),
pcap_to_22000 (parser Rust nativo), profile_target, inspect_hash/filter_hash/list_crack_assets/crack_custom,
wpa3_audit/gen_rogue_conf, gen_eviltwin_kit/verify_candidate, wifi_connect/wifi_disconnect,
keygen_detect/keygen_run/thomson_run. Módulos: commands, connect, crack, eviltwin, keygen, profiler,
tools_detect, wifi_adapter, monitor_mode, pcap_convert, wpa3.
- ✅ Funcional en Windows (sin RF): scan netsh, profiler, pcap_to_22000, inspect/filter/crack_custom +
  pmkid_crack/crack_handshake (si hay hashcat.exe), wifi_connect/disconnect (netsh), keygen Comtrend
  + Thomson real (RouterKeygen SHA1, threads), gen_rogue_conf / gen_eviltwin_kit / wpa3_audit
  (solo generan/guían, ejecutan en Kali).
- ✅ Servicios similares que SÍ funcionan sin binario externo: `pmkid_convert` y el paso 2 de
  `capture_handshake(_bg)` usan el conversor nativo si falta hcxpcapngtool; todo comando con
  binario ausente falla con hint accionable (preflight en `run_bin`/`run_bin_bg`).
- ⚠️ Requiere HW + binarios ausentes: RF/inyección (hcxdumptool, aireplay/airdump/airbase,
  mdk3, reaver/wash) exige Npcap Dot11 + adaptador compatible (RTL8812AU/AR9271; Intel = sin monitor).
- ❌ Ausente en `tools/` (sin port Win, van en Kali): hcxdumptool.exe, hcxpcapngtool.exe,
  mdk3.exe, bully.exe, pixiewps.exe.
- ✅ Presente en `tools/` (2026-09-13): reaver.exe + wash.exe RECOMPILADOS (iface real,
  `-liphlpapi`, DLLs libpcap/libwinpthread/libcrypto/libssl incluidas — sin ellas 0xC0000135),
  hashcat 7.1.2 oficial (exe+modules/rules/masks/OpenCL), rockyou.txt (140 MB),
  suite aircrack-ng 1.7 Cygwin. `reaver -h` / `wash -h` / `hashcat --version` verificados.
  Binarios ignorados en git (ver `.gitignore`); SÍ se rastrean fuentes, stubs y el parche
  portable `tools/build/patches/reaver-win-port.patch`; los clones anidados quedan ignorados
  (`tools/build/BUILD.md` documenta upstream, HEAD usado y receta de rebuild).
- Frontend: AUTO-attack 9 pasos y quick-attack mezclan comandos sanos con comandos sin binario
  (ahora fallan con mensaje claro, no críptico); consola tiene matar-PID; scan tiene restaurar-managed.

## Stubs/fakes — estado tras la reparación 2026-09-13
1. `tools/build/stubs/` (arpa, linux/*.h, net/*, sys/*, unistd.h…): shims POSIX SOLO de compilación
   para reaver en MinGW64. No son runtime; se quedan (sin ellos no compila). Fuente de reaver sí reparada.
2. ✅ reaver Win `iface.c` REPARADO: `read_iface_mac()` real vía GetAdaptersAddresses (match por
   GUID/nombre, `-liphlpapi` añadido al link MinGW); `next_channel()` no-op deliberado y
   `change_channel()` registra+avisa (Npcap no deja a reaver mover la radio; el canal se fija con
   `-c` + `set_monitor_channel`). Requiere recompilar con `tools/build/build_all.ps1` (MSYS2/MinGW64).
3. ✅ `keygen.rs` Thomson REAL: SHA1("CP"+YY+WW+hex3) con diccionario [A-Z0-9]³ generado (46656) ×
   años 2004-2012 × semanas 01-52 = 21.840.192 hashes con threads + early-exit; verificado con el
   vector público (SpeedTouchF8A3D0 → 742DA831D2, S/N CP0615313039). `thomson_run` REGISTRADO en
   `lib.rs`; `keygen_run` deriva Thomson automáticamente (máx. 5).
4. ✅ `pcap_convert.rs` limpio: eliminados structs `PcapHdr`/`PktHdr` sin uso y `a4()` placeholder;
   `cargo build` sin warnings (las fns frame_type/to_ds/mac_str/hex SÍ se usan en el parser M1/KDE+EAPOL).
5. ✅ `wifi_adapter.rs` REAL: `Get-NetAdapter → CSV` (nombre, descripción, estado, MAC con `:`,
   InterfaceGuid, PnPDeviceID para VID:PID); `monitor_active` vía `monitor_status` (GET OID);
   `AdapterInfo.guid` nuevo (el modal usaba VID:PID como GUID — bug); URL AR9271 corregida.
6. ✅ `monitor_mode.rs` (trabajo previo sin commitear) NO COMPILABA (partial moves en
   set_monitor_channel/restore_managed): reparado cerrando el handle dentro de cada rama.
7. Doc vieja: decía "4 attacks", `wsl2_run/wsl2_info` (no existen), `mdk4` (no existe).
   `crack.rs` menciona "placeholders" solo como sintaxis de máscara hashcat, no es stub.

## Estado verificado 2026-09-15 (revisión completa; commits 7891cbb / 08eea7e)
- `cargo test --lib`: 28 tests → **23 passed / 5 ignored** (los `lab_*` que exigen HW real).
  `cargo build --release` limpio (**0 warnings**, 1m48s).
- `npm run lint` OK; `npx vite build` OK (`dist/index.html` 88.8 kB + 32.4 kB JS).
- JS inline de `index.html` validado: 1 `<script type=module>` (bridge Tauri) + 1 clásico
  de 246 líneas, ambos compilan sin errores.
- Cableado UI↔backend re-auditado con script (43 invokes literales): **0 invokes sin
  handler** y 0 `onclick` sin función; único id dinámico `monitor-modal-overlay`.
  Handlers registrados sin botón en la UI (superficie muerta, no rota): `monitor_status`
  (interno, lo usa `detect_adapters`), `set_monitor_mode`, `set_managed_mode`,
  `set_monitor_channel`, `set_monitor_freq`, `wash_scan_bg` (la UI usa `wash_scan`),
  `wps_bruteforce_reaver` (la UI usa `_bg`) y `wsl_from_win`/`wsl_to_win` (helpers internos).
- Instalador regenerado: `target/release/bundle/nsis/UIFIPILL_1.0.0_x64-setup.exe`
  (7.117.995 B, 2026-09-15 08:35).
- Limpieza de repo: 5 ramas sin commits propios borradas (+ 4 worktrees scratch
  `D:\PROJECTS\UIFIPILL-wt-*` liberados) y el stash antiguo (base divergente `0e07795`,
  con WIP abandonado de mdk4/stream_* que NO está en master) respaldado en
  `D:\PROJECTS\uifipill-stash-backup-2026-09-15.patch` antes de dropearlo.


## External tools required (lab machine)
- Npcap (NPcap.dll driver)
- hcxdumptool + hcxpcapngtool (hcxtools)
- hashcat
- bully (optional — requires pixiewps, no confirmed Win native build)
