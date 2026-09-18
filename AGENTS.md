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
  Binarios ignorados en git (ver `.gitignore`). Nota: el sistema de build de reaver (stubs,
  scripts y parches) fue eliminado del repo; si hace falta recompilar, usar upstream +
  parche portable fuera de este repo.
- Frontend: AUTO-attack 9 pasos y quick-attack mezclan comandos sanos con comandos sin binario
  (ahora fallan con mensaje claro, no críptico); consola tiene matar-PID; scan tiene restaurar-managed.
- **Pre-vuelo inyección**: `check_injection_capability(iface_guid)` prueba `pcap_sendpacket`
  (CTS-to-self) y devuelve `supported: true/false` + mensaje detallado. Todos los comandos
  WPS nativos (reaver/pixie/pbc) la consultan antes de lanzar; si falla, sugieren
  «wsl (Kali-WSL2)» en la UI como alternativa.
- **WSL2 en WPS UI**: el selector de tool de WPS PIN bruteforce ahora incluye `wsl (Kali-WSL2)`
  que invoca `wsl_reaver` directamente (60s, canal desde scan). Idem disponible en auto-attack.

## Estado verificado 2026-09-15 (revisión completa; commits 7891cbb / 08eea7e)
- `cargo test --lib`: 28 tests → **23 passed / 5 ignored** (los `lab_*` que exigen HW real).
  `cargo build --release` limpio (**0 warnings**, 1m 16s).
- `npm run lint` OK; `npx vite build` OK (`dist/index.html` 84.5 kB + 36.5 kB JS).
- JS inline de `index.html` validado: 1 `<script type=module>` (bridge Tauri) + 1 clásico
  de 246 líneas, ambos compilan sin errores.
- Cableado UI↔backend re-auditado con script (43 invokes literales): **0 invokes sin
  handler** y 0 `onclick` sin función; único id dinámico `monitor-modal-overlay`.
  Handlers registrados sin botón en la UI (superficie muerta, no rota): `monitor_status`
  (interno, lo usa `detect_adapters`), `set_monitor_mode`, `set_managed_mode`,
  `set_monitor_channel`, `set_monitor_freq`, `wash_scan_bg` (la UI usa `wash_scan`),
  `wps_bruteforce_reaver` (la UI usa `_bg`) y `wsl_from_win`/`wsl_to_win` (helpers internos).
- Instalador regenerado: `src-tauri/target/release/bundle/nsis/UIFIPILL_1.0.0_x64-setup.exe`
  (7.119.830 B ≈ 6,8 MB, 2026-09-15 11:14).
- Limpieza de repo: 5 ramas sin commits propios borradas (+ 4 worktrees scratch
  `D:\PROJECTS\UIFIPILL-wt-*` liberados) y el stash antiguo (base divergente `0e07795`,
  con WIP abandonado de mdk4/stream_* que NO está en master) respaldado en
  `D:\PROJECTS\uifipill-stash-backup-2026-09-15.patch` antes de dropearlo.


## Correcciones fakes/stubs — ronda 2026-09-17 (2ª auditoría)
- `pmkid_capture(_bg)` y `capture_handshake(_bg)`: pasaban `-i` SIN valor a hcxdumptool
  (el flag se tragaba el siguiente arg `-t`). Ahora param `iface: Option<String>` real +
  `resolve_iface()` con aviso NPF. La UI ya pasa `pmkid-iface`/`handshake-iface`, que
  existían en el HTML pero nadie leía.
- Falso éxito en crack: `pmkid_crack` y `crack_handshake` daban "PASSWORD CRACKEADA" si
  cualquier línea del stdout de hashcat contenía `:` (los status/headers siempre la
  contienen). Ahora el éxito SOLO se declara si hashcat escribió el `.cracked`.
- `pcap_convert.rs` DLT 105: aplicaba el skip de cabecera radiotap también en DLT 105
  (que NO lleva radiotap) → habría corrompido el parseo de capturas sin radiotap. Solo
  en DLT 127 ahora; en 105 el frame control empieza en offset 0.
- `list_attack_processes`: devolvía `success: true` aunque powershell fallara; ahora
  refleja el exit status real y propaga stderr.
- Nota no-bug: `wps_pin_bruteforce` et al. usan `success: ok || r.success` — reporta el
  éxito del proceso y destaca el PIN parseado aparte; no es fake.
- Verificación: `cargo test --lib` 23 passed / 0 failed (5 ignored), `cargo build
  --release` 0 warnings, `npm run lint` OK, `npx vite build` OK.

## Correcciones UI/honestidad — ronda 2026-09-17 (3 reportes de uso real)
- Icono Escanear roto: `doScan`/`scanAndAutoAttack` restauraban el botón con
  `textContent = '&#x1F50D; …'` (la entidad NO se parsea en textContent y se veía
  el literal). Ahora `innerHTML`. Solo se rompía tras el primer escaneo.
- Cuelgue al cambiar de tab: `run_bin` emite un evento `attack-progress` POR
  LÍNEA de stdout; el frontend hacía `log()` + `liveAppend()` + `scrollTop`
  (layout síncrono) por evento → con salidas verbose el hilo UI se saturaba.
  Ahora scroll agrupado por frame (`queueScroll`, solo si el tab está visible)
  y panel live con buffer + volcado por frame.
- Ataque «falso»: `wps_pin_bruteforce`/`wps_pbc_attack`/`wps_pixiedust`/
  `wps_bruteforce_reaver` devolvían `success: ok || r.success` (exit 0 sin PIN =
  ✅ falso) e `injection_test` `r.success || ok`. Ahora `success` = PIN
  recuperado / «injection is working» en salida. El auto-attack cuenta pasos
  con resultado real y cierra con «SIN RESULTADO» si todo se omitió (antes
  siempre «COMPLETADO»). `doCapture` ya no bloquea 30s con `pmkid_capture`
  si falta hcxdumptool.exe (sin port Win): deriva a captura Npcap nativa o
  avisa sin bloquear.
- Verificación: `cargo test --lib` 23 passed / 0 failed (5 ignored),
  `cargo build` 0 warnings, `npm run lint` OK, `npx vite build` OK (dist/ actual).

## Correcciones rendimiento/layout — ronda 2026-09-18 (reporte de uso real)
Síntoma del usuario: «una vez se lanza el ataque, si vas a Consola ya no vuelves»
(UI congelada) + «la pantalla está mal optimizada, zonas que no se ven».
- Causa raíz 1 (backend): `run_bin`/`run_bin_bg` emitían **un evento
  `attack-progress` por línea/chunk** de stdout (miles/s en reaver/hashcat/wash/
  hcxdumptool) → el handler JS corría en el hilo principal y los clics del
  sidebar dejaban de responder. Ahora `emit_chunked` (trozos de 4 KB, techo de
  32 eventos/salida, resumen de lo omitido) + coalescencia en `run_bin_bg`
  (`flush_buffers` cada 4 KB o 120 ms). Resultado: ~8 eventos/s en vez de miles.
- Causa raíz 2 (frontend): `log()` pintaba en el DOM **en cada evento**, incluso
  con el tab Consola oculto, y actualizaba `#mini-cv` siempre. Ahora: historial
  `_logHist`/`_logPending` + pintado agrupado por frame (`requestAnimationFrame`,
  máx. 200 entradas/frame), **cero DOM si el tab está oculto** y redibujado
  completo (`_renderConsoleTail`) al abrir Consola. Los eventos
  `attack-progress` se encolan y se vuelcan 1 vez/frame (techo 300/frame).
- `showTab` idempotente + delegación de clic en `.sidebar` (respaldo del
  `onclick` inline, a prueba de escapes de Vite): al cambiar de tab refresca
  consola / salida rápida / panel verbose según destino.
- Layout: `.phead` con `flex-wrap` + `.phead-actions` (antes los últimos botones
  del header se salían de la ventana = «zonas que no se ven»), botones
  secundarios `.scan-btn.sm`, sidebar y `.nav-group` con scroll propio, `.app`
  con `100dvh` y `overflow:hidden`, `.console-wrap`/`.tbl-wrap` con `min-height`,
  panel verbose `#live-cv` con clase `.live-view` y altura `clamp(110px,24vh,190px)`,
  **mini-consola plegable** (`toggleMiniConsole`, `#mini-wrap`) y **guía del
  ataque plegable** (`<details class="guide">`, cerrada por defecto) para
  recuperar altura útil, y `@media (max-height:700px)` con paddings compactos.
- Extra: `window.clearLive()` real (el botón «Limpiar» solo vaciaba el DOM y el
  buffer pendiente se volvía a volcar) y `toggleLive()` vuelca lo acumulado.
- Verificación: `cargo build --release` 0 warnings (2m17s), `cargo test --lib`
  23 passed / 0 failed (5 ignored), `npm run lint` OK, `npx vite build` OK
  (dist/index.html 92.25 kB + 48.07 kB JS), inline scripts de `index.html`
  compilados con `node -c`, 0 ids duplicados y 0 handlers huérfanos.
- Nota entorno: el runner de comandos corta a 30 s y espera a todo el árbol de
  procesos → los builds largos se lanzan con `schtasks /create + /run` y se
  sondean con `Get-Content ...log` (borrar la tarea al terminar).

## External tools required (lab machine)
- Npcap (NPcap.dll driver)
- hcxdumptool + hcxpcapngtool (hcxtools)
- hashcat
- bully (optional — requires pixiewps, no confirmed Win native build)
