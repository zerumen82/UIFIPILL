# memory.md — Memoria del proyecto UIFIPILL

> Fusión (2026-09-29) de: la bitácora de sesiones que vivía en `AGENTS.md` +
> `ROADMAP_WSL2.md` + `DRIVER_RE.md` + `driver_re/usb_tx/RX_PLAN.md` +
> `driver_re/usb_tx/LINUX_EN_WINDOWS.md` (los 4 md originales se eliminaron).
> **Reglas, objetivos y estado vigente → `AGENTS.md`. Historia y detalle → aquí.**
> Referencias `§N` = secciones de ESTE documento.

## ÍNDICE
- §1 — Bitácora de sesiones (2026-09-13 → 2026-11-21, orden cronológico; última entrada: 2026-11-21 cont. 4)
- §2 — Roadmap motor dual WSL2/Kali (origen ROADMAP_WSL2.md)
- §3 — Ingeniería inversa de netr28ux.sys (origen DRIVER_RE.md)
- §4 — Investigación RX por WinUSB (origen RX_PLAN.md)
- §5 — Por qué no funciona igual en Windows (origen LINUX_EN_WINDOWS.md)
- **§6 — TRASPASO / HANDOFF (2026-11-21): estado real, cómo verificar, deudas y trampas — EMPEZAR AQUÍ si relevas al agente**

# PARTE 1 — BITÁCORA DE SESIONES (orden cronológico; origen: AGENTS.md)

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

## Correcciones fakes/stubs — ronda 2026-09-14
- `scan_airodump(_bg)` y `beacon_flood` ignoraban la interfaz (`wlan0mon` hardcodeado;
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
- Plan motor dual en `§2` (documento vivo con bitácora): UI Windows +
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

## Estado actual — rev. 2026-09-18 (fin de sesión, dónde está la app)
Instalado en `%LOCALAPPDATA%\UIFIPILL\uifipill.exe` (md5 `4d02e333b2f3a2545ffa0e0fb59908ce`,
instalador `src-tauri/target/release/bundle/nsis/UIFIPILL_1.0.0_x64-setup.exe` con las
DLLs del bundle incluidas). Master con 12 commits sin push.

### Cableado nuevo desde la última revisión de AGENTS.md
- **Fix crítico instalador**: Tauri NSIS aplana rutas `../tools/*` en `<exe>\_up_\tools\`,
  y `require_bin`/`install_dirs` NO lo miraban → en el instalado ningún binario se
  resolvía (reaver/wash/aircrack «no encontrados») y por eso «los ataques no van».
  Ahora: `_up_/tools` + `_up_/tools/aircrack-ng-win` en `install_dirs()`;
  `require_bin` consulta esas carpetas; `run_bin`/`run_bin_bg` resuelven RUTA
  ABSOLUTA (`resolve_bin_abs`) antes de lanzar (una ruta desnuda no arranca aunque
  exista). **Segundo fix del mismo típo**: el bundle no empaquetaba las DLLs que
  reaver/wash enlazan (`libpcap.dll`, `libwinpthread-1.dll` → 0xC0000135 silencioso,
  el proceso moría al instante sin output). Añadidas las 4 a `resources` en
  `tauri.conf.json`. Verificado: `reaver -h` exit 0 desde `<inst>\_up_\tools`.
- **Lock de UI durante ataque**: barra roja `#attack-status-bar` (punto pulsante +
  label + ■ Detener SIEMPRE activo, clase `.atk-stop`); todos los `.atk-btn` se
  deshabilitan (setAttackRunning). Eventos backend `attack-started/completed/error`
  sincronizan el lock. **Fail-safe**: al entrar al tab Escanear, si
  `list_attack_processes` no ve procesos vivos, el lock se libera solo (antes un
  ataque que moría sin `attack-completed` dejaba el Escanear bloqueado para siempre).
- **Tab Ataque reordenado**: objetivo BSSID + veredicto del perfilador arriba,
  secciones numeradas 1→10 en orden de flujo (Auto → PMKID → Handshake → Crack →
  WPA3 → WPS → Inyección → DoS → Evil Twin → Acceso al final).
- **Verbose**: sin recorte de 80 líneas en invokeAttack (stdout íntegro + stderr
  separado); backend `EMIT_CHUNK_BYTES` 8 KB / `EMIT_MAX_EVENTS` 64.
- **Rediseño WIZARD (último cambio, commit 2ae8ab4)**: las secciones 1–3 (Auto/
  PMKID/Handshake, 8 tarjetas con campos repetidos) se sustituyen por 4 pasos
  A→D (`.wiz-step`, círculo de letra, botón principal, línea de estado
  `#wsl-health-line`, avanzados en `<details class="wiz-more">`): A=atacar con
  Kali (arriba del todo), B=capturar Windows Npcap, C=convertir .22000,
  D=crack hashcat. Los inputs mantienen sus IDs (pmkid-dur, crack-hash,
  auto-wordlist, wsl-busid, etc.) para no romper app.js.
- **Streaming WSL en vivo (`wsl_stream_run`)**: recetas en Kali lanzadas con
  `wsl -d kali-linux -- sudo -n timeout -s INT …` emitiendo
  attack-progress/started/completed chunked (igual que run_bin_bg) y child
  registrado en running_attacks (cancelable). Comandos nuevos:
  `wsl_pmkid_stream`, `wsl_airodump_stream`, `wsl_deauth_stream` (todos
  Result<WslExecResult,String> por la restricción de State en commands Tauri).
  Handlers JS `wslStreamPmkid/Airodump/Deauth` + `invokeWslStream`.
- **wsl_health** (comando + botón «Salud RF»): distro → wlan0 → monitor → canal
  → probe RX 6 s con tcpdump; distingue «sin attach» de «attach con RX muerta»
  y actualiza `#wsl-health-line` con color.

### Límites HARDWARE medidos (no cambian por más código)
- RT3070 en Windows: sin TX/injección (err 31, Npcap #85), sin cambio de canal
  (3 vías OID agotadas incluida vendor 0xFF0100C8). Npcap = solo captura pasiva.
- RT3070 vía usbipd→WSL2/Kali: driver perfecto pero **RX MUERTA** (0 pkts en
  CH11 en 20 s, re-medido 3 veces: 2026-09-14, 09-15 y 09-18). El problema es el
  transporte, no el driver ni el código.
- Driver Windows del RT3070: `netr28ux.sys` MediaTek 2015, cerrado y firmado —
  parchearlo exigiría desactivar Secure Boot/testsigning + certificado EV +
  reversing de 2.2 MB. Descartado explícitamente como opción.
- **Vías reales para RF activa — DECISIÓN FINAL (2026-09-20, rev. 2)**: la
  antena RTL8812AU NO llegará y el driver netr28ux.sys NO se parchea. La app se
  adapta a la antena ACTUAL (RT3070): pipeline pasivo en Windows + «Kit Kali»
  para exprimirla con RF activa. **Implantación**:
  · `index.html`: línea `#hw-cap-line` INFORMATIVA arriba del tab Ataque (sin
  gating: ningún botón se bloquea ni se grisa — decisión del usuario);
  numeración de secciones 1 wizard → 2 crack → 3 WPA3 → 4 WPS → 5 inyección →
  6 DoS → 7 Evil Twin → 8 Acceso.
  · `app.js`: `applyHardwareGates()` ahora SOLO informa (detect_adapters +
  check_injection_capability al abrir el tab); nuevo `genKaliKit()` — botón
  «Kit Kali para este objetivo» en el paso A del wizard: genera bloque de
  comandos Kali live USB (monitor + canal del scan + hcxdumptool + aireplay +
  hcxpcapngtool + hashcat + reaver) con BSSID/SSID/canal del objetivo ya
  sustituidos, en `#kali-kit-out`. La vía RF activa real con esta antena es
  Kali live USB (rt2800usb nativo).
  · Limpieza: retirada la tarjeta «Puente WSL2/Kali» duplicada de la sección Evil
  Twin (duplicaba 5 ids del wizard paso A). Wizard paso A = dueño único.
  · Verificado: `npm run lint` OK, `npx vite build` OK (98.11 kB + 53.63 kB),
  0 ids duplicados, HTML balanceado, onclick↔función cableados.
- **Funcionando HOY en Windows con este hardware**: scan, captura pasiva Npcap
  (392 pkts reales), pcap_to_22000, crack hashcat+rockyou, keygen
  Comtrend/Thomson, wifi_connect. Pipeline completo pasivo end-to-end.

### Deuda abierta conocida
- **RESUELTA 2026-09-22**: el tab Ataque completo (secciones 2–8: crack, WPA3, WPS,
  inyección, DoS, Evil Twin, Acceso) unificado al estilo wizard `.wiz-step` — todas
  las tarjetas `.attack-grid`/`.atk-card` retiradas; campos avanzados en
  `<details class="wiz-more">` (constructor hashcat, candidato ET, VH inputs, WEP
  agrupado en un solo paso con sub-bloques `.wiz-sub`). 24 `.wiz-step` en total,
  0 ids duplicados, 0 onclicks huérfanos, HTML balanceado; `npm run lint` OK,
  `npx vite build` OK (105.35 kB + 53.63 kB JS). Todos los ids y handlers
  conservados (no cambió app.js).
- VirtualHere USE sigue bloqueado por licencia (trial = API Timeout).
- §2 §ESTADO ACTUAL (2026-09-18) tiene la medición completa del
  puente; decisión de camino RF (Kali USB vs antena nueva) sin cerrar.

## Parche driver RT3070 (netr28ux.sys) — rev. 2026-09-20 (§3)
- RE completa del driver MediaTek 5.01.25.0 (objdump, `driver_re/`): el OID
  canal (0x0D010335) está implementado y llama a SwitchChannel (0x14006a7f4,
  rutina RF real con registros RF + BBP + comando USB 0x5D4), pero se
  auto-bloquea con el gate del bit17 (0x20000) de opState (+0x32D468) cuando
  el adaptador está en modo extensible/monitor → doble bloqueo (OID devuelve
  0xC0232002 y la rutina sale sin tocar la radio).
- Parche construido (17 bytes): NOP del test+jne del gate bit17 en
  SwitchChannel (0x14006a8c2, 11 B + 6 B) y je→jmp en el OID canal para
  aceptar SET en ExtSTA (0x1402163df). Verificado con objdump.
- `netr28ux_patched_clean.sys` firmado (Authenticode SHA-256, cert
  `UIFIPILL Lab Test` self-signed, makecert+signtool), checksum PE
  recalculado, firma vieja eliminada.
- `deploy_driver.ps1` (admin: backup → confiar cert → testsigning on →
  copiar .sys con fallback a PendingFileRenameOperations si está bloqueado)
  y `restore_driver.ps1` (rollback completo). Requieren reinicio.
- PENDIENTE (no automatizable sin elevar): ejecutar deploy como admin,
  reiniciar, verificar que la radio cambia de canal en monitor. TX/inyección
  puede seguir bloqueada por Npcap #85 (capa independiente).

## Parche driver RT3070 — VERIFICADO FUNCIONA (2026-09-21)
- Driver parcheado CARGANDO en Windows (testsigning ON, hash `d2c7cf43...` en
  `C:\Windows\System32\drivers\netr28ux.sys`; el redeploy con rutas NT `\\??\`
  resolvió el fallo del primer deploy).
- **La radio SÍ cambia de canal en monitor** (primera vez en el proyecto): probado
  con capturas reales — pidiendo CH1 solo aparecen beacons de APs del canal 1
  (Livebox7, MiFibra…); pidiendo CH6, solo APs del canal 6 (La Fory 3H,
  MOVISTAR_2B4A_EXT…), vía DS Parameter Set de los beacons. El gate del bit17
  parcheado hace que el SET del OID 0x0D010335 se ACEPTA (antes devolvía
  0xC0232002).
- Residual: el GET del OID devuelve un valor CACHEADO (siempre 11) — no es fiable.
  `set_monitor_channel` ahora reintenta el readback y da éxito si el SET fue
  aceptado, con nota de readback no fiable. `set_monitor_freq` sigue rechazando el
  OID frecuencia (0xE0010017): irrelevante, la vía canal funciona.
- TX/inyección: sigue bloqueada por Npcap #85 (capa independiente del driver).
- Verificación: `cargo test --lib` 23 passed / 0 failed (5 ignored),
  `cargo build --release` 0 warnings.

## Fase TX netr28ux — MEDIDA Y CERRADA (2026-09-22, veredicto negativo concluyente)
- Variantes TX-1/TX-2/TX-3 (`driver_re/netr28ux_tx*.sys`) con TODOS los gates
  del send engine neutralizados (validador de cola, BSS-mismatch, 9 kills raw
  en dequeue). Despliegue en caliente verificado (ciclo PnP sin reinicio,
  `hotswap_tx1.ps1`).
- `lab_inject_probe` con TX-3: err 31 persiste; OVERSIZE 6000 B cambia a err 20
  → el paquete SÍ llega al stack del driver, el rechazo es del LWF/path NPC
  (Npcap #85), NO de la cola NDIS ni del driver. Ningún parche adicional de
  netr28ux.sys lo resolverá.
- `lab_assoc_send_test` (envío asociado a BSS abierto) CUELGUE el proceso de
  test en kernel (zombie no matable). Documentado; no repetir sin VM.
- **Línea de RE del driver CERRADA**: canal ganado (hito 2026-09-21), TX
  imposible por software con RT3070+Npcap. Vías restantes: Kali live USB
  (rt2800usb) o antena con driver NDIS6 802.11 nativo.
- Verificación: `cargo test --lib` 23 passed / 0 failed (7 ignored),
  `cargo build --release` 0 warnings, `npm run lint` OK, `npx vite build` OK.

## 🚀 HITO MAYOR — TX POR USB CRUDO RT3070 EN WINDOWS (2026-09-22, verificado con hardware real)
Primer TX activo en Windows de todo el proyecto, SIN Npcap, SIN Kali, SIN usbipd.
Nuevo módulo `driver_re/usb_tx/` (Rust + rusb 0.9, binarios rt3070_probe/diag/init/tx).

Pipeline verificado end-to-end con la antena real (RT3070):
1. **WinUSB rebind**: `rt3070_winusb.inf` (firmado con cert lab vía makecat+signtool,
   `oem180.inf` en store) + `force_winusb_elevated.ps1`/Zadig. `Service: WinUSB` OK.
2. **Sonda** (`rt3070_probe`): abre device, reclama interfaz 0, 7 EP bulk visibles
   (0x81 RX + 0x01–0x06 TX).
3. **Firmware** (`rt3070_init 11 rt2870.bin`): CSR ready → 64/64 chunks a
   FIRMWARE_IMAGE_BASE → MCU UP → radio ON (MAC_SYS_CTRL=0x0C) → canal 11.
4. **TX beacons** (`rt3070_tx "SSID" 11 N`): frames TXINFO+TXWI+802.11 a EP 0x01.
   Primeros frames escritos al chip (backpressure a refinar).

### Bugs clave descubiertos (documentar para no repetir)
- **Tabla de vendor requests INCORRECTA era la causa raíz de todo el bloqueo**:
  en rt2x00usb.h el RT2800/RT3070 usa SINGLE_WRITE=2, SINGLE_READ=3,
  MULTI_WRITE=6, MULTI_READ=7 (la tabla 2/3/4/5 que usábamos era la de RT2500/RT73).
  Con la tabla bien, el chip responde TODO: ASIC 0x30700201, registros, firmware.
- `USB_DEVICE_MODE(reset)` ANTES de leer registros DEJA EL CHIP SORDO (CPU parada
  esperando firmware) — nunca hacerlo en probe; solo rt2800usb lo usa en watchdog.
- Windows/WinUSB: `set_active_configuration(1)` explícito (Linux lo hace solo).
- Deshabilitar device en PnP NO libera el driver para libusb: hace falta rebind
  real a WinUSB (Zadig o UpdateDriverForPlugAndPlayDevices; pnputil add-driver solo
  acepta INF firmado con .cat).
- El chip se degrada (STALL→timeout total) tras intentos fallidos: cada cambio de
  codificación exige power-cycle (desenchufar 15 s).

### Scripts (todos en `driver_re/usb_tx/`)
- `run_zadig.cmd` + `zadig_helper.ps1`: rebind manual WinUSB con guía.
- `force_winusb_elevated.ps1` / `run_force_winusb.cmd`: rebind SetupAPI (fallback).
- `restore_netr28ux.ps1`: REVERSIÓN completa (elimina paquete WinUSB del store,
  reinstala netr28ux desde DriverStore, restaura INF inbox si fue ocultado).
- `rt3070_winusb.inf` + `rt3070_winusb.cat` (firmado lab) — paquete en store oem180.
- `winusb_install_log.txt` / `force_log.txt` / `rebind_log.txt`: bitácoras.

⚠️ Con WinUSB activo la antena NO es WiFi para netsh/Windows (sin RF nativa).
Revertir con `restore_netr28ux.ps1` + re-enchufar cuando se necesite modo normal.
Siguientes pasos TX: refinar TXINFO/TXWI (beacons reales visibles en otra radio),
RX por EP 0x81 (captura por USB crudo), y puente de la app (uifipill) a estos binarios.

## HITO — TX REFINADO: TXWI/TXINFO REALES + FEEDBACK TX_STA_FIFO (2026-09-23, código verificado; pendiente prueba con hardware)
Auditoría del TX del 2026-09-22 contra el driver Linux REAL (torvalds/linux 6.6:
rt2800.h, rt2800usb.h/c, rt2800lib.c — descargados y grepeados, NO de memoria).
Hallazgos y correcciones (todo en `driver_re/usb_tx/src/`):
- **TXWI estaba MAL construido**: la longitud del frame se ponía en w0 bits16-27
  (que son MCS+BW) y w1 iba a 0. REAL: w0 = MCS(bits16-22) | PHYMODE(bits30-31,
  CCK=1); w1 = ACK(bit0) | WCID(bits8-15) | MPDU_TOTAL_BYTE_COUNT(bits16-27) |
  PACKETID(bits28-31, ≠0 → feedback). Ahora `txwi_bytes()` en common.rs.
- **TXINFO activaba bits equivocados**: `| (1<<30) | (2<<26)` = NEXT_VALID y
  SW_USE_LAST_ROUND en vez de WIV(bit24) y QSEL=2(bits25-26); y restaba 4 al
  PKT_LEN cuando el campo es TXWI+802.11 sin TXINFO. Corregido en tx.rs/deauth.rs.
- **MAC_ADDR/BSSID mal direcciones**: ADDR_DW0=0x1008 (no 0x1004), ADDR_DW1=0x100C,
  BSSID_DW0=0x1010, BSSID_DW1=0x1014 (0x1004 es MAC_SYS_CTRL, ya correcto).
- **USB_DMA_CFG es 0x02a0, no 0x0250** (0x0250 es TX_BASE_PTR2 del bus PCI).
- **FEEDBACK TX REAL (TX_STA_FIFO 0x1718)**: con PACKETID≠0 el chip deja una
  entrada VALID|TX_SUCCESS|MCS|PHYMODE por frame procesado. `drain_tx_status()`
  en common.rs; tx.rs y deauth.rs imprimen líneas `[status] TXDONE success=…`;
  usb_raw.rs las resume en el message («TXDONE: N confirmaciones (TX al aire OK)»).
  Esto resuelve el «backpressure a refinar»: ahora SABEMOS si el chip TX-eó.
- **Canal REAL portado** (`config_channel_rt3070` en common.rs): la vía rf53xx que
  usa RF3070 — RFCSR8=N/RFCSR9=K/RFCSR11.R de la tabla rf_vals_3x[] del driver
  (ej. CH11 = N=246,K=2,R=2), RFCSR1 (bloques RF/PLL), RFCSR30 (20MHz),
  RFCSR3.VCOCAL_EN, BBP 62/63/64/82/75/86, TX_BAND_CFG=BG. El código anterior
  escribía RF_CSR_CFG sin el bit WRITE (bit16) → **el chip ignoraba el canal**.
- **PA nunca se encendieron** (`enable_tx_pa`): TX_PIN_CFG (0x1328) con
  PA_PE_G0_EN|LNA_PE|RFTR|TRSW — sin esto el chip modula pero no sale al aire.
- Verificación: `cargo build --release` en usb_tx **0 warnings**; src-tauri
  `cargo build --release` limpio y `cargo test --lib` 23 passed / 0 failed.
- **Siguiente paso con HW**: power-cycle + WinUSB rebind → `rt3070_init 11` →
  `rt3070_tx "SSID" 11 20` y mirar las líneas TXDONE; si success=true, buscar el
  SSID en la OTRA radio (netsh). Ajustar MCS/PHYMODO o potencia (RFCSR49) si no.

### Refinado TX + SCAN por USB crudo (2026-09-23, build OK, pendiente hardware)
- TXWI corregido con layouts REALES de rt2800.h (MCS/PHYMODE en W0, len en W1
  MPDU_TOTAL_BYTE_COUNT, PACKETID≠0 → feedback TX_STA_FIFO). TXINFO: WIV sin
  invertir, packet len = TXWI+frame. CCK 1Mbps. Hallazgos del refinado:
  el RF write de init no ponía el bit WRITE (canal nunca se programó de verdad)
  y USB_DMA_CFG estaba en 0x0250 (dirección PCI) en vez de 0x02a0 (USB).
- `config_channel_rt3070` + `enable_tx_pa` (TX_PIN_CFG PA_PE_G0|LNA|RFTR|TRSW)
  portados de rt2800_config_channel_rf53xx (RFCSR8=N/9=K/11.R + VCOCAL + BBP).
- **SCAN como Linux**: nuevo bin `rt3070_scan` — hopping 1-13 (dwell 250ms),
  parseo de beacons/probe-resp (SSID IE0, canal IE3, BSSID, RSSI RXWI_W2),
  salida `AP|ssid|bssid|ch|rssi%`. Comando `usb_raw_scan` (JSON) + botón
  «Escanear redes (USB crudo)» en el paso TX: fusiona con lastNets y refresca
  tabla+selector — elegir objetivo = canal auto para los ataques USB.
  `init_radio()` en common.rs compartida por init/scan (firmware+radio+canal).
- Verificación: usb_tx 0 warnings; src-tauri 0 warnings; cargo test --lib
  23 passed / 0 failed (7 ignored); lint+vite OK.

### TEST HARDWARE REAL 2026-09-23 — RX aún bloqueado por BBP mudo (documentado)
Sesión de prueba del scan con la antena real. Resultado medido, no adivinado:
- **USB_DMA_CFG con AGG_LIMIT mal COLGABA EL USB** (desconexión del bus en
  segundos, device phantom repetido): el valor antiguo ponía RX_BULK_AGG_EN=1
  con 0x9C en el campo bajo (mezclaba AGG_TIMEOUT con bits de AGG_LIMIT).
  Sin AGG_LIMIT el chip SOBREVIVE al scan/init (probado) pero no entrega URBs
  de RX (0 frames). **Fix final aplicado**: valor EXACTO de
  `rt2800usb_enable_radio` — AGG_EN=0, TIMEOUT=128,
  AGG_LIMIT=(128*2432/1024)-3=**301**, RX|TX_BULK_EN.
- **Constantes del mailbox H2M ERRÓNEAS (crítico)**: H2M_MAILBOX_CSR real es
  **0x7010** (no 0x070C), H2M_MAILBOX_CID **0x7014** (no 0x0704), y
  H2M_BBP_AGENT **0x7028** — nuestro código lo escribía en 0x0800
  (=FIRMWARE_IMAGE_BASE, ¡pisando el firmware cargado!). MCU_CMD va vía
  HOST_CMD_CSR (0x0404) con argumentos en los mailbox, no por 0x0704.
- **BBP_RW_MODE (bit 16=0x80000... concretamente 1<<19=0x00080000)** faltaba en
  TODOS los accesos BBP (rt2800_bbp_read/write lo ponen siempre). Añadido.
- **El BBP responde 0x00 SIEMPRE** (write+readback no pega) aunque:
  ASIC vivo (MAC_CSR0=0x30700201, rev F), firmware 64/64, MCU UP,
  MAC_STATUS_CFG (0x1200) reporta BBP despierto (bits=0), PBF READY=1.
  Sin BBP no hay demodulación → 0 frames RX con chip sano.
- **Sospecha principal: EEPROM vacía**. La lectura EEPROM (vendor MULTI_READ,
  protocolo verificado contra rt2x00usb) devuelve 0xFFFF en toda la zona de
  config (EEPROM_NIC_CONF0=0xffff). Antena sin EEPROM válida o efuse: el
  driver usa la EEPROM para RF type, TXMIXER gain, LNA… la secuencia de init
  del vendor puede quedar coja sin datos válidos.
- **Diagnóstico**: bin `rt3070_bbpdiag` (efuse present + volcado 64 words +
  comparativa EEPROM vendor + test BBP write/readback + RFCSR readback).
  Repetir con power-cycle ANTES de más iteraciones (el chip se degrada:
  STALL → desconexión de bus).
- **PORTADO 2026-09-24 (sesión «SIGUE»)**: los 3 pasos del plan de desbloqueo
  ya están en código — `init_bbp_rt3070` (wait_bbp_ready + rt2800_init_bbp_30xx
  17 regs, BBP103=0xc0 rev F), `init_rfcsr_rt3070` (rt2800_init_rfcsr_30xx rev F
  19 regs + rx_filter_calibration REAL con loopback BBP/tono, NO LDO/31=0x14
  que son RT3071/3090) y `init_registers_rt3070` (init_registers completo con el
  SEGUNDO reset MAC+BBP DESPUÉS del firmware). Todo integrado en `init_radio`
  (orden real de rt2800_enable_radio: init_registers → wait_bbp_rf_ready →
  BOOT_SIGNAL → wait_bbp_ready → init_bbp → init_rfcsr → MCU_CURRENT →
  MAC enable TX+RX → canal+PA). Añadido también acceso EFUSE (port
  rt2800_efuse_read, ADDRESS_IN bits17-25, lectura end-to-start DATA3→DATA0)
  para verificar la sospecha de EEPROM vacía. **Pendiente con HW**:
  power-cycle → `rt3070_bbpdiag` (si efuse present=false y eeprom=0xffff, la
  antena no tiene datos calibración → init vendor cojo confirmado) → si BBP
  despierta, `rt3070_scan` debería ver redes; si sigue mudo tras 2 power-cycles,
  el BBP/EEPROM de ESTA antena está dañado → probar otra RT3070.
- **Estado de build (2026-09-24)**: usb_tx `cargo build --release` 0 warnings
  (fix: `?` sobre rusb::Error en init_registers_rt3070 → unwrap_or); src-tauri
  build release limpio + `cargo test --lib` 23 passed / 0 failed (7 ignored);
  `npm run lint` OK, `npx vite build` OK (dist 109 kB + 61.3 kB JS).
  Mensaje de `usb_raw_scan` actualizado (ya no dice "pendiente init_rfcsr").
- `diag_usb.ps1`: diagnóstico PnP del device (estado, eventos Kernel-PnP).

## TEST HARDWARE REAL 2026-09-24 (sesión «SIGUE») — BBP mudo: diagnóstico profundo

Power-cycle + re-diagnóstico sistemático. Binarios nuevos: `rt3070_bootdiag`
(init paso a paso con verificación de consumo MCU) y `rt3070_bbpexp`
(experimentos quirúrgicos de read/write paths).

### Qué responde y qué no (MEDIDO, firmware cargado + BOOT_SIGNAL)
- ✅ MAC registers: write+readback perfecto (MAC_ADDR_DW0=0x11223344 pega).
- ✅ Mailbox H2M (0x7010): write+readback perfecto.
- ✅ RFCSR: lecturas plausibles de fábrica (r0=0x42, r4=0x33, r7=0x60…) y
  WRITE+READBACK funciona (rfcsr[2]=0x80 pega) — el bloque RF está vivo.
- ✅ Efuse: PRESENT=true, map legible (word0=0x3070, MAC C0:00:59:CA:B5:F8,
  version 0x0101). La sospecha «EEPROM vacía» se matiza: NO está vacía,
  está PARTIALMENTE programada (ver abajo).
- ✅ MCU: con el fix del OWNER (bug encontrado hoy), BOOT_SIGNAL #1 y #2 se
  CONSUMEN (OWNER vuelve a 0) — el firmware corre y atiende comandos.
- ❌ BBP: lee 0x00 SIEMPRE: con RW_MODE=0 y 1, antes/después de firmware,
  con/without init. BUSY se limpia (bit17=0) pero VALUE=0. El «write pega»
  de bbp[4] anterior era falso positivo (escribir 0 y leer 0).

### Bugs corregidos esta sesión (código)
- **MCU OWNER mal formado**: escribíamos 0x8000_0000 (OWNER=0x80); el driver
  usa FIELD32(0xff000000)=1 → **0x0100_0000**. Con el bug el MCU ignoraba los
  comandos (probablemente también los TXDONE previos venían del auto-drain).
  `mcu_request_wait()` nuevo verifica consumo real (OWNER→0, timeout).
- **AUTOWAKEUP_CFG (0x1208)=0 antes del firmware** — faltaba (rt2800_load_firmware).
- Fix build: `?` sobre rusb::Error en init_registers_rt3070 → unwrap_or.

### Verificaciones del port (contra driver Linux 6.6 descargado, NO de memoria)
- init_bbp_30xx, init_rfcsr_30xx (tabla 19 regs), rx_filter_calibration
  (loopback BBP + tonos, filter_target 0x16/0x19), normal_mode_setup_3xxx
  (RFCSR17 R=bit5 0x20, RFCSR27), mcu_request (OWNER=1, wait OWNER=0),
  efuse_read (ADDRESS_IN bits17-25, DATA3→DATA0 end-to-start),
  enable_radio (orden completo) — TODO coincide con rt2800lib.c. La
  secuencia NO es el problema.
- El firmware/MCU NO toca el RF al boot: RFCSR idéntico antes y después de
  firmware+BOOT_SIGNAL.

### Dato clave: EFUSE parcialmente programado (map medido)
- words 0-4 OK: chip ID 0x3070, version 0x0101, MAC C0:00:59:CA:B5:F8.
- **words 5-7 (NIC_CONF0, NIC_CONF1, FREQ) = 0x0000** → RF_TYPE=0 (RF2820,
  inexistente en este chip), RXPATH=0, TXPATH=0 (¡cero antenas!).
- words 8-15 = 0xffff; zona LNA/RSSI/txpower (0x30-0x5e) CON datos
  (0511, 00a6, 0808, 0708…); 0x60+ firma/hash (c5ff a5b5 627a 3a50).
- Un driver Linux ante esto: NIC_CONF0=0 ≠ 0xffff → NO defaulta (solo
  defaulta ante 0xffff) y sigue con RXPATH/TXPATH=0. Puede ser legal para
  el probe del driver, pero el firmware del MCU podría leer este mapa al
  boot y dejar el BBP sin habilitar.

### Estado y siguientes pasos (rev. 2, pendiente prueba)
1. TX ya verificado (TXDONE success=true) — probar `rt3070_tx` y comprobar
   en la otra radio si los beacons salen AL AIRE aunque el RX BBP esté mudo.
2. Escribir efuse words 5-7 con NIC_CONF0 válido (RF_TYPE=RF3070=0x9 →
   word=0x0901? verificar layout RXPATH/TXPATH/RF_TYPE en rt2800.h:2681) vía
   EFUSE_CTRL MODE=1. ALTO RIESGO OTP-like: documentar y solo si se acepta
   perder la antena.
3. Si no: aceptar límite RX de ESTA antena (TX sí, RX no) y antena B para RX.

## SESIÓN «RX MISMA ANTENA» 2026-09-24/25 — HOT-REBIND + BUG USB_DEVICE_MODE

### Hot-rebind netr28ux ↔ WinUSB SIN power-cycle (FUNCIONA)
- `hot_rebind.ps1` + `run_hot_rebind.cmd` (auto-elevado): cambia el driver con
  UpdateDriverForPlugAndPlayDevices (newdev.dll) SIN disable del device →
  el chip NO se resetea y conserva el estado. MEDIDO: tras el cambio, el chip
  conservaba EXACTAMENTE el estado del vendor (USB_DMA_CFG=0x00c12d80 = valor
  del vendor, TX_PIN_CFG=0xf0347 PAs on, MAC_SYS_CTRL=0x20408 RX on).
- Reversión a netr28ux: el INF está en
  `C:\Windows\System32\DriverStore\FileRepository\netr28ux.inf_amd64_2613a90929adebda`
  (el inbox C:\Windows\INF\netr28ux.inf sigue ocultado por rebind_winusb.ps1).
  El script ya busca en el DriverStore. UAC pendiente de confirmar en pantalla
  al ejecutar (run_hot_rebind.cmd auto-eleva).
- **Estado conservado ≠ BBP vivo**: con el estado heredado del vendor, el BBP
  siguió leyendo 0x00 y MCU_WAKEUP (0x31, tok 0xff, arg 0,2) se consumió sin
  despertarlo → 0 frames. El vendor despierta el BBP por otra puerta.
- Nuevo bin `rt3070_hotread`: abre sin reset/firmware, vuelca estado vivo
  (MAC/BBP/RF/mailbox), opcional --wake (MCU_WAKEUP), --chan N (canal RF) y
  lee EP 0x81 con el parser de scan. Con el BBP mudo: 0 frames (correcto).

### BUG CRÍTICO ENCONTRADO: USB_DEVICE_MODE mal mandado desde el inicio
Verificación fina de rt2x00usb.h/rt2800lib.c (2026-09-24, NO de memoria):
- **El modo va en wValue, NO en wIndex**: vendor_request_sw(USB_DEVICE_MODE,
  offset=0, value=MODE). Nosotros: value=0, index=mode → NUNCA llegó el modo.
- **USB_MODE_FIRMWARE = 8, NO 2**: el enum real es RESET=1, UNPLUG=2,
  FIRMWARE=8, AUTORUN=17. Nuestro «FIRMWARE=2» era UNPLUG.
- Con el fix, el RESET correcto (wValue=1) esta vez SÍ pegó y dejó el CPU
  parado esperando firmware (comportamiento REAL del reset, ya documentado
  2026-09-22: «USB_DEVICE_MODE reset DEJA EL CHIP SORDO»). Antes el reset
  no hacía nada porque iba mal mandado — por eso el chip «respondía siempre».
- **El BBP lo habilita el firmware del MCU al arrancar** (rt2x00: «BBP was
  enabled after firmware was loaded, but we need to reactivate it now»). Si
  el MCU nunca arrancó (FIRMWARE=2=UNPLUG + wValue=0), el BBP nunca
  despertó: bbp0=0x00 para siempre. ESTE es el candidato raíz del BBP mudo.
- FIXES APLICADOS en common.rs (+bbpexp/bootdiag): USB_MODE_FIRMWARE=8,
  USB_MODE_UNPLUG=2, y todas las write_control/read_control de DEVICE_MODE
  con (wValue=mode, wIndex=0).

### ESTADO ACTUAL (rev. 3, chip degradado tras el fix — pendiente power-cycle)
- Tras aplicar el fix y ejecutar init: el reset correcto dejó el chip sordo
  (control pipe timeout total, probe/diag/hotread no responden; PnP Status OK
  en WinUSB oem395). Estado de degradación conocido; NO es daño nuevo.
- **Siguiente paso inmediato**: power-cycle 15 s y al re-enumerar ejecutar:
  1. `rt3070_probe` (verificar vivo)
  2. init SIN reset inicial — firmware PRIMERO (Linux: probe no manda reset;
     el reset va en init_registers DESPUÉS de cargar firmware). Secuencia a
     probar: AUTORUN check → escribir fw → CID/STATUS=~0 → DEVICE_MODE
     FIRMWARE(8) → wait CSR → BOOT_SIGNAL → enable_radio completo.
  3. Si el MCU arranca de verdad (BOOT_SIGNAL consumido + PBF ready), el BBP
     debería despertar → `rt3070_scan` para RX completo EN LA MISMA ANTENA.
- Si el BBP despierta, revertir también los cambios de orden si hiciera
  falta y correr scan/deauth/tx de usb_tx ya con RX real.
- Archivos tocados esta ronda: common.rs (fixes), hotread.rs, bootdiag.rs,
  bbpexp.rs (diag), hot_rebind.ps1, run_hot_rebind.cmd. Sin commitear.

## SESIÓN 2026-09-25 — KICK FIRMWARE: CAUSA RAÍZ ENCONTRADA (MCU del vendor VIVO) + fwverify/fwreenum/mcutest/vendorradio
Diagnóstico sistemático del «kick FIRMWARE(8) deja el chip sordo». Binarios nuevos en
`driver_re/usb_tx/src/`: `fwverify.rs` (write+readback del fw SIN kick), `fwreenum.rs`
(kick + re-enum + discriminador ep0/vendor), `mcutest.rs` (¿el MCU ejecuta código?),
`vendorradio.rs` (radio sobre MCU vendor sin carga de fw). `linuxfull.rs` reescrito
(autorun_detect con valor crudo, re-enum REAL que exige desaparición del device,
port reset automático al reaparecer, modo --resume).

### ME DIDO (todos los datos de esta sesión, chip RT3070 real)
1. **Tras power-cycle el MCU 8051 ESTÁ CORRIENDO**: `MCU_CURRENT` (0x30) se
   CONSUME ×3, mailbox STATUS responde `0xf00f1587`, PBF READY=1. El firmware del
   vendor (netr28ux, tras hot-rebind a WinUSB) deja el MCU ejecutando.
2. **autorun_detect = 0x00000000** (modo NORMAL) — NO autorun. PERO esto NO
   significa «hay que cargar fw»: es el valor que da el chip con el fw del vendor
   ya arrancado por otra puerta (el driver vendor no usa DEVICE_MODE).
3. **MAC_CSR0=0x30700201 NO es «firmware no cargado»**: es el ID de ASIC, responde
   en frío Y con MCU vivo. Linux lo lee en probe ANTES de load_firmware.
4. **Cargar fw 64/64 chunks SÍ entra** (ACKs), pero el readback de la zona
   0x0800-0x17FF devuelve TODO 0x00 (fwverify): la zona no es legible por
   MULTI_READ (o no pega — indistinguible desde fuera; MAC normal 0x1008 sí pega).
5. **El kick FIRMWARE(8) sobre MCU VIVO MATA el chip de 2 formas**:
   - Modo A (2×): el device DESAPARECE del bus y re-enumeración REAL (nueva addr,
     019→020→022→023) pero vuelve SORDO (control pipe timeout, ni con port reset).
   - Modo B (2×): el device NO re-enumera y queda sordo in-situ.
   En ambos: solo power-cycle físico recupera. REPRODUCIBLE 4/4.
6. **Radio sobre el MCU vivo del vendor SIN kick** (vendorradio): el chip
   SOBREVIVE a toda la secuencia (WAKEUP, init_registers, BOOT_SIGNAL), pero el
   BBP sigue leyendo 0x00 — mudo en TODOS los estados probados hasta la fecha.

### CONCLUSIÓN (rev. 4 del bloqueo RX)
- El «kick sordo» NO era un bug de nuestro protocolo USB: es que **reiniciar el
  MCU cuando ya corre fw del vendor no tiene sentido** y el chip muere al
  re-arrancar sin las condiciones del boot frío (Linux nunca lo hace: carga fw
  solo desde probe con el chip recién enumerado, y Windows ya nos da el chip
  «calentado» por el driver vendor).
- El BBP mudo persiste incluso con MCU vivo + WAKEUP + init completo → la
  hipótesis **efuse parcial (words 5-7 = 0x0000, RXPATH=0/TXPATH=0)** gana
  fuerza: esta antena puede tener el BBP sin habilitar de fábrica o dañado.
- Próximos pasos: (a) otra RT3070 distinta para comparar (la vía más barata);
  (b) si se acepta el riesgo OTP: escribir efuse words 5-7 (RF_TYPE=RF3070=0x9,
  RXPATH/TXPATH=1) vía EFUSE_CTRL MODE=1 — ALTO RIESGO, documentar antes;
  (c) TX ya verificado (TXDONE success=true) — probar beacons al aire con
  `rt3070_tx` aunque el RX esté mudo.
- Lección de método: **leer el estado del MCU con MCU_CURRENT antes de decidir
  si cargar firmware** (lo que hace mcutest). Nunca dar el kick sobre MCU vivo.

### COMPARATIVA ANTENA VIEJA (…B5:F8) vs NUEVA (…D8:A7) — 2026-09-25, MISMA TARDE
La antena nueva (identica, RT3070) llegó y se probó con la batería completa:
- **efuse NUEVA**: mismo patrón que la vieja — word0=0x3070, MAC C0:00:59:CA:D8:A7
  (vieja: …B5:F8), version 0x0101, **words 5-7 (NIC_CONF0/1, FREQ) = 0x0000**,
  zona 0x10-0x2e = 0xffff, zona LNA/txpower con datos (0511, 00a6…), firma
  0x6e-0x74 (c5ff a5b5 627a 3a50). **El efuse parcial NO es defecto de UNA
  antena: así salen estas RT3070 de fábrica** (o ambas comparten el mismo
  defecto de lote).
- **BBP NUEVA**: mudo IGUAL que la vieja — write/readback no pega (bbp[1]=0x11
  leído 0x00), rfcsr vivo (r0=0x42, r3=0x33, r7=0x60). MAILBOX arranca en
  0xb555584f (OWNER=181) — distinto del estado tras vendor (0xf00f1587).
- **Degradación en vivo**: durante mcutest el chip NUEVA dejó de responder a
  mitad del test (0xffffffff en mailbox/PBF tras MCU_CURRENT #2) — se degrada
  más rápido que la vieja. Solo power-cycle.
- **Consecuencia**: el BBP mudo NO es avería de la antena vieja. O ambas están
  tocadas igual, o (más probable) falta la pieza que despierta el BBP en el
  boot frío — que solo ocurre en el boot real del chip (reset USB completo,
  no power-cycle del puerto). La vía Kali/rt2800usb sigue siendo la única
  probada para RX.
- Prueba limpia pendiente tras power-cycle: mcutest + vendorradio en la NUEVA
  con chip fresco (sin nada más antes), para descartar estado heredado raro.
- **RESULTADO prueba limpia NUEVA (rev. final 2026-09-25)**:
  · mcutest fresco: MCU_CURRENT ×3 CONSUMIDO ✅ (MCU vivo, igual que vieja)
    pero STATUS=0x00ff0000 (vieja: 0xf00f1587) y **BOOT_SIGNAL timeout** —
    primera diferencia real entre chips.
  · vendorradio directo (sin mcutest antes): el chip **DESAPARECIÓ DEL BUS
    POR COMPLETO** durante el init (ni phantom en PnP) — la NUEVA se degrada
    más rápido y de forma más violenta que la vieja. No vuelve sola.
  · **VEREDICTO COMPARATIVA**: la nueva NO despierta el BBP (mudo igual) y
    además muere más fácil. El efuse parcial idéntico en ambas confirma que
    **así salen estas RT3070 de fábrica** (efuse parcial = normal en este
    hardware barato), no es avería de la vieja.
- **CONCLUSIÓN FINAL DE LA VÍA USB CRUDO EN WINDOWS**: con WinUSB no podemos
  reproducir el boot frío (el driver vendor siempre llega primero y deja un
  estado del que no podemos partir para el BBP). El BBP mudo en DOS antenas
  distintas + TX verificado (TXDONE success=true) deja el mapa: **TX posible,
  RX bloqueada por hardware/boot, no por código**. La vía probada para RX
  sigue siendo Kali/rt2800usb. Siguientes pasos razonables:
  (a) cerrar la vía TX al aire (rt3070_tx + verificar beacons desde otra radio);
  (b) Kali live USB para RX (rt2800usb nativo hace el boot frío completo);
  (c) NO comprar más RT3070 para RX por WinUSB.

## SESIÓN 2026-09-25 (cont.) — ANTENA NUEVA + COLD BOOT UNPLUG: re-enum frío FUNCIONA, kick sigue matando

### Contexto
User trajo UNA SEGUNDA RT3070 idéntica (…D8:A7) para comparativa. Luego preguntó
«¿es que tu driver no lo has copiado de Linux?» → la respuesta motivó el último
experimento: el UNPLUG del watchdog rt2x00 para replicar el «chip fresco» de
Linux sin power-cycle.

### Comparativa antena VIEJA (…B5:F8) vs NUEVA (…D8:A7) — MEDIDO
- efuse NUEVA: patrón idéntico a la vieja (word0=0x3070, MAC C0:00:59:CA:D8:A7,
  words 5-7 NIC_CONF=0x0000, zona LNA/txpower con datos, misma firma 0x6e-0x74).
  → **El efuse parcial es DE FÁBRICA en estas RT3070, no avería de la vieja.**
- BBP NUEVA: mudo igual (write 0x11 lee 0x00). RFCSR vivo (r0=0x42).
- mcutest NUEVA fresca: MCU_CURRENT ×3 consumido ✅ pero mailbox STATUS
  0x00ff0000 (vieja: 0xf00f1587) y **BOOT_SIGNAL timeout** — diferencia real.
- vendorradio NUEVA: el chip **DESAPARECIÓ DEL BUS COMPLETO** durante el init
  (peor que la vieja). No vuelve sola. La NUEVA se degrada más rápido.
- Veredicto: misma conducta base (MCU vivo + BBP mudo) pero la NUEVA es más
  frágil. El BBP mudo NO es defecto de una antena: es del boot.

### COLD BOOT vía USB_MODE_UNPLUG(2) — nuevo bin `rt3070_coldboot` (coldboot.rs)
Idea: el watchdog rt2x00 usa DEVICE_MODE OUT UNPLUG para forzar desconexión →
re-enum en frío → replicar el punto de partida de Linux (chip fresco) sin
power-cycle físico. Resultados:
1. **UNPLUG FUNCIONA**: el chip SE DESCONECTA y re-enumeración REAL con nueva
   addr (033→034→035) — PRIMERA VEZ que conseguimos boot frío sin cable.
   (err Pipe al enviar es normal: el chip corta el pipe al instante.)
2. **Tras UNPLUG el chip vuelve con firmware autoload de fábrica ya arrancado**
   (MAC_CSR0 vivo, mailbox OWNER=181 = estado autoload, igual que hotread
   heredado). NO queda «virgen»: el chip tiene boot ROM/autoload propio.
3. autorun_detect = 0x00000000 (NORMAL) también en frío → cargamos fw 64/64 ✅
4. **Kick FIRMWARE(8) sobre el autoload: MATA el chip igual** (CSR no ready,
   device sordo en su addr). REPRODUCIDO TAMBIÉN EN FRÍO — no era el estado
   del vendor: **el kick en sí es lo que el chip no tolera bajo WinUSB**.
5. Bug corregido en coldboot v2: addr_before se captura ANTES del UNPLUG
   (v1 lo capturaba después y la re-enum era invisible al bucle).

### CONCLUSIÓN MAESTRA (rev. 5, cierra la cuestión «copiado de Linux»)
- El port del protocolo ES fiel a Linux (verificado línea a línea contra
  rt2x00usb.h/rt2800usb.c/rt2800lib.c de 6.6). La diferencia no es el driver:
  **es que el kick FIRMWARE(8) deja sordo el chip bajo WinUSB incluso desde
  boot frío replicado**, mientras que en Linux (misma secuencia byte a byte)
  funciona. Causa probable: el kernel Linux re-sondea/re-vincula y gestiona la
  re-enum resultante; WinUSB/Windows no nos deja un device utilizable tras el
  corte del MCU (re-enum con device sordo o muerte total).
- Vías agotadas en Windows: kick tras power-cycle (modos A/B), kick en frío vía
  UNPLUG, radio sobre MCU vendor sin kick (BBP mudo ×2 antenas).
- **El BBP mudo es consustancial al estado que podemos alcanzar por WinUSB**:
  sin boot frío REAL (replug eléctrico con autoload corriendo desde ROM) el BBP
  no despierta, y el kick que lo despertaría mata el USB.
- **DECISIÓN: la vía USB crudo Windows queda cerrada para RX.** TX ya verificado
  (TXDONE success=true) — pendiente solo confirmar beacons al aire desde otra
  radio. Para RX: Kali live USB (rt2800usb hace el boot frío completo en kernel).
- Siguientes pasos: (a) TX al aire (rt3070_tx + verificación con otra radio);
  (b) integrar TX por USB crudo en la app uifipill (deauth/beacon con RX solo
  por Kali); (c) NO comprar más RT3070 para RX bajo WinUSB.

## 🎉 HITO TX AL AIRE CONFIRMADO (2026-09-25, veredicto definitivo)
**La red "TXTEST" (beacon generado por rt3070_tx por USB crudo) es VISIBLE desde
otro dispositivo.** El pipeline TX completo funciona: TXINFO+TXWI+802.11 → EP 0x01
→ chip modula → **SALE AL AIRE de verdad**. Primera TX RF activa del proyecto en
Windows sin Npcap, sin Kali, solo WinUSB.
- Condiciones del test: power-cycle → rt3070_tx "TXTEST" 11 20 UNA sola vez.
- Patrón TX reproducido: los 2 primeros frames entran (bulk ACK) y el resto dan
  Timeout (EP se atasca sin drain de TX status en estado heredado). Con 2 frames
  por burst basta para beacons continuos si se re-lanza o se añade drain.
- TXDONE del hito 2026-09-23 (TX_STA_FIFO success=true) + beacons visibles ahora
  = doble confirmación (chip TX-eó Y sale al aire).
- Implicación: la app puede hacer ATAQUES TX activos (deauth/beacon flood) por
  USB crudo en Windows, aunque el RX siga por Kali.

## SESIÓN RX 2026-09-25 (final) — nueva hipótesis H1 + plan en (RX_PLAN) §4
- TX al aire CONFIRMADO y anotado (ver hito arriba). Investigación RX continúa.
- **Dato que reabre RX**: la antena SÍ capturaba bajo netr28ux (392 pkts Npcap,
  hito 2026-09-14) → el BBP NO está roto; el vendor lo deja VIVO y algo lo duerme
  antes de nuestro acceso.
- **H1 (principal)**: el driver vendor manda MCU_SLEEP al detach/rebind (como
  rt2800usb_set_device_state STATE_RADIO_OFF) → heredamos BBP dormido y el
  WAKEUP solo no lo re-arma (medido).
- **E1 (siguiente experimento)**: hot_rebind to-winusb CON LA RADIO ACTIVA
  (netsh escaneando en bucle o captura Npcap en monitor) + rt3070_hotread en
  <2 s — si el BBP hereda vivo, vendorradio → RX directa.
- **E2**: ciclo MCU_SLEEP(0xff,0xff,2) → MCU_WAKEUP(0xff,0,2) completo (el
  firmware puede requerir el ciclo; probar arg1=0 vs 2).
- Verificado en fuente real: USB_RX_CONTROL(0x0C) SOLO se usa para DESactivar
  la radio (rt2x00usb_disable_radio); la activación es submit de URBs bulk IN
  (lo que ya hacemos). No hay vendor request mágico de RX-ON que nos falte.
- Plan completo y órden de experimentos: `(RX_PLAN) §4`.
- Cerrado definitivamente (no reintentar): kick FIRMWARE en cualquier estado,
  re-enum tras kick con reapertura simple, confiar en MAC_CSR0 como señal de fw.

## SESIÓN RX 2026-09-25 (E1 ejecutado) — H1 falsada, mecanismo de rebind resuelto
- **E1 ejecutado completo** (e1_run3.ps1, log en e1_log.txt):
  1. netr28ux publicado + oem395 desinstalado → **netr28ux tomó el control**
     (vía pnputil /add-driver + /delete-driver oem395 /uninstall + /scan-devices).
     NOTA: UpdateDriverForPlugAndPlayDevices (newdev.dll) da win32err=2 SIEMPRE
     con este device — la vía que funciona es pnputil delete+rescan (3-4 min
     por el timeout del uninstall, paciencia).
  2. netsh escaneando en bucle (radio EN USO) durante el rebind.
  3. delete netr28ux + rescan → **WinUSB (oem395) heredó el device SIN
     power-cycle** — Status OK, chip respondiendo (MAC_CSR0 vivo, RF vivo,
     mailbox OWNER=0 limpio).
- **RESULTADO H1: FALSADA.** Incluso heredando el chip del vendor con la radio
  EN USO (escaneo activo), el BBP sigue leyendo 0x00. El vendor NO lo duerme al
  soltar el device: **el BBP está VIVO para el vendor pero INVISIBLE para
  nosotros** (sus capturas Npcap funcionan, nuestros reads dan 0).
- mcutest post-rebind: MCU_CURRENT #1 consumido, luego se degradó en vivo
  (0xffffffff) — el chip heredado es frágil y cada acceso nuestro lo tumba.
- **Hipótesis H2 (nueva, ganadora provisional)**: el acceso BBP del vendor no
  usa BBP_CSR_CFG (0x11C) vía vendor request — posiblemente usa la vía MCU
  (mailbox H2M con comandos de lectura/escritura BBP del firmware) o un path
  interno distinto. Nuestro write+readback en 0x11C se devuelve vacío porque el
  arbiter BBP del firmware atiende otra puerta. INVESTIGAR: comandos MCU
  del firmware RT3070 para acceso BBP/RF (reversing del firmware o del driver
  vendor netr28ux.sys — tenemos el .sys en driver_re/).
- Estado del device tras E1: WinUSB oem395 activo, Status OK. Chip probablemente
  degradado tras los tests (power-cycle antes del siguiente intento).
- Actualización de (RX_PLAN) §4: E1 falsado → H2 con plan de reversing.

## SESIÓN RX 2026-09-25 (E2 + reversing netr28ux) — H2 en marcha, hallazgos del .sys
- **E2 ejecutado** (`rt3070_mculoop`): ciclo SLEEP(0xff,0xff,2)→WAKEUP(0xff,0,2),
  WAKEUP(arg1=0), WAKEUP×2 — **el BBP NO despierta en ninguna variante**. E2 descartado.
- **heredrx** (`rt3070_heredrx`): sobre estado heredado del vendor SIN tocar BBP/MCU,
  solo USB_DMA_CFG=0x00c12d80 + ENABLE_RX + filtro monitor → 0 frames. El DMA no
  basta: el BBP no demodula nada (para nadie).
- **REVERSING netr28ux.sys (2.2 MB, x64, sin empacar)** — hallazgos:
  · PE limpio: .text file 0x400-0x1bbc00 (rva 0x1000+0x400), strings de nombres
    de funciones DENTRO de .text (p.ej. RTMPApplyPacketFilter @file 0x1b0c9c).
    Mapeo file→rva: rva = file - 0x400 + 0x1000 (ojo: no es .rdata).
  · El vendor habla con el chip POR BULK OUT (URB_FUNCTION_BULK_OR_INTERRUPT
    _TRANSFER=0x2B, wrapper @0xade0, 1040 llamadas) — NO solo por control pipe.
    Nombres RTUSBBulkOutPktCmd / RTUSBBulkOutMLMEPacket / RTUSBBulkReceive:
    hay PIPES DE COMANDOS BULK dedicados (además de 0x01-0x06/0x81).
  · Strings BBP localizados (en .text): AsicBbpTuning, AsicWriteBBPR66,
    BbpInit7601, PostBBPInitialization, NICRestoreBBPValue, WriteBBPR66.
  · Rutina de logs/asserts con edx=0x11C y [rsp+0x20]=0x208f (@0xd11f) — el
    vendor TAMBIÉN usa el registro BBP_CSR_CFG 0x11C, aunque su acceso real
    parece ir por el bulk command pipe.
  · Par de comandos 0x41/0x42/0x43 con strings de función cerca (@0x69cc/
    0x6a0a/0x6a2e) — la llamada 0xade0 con r9=string es un LOG con el nombre
    de la función (debug wrapper), no el comando en sí.
- **PENDIENTE próxima sesión**: xrefs correctos de BbpInit7601/AsicBbpTuning
  (los lea rip-rel no cuadran porque los strings están en .text — recalcular con
  rva=file-0x400+0x1000 y buscar disp32 que apunten al STRING EXACTO) y extraer
  el formato del paquete de comando BBP por el bulk pipe; alternativa: sniffer
  USB (USBPcap) capturando lo que el vendor manda al hacer scan y replicarlo.
- Estado hardware al cierre: chip heredado degradado por los tests → power-cycle
  antes del siguiente experimento.

## SESIÓN RX 2026-09-25 (cierre definitivo) — coldrx: el autoload NO deja el BBP vivo
- Experimento final (`rt3070_coldrx`): UNPLUG → re-enum frío (addr 42→43) →
  SOLO config host-side (USB_DMA_CFG + ENABLE_RX + filtro) SIN tocar BBP/MCU
  → RX 25 s → **0 frames, bbp[0]=0x00 en frío**.
- **CONCLUSIÓN DEFINITIVA DEL MISTERIO BBP**: el autoload de fábrica del chip
  (el que corre tras UNPLUG/power-cycle sin driver) NO deja el BBP ni
  inicializado ni accesible por 0x11C. El BBP del RT3070 SOLO se activa cuando
  el firmware del MCU lo hace en un arranque completo con carga de firmware —
  y ese arranque (kick FIRMWARE(8)) deja el control pipe sordo bajo WinUSB
  (medido 5/5: power-cycle ×4 + UNPLUG frío ×1).
- El vendor netr28ux SÍ logra el arranque completo porque rehace TODO el boot
  desde su IRP_MN_START (incluida la carga de fw) y su wrapper USB bulk soporta
  la ventana crítica del re-arranque del MCU — algo que el control pipe WinUSB
  no nos permite (el device desaparece y Windows no nos devuelve un handle
  utilizable).
- **Cierre de la vía USB crudo WinUSB para RX: DEFINITIVO.** TX al aire sigue
  confirmado (hito de hoy). RX de esta antena: SOLO Kali live USB (rt2800usb).
- Para el próximo acceso RX-WinUSB (si algún día se intenta): la única vía
  abierta es descubrir el formato de comando del bulk pipe del vendor
  (RTUSBBulkOutPktCmd) vía USBPcap sniffer mientras netr28ux escanea, y
  replicar esos paquetes — el resto de vías están cerradas MEDIDAS.

## SESIÓN 2026-09-28 — RESET DE PC REVELÓ: HVCI activado bloquea netr28ux (código 31/56)
- Tras reinicio: antena con ProblemCode 56 (CM_PROB_NEED_CLASS_CONFIG) → 31
  (CM_PROB_FAILED_ADD) con **ProblemStatus 0xC0000495 =
  STATUS_SYSTEM_INTEGRITY_POLICY_VIOLATION**.
- Causa raíz: el reinicio activó **VBS+HVCI (Integridad de memoria)** —
  Win32_DeviceGuard SecurityServicesRunning={2}, HVCI Enabled=1. HVCI bloquea
  el netr28ux 5.01.22 (2007, firma no HVCI-compatible) del DriverStore
  (netr28ux.inf_amd64_2613a90929adebda, md5 ba0c8f0b...). Antes del reinicio
  funcionaba porque el driver ya estaba cargado de un boot anterior.
- Fixes aplicados (scripts en usb_tx/): fix_dev56.ps1 (remove-device+rescan),
  fix_dev56b.ps1 (limpia ConfigFlags 0x80000 FAILEDINSTALL),
  fix_dev56c.ps1 (arranca NetSetupSvc+NetMan y restart-device) — necesarios
  pero insuficientes hasta desactivar HVCI.
- **fix_hvci_off.ps1**: HVCI Enabled 1→0, VBS 0 (registro DeviceGuard).
  PENDIENTE: reinicio + verificar SecurityServicesRunning sin {2} + netr28ux
  Status OK → sniffer USBPcap (USBPcap 1.5.4.0 YA INSTALADO, revisar que
  sobrevivió al reinicio: C:\Program Files\USBPcap\USBPcapCmd.exe ✅).
- Nota: sistema shell NO es admin; todo fix requiere UAC (scripts auto-elevan
  con transcript a *_log.txt para lectura posterior).

## SESIÓN 2026-09-29 — POST-REINICIO: HVCI off logrado; device en 31/56 por post-install colgado
- Reinicio aplicó fix_hvci_off.ps1: **HVCI DESACTIVADO** (Win32_DeviceGuard
  SecurityServicesRunning={0}, antes {2}). Code Integrity ya NO registra
  rechazos del netr28ux tras el boot.
- PERO el device quedó envenenado: el boot de 02:29 arrancó AÚN CON HVCI
  activo → kernel colgó el post-install del netr28ux 2007 → setupapi.dev.log:
  «Timed out waiting for device post-install» Error 0x5B4 → ProblemCode 31
  (FAILED_ADD) con ConfigFlags 0x80000. Todos los reinicios de device
  posteriores heredan el estado (re-plug físico NO relanza instalación).
- Cadena de fixes ejecutada (todos con log en driver_re/usb_tx/*_log.txt):
  1. fix_dev56.ps1 (remove+rescan) → limpió ProblemStatus 0xC0000495
     (STATUS_SYSTEM_INTEGRITY_POLICY_VIOLATION, era cacheado del boot) pero
     56 persiste.
  2. fix_dev56b.ps1 → ConfigFlags 0x80000 limpiados, restart → ahora 31.
  3. fix_dev56c.ps1 → NetSetupSvc/NetMan running, restart → sigue 31.
  4. fix_netr28_min.ps1 (NUEVO, logs directos a fichero sin transcript): full
     clean reinstall — backup del DriverStore a C:\Windows\Temp\netr28ux_backup,
     remove-device, paquete netr28ux es INBOX (sin oemXXX que borrar),
     sc delete netr28ux + re-add del INF + rescan → servicio recreado OK,
     device pasa a 56.
  5. fix_netsetup.ps1 → WlanSvc y nlasvc arrancados (estaban STOPPED/
     START_PENDING — el stack de config de red estaba colgado, causa del
     timeout 0x5B4). NetSetupSvc es on-demand (Manual) y se para en idle:
     NORMAL.
  6. fix_restart_dev.ps1 (remove+rescan final) — PENDIENTE DE EJECUTAR:
     la elevación UAC no llega (consent.exe pendiente no aceptado / procesos
     que mueren sin loguear; schtasks //create también denegado sin admin).
- Hallazgo de método: PowerShell 5.1 de este equipo falla al cargar
  Microsoft.PowerShell.Security (TypeData duplicado) → Get-AuthenticodeSignature
  inservible; y Start-Transcript en scripts auto-elevados NO fiable (el
  transcript del lanzador no-elevado sobreescribe el del proceso elevado).
  USAR logging directo a fichero con ruta absoluta hardcodeada.
- **PENDIENTE (siguiente sesión)**: ejecutar fix_restart_dev.ps1 en consola
  admin manual (PowerShell como administrador → una línea, el script loguea a
  fix_restart_dev_log.txt). Si sigue 56 con el stack WLAN sano → REINICIAR:
  el próximo boot instala el device con CI ya sin HVCI + stack reparado, debe
  levantar limpio (el boot envenenado fue el único con HVCI activo).
- Verificación final tras levantar: device Status OK + netsh wlan show
  interfaces muestra la antena + (opcional) activate_monitor de la app.


## SESIÓN 2026-09-30 — CAUSA RAÍZ DEL 56 ENCONTRADA: faltaba `C:\Windows\INF\netr28ux.inf` (0x80070002)

Post-reboot con HVCI ya OFF (`SecurityServicesRunning={0}`, WlanSvc/nlasvc
Running), el device seguía en ProblemCode 56 y `netsh` no veía interfaz.

### Diagnóstico nuevo (la pieza que faltaba)
- Clase `{4d36e972}\0006` del device: **`NetworkInterfaceInstallResult =
  2147942402 = 0x80070002` (ERROR_FILE_NOT_FOUND)** y **sin `NetCfgInstanceId`**
  → la instalación de la interfaz de red (NetSetup/INetCfg) fallaba por fichero
  no encontrado → la class config nunca completaba → 56 permanente.
- Causa: **`C:\Windows\INF\netr28ux.inf` estaba BORRADO** (solo quedaba el
  `.PNF`; lo había eliminado `rebind_winusb.ps1` — §1 lo citaba como «oculto»,
  en realidad ya no estaba). `InfPath` del driver key = `netr28ux.inf`.
- `setupapi.dev.log` no mostraba NINGUNA sección «Install Device» tras los
  rescan (solo Delete) — el instalador moría en la fase de clase antes de log.
- Intentos intermedios medidos (NO bastaron): `fix_restart_dev.ps1`
  (remove+rescan) → 56; `fix56_services_restart.ps1` (NetSetupSvc+NetMan up +
  `/restart-device` + remove+rescan) → 56 (NetSetupSvc START_PENDING→Stopped
  en 3 s).

### FIX (fix56_inf_restore.ps1) — RESULTADO
1. Restaurar `netr28ux.inf` + `netr28ux.PNF` desde
   `DriverStore\FileRepository\netr28ux.inf_amd64_2613a90929adebda` → `C:\Windows\INF\`.
2. Limpiar `ConfigFlags 0x80000` (FAILEDINSTALL) → 0 en la Enum key.
3. `pnputil /remove-device` + `/scan-devices`, esperar 45 s.
→ **Device Status=OK, problem=0**, `NetCfgInstanceId={FDA1B084-61E6-4AAA-8A25-56F2549735C8}`,
`netsh` muestra «Wi-Fi — 802.11n USB Wireless LAN Card» (MAC 00:c0:ca:59:f8:b5,
HW+SW radio activado) y **scan real: 10 redes visibles** (RF viva).

### Estado del driver y del entorno tras el fix
- `System32\netr28ux.sys` = sha256 `94734AEF…` = **copia INBOX del DriverStore
  (5.1.22.0, 2007)** — el driver PARCHEADO (`d2c7cf43…`, canal en monitor §3) se
  perdió en la cadena de fixes del 09-29. Hasta no redeployar, el canal en
  monitor NO cambia (gate bit17 activo otra vez). testsigning sin verificar
  (bcdedit exige admin).
- usbipd: antena 1-6 `148f:3070` estado **Shared** (no estorba para Windows);
  aviso conocido: filtro USBPcap incompatible con usbipd (bind --force si se usa).
- Logs de esta sesión: `driver_re/usb_tx/fix_restart_dev_log.txt`,
  `fix56_log.txt`, `fix56_inf_log.txt`.

### Deploy del driver parcheado (misma sesión, tarde) — hecho, FALTA EL REBOOT
1. `deploy_patched_hot.ps1` (wrapper nuevo): chequea testsigning → hot-swap vía
   `hotswap_driver.ps1` (disable PnP → copia → enable, sin reinicio) → verificación.
   Resultado: **`.sys` parcheado `d2c7cf43…` EN SITIO** ✅, pero **Problem 52
   (CM_PROB_UNSIGNED_DRIVER, 0xC0000428)**: `bcdedit` reveals **testsigning estaba
   OFF** (se perdió en algún punto de la crisis 09-28/29; el 09-21 estaba ON).
   Sin testsigning el kernel rechaza la firma de lab → no carga → sin interfaz.
2. **BUG de wrapper encontrado y corregido**: la condición era
   `if ($tsLine -notmatch 'Yes|S[ií]')` — `-match` es case-insensitive y `S[ií]`
   casaba con el **"si" de "testsigning"** → la rama siempre iba a "ON". Fix:
   regex anclada `(?m)^testsigning\s+Yes`. (Anotado para no repetir: nunca usar
   fragmentos cortos sin anclar contra la propia palabra que estás buscando.)
3. `set_testsigning.ps1` (nuevo): `bcdedit /set testsigning on` → verificado
   **`testsigning Yes`**, Secure Boot **False**, hash sigue `d2c7cf43…`.
4. Preflight antes de pedir el reboot (todo OK): cert `labtest.cer` confiado en
   LocalMachine Root + TrustedPublisher (thumb AB1C71D6…), HVCI
   `Enabled=0` + `SecurityServicesRunning={0}`, Secure Boot off.
   → **El próximo boot debe levantar el device con el parche cargado.**

### Siguiente (orden) — tras el REBOOT pendiente
1. Verificar: `bcdedit testsigning Yes` + device Status OK (problem 0) +
   `netsh` ve la antena + hash `d2c7cf43…`.
2. `cargo test --lib lab_monitor_cycle -- --ignored --nocapture` (monitor CH6 →
   set CH1 → restore) = prueba del cambio de canal parcheado. Prueba RF dura
   como el 09-21: `activate_monitor(guid, 1)` + `native_capture` y ver que solo
   aparecen APs del CH1 (DS Parameter Set).
3. Si el boot no levanta el parche (52 otra vez): revisar registro de arranque
   (HVCI puede haber vuelto, CI policy) — rollback posible: copiar la copia
   inbox `94734AEF` desde DriverStore con el mismo hot-swap (interfaz WiFi
   recuperada aunque sin cambio de canal).
4. Después: `hot_rebind.ps1 to-winusb` → power-cycle 15 s → TX crudo en la app
   (paso A). RX Windows = Npcap pasiva; RX completa = Kali live USB.

## SESIÓN 2026-10-01 — PROTOCOLO BBP vía MCU descifrado + BUG del parser RX (offline verificado)

Origen: captura USBPcap `driver_re/usb_tx/vendor.pcap` (11.6 s, dev=3, 380 vendor
requests, 111 bulk IN / 43 040 B) analizada con `usb_sniff_stats` (§1-§6).

### Hallazgos MEDIDOS
1. **El vendor NUNCA toca BBP_CSR_CFG (0x101C)**: las 19 ops BBP van por
   H2M_BBP_AGENT (`0x7028` valor / `0x702A` flags) + H2M_MAILBOX_CSR (`0x7010`,
   OWNER=1 `0x01000000`, TOKEN=0xff) + HOST_CMD_CSR (`0x0404`) = **MCU_BBP_SIGNAL
   (0x80)**. Valores leídos reales: BBP1=0x40, BBP49=0x8a, RF3=0x32, RF6=0x02 →
   **el BBP está VIVO bajo el vendor**. Word agente: VALUE[7:0] | REGNUM[15:8] |
   flags<<16 (bit0=READ, bit1=BUSY, bit3=RW_MODE) → 0x0b/0x0a pendiente,
   0x09/0x08 completado. Escrituras vistas: 62/63/64=0x37, 82=0x62, 75=0x46,
   66=0x1c, 1=0x40.
2. **BUG `MCU_CURRENT`**: estaba en `0x30` = **MCU_SLEEP** (rt2800.h:3005); el
   real es **0x36** (rt2800.h:3008, rt2800lib.c:10709). Nuestro `init_radio`
   mandaba un power-save al 8051 justo al acabar el init (candidato a RX muda).
   `MCU_WAKEUP=0x31` y `MCU_BOOT_SIGNAL=0x72` sí eran correctos.
3. **BUG del parser RX → falso negativo de "0 frames"**: layout real del EP 0x81
   = `[4 B len][RXWI 16 B][802.11 en el byte 20]` con `n = len + 8` (**111/111
   URBs**); el código asumía `[4 dma][32 rxwi]` → FC@36 → **0 frames contados
   aunque el chip entregara datos**. BSSID además se leía de addr1 (DA = broadcast
   FF:FF:…) en vez de addr3. RSSI sigue en RXWI_W2 bits 0-7 (offset 12).
4. El vendor **no** hace kick `FIRMWARE(8)` ni re-enumeración en la ventana: solo
   bReq 2 (SINGLE_WRITE, 16-bit sin payload ×258) / 3 / 6 (MULTI_WRITE) /
   7 (MULTI_READ ×122). Comandos MCU vistos: 0x80, 0x50 (LED), 0x74 (FREQ_OFFSET),
   0x30 (MCU_SLEEP). Re-init en t=3.489→4.192 s: US_CYC_CNT, PBF_CFG=0x00f40006
   (idéntico al nuestro), BCN_TIME_CFG=0x640 (idéntico), MAC_SYS_CTRL, TX_RTS_CFG.
5. RF por RF_CSR_CFG 0x0500 como dos SINGLE_WRITE con RMW — misma codificación
   que nuestro `rfcsr_write` (los valores del vendor difieren de la tabla Linux).

### Código
- `common.rs`: `reg_write16` (SINGLE_WRITE), `MCU_BBP_SIGNAL=0x80`,
  `MCU_CURRENT→0x36`, `agent_wait`/`mcu_bbp`, `bbp_read`/`bbp_write` **por MCU**
  (la directa 0x101C queda como `bbp_read_direct`/`bbp_write_direct` para
  diagnóstico), y **`walk_rx`/`parse_beacon`/`rx_monitor`** con el layout medido.
- `usb_sniff_stats.rs`: §3b (resumen BBP vía MCU), §5b (layout/longitud/offset),
  §5c (**validación OFFLINE del parser sobre la captura**).
- Parser corregido (mismo layout) en: `scan.rs`, `sniff.rs`, `linuxfull.rs`,
  `vendorradio.rs`, `coldrx.rs`, `heredrx.rs`, `hotread.rs`, `coldboot.rs`,
  `fwfirst.rs`, `fwreenum.rs`.
- Bin nuevo `rt3070_bbpmcu`: fase 1 sondeo (MCU 0x36 vs BBP directo), fase 2
  radio **sin kick FIRMWARE(8)**, fase 3 RX con parser correcto + hex crudo.
- Tests: 3 unitarios en `common.rs` (layout medido, garbage, constantes MCU).

### Verificación (2026-10-01)
`npm run lint` ✅ · `npx vite build` ✅ · `src-tauri cargo build --release` ×2 ✅
(0 warnings) · `cargo test --release --lib` **23 passed / 0 failed / 8 ignored** ✅
· `driver_re/usb_tx cargo build --release` ✅ (0 warnings) · `cargo test` 3/3 ✅.
Offline sobre vendor.pcap: **111 frames / 84 beacons / 4 APs reales**
(TP-Link_29D6 ch3, MIWIFI_APUM ch11, sagemcomDEF0_Plus ch11, Livebox6-34D0 ch11).

### Pendiente (próxima acción — requiere UAC, la antena está en netr28ux)
`fix_switch_winusb.ps1` → `rt3070_bbpmcu 20 11` → `restore_netr28ux.ps1`.

### Cómo llegar a TX operativo (consolidado)

## SESIÓN 2026-10-01 (cont.) — ?? HITO: RX POR USB CRUDO WINUSB CONFIRMADA (524 frames)

Cierre del diagnóstico "RF sordo" (runs bbpmcu 5/6/7: ΔCCA=0 en 1..14 pese a
BBP/PA/MAC programados; RFCSR8 devolvía siempre 0x42).

### Causa raíz: path de canal equivocado (rf53xx vs rf3xxx)
- Nuestro `config_channel_rt3070` portaba `rt2800_config_channel_rf53xx`
  (N→RFCSR8, K→RFCSR9, R→RFCSR11.R) — ese path es para RF5370/Xtal20M.
  **Este chip es path 3xxx** (Ralink `RT30xx_ChipSwitchChannel`,
  chips/rt30xx.c:590 = Linux `rt2800_config_channel_rf3xxx`, rt2800lib.c:2466):
  **N→RFCSR2, K→RFCSR3[3:0], R→RFCSR6[1:0]**.
- Evidencia (3 vías): (a) vendor.pcap #184 escribe `RFCSR2=0xF6` (N canal 11);
  (b) Ralink vendor: `RT30xxWriteRFRegister(RF_R02, FreqItems3020[].N)`;
  (c) **RFCSR8 = ID de versión del RF** (0x42 fijo en 1..14 — ahí caía el N y
  el PLL nunca sintonizó → ΔCCA=0). `FreqItems3020` = `rf_vals_3x` idéntica
  a nuestra `RF_VALS_3X` ✅ (el destino, no la tabla, era el bug).
- Correcciones añadidas en el mismo port: RFCSR1 streams 1T1R =
  `(r1 & 0x01) | 0xA0 | 0x50` → **0xf1** (antes `|0x0f`=0xff y, con RMW de
  bits1:0 heredado de runs viejos, 0xf3 = PLL_PD=1 — medido en run8);
  RFCSR12/13 TX power 6/5 (campo 0x1f, RMW); RFCSR24/31 = calib BW20 medida
  en init (`CALIB_BW20`, 0x07-0x09 según run; antes quedaban en estado BW40);
  RFCSR7.RF_TUNING=1; **pulso VCOCAL RFCSR30 bit7 1 ms** (antes bit7 de
  RFCSR3 = ese bit es bias PA2 CCK, registro equivocado); RFCSR23 sin tocar
  (recorte de cristal del silicio).

### Resultados MEDIDOS
- **run8** (tras el port): ΔCCA>0 en **los 14 canales** (antes 0 en todos);
  readback N por canal 0xf1…0xf8 ✅, K 0xb2/0xb7 alternando ✅, A/B RFCSR2
  persiste (0xf6 a 10 ms y 100 ms) ✅; FASE3: 0 URBs pero RX_STA_CNT1 movió
  (F-CCA=3461, CRC_ERR=106) → el MAC oye; faltaba entregar a USB.
- **run9** (RFCSR1→0xf1 + RFCSR23 sondeado): **?? URBs=611, bytes=182992,
  frames=524 en 15 s**, FC siempre offset 20, **3 APs con beacon real**:
  MIWIFI_APUM (88:0F:A2:49:39:A4), sagemcomDEF0_Plus (CC:D8:43:9F:BC:61),
  Livebox6-34D0 (E4:C0:E2:9C:34:D4) — todos ch11. FASE4: ΔCCA y ΔCRC>0 en
  casi todos los canales (ch12: ΔCRC=113, ch13: 82, ch10: 68…).
- Detalles run9: RFCSR23=0x00 (vendor escribe 0x09 — recorte pendiente, NO
  bloqueante), RFCSR7=0x60 (bit0 se autolimpia tras el tuning), RFCSR8=0x23
  (registro de estado, antes 0x42), calib BW20=0x07, RFCSR1=0xf1 ✅.

### Impacto (docs)
- **RX por USB crudo WinUSB REABIERTA** — las líneas "RX … CERRADA" de
  memory §4/§5 y AGENTS.md §3 estaban basadas en "BBP mudo"; la raíz era
  N→RFCSR8 + PLL_PD, no el boot ni el chip. Sniffer USBPcap ya no es la
  única vía RX.
- Código: `common.rs` (config reescrita, `CALIB_BW20`/`calib_bw20()`,
  comentario RF_VALS_3X), `bbpmcu.rs` (fase4: regs 2/3/6/23, A/B RFCSR2,
  readback por canal 2/3/6, imprime calib). Wrappers `run_bbpmcu8/9.ps1`
  + logs `bbpmcu_run8/9.txt`.
- Verificación: `cargo build --release` usb_tx ✅ 0 warnings (runs 8 y 9).

### Continuación (2026-10-01, mismo día) — scan sostenido + RFCSR23 + init_radio seguro
- **scan1** (`run_scan1.ps1`, 30 s, RFCSR23=0x00): **247 frames / 10 redes**
  en 5 canales (1/3/6/11/13), parser FC@20, **sin kick FIRMWARE(8)**. Fixes
  que lo habilitaron en `common.rs`:
  1. **`init_radio` sin kick**: gate de MCU vivo (`MCU_CURRENT`) → si vivo se
     salta el firmware (como Linux con autorun); si muerto → error accionable
     (power-cycle físico). El bloque de carga+kick FIRMWARE(8) se eliminó.
  2. **Sonda de transporte BBP** (`bbp_probe_transport`): el default
     `BBP_VIA_MCU=true` daba timeout en `mcu_bbp` sin firmware (runs 8/9:
     0/5) → `unwrap_or(0)` → "BBP no responde". La sonda (mismo criterio que
     bbpmcu) conmuta a **directa `BBP_CSR_CFG`** (2/2) → init_bbp OK.
- **scan2** (RFCSR23=**0x09** ya aplicado en `config_channel_rt3070`, valor
  vendor de ESTA antena, bit7 preservado): **225 frames / 12 redes** (+2
  nuevas: MOVISTAR_2B4A_EXT ch6, vodafone3048 ch1) — sin degradación →
  r23=0x09 se queda.
- Wrappers/logs nuevos: `run_scan1/2.ps1`, `scan_run1/2.txt`.

### ~~Pendiente~~ CERRADO (2026-10-01)
1. ~~RX sostenida/scanning~~ → scan1/scan2 arriba ✅ (integración UI abajo).
2. ~~Recorte RFCSR23=0x09~~ → aplicado y comparado (scan2) ✅.
3. ~~Actualizar AGENTS.md + memory §4/§5~~ → AGENTS.md en la sesión previa;
   memory §4/§5 en esta misma edición ✅.
4. ~~Integración en la app UI~~ → sesión siguiente (abajo).

## SESIÓN 2026-10-01 (cont. 2) — INTEGRACIÓN UI USB-CRUDO E2E (scan + ataque)
Decisión del usuario: **"sin límites"** (fallback automático), **auto-convertir**
el handshake a `.22000`, deauth al aire lo prueba él tras el build.
Hallazgo clave previo: **`rt3070_probe` abre el chip SIN admin** desde el shell
no-admin → la app (asInvoker) puede usar WinUSB; elevación no hace falta.
Bugs reales encontrados en el cableado ya existente (8 comandos `usb_raw_*`):
1. **Ruta pcap `%TEMP%` literal** en `usb_raw_sniff` (`Command` no expande env)
   → el bin no podía crear el fichero → fix con `std::env::temp_dir()`.
2. **Sniff sin init previo**: `usbRawSniff` no hacía `usb_raw_init` → RX no
   armado; además usaba `|| 11` en vez de `selectedChannel()` (regla 8) → fix.
3. **Sniff exit 0 con 0 frames** (regla 2): `sniff.rs` ahora `exit(2)` +
   backend marca `success=false` con mensaje accionable.
4. **Seguridad inventada**: merge USB usaba `security: 'WPA2'` por defecto →
   (mantenido: las redes netsh conservan su seguridad real; USB-only hereda).
5. **`wpa3-bssid` huérfano** (input inexistente en index.html) → `wpa3Audit()`
   y `fillWacker()` crasheaban al hacer click (regla 6) → input añadido.
Cambios nuevos (visibilidad + ataque):
- Botón **«📡 Escanear (USB crudo)»** en `tab-scan` (junto a «Escanear ahora»).
- **Fallback automático**: `scanAndDisplay` → si netsh da error o 0 redes →
  lanza `usbRawScan()` solo; idem en `scanAndAutoAttack`.
- **Selección de red rellena el panel USB** (`usbraw-bssid/ssid/chan` en
  `syncTargetEverywhere`) → canal heredado en todos los flujos USB.
- **Handshake auto-convierte**: deauth+sniff → `pcap_to_22000` automático →
  `.22000` prellenado en `crack-hash`+`insp-hash` (fallo honesto si 0 hashes).
- Textos sin-kick: «Init radio (canal, sin kick)»; `usb_raw_init` ya no pasa
  firmware (init_radio no lo usa).
- Auditoría cableado (regla 6): **0 onclick sin definir, 0 invoke sin
  registrar, 0 ids inexistentes** (script de regex sobre index/app/lib).
Verificación: `node -c` ✅ · `npm run lint` ✅ · `vite build` ✅ ·
`cargo build --release` src-tauri **0 warnings** ✅ · usb_tx 0 warnings +
tests 3/3 ✅.
**E2E real SIN admin (shell no-admin, chip en WinUSB)**:
- `rt3070_scan 10s` → **11 redes con seguridad detectada** en el beacon
  (10×WPA2 + HP-Print «Abierta o WEP» — nada inventado; parser nuevo RSN
  tag48 → AKM SAE=8→WPA3, vendor 221 00:50:F2→WPA, sin IE→«Abierta o WEP»).
- `rt3070_init 11` → «MCU vivo — sin kick» + transporte directo ✅.
- `rt3070_sniff 10s` → **359 frames** y pcap real en `%TEMP%` ✅. La causa
  del bajo recuento anterior (3 frames): sniff pisaba `USB_DMA_CFG` (AGG
  perdido) — ya NO lo pisa (usa `rx_filter_monitor` como scan). Caso de 0
  frames verificado también: **exit 2** + mensaje accionable (regla 2).
Pendiente de usuario: prueba E2E en la app (scan→objetivo→handshake→crack) y
deauth al aire con red de lab propia.

## SESIÓN 2026-10-01 (cont. 3) — DRIVER RESTORE + WASH E2E + REAVER: mapa honesto
**1. Restore del driver a netr28ux (mañana)**: `restore_run3.ps1` (secuencia
fusionada en **un solo UAC**: INF+PNF desde DriverStore → ConfigFlags=0 →
remove+rescan → borrar paquete WinUSB `oem180` → add netr28ux → 45 s → verify)
→ **device Status=OK ProblemCode=0**, `netsh` ve 8-18 SSIDs (Wi-Fi, GUID
`B2449CCC-38E7-4229-9658-A2EFB3C966B6`, MAC 00:c0:ca:59:f8:b5), módulo cargado
`= D2C7CF43` (**parcheado**), hash idéntico al de System32. `lab_monitor_cycle`
pasó no-admin. Lecciones: (a) `Start-Transcript` + `pnputil` elevado en PS 5.1
**deadlock** → `Start-Process -RedirectStandardOutput` + watchdog; (b) si el
proceso lanzador muere antes del clic del UAC, el click NO eleva nada (UAC
huérfano) → relanzar; scripts con log a ruta absoluta (regla 5). Rollback:
`rebind_winusb.ps1` (vuelta a WinUSB).
**2. WPS/wash: cuatro causas raíz (todas medidas, todas fixeadas)**:
(1) `tools/libpcap.dll` era build msys64 con backend **`pcap-null.c`** («live
packet capture not supported» → `pcap_open_live` NULL → «couldn't get pcap
handle») → **swap**: backup `libpcap.dll.nullbak` + copia de
`C:\Windows\System32\Npcap\wpcap.dll` → `tools/libpcap.dll` + `Packet.dll`
(wpcap importa Packet.dll del mismo dir); `Packet.dll` **añadido a
`tauri.conf.json` resources** para el NSIS. (2) `wash -i \Device\NPF_{GUID}`
(dlt=**1** ethernet) no parsea WPS; el correcto es **`\Device\NPF_WIFI_{GUID}`
(dlt=127 radiotap)** — fix `normalize_wps_iface()` aplicado a los 8 comandos
WPS (wlan0/vacío → detect_adapters + aviso). (3) El RT3070 añade FCS y wash
descarta todo («bad FCS, skipping» → 0 filas) → flag **`-F`** en wash y en los
8 args de reaver (reaver también lo soporta: `-F, --ignore-fcs`). (4) wash en
survey **nunca termina** → `run_bin_bg_opts()` con watchdog (`timeout 30 s` con
`-c`, `60 s` sin él), child registrado en AppState → «Cancelar» funciona.
**3. Parser `parse_wash`** reescrito con salida REAL: `Lck` es `Yes/No` (no la
palabra «locked»), vendor **truncado a 8 chars** («RalinkTe») → match por
prefijo y solo si hay ESSID detrás, `(null)`→vacío, dedupe por BSSID (wash
repite filas en updates). 3 tests unitarios con fixture de salida real.
**4. Modo monitor — lección de réplica**: el set OID correcto usa **data de 8
bytes con el modo ULONG en offset 4** (`activate_monitor` de `monitor_mode.rs`
lo hace bien desde siempre); una réplica C# con data de 4 bytes devolvía
«SET OK» pero era **no-op** (0 frames). Verificado con la forma correcta:
**278 frames/8 s**. `WlanHelper mode` lee vía WLAN API y NO refleja el modo
raw (siempre «managed») — no usar como veredicto.
**5. E2E wash con los args finales del comando** (`-i NPF_WIFI_ -c 11 -F`,
kill a 30 s) → **7 filas**: MIWIFI_APUM (WPS2.0 LOCKED), **sagemcomDEF0_Plus
(WPS2.0, Lck=No — desbloqueado)**, Livebox6-34D0 (LOCKED, además una fila
WPS1.0). `parse_wash` clava las 7.
**6. REAVER — veredicto honesto**: binario dual OK (wash==reaver, argv[0]),
cableado UI/commands OK, args ya correctos (-i/-b/-c/-vv/-L/-F), inj_warn
dispara… pero **`pcap_sendpacket` en NPF_WIFI_ da `rc=-1, err 203`** → **la
TX por Npcap sigue cerrada (Npcap #85)** → reaver/bully/aireplay **no pueden
asociar en Windows nativo**. La ruta para E2E real de WPS sigue siendo
**Kali live USB** (plan A cerrado 2026-09-25). En la app, reaver saldrá con el
aviso honesto de inyección y `success=false` (regla 2, sin fakes).
**7. Cierre**: managed restaurado (mismo set OID 8-byte con mode=4 → netsh
18 SSIDs). Verificación: lint ✅ vite ✅ `cargo build --release` **0 warnings**
✅ `cargo test --lib` **26 passed** (23+3 nuevos) / 8 ignored ✅ auditoría
**0 invokes huérfanos / 0 onclick reales** (los 12 «doVh*/doMonitor…» son
funciones inline de index.html) ✅.
Pendiente: E2E del usuario en la app (scan→USB→handshake) + decisión: intento
de reaver en Windows (solo contra AP propio) para ver el fallo honesto, o
directamente Kali live USB para WPS real.

## SESIÓN 2026-10-01 (cont. 4) - INTEGRACIÓN UI WPS/WASH VISUAL + AUDITORÍA COMPLETA

Petición: «integrar todo en la UI de manera visual y revisar la UI ante posibles
fallos/errores/incongruencias» → plan mostrado y aprobado, implementado íntegro.

**Auditoría UI (app.js + index.html) — fallos hallados:**
- F1: los resultados de wash solo iban a la consola (sin tabla ni selección).
- F2: `autoAttackBtn` referenciado en app.js:1298/1330 pero **no existía** en
  index.html (guard `if (btn)` → el texto del auto-ataque nunca se actualizaba).
- F3: placeholder `NPF_{GUID} o nombre` enseñaba el bug ya vivido (lo correcto:
  `\Device\NPF_WIFI_{GUID}`, dlt127) — fix en las 4 tarjetas WPS.
- F4: la tarjeta WPS PIN no mostraba el objetivo (dependía del dropdown sin
  indicador visual) — inconsistente con PBC/Pixie.
- F5 (medio-alto): varios handlers fijaban `currentAttackId` ANTES del
  busy-check de `invokeAttack` → si se lanzaba con ataque en curso, el
  `attack-completed` real no hacía match y el **lock de UI quedaba pegado**
  (los botones de la barra `scan-attack-bar` no están en `#tab-attack .atk-btn`).
- F6: barra de progreso de wash congelada al 10% durante el survey (30-60 s).
- F7: `doWashScan` sin busy-check (el resto lo tenía).
- F8: sin panel de resultado WPS (PIN/PSK solo en consola).
- Checks: 167 ids sin duplicados, 63 handlers todos definidos, 47 invokes con
  handler. Antes: 1 id faltante (F2).

**Implementación (index.html + src/app.js; commands.rs intacto):**
- Tabla wash `#wash-results`/`#wash-tbody` con columnas BSSID/CH/WPS/Lck/ESSID
  (solo `r.entries` reales; filas con `escAttr`, delegación de click) →
  `selectWashTarget()`: fusiona el AP en `lastNets` si netsh no lo vio, marca
  fila `selected`, `syncTargetList` + `selectNetwork` → objetivo sincronizado
  en TODAS las tarjetas (canal incluido, regla 8).
- `claimAttack(id)`: busy-check + set de `currentAttackId` atómicos; usado en
  capturePmkid/handshake/airodump/wpsBrute/wash/pbc/pixie; `quickAttack` con
  busy-check (antes limpiaba `_liveBuf` del ataque en curso); rama WSL de
  `wpsBrute` ahora con lock (`setAttackRunning`) y cleanup de id.
- Línea «Objetivo: SSID · BSSID · CH» en la tarjeta WPS PIN
  (`#wps-target-txt`, actualizada en `syncTargetEverywhere` + `clearSelection`).
- Panel `#wps-result`: parsea SOLO stdout real (`WPS PIN:`/`WPA PSK:` de reaver,
  `Pin is`/`Key is` de bully) en `invokeAttack` para `wps_*` + rama WSL; sin
  match → oculto (nunca un PIN inventado, regla 2).
- `renderWashRows(null|[])` estado «Encuestando…»; ticker de progreso con
  segundos (limpio en `finally`); `#autoAttackBtn` id añadido; placeholders WPS.

**Hallazgo E2E CRÍTICO (medido esta sesión):** wash **solo ve frames en modo
monitor**. Con el driver restaurado a managed (estado de cierre de la sesión
cont. 3): canal OID SET `ok=True` pero wash **0 filas/32 s** en ch11 (3 APs WPS
visibles para netsh). Tras `SET modo monitor + canal 11` (réplica exacta de
`activate_monitor`): **27 filas** — sagemcomDEF0_Plus (Lck=No), MIWIFI_APUM +
Livebox6-34D0 (LOCKED), MOVISTAR_2B4A_EXT, EPSON… Por eso el E2E de la mañana
funcionó (corrió en monitor) y el de esta tarde no (managed). Fix en UI:
`washModeHint()` — con 0 filas consulta `monitor_status` y si es `managed` pinta
hint accionable («Activa modo monitor y repite») en la tabla + log. NOTA: la
réplica PowerShell inicial falló por constantes decimales mal convertidas
(BIOCSETOID=2205728 y OIDs 218170120/218170165 son los correctos; err=1 =
constante incorrecta, NO rechazo del driver).

**Verificación:** `npm run lint` ✅ · `npx vite build` ✅ · `cargo build
--release` 0 warnings ✅ · `cargo test --lib` **26 passed**/8 ignored ✅ ·
auditoría 0 huérfanos (ids/onclick/invokes) ✅ · wash E2E real 27 filas ✅ ·
`UIFIPILL_1.0.0_x64-setup.exe` recompilado ✅ · radio **restaurada a managed**
al cierre (set OID mode=4 ok) ✅. Scripts de prueba en
`%TEMP%\opencode\set_mode.ps1` / `set_chan.ps1` (réplicas de los OIDs, fuera
del repo).

## SESIÓN 2026-10-01 (cont. 5) - INSTALADOR «SE ABRE Y SE CIERRA» — BINARIO PRINCIPAL

Petición: «lo instalé, se abre y se cierra».

**Diagnóstico (medido):** el NSIS instaló `send_assoc.exe` (224.768 B, la sonda
TX de laboratorio de `src/bin/send_assoc.rs`,21/09) en `%LOCALAPPDATA%\UIFIPILL\`
en vez de `uifipill.exe` (15,7 MB); el acceso directo lanzaba la sonda → sin
args → `.expect()` panic → «se abre y se cierra». Raíz: el paquete tiene 2 bins
y **`tauri-cli 2.11.2` con autobins y SIN `package.default-run` ni `[[bin]]`
explícitos marcaba `main` de forma no determinista** → `get_binaries()` solo
setea main si hay `default-run`, bins==1, o nombre==paquete en el array `bin`;
sin ninguna de las3, el builder empaquetó `send_assoc` (`installer.nsi`:
`MAINBINARYNAME "send_assoc"`). El installer del 18/08 funcionaba porque había
un único bin.

**Intento fallido nº1 (documentado):** añadir solo `"mainBinaryName":
"uifipill"` a `tauri.conf.json` — **insuficiente**: esa clave solo ejecuta
`rename_app()` (renombra el binario YA elegido), no cambia la selección. Medido:
el build renombró `send_assoc.exe`→`uifipill.exe` (224.768 B) **pisando** el real
(deps\uifipill.exe 15,69 MB intacto en `deps/`; patch warn `__TAURI_BUNDLE_TYPE
variable not found` presente = binario sin crate `tauri`). El instalador resultante
seguía roto.

**Fix definitivo:** `src-tauri/Cargo.toml` → `default-run = "uifipill"` +
`[[bin]]` explícitos (`uifipill`=src/main.rs, `send_assoc`=src/bin/…) →
`get_binaries()` garantía el main por 3 vías independientes. Verificado:
`MAINBINARYNAME "uifipill"` + `MAINBINARYSRCPATH`→uifipill.exe **15.692.288 B**,
patch warn desaparecido (binario con crate tauri).

**Reinstalación y prueba:** `/S` silencioso (installMode currentUser, sin UAC)
→ instalado `uifipill.exe` 15.692.288 B; diff byte a byte vs fuente = **3 bytes**
(el tag `__TAURI_BUNDLE_TYPE`: `NSS` en el instalado vs `UNK` en el fuente — el
CLI lo re-parchea tras bundlear; instalado = etiqueta NSIS correcta). App
lanzada: **viva, título `UIFIPILL`, responding** → «se abre y se cierra»
RESUELTO. Nota: el NSIS instala también `send_assoc.exe` (224 KB) al lado
(binario secundario del bundle) — inofensivo: nada lo lanza, los accesos
directos apuntan a `uifipill.exe`; registro `MainBinaryName` limpio (los
upgrades borran el bin viejo automática y una capa).

**Verificación:** `npm run lint` ✅ · vite (dentro del build) ✅ · `cargo build
--release` 0 warnings ✅ · `cargo test --lib` **26 passed**/8 ignored ✅ ·
instalado arranca y persiste ✅.

# PARTE 2 — ROADMAP MOTOR DUAL WSL2/KALI (origen: ROADMAP_WSL2.md)

Objetivo: motor dual. UI Tauri en Windows + ejecución RF en Kali-WSL2 (mismo RT3070
vía usbipd). Lo offline sigue nativo Windows. Cada fase tiene criterio de "hecho"
verificable; no se avanza sin marcarlo. Estado actual arriba del todo.

## ESTADO ACTUAL (2026-09-29 — decisión RF cerrada)
- **DECISIÓN DE RF CERRADA (2026-09-25)**: plan (d) **Kali live USB** como vía
  para RF activa/RX (rt2800usb hace el boot frío completo en kernel) + **TX por
  USB crudo en Windows** (hito 2026-09-25: beacons "TXTEST" al aire con
  `rt3070_tx`, deauth/beacon por WinUSB posible). El motor dual WSL2/VirtualHere
  queda como opción secundaria: usbipd sigue con RX muerta (3 mediciones:
  09-14/15/18), VirtualHere bloqueado por licencia (API Timeout).
- RX por USB crudo WinUSB: **CONFIRMADA 2026-10-01** (runs 8/9: 524 frames/15 s;
  scan1/2: 247 y 225 frames/30 s, 10-12 redes). La raíz del "BBP mudo" era el
  port del canal (rf53xx→RFCSR8 en vez de rf3xxx→RFCSR2) + RFCSR1 PLL_PD —
  ver §4/§5. Sniffer USBPcap queda como cotejo histórico, no como única vía.
- Pendiente de infra: ~~device netr28ux en ProblemCode 31/56 post-HVCI~~ →
  **SANEADO 2026-09-30** (raíz: faltaba `C:\Windows\INF\netr28ux.inf`;
  ver bitácora §1 sesión 2026-09-30). USBPcap 1.5.4.0 instalado (sniffer RX
  pendiente).
- Fases 0-6b del motor dual: completas en código (ver secciones históricas
  debajo); Fase 7 (cierre) pendiente de decisión RF — ya tomada arriba.
- Detalle de las mediciones del puente: ESTADO ACTUAL histórico 2026-09-18.

## ESTADO ACTUAL (histórico 2026-09-18 — re-medición completa del puente)
- **Re-verificado hoy end-to-end** (con la app cerrada, por CLI): kali-linux Running,
  usbipd attach 1-10 OK, `wlan0` (00:c0:ca:59:a7:d8) monitor OK, `iw wlan0 set channel 11`
  OK, hcxdumptool 7.0.0 y airodump-ng ejecutan… **RX sigue MUERTA: 0 pkts en 20 s en CH11**
  (mismo resultado que 2026-09-14). El bloqueador usbipd-RX persiste con kernel stock y
  sin cambios de entorno; nada apunta a código — es transporte.
- **wsl_health (nuevo comando + botón «🩺 Salud RF»)**: diagnostica en un tirón distro →
  wlan0 → monitor → canal → probe RX 6 s con tcpdump (solo lectura, sin TX). Devuelve
  veredicto por paso; distingue «sin attach» de «attach con radio muerta». Con este check,
  cualquier «no funciona el WSL» queda caracterizado en un clic.
- **sudo -n verificado**: la whitelist `/etc/sudoers.d/uifipill-lab` funciona tal cual
  (probado: `sudo -n timeout -s INT 3 airodump-ng` exit 0). Las recetas no necesitan cambios.
- **Fix crítico aparte (commands.rs/tools_detect.rs)**: el instalador NSIS pone tools en
  `<exe>\_up_\tools\` y `require_bin`/`install_dirs` no lo miraban → en el instalado
  «no ataca / no verbose» porque NINGÚN binario se resolvía (reaver, wash, aircrack).
  Ahora: `_up_/tools` en install_dirs + `resolve_bin_abs` en run_bin/run_bin_bg (ruta
  absoluta; una ruta desnua no arranca aunque exista). Verificado instalando y arrancando.
- **UI (misma ronda)**: lock de ataque (barra roja + botones bloqueados + ■ Detener
  siempre activo), verbose completo (sin recorte de 80 líneas; chunks 8 KB/64 eventos),
  tab Ataque reordenado 1→10 con objetivo arriba y Acceso al final.

## ESTADO ACTUAL (histórico 2026-09-15)
- Fases 0-5 completas. Fase 6 PARCIAL + Fase 6b NUEVA (UI VirtualHere automatizada,
  ver abajo). Post-reboot: Kali OK con kernel `bzImage-84test` (6.6.84.1+, rt2800usb
  + rt2870 builtin + USBIP_VHCI verificados), RT3070 en busid **1-7** (era 1-5),
  servidor VH 4.8.8 como servicio SYSTEM en 7575.
- Veredicto VirtualHere 2026-09-15: LIST ve el RT3070 (`802.11 n WLAN`), pero USE
  → `FAILED: API Timeout`. Causa oficial del desarrollador (foro #4683): el cliente
  consola EXIGE licencia de pago; con trial el USE se rechaza por diseño. NO es
  problema de elevación/driver/red (puerto accesible desde Kali, vhci cargado).
- Decisión pendiente (única tarea abierta del proyecto): cómo conseguir RF real.
  **[CERRADA 2026-09-25 — ver ESTADO ACTUAL: Kali live USB + TX crudo Windows]**

| Opción | Coste | Esfuerzo | Prob. de RF real | Notas |
|---|---|---|---|---|
| (a) Licencia VirtualHere | ~49 USD | Muy bajo | Alta | El server ya LISTa el RT3070; lo único rechazado es el USE. Requiere RF-check después. |
| (b) Cliente GUI Linux bajo WSLg (trial) | 0 | Bajo | Media | WSLg ya está; el cliente GUI puede tener otra ruta de licencia. Intento de ~15 min. |
| (c) Depurar RX en usbipd | 0 | Alto | Baja-media | 0 pkts en Kali donde Windows ve 370 pkts/15 s; sin fix conocido en usbipd-win 5.3.0 + vhci. |
| (d) Kali bare-metal (USB live/persistente) | 0 | Medio | Muy alta | Único camino sin transporte USB virtual; el RT3070 ya funciona con rt2800usb. |

Recomendación: (d) como plan A para RF fiable hoy y (b) como intento gratuito de
15 minutos antes de descartar del todo VirtualHere.

- Contexto 2026-09-14: Fase 6 PARCIAL con 5 recetas + kill (smoke OK, RF muerta en
  Kali: RX 0 pkts sobre usbipd; Windows ve 370 pkts/15 s) + VM WSL2 reciclada por el
  host cada ~5–60 min (cualquier kernel). Kernels custom 6.6.123/6.6.84.1+ listos en
  `%USERPROFILE%\wsl-kernel`.

## Fase 0 — Base Windows [HECHO 2026-09-14]
- Scan netsh, monitor on/off Npcap, captura nativa wpcap (392 pkts), conversor,
  hashcat 7.1.2 + rockyou, keygen Comtrend/Thomson real, reaver/wash recompilados.
- Límites medidos: sin inyección (err 31), sin cambio de canal (las 3 vías OID fallan).

## Fase 1 — Plataforma WSL2 [HECHO 2026-09-14]
- La plataforma YA estaba presente: WSL 2.5.6.0 + kernel 6.6.84.1-1, Win11 b26200.
  Solo faltó `wsl --set-default-version 2` (hecho, sin admin, sin reinicio).
- Sin distros instaladas. HECHO verificado con `wsl --version`.

## Fase 2 — usbipd-win [HECHO 2026-09-14]
- usbipd-win 5.3.0 vía winget (ya estaba, actualizado OK).
- `usbipd list`: RT3070 `148f:3070` en busid **1-5, estado Shared**. HECHO.

## Fase 3 — Kali + herramientas [HECHO 2026-09-14]
- Kali Rolling 2026.2 instalada (`wsl --install -d kali-linux`, usuario `kali` +
  `/etc/wsl.conf [user] default=kali`); `apt install` de hcxdumptool hcxtools
  aircrack-ng reaver bully pixiewps mdk4 hostapd dnsmasq iw wireless-tools usbutils.
- HECHO verificado: hcxdumptool 7.0.0, hcxpcapngtool 7.1.0, reaver/wash 1.6.6,
  aireplay-ng 1.7, bully 1.4, pixiewps 1.4, mdk4 4.2, iw 6.17 responden en Kali.

## Fase 4 — Handoff USB + prueba de fuego [CORREGIDA 2026-09-14: veredicto parcial]
- attach 1-5 → `iw dev` mostró wlan0 (rt2800usb, RT3070 rev 0201) → `iw info`
  reportó canal 1→6→1 → detach → Windows lo ve (managed CH11).
- ⚠️ CORRECCIÓN: el "cambio de canal" era solo estado SOFTWARE. Prueba RF
  posterior (`survey dump` en 2462 MHz con 370 pkts/15 s confirmados en Windows
  en el mismo momento: busy 0 ms en 15–55 s + tcpdump 0 pkts) demuestra que la
  radio NO sintoniza ni recibe por usbip. El criterio HECHO original no se
  cumplió en RF; queda como bloqueador de Fase 6.
- Requirió KERNEL PERSONALIZADO (`%USERPROFILE%\wsl-kernel\bzImage-uifipill-fw2`,
  registrado en `~\.wslconfig`): el kernel stock de Microsoft solo trae WLAN
  Intel/RSI (`CONFIG_RT2X00 is not set`); compilado `linux-msft-wsl-6.6.y` +
  RT2X00/RT2500USB/RT73USB/RT2800USB (+variantes RT33XX/35XX/3573/53XX/55XX),
  MT7601U/MT76x0U/MT76x2U/MT7663U, ATH9K_HTC, RTL8XXXU; fuente en `~/wsl2-kernel`
  dentro de Kali; módulos en `/lib/modules/6.6.123.2-microsoft-standard-WSL2+`.
- Quirk documentado: el firmware-loader del kernel NO lee `/lib/firmware` en este
  WSL2 (`Direct firmware load … failed -2` hasta con `regulatory.db` presente,
  también en kernel stock). Solución: `CONFIG_EXTRA_FIRMWARE="rt2870.bin"` con
  `CONFIG_EXTRA_FIRMWARE_DIR="/lib/firmware"` (el árbol MSFT lo genera en
  `drivers/base/firmware_loader/builtin/`): dmesg `Firmware detected v0.36`,
  `ip link set wlan0 up` OK. Ojo al reiniciar: acceder a `\\wsl$` arranca la VM
  y congela el kernel en memoria — cambiar `.wslconfig` exige `wsl --shutdown`
  ANTES de arrancar, y copiar el bzImage a nombre NUEVO (el viejo queda
  bloqueado mientras corre).

## Fase 5 — Puente en el backend [HECHO 2026-09-14]
- Comandos nuevos en `src-tauri/src/wsl.rs`: `wsl_exec` (puente genérico
  `wsl -d kali-linux …` sin shell intermedia, timeout 5–600 s),
  `wsl_attach`/`wsl_detach` (usbipd, busid validado `^[\d-]+$`),
  `wsl_from_win`/`wsl_to_win` (`D:\…` ↔ `/mnt/d/…`); distro validada
  alfanumérica (anti-inyección). UI: tarjeta «WSL2/Kali — puente RF» en el tab
  de ataque (busid + comando + 3 botones, log a consola).
- HECHO verificado: `cargo test` 20 passed (3 tests nuevos de traducción/
  validadores), `wsl -d kali-linux -- echo ok` → ok, attach/detach 1-5
  verificados en Fase 4, `npm run lint` + `vite build` OK.

## Fase 6 — Recetas de ataque v1 [PARCIAL 2026-09-14: código + smoke sin RF]
- Backend en `wsl.rs` + UI (BSSID/canal + 6 botones): `wsl_pmkid_capture`
  (hcxdumptool `-c Na --rds=1` → .pcapng en `%TEMP%` vía `/mnt/c`), `wsl_airodump`
  (CSV en staging), `wsl_wash`, `wsl_deauth` (fija canal con `iw` + aireplay),
  `wsl_reaver` (`-K 1` opcional), `wsl_kill` (`pkill -INT`; el timeout del
  cliente `wsl` NO mata procesos en la VM). Todo con `sudo -n` (NOPASSWD en
  `/etc/sudoers.d/uifipill-lab`) + `timeout -s INT` en-VM. `cargo test` 21 OK.
- Smoke en vivo 2026-09-14 (sin RF útil): airodump 20 s → CSV de 236 B
  (cabeceras) en TEMP ✅ staging; hcxdumptool 30 s → `5 ERROR(s) (broken
  driver)`, `0 Packet(s) captured by kernel`, pcapng 488 B ✅; wash 20 s →
  tabla vacía ✅; `aireplay --test` → probes sin error TX, `No Answer` ✅;
  reaver 35 s vs BSSID dummy → `Waiting for beacon` + salida limpia ✅.
- 🛑 BLOQUEADOR RF: RX muerta sobre usbipd-win 5.3.0 + vhci (última versión;
  sin fix disponible). Evidencia: tcpdump/airodump/survey/tcpdump 0 pkts donde
  Windows captura 370 pkts/15 s con el mismo HW; hcxdumptool confirma
  `0 captured by kernel`. TX aceptado por el driver pero sin verificación RF.
  Pendiente de: (a) datos del AP propio para pruebas dirigidas cuando haya RF,
  (b) probar otro adaptador (AR9271/RTL8812AU) u otro transporte (VirtualHere).
- 🛑 BLOQUEADOR ESTABILIDAD: la VM WSL2 se RECICLA sola (DELETE/CREATE en
  Hyper-V) cada ~5–60 min con kernel stock Y custom, en idle Y con USB:
  exonerado el kernel custom. Host Canary b26200 + WSL 2.5.6.0. Metodología
  hasta resolver: ráfagas cortas (attach→test 2–3 min→detach) + `boot_id` check.
- HECHO por receta (prueba en vivo contra AP propio) queda PENDIENTE de RF.
- Objetivo lab designado 2026-09-14 (usuario: «la fory»): SSID `La Fory 3H`,
  BSSID `d8:a7:56:31:bf:13`, canal 6, WPA2-Personal CCMP, señal 100%
  (visto por netsh vía RT3070). Sin clave PSK todavía: test de asociación
  managed (wpa_supplicant, discriminaría monitor-vs-transporte) pendiente de
  autorización + clave. Sin segundo adaptador USB (solo «tarjeta normal»).
- Transporte alternativo 2026-09-14: descartado `usbip-win2` (es CLIENTE,
  dirección contraria). Probado VirtualHere trial (server Win userspace +
  cliente Kali estático): el server lista los 4 USB incluido `802.11 n WLAN`,
  pero `USE` falla con `API Timeout` — el server sin admin no puede quitarle
  el dispositivo al driver Ralink. Intento RunAs dejó zombie elevado (PID 8252,
  sin listener en 7575, inmatable sin admin). `vhusbdwinw64.exe` copiado al
  escritorio. Pendiente del usuario: reboot + ejecutar server como admin.
- Intento de deauth 2026-09-14 contra `La Fory 3H` (CH6, x20/x5/x3, autorizado):
  `aireplay-ng --deauth` se CUELGA en «Waiting for beacon» sin imprimir nada
  (stdout bloqueado por pipe; solo visible al quitar `timeout`); matado por
  timeout/pkill. Cero tramas verificables enviadas. Confirma RX muerta; TX
  queda no-confirmado. Adaptador devuelto a Windows.

## Fase 6b — VirtualHere en la UI [HECHO 2026-09-15: código; RF BLOQUEADA por licencia]
- Backend `wsl.rs` + tarjeta UI «VirtualHere — USB a Kali»: `vh_server_check`
  (TCP 7575 sin WSL), `vh_provision` (wget cliente 6.0.2 a `~/`), `vh_daemon`
  (`modprobe vhci-hcd` + `-n` vía `wsl -u root`, inputs validados), `vh_hub_add`,
  `vh_list`, `vh_use` (con hint de licencia si `API Timeout`), `vh_stop`,
  `vh_rf_check` (`iw info` + `ip up` + `tcpdump -c 30` + firmware + veredicto
  VIVA/MUERTA). `cargo test` 22 OK (nuevo `vh_validators`), build sin warnings,
  `npm run lint` + `vite build` OK. Orden en UI: Servidor→Provision→Demonio→
  +Hub→List→USE→RF-check→STOP.
- Evidencia 2026-09-15 (manual, mismo flujo que la UI): servidor 4.8.8 (última
  versión Windows) como servicio SYSTEM, puerto accesible desde Kali
  (172.29.80.1:7575), vhci-hcd cargado, demonio OK, LIST muestra
  `802.11 n WLAN (DESKTOP-33B27GN.8)`; USE → `FAILED: API Timeout 3 sec`.
  Respuesta oficial del desarrollador (foro #4683): el cliente consola exige
  licencia de pago. HECHO de RF queda pendiente de (a)/(b).
- Nota kernel: `.wslconfig` apunta a `bzImage-84test` (6.6.84.1+) que YA trae
  rt2800usb + `CONFIG_EXTRA_FIRMWARE="rt2870.bin"` + USBIP_VHCI_HCD (verificado
  `zgrep`+`modinfo`); equivale al `fw2` citado en Fase 4.

## Fase 7 — Cierre [PENDIENTE]
- Tests, AGENTS.md final, veredicto funcional dual-engine.

## Bitácora (cada avance añade línea: fecha + qué + evidencia + siguiente)
- 2026-09-14: creado el plan. Siguiente: Fase 1.
- 2026-09-14: Fase 3 HECHO (Kali 2026.2 + 12 herramientas RF verificadas).
- 2026-09-14: Fase 4 HECHO (kernel custom 6.6.123.2+ con rt2800usb; canal 1→6→1
  en Kali; detach → Windows managed CH11). Siguiente: Fase 5 (puente backend).
- 2026-09-14: Fase 5 HECHO (`wsl.rs` + tarjeta UI; 20 tests OK; echo ok en Kali).
  Siguiente: Fase 6 (recetas de ataque v1).
- 2026-09-14: Fase 6 PARCIAL (recetas + UI + 21 tests; smoke 5/5 herramientas;
  staging Kali→TEMP OK; RF bloqueada: RX 0 + VM recicla sola). Siguiente:
  datos del AP propio y/o decisión sobre adaptador alternativo.
- 2026-09-15: reboot OK (kernel 84test + busid 1-7 + VH service). Fase 6b HECHO
  en código+UI (8 comandos vh_*, 22 tests, build limpio); USE bloqueado por
  licencia (foro #4683). Siguiente: decisión (a) licencia / (b) GUI+WSLg /
  (c) debug RX usbipd / (d) Kali bare-metal.
- 2026-09-15 (revisión pre-uso): flujo VH re-ejecutado comando a comando como la
  UI (provision→daemon idempotente→hub→list→use→stop→rf sin wlan0): 8 paths de
  binarios OK, USE sale por stdout (el hint de licencia dispara), STOP sin USE
  avisa honesto, daemon arranca limpio en 2.º intento. Añadidos `tcp_connect_once`
  testeable (23 tests) y mensaje STOP honesto. JS inline validado (bloque clásico
  OK; el `import` es del bridge `type=module` validado por vite). Kali dejado
  limpio (demonio de prueba muerto).
- 2026-09-15 (revisión + limpieza de repo): el trabajo de las Fases 0-6b se
  commiteó en 3 bloques (`7891cbb` Windows/Npcap+keygen, `08eea7e` motor dual
  WSL2/VH + UI, `a3b42c4` docs) y se pusheó a `origin/master`; instalador NSIS
  regenerado (7.117.995 B, 08:35). `.gitignore` arreglado: los clones anidados ya
  no se añaden como gitlinks. (Nota posterior: el sistema de build de reaver
  `tools/build/`, incluido `patches/reaver-win-port.patch`, se eliminó del repo
  en una limpieza posterior — ver §1 de este documento.) Limpieza: 5 ramas
  sin commits propios y 4 worktrees scratch `D:\PROJECTS\UIFIPILL-wt-*` eliminados;
  stash antiguo (base divergente `0e07795`, con WIP abandonado de mdk4/stream_*
  que NO está en master) respaldado en
  `D:\PROJECTS\uifipill-stash-backup-2026-09-15.patch` y dropeado. Estado:
  `cargo test` 23 OK / 5 ignored, build sin warnings, 0 invokes sin handler,
  instalador al día. Siguiente: elegir opción de RF de la matriz de arriba.
- 2026-09-25: decisión RF cerrada — Kali live USB (plan d) + TX por USB crudo
  Windows (beacons "TXTEST" al aire); RX-WinUSB cerrada (BBP mudo). Siguiente:
  integrar TX en la app + sniffer USBPcap (vía RX investigable).
- 2026-09-28/29: reset PC → HVCI bloqueó netr28ux (ProblemCode 31/56); HVCI
  desactivado, device aún por sanear (`fix_restart_dev.ps1` + reboot). USBPcap
  instalado. Siguiente: sanear device → hot_rebind → TX operativo en la app.


# PARTE 3 — INGENIERÍA INVERSA DE netr28ux.sys (origen: DRIVER_RE.md)

> Bitácora viva. Objetivo: hacer que el driver MediaTek del RT3070 acepte el cambio
> de canal en modo monitor (y a medio plazo, TX) — igualar lo que `rt2800usb`
> hace en Linux. Lab-only.

## 0. Objetivo de análisis

| Dato | Valor |
|---|---|
| Binario | `C:\Windows\System32\drivers\netr28ux.sys` |
| Copia de trabajo | `driver_re/netr28ux.sys` (2.244.952 B) |
| Versión | 5.01.25.0 (MediaTek/Ralink, 2015), x64, NDIS 6.x, WDF |
| Desensamblador | `objdump -d` (Cygwin, AT&T syntax) → `driver_re/netr28ux.asm` (~500k líneas) |
| Imagen base | `0x140000000` (formato: `140xxxxxx: VA`) |

## 1. Hallazgos fase 1 (mapeo del despachador de OIDs) — 2026-09-20

### 1.1 Despachador principal: función en `140215788`

Es el switch gigante de OIDs SET. Estructura (VA):

```
140216070  cmp/sub por rangos de OID:
           0x0D01031A, 0x0D01031B(?), 0x0D01031C, 0x0D01031D
140216260  rango 0x0D010327 / 0x0D010335 / 0x0D010342 / 0x0D01034B-4C
           ├─ 0x0D010335 (OID_DOT11_CURRENT_CHANNEL) → 1402163A1
           ├─ 0x0D010336 (frecuencia) — NO aparece como cmp literal;
           │   se alcanza por aritmética (sub/dec) en el rango
           └─ 0x0D01034B/4C → cae en 1402162A6 (≠ caso canal)
140216535  rango vendor 0xE010178, 0xE010179(?)... y default → 1402192A0
           (default = STATUS_INVALID_DEVICE_REQUEST)
```

**Dato crítico: el OID de canal NO es un stub de "no soportado".** Tiene
handler real con validación y almacenamiento.

### 1.2 Handler del OID canal — dos caminos según `opState`

Entrada en `1402163A1` (validación básica: requiere algo == 1 en
`0x28(%r13)` = InformationBuffer length, si no → `0xC0000184`
STATUS_INVALID_PARAMETER... err. `0xC0232002` en el otro caso):

```
1402163D5   testl $0x20000, 0x32D468(%rsi)      ; opState bit 17
            ├── bit SET (modo ExtENSIBLE/ExtSTA AP?) → 140216413:
            │     registra evento y devuelve 0xC0232002
            │     (STATUS_INVALID_DEVICE_STATE)  ← el rechazo que ve WlanHelper
            └── bit CLEAR → 140216420:
                  14021642E   cmp %r12d, 0x230(%rbp)   ; valida longitud buffer
                  14021643B   mov (%r14), %ebx         ; lee canal pedido
                  14021645B   mov 0x68(%rsp), %rax
                  140216460   mov %r12d, (%rax)        ; bytesWritten
                  ; ... logs WPP 0x258 ...
                  → STATUS_SUCCESS (por el camino de 140219F8F)
```

**Por eso Npcap/WlanHelper "acepta" el OID pero la radio no se mueve:**
el handler de canal en modo no-Extensible SOLO ALMACENA el valor pedido
(no programa el PHY en este handler). La programación real del canal debe
ocurrir después, en la transición de estado del adaptador que consume ese
valor almacenado. En modo Extensible directamente rechaza.

> **SUPERADO (mismo día — ver §4 «Descubrimiento posterior»)**: el camino
> no-Extensible SÍ llamaba a SwitchChannel(ctx, canal, 0); el fallo real era
> el gate bit17 DENTRO de SwitchChannel (2b.2). Las conclusiones §1.2/§2 de
> esta fase quedan corregidas por ese descubrimiento; se conservan por
> trazabilidad.

### 1.3 Estado del adaptador (offsets sobre contexto en %rsi/%rbx)

| Offset | Significado |
|---|---|
| `+0x32D468` | `opState` (bitmask). Bit 17 (0x20000) = "Extensible/extSTA activo" — gate del OID canal. Bit 14 (`0x4000`, btsl/btrl) aparece como otro estado de operación |
| `+0x332390` | flags de "opzone/halted" — si bit (== `%cl`=1) → `0xC0232000` (STATUS_INVALID_DEVICE_STATE otro) |
| `+0x230` (`%rbp`) | longitud del InformationBuffer |
| `+0xCDF2` | WORD: se escribe `0x92B`-capped (función `140216096`); valor con tope 0x92B, `shl $8` etc. (probable PHY/txpower relacionado) |
| `+0xCD80` | WORD: canal anterior pedido (función `1402161CF`, OID 0x0D01031C, escribe `mov %r9w,0xCD80(%rsi)`) |
| `+0x3143E9` | BYTE: valor OID 0x0D01031D (función `14021613D`) |

### 1.4 OIDs que sí se muestran como literales en el binario

```
0x0D010308, 0x0D01030B, 0x0D010310, 0x0D010311   (1400406E7..140040BBD)
0x0D01031B, 0x0D01031C, 0x0D010326, 0x0D010327   (1400435AE..14004396A)
0x0D010338, 0x0D01033D, 0x0D010342, 0x0D010344   (1400439F0..140043A9B)
0x0D01034B, 0x0D01034C, 0x0D01034B-4C default
0x0D010704                                       (140216260)
0xFF710335                                       (14004C594) — vendor Ralink
```

Nota: `0x0D010336` (frecuencia) no aparece como literal de 4 bytes con
`cmp` — se procesa por aritmética de rangos. El OID canal sí aparece
explícito: `cmp $0xd010335, %ecx` en `14021627F` → `1402163A1`.

## 2. Estado / conclusiones

> **NOTA (2026-09-20 posterior)**: las conclusiones 1-2 de abajo fueron
> SUPERADAS por 2b/§4 — el handler YA llamaba a SwitchChannel; el bloqueo era
> el gate bit17 interno. Se conservan por trazabilidad.

1. **El driver no "no soporta" el canal** — lo acepta y lo almacena, pero
   **no lo propaga al hardware** en modo no-Extensible (el modo en el que
   Npcap trabaja con el adaptador en monitor). Hay que encontrar el
   consumidor de ese valor almacenado y ver por qué no programa el PHY.
2. Hipótesis de trabajo: la programación del canal al PHY ocurre en la
   **máquina de estados de conexión/scan** (que nunca corre en monitor) o
   vía **comandos USB al firmware** que solo se disparan desde esa
   transición. El valor queda en memoria pero nunca llega al chip.
3. Bit 17 de `opState` (0x20000): si se pudiese SETEAR artificialmente,
   el handler pasaría por el otro camino (Extensible) — pero ese camino
   rechaza con 0xC0232002, así que no es la vía. La vía es la contraria:
   encontrar el consumidor del canal almacenado.

## 2b. Fase 2 parcial — rutina de sintonía RF identificada (2026-09-20)

La llamada misteriosa del handler de canal (`14006a7f4`) **ES la rutina de
cambio de canal real** (63 call sites — es la central de sintonía).

### 2b.1 `14006a7f4` — SwitchChannel(ctx, channel en dl, flag en r8b)

1. **Gates de entrada** (si fallan → epígono `14006e000`, sin tocar RF):
   - `0x3335d1(ctx) != 0 || 0x3335d4(ctx) == 0` → sale
   - `opState bit17 (0x20000)` SET → **sale** (mismo gate que el OID)
   - `0xCD48(ctx)>>16 == 0x76xx` (otros chips MT7601/7612/…) → sale (RT3070 sigue)
2. **Programación PHY real** (helpers identificados):
   - `1400b5690(ctx, dl=reg, r8b=val)` → **escritura de registro RF**
   - `1400a6d78(ctx, dl=reg, r8=ptr)` → **lectura BBP**
   - `1400a7020(ctx, dl=reg, r8b=val)` → **escritura BBP**
   - `1400b0728(ctx, edx=cmdId, r8=ptr)` → **envío de comando USB al firmware**
     (id 0x5D4 en el final de la sintonía; 0x1344 en el OID 0x0D01031B)
3. Tabla de frecuencias implícita en el código (cmp %dl con umbrales
   0x0E = canal 14 separado; ramales por canal/banda).

### 2b.2 Conclusión del fallo

El driver **SÍ sabe sintonizar** (la rutina existe y es la misma que usa
el scan/conexión). El problema es de **gating**: cuando el adaptador pasa
a modo monitor/extensible, `opState` bit17 se pone a 1 y:

- el handler del OID canal rechaza con `0xC0232002`, y
- aunque llegara a llamarse, SwitchChannel **también** se corta en su
  gate `14006a8c2` (bit17) → doble bloqueo.

Quién SETEA el bit17: `btsl $0x11, 0x32D468` en `140020e90` y `14008b5f8`
(esas dos rutinas son la transición a modo extensible/monitor).

## 3. Plan de parche (fases)

### Fase 2 — Encontrar el consumidor del canal (RESUELTA: es SwitchChannel, ver 2b)
- Rastrear el offset donde el handler almacena el canal pedido (el store
  final del handler de canal en el camino no-Extensible) y buscar todos
  los readers de ese offset.
- Identificar la rutina de "sintonizar PHY" (probablemente cerca de
  comandos USB con el canal como parámetro: buscar constantes 2412/5000
  kHz·5 → `0x96C` para 2412 MHz, o la tabla de canales del RT3070).
- Verificar qué dispara esa rutina (scan start? OID 0x0D010311? media
  connect?) y por qué nunca corre en monitor.

### Fase 3 — Parche binario (EJECUTADA — ver §4)

El parche mínimo y más prometedor ahora que conocemos los gates:

- **NOPear el gate del bit17 en SwitchChannel** (`14006a8c2`, 6 bytes:
  `f7 86 68 d4 32 00 00 00 02 00` + `jne` → NOP del test y del salto).
  Con eso SwitchChannel sintoniza aunque estemos en modo extensible.
- **Redirigir el handler del OID canal** (`140216420`, camino no-Extensible)
  para que tras validar llame a `14006a7f4(ctx, canal, 0)` en vez de solo
  loguear y devolver SUCCESS. Alternativa más simple: parchear el camino
  Extensible (`140216413`) para que en lugar de devolver `0xC0232002`
  caiga en el camino no-Extensible (cambiar el `je 140216420` por
  incondicional), y ahí añadir la llamada a SwitchChannel.
- También hay que revisar el gate del bit14 (btsl/btrl `$0xe`) por si la
  transición monitor usa otro bit que bloquee más rutas.
- Firma: `bcdedit /set testsigning on` + Secure Boot off + cert propio
  (`signtool`). Probar SIEMPRE en VM snapshot primero.
- Herramientas: parcheo con Python (pefile para VA↔file offset),
  verificación con objdump del binario parcheado, re-firma con signtool.

## 4. Parche construido y firmado (2026-09-20)

Artefactos en `driver_re/`:

| Fichero | Qué es |
|---|---|
| `netr28ux_patched.sys` | binario con los 3 parches (17 bytes), firma vieja aún dentro |
| `netr28ux_patched_clean.sys` | **ENTREGABLE**: firma vieja eliminada, overlay truncado, checksum recalculado (0x22DC3C), firmado Authenticode SHA-256 con `CN=UIFIPILL Lab Test` |
| `labtest.cer` | cert self-signed del laboratorio (makecert, EKU CodeSigning) — clave privada en el store personal del usuario |
| `deploy_driver.ps1` | instalación admin: backup del original → confiar cert → testsigning on → copiar a System32\drivers → re-scan. Requiere reinicio |
| `restore_driver.ps1` | rollback completo (driver original + testsigning off) |

### Parches aplicados (verificados con objdump)

1. **P1a** (`14006a8c2`, fileoff `0x69CC2`, 10 B): `testl $0x20000,0x32D468(%r14)` → NOP×10
2. **P1a-fix** (`14006a8cc`, fileoff `0x69CCC`, 1 B): byte alto del imm32 → NOP (la instrucción era de 11 B, no 10)
3. **P1b** (`14006a8cd`, fileoff `0x69CCD`, 6 B): `jne 0x14006e000` → NOP×6
4. **P2** (`1402163df`, fileoff `0x20D7DF`, 2 B): `je 0x140216420` → `jmp 0x140216420`

Resultado: SwitchChannel ya no se corta por el bit17 y el OID canal
acepta el SET también en modo ExtSTA, delegando en SwitchChannel.

### Descubrimiento posterior que simplifica el parche

Releyendo `14021643E–44F` (camino no-Extensible del OID): **el handler YA
llamaba a SwitchChannel(ctx, canal, 0)** — el OID "aceptaba" pero la
rutina de sintonía se auto-bloqueaba con su propio gate bit17. Por eso
P1 (NOP del gate en SwitchChannel) es el fix principal y P2 solo amplía
el OID al modo ExtSTA.

### Hashes

- original: sha256 `ADE38351A626DC0A6BCDE1D09B214C94…` (2.244.952 B)
- parcheado+firmado: sha256 `1EC0E62E580AADD54CA28508198B8A52…` pre-firma
  (2.228.736 B tras truncar overlay; la firma Authenticode se añade después)

### Estado y siguientes pasos

- [x] Parche binario construido y verificado por desensamblado
- [x] Cert de testsigning creado y usado para firmar
- [x] Scripts deploy/restore escritos
- [x] **Despliegue ejecutado** (2026-09-20, `run_deploy.cmd` vía UAC
  aceptado): backup creado (`netr28ux.sys.orig`), cert en
  TrustedPublisher+Root, testsigning ON, y reemplazo del .sys AGENDADO en
  `PendingFileRenameOperations` (el driver estaba RUNNING; se sustituye
  en el arranque antes de cargarlo) — verificar tras reiniciar
- [x] Fix redeploy: el primer reemplazo falló (rutas escritas como `??`
  en vez de `\??\` NT); `redeploy_pending.ps1` re-agendó con staging
  (log en `redeploy_log.txt`) — tras reinicio, hash en disco =
  `d2c7cf43…` (PARCHREADO) y servicio netr28ux RUNNING.
- [x] **VERIFICACIÓN POST-REINICIO — HITO (2026-09-21): la radio SÍ
  cambia de canal en monitor.** `lab_monitor_cycle` ok (monitor activa,
  restore ok). Prueba de canal REAL por captura + beacons (DS Parameter
  Set): pidiendo CH1 solo aparecen APs del canal 1 (Livebox7-A58C,
  MiFibra-A4DA, NOVAHOME…); pidiendo CH6 solo APs del canal 6 (La Fory
  3H, MOVISTAR_2B4A_EXT, Vodafone-7D84). El SET del OID 0x0D010335 se
  acepta sin error (antes 0xC0232002): el NOP del gate bit17 funciona.
- [x] Residual documentado: el GET del OID canal devuelve valor
  CACHEADO (siempre el canal previo) — no fiable para readback.
  `set_monitor_channel` ahora reintenta y da éxito si el SET fue
  aceptado (nota de readback no fiable en el mensaje). El OID frecuencia
  (0x0D010336) sigue rechazado (0xE0010017): irrelevante.
- [ ] Si el canal funciona, medir TX (check_injection_capability) — puede
  seguir bloqueada por Npcap #85 (capa independiente del driver)
- [ ] Decidir: mantener modo de prueba (testsigning) mientras dure el
  driver parcheado; rollback con `restore_driver.ps1` +
  `bcdedit /set testsigning off` cuando ya no se necesite

## 5. Fase 3 — Ruta TX (inyección) — reconocimiento 2026-09-21

Objetivo: eliminar el error 31 (STATUS_UNSUCCESSFUL → ERROR_GEN_FAILURE) de
`pcap_sendpacket` en modo monitor. Recon estático completo, SIN parchear todavía
(la ruta TX corre en kernel: parche a ciegas = BSOD).

### 5.1 Mapa de la ruta de envío

| Componente | VA | Notas |
|---|---|---|
| `SendNetBufferListsHandler` | `0x140040e8c` | Slot +0x40 de la tabla NDIS (rev 2, 0x98 B). Solo ENCOLA en `0x21f0/0x21f8/0x2200` con `lock decl 0x332244` |
| Send engine (dequeue) | `0x14000f98f…0x1400108f2` | Colas por prioridad `0x3312e8/f0/f8` (8 slots × 0x80 B); lee `0x21f0/0x21f8/0x2200` en `0x140010b13` |
| Validador por paquete | `0x140015d18` | Args: adapter(rcx), tipo(dl=0xa), idx(r8b), contador_paquete(r9d). Devuelve STATUS en eax |
| Descarte por BSS-mismatch | `0x140010656…06c5` | `pkt->0x88->0x20 != adapter->0x172d98->0x8` → `0xc0000001` en `0x8c(%rdx)` + complete vía `0x1401afbb8` (NdisMSendNetBufferListsComplete, thunk único) |

### 5.2 Gates encontrados en el validador `0x140015d18`

- `tipo != 0x0a/0x0c` → return `0xc0000001` (WPP log 0x32).
- Path 0x0a (el que usa el caller `0x140010123`): comprueba profundidad de cola
  por slot (`0x33143c/0x331444/0x331440` con stride `idx<<7`): **falla si
  `C − A ≤ 0xf000` o `A ≥ C`**. Si los contadores no se inicializan en modo
  monitor (solo se usan en modo infra), todo paquete muere aquí — coincide con
  el error 31 medido.
- Cuando la cola "se llena" setea flag de pausa: `or (0x10000<<idx), 0x3322d4(adapter)`.

### 5.3 Inventario de gates bit17 de opState (0x32d468) — 14 sites

Parcheados: `0x14006a8c2` (SwitchChannel) + `0x1402163df` (OID canal, je→jmp).
Pendientes de clasificar: `0x14003e866, 0x140047327, 0x14007d6a4, 0x140099042,
0x1400a0b8b, 0x1400aa681, 0x140108d2d, 0x140118765, 0x140118ea7, 0x140119853,
0x14021a52c, 0x1402162f5`. `0x14021a52c` está en un bloque de teardown/conexión
(no TX normal). Cualquiera puede estar en el path TX según el modo.

### 5.4 Siguiente paso (instrumentar, no parchear)

1. WinDbg (livekd) con bp en `0x140015d18+X` (paths de fallo) y `0x140010656`.
2. Lanzar `cargo test --lib lab_inject_probe -- --ignored --nocapture`.
3. Ver qué gate dispara de verdad (¿colas sin init en monitor? ¿BSS mismatch?
   ¿bit17 en otro sitio? ¿NDIS rechaza antes de llegar al driver?).
4. Parchear SOLO el gate confirmado + test + medir de nuevo.

Riesgo conocido: NDIS puede rechazar el send antes de que el driver vea el
paquete (Npcap #85 habla de la capa NDIS/NPC); si el bp no llega a dispararse,
el bloqueo está por encima del driver y este RE no puede arreglarlo.

## 6. Fase 3 — Medición con parches TX en sitio (2026-09-22)

Variantes construidas tras el recon §5 (todas partiendo de
`netr28ux_patched_clean.sys` d2c7cf43…, canal ya funcional):

| Variante | sha256 (prefijo) | Contenido |
|---|---|---|
| TX-1 → `netr28ux_tx1.sys` | 5684849f | validador `0x140015d18`: forzar success del check de cola (xor edi,edi) |
| TX-2 → `netr28ux_tx2.sys` | ad3e2d41 | gates send `0x140010656…06c5` (BSS-mismatch) neutralizados |
| TX-3 → `netr28ux_tx3.sys` | 30080e89 | **9 kills raw** en send engine `0x14000f98f…0x140010b13` (checks de cola y descartes por estado) |

Despliegue en caliente verificado (`hotswap_tx1.ps1`, ciclo PnP disable→copy→
enable, SIN reinicio): TX-3 en sitio, servicio RUNNING.

### Resultado medido (TX-3 activo)

`lab_inject_probe` (monitor + CTS-to-self por `\Device\NPF_WIFI_{GUID}`):

```
SEND con-radiotap len=18  -> rc=-1 err 31 (ERROR_GEN_FAILURE)   [antes igual]
SEND sin-radiotap  len=10 -> rc=-1 err 31                        [antes igual]
SEND OVERSIZE len=6000    -> rc=-1 err 20 (ERROR_FILE_NOT_FOUND)
```

**Hallazgo del OVERSIZE discriminator**: con 6000 B el error CAMBIA (31 → 20).
Eso demuestra que el paquete SÍ llega al driver/NPC y que el rechazo depende
del tamaño/contenido — NO es un rechazo ciego de NDIS por encima. El bloqueo
de TX sigue DENTRO del stack del driver (o del LWF Npcap), no es la capa NDIS
genérica.

Discriminador managed (`lab_send_managed_ethernet`): en modo MANAGED, un ARP
por `NPF_` (LWF Ethernet normal) da error 2150891551 (media desconectada,
esperado sin asociar) y el MISMO frame por `NPF_WIFI_` da err 31 → el path
802.11 nativo rechaza aunque el otro LWF funcione.

### Prueba asociado (lab_assoc_send_test) — INCONCLUYENTE / CUELGUE

La prueba (asociarse a red abierta + enviar CTS/QoS-null por NPF_WIFI_)
CUELGUE el proceso de test: el binario de test queda zombie (1 hilo, Wait:
Executive, no matable ni con NtTerminateProcess desde otro proceso) tras el
paso de conexión/con envío. 3 ejecuciones, mismo resultado. Hipótesis: el
cmdlet send/open del NPF_WIFI_ con BSS activo bloquea en kernel (deadlock
send vs. contexto del propio NDIS send del sistema asociado). NO repetir sin
VM snapshot.

**Cleanup**: los 3 procesos colgados son incorregibles sin reinicio (no
consumen CPU; la interfaz queda re-asociada a la impresora — desconectar con
`netsh wlan disconnect`).

### Veredicto TX

- TX en monitor con RT3070+Npcap: **SIGUE BLOQUEADA** aunque se neutralicen
  TODOS los gates identificados en el driver (err 31 persistente).
- El cambio de error con tamaño (31 → 20 en OVERSIZE) localiza el rechazo
  en el LWF/path NPC, no en la cola NDIS ni en NDIS genérico.
- **Conclusión**: Npcap #85 confirma su naturaleza — el filtro Npcap no
  completa el path de envío 802.11 nativo sobre LWF. Ningún parche adicional
  de netr28ux.sys lo va a resolver. Cerrar la línea de RE del driver: el
  canal queda ganado (hito 2026-09-21); TX queda descartada por software en
  esta combinación. Vías restantes: antena con driver NDIS6 nativo 802.11 TX
  (RTL8812AU con su port) o Kali live USB (rt2800usb).
- [x] Fase 3 completada: instrumentada y medida. Resultado negativo pero
  concluyente (bloqueo FUERA del driver).

## 7. Diario


- 2026-09-20: fase 1 completa. Desensamblado completo generado. Handler
  de canal localizado y leído.
- 2026-09-20 (2): **fase 2 resuelta** — `14006a7f4` es SwitchChannel
  (lect/esc RF + BBP + comando USB 0x5D4). El bloqueo es el gate del
  bit17 de opState, duplicado en OID-handler y en la propia rutina.
- 2026-09-20 (4): despliegue completado con `run_deploy.cmd` (una capa de
  quoting; los intentos con Start-Process anidado fallaban por quoting).
  Log en `deploy_log.txt`, EXITCODE 0. Backup + cert + testsigning OK,
  .sys agendado para sustitución en el próximo arranque.
- 2026-09-20 (3): parche aplicado (P1a+fix+P1b+P2, 17 bytes), verificado
  con objdump. Limpiado security directory viejo, overlay truncado,
  checksum recalculado. Cert `UIFIPILL Lab Test` creado con makecert y
  binario firmado con signtool (SHA-256). Scripts deploy/restore listos.
  Pendiente: despliegue con admin + reinicio + medición.
- 2026-09-21: **HITO — canal funciona.** Driver parcheado cargando
  (testsigning ON, hash verificado). SET del OID canal aceptado y la
  radio se mueve de verdad (probado por beacons capturados en CH1 vs
  CH6, DS Param). GET cacheado documentado; fix en
  `set_monitor_channel` (éxito por SET aceptado + reintentos). Tests:
  23 passed / 0 failed (5 ignored), build release 0 warnings. Primera
  vez en el proyecto que el RT3070 cambia de canal en Windows.
- 2026-09-22: fase TX medida con variantes TX-1/2/3 desplegadas en
  caliente (ciclo PnP). err 31 persiste; OVERSIZE (6000 B) cambia el
  error a 20 → el paquete SÍ llega al stack, rechazo del LWF/path NPC.
  `lab_assoc_send_test` cuelga el proceso de test en kernel (zombie no
   matable) — documentado, no repetir sin VM. **Veredicto: TX imposible
   por software con RT3070+Npcap; cerrada la línea de RE.** Canal ganado,
   TX vía Kali live USB o antena compatible. Tests: 23 passed / 0 failed
   (7 ignored), build release 0 warnings, lint+vite OK.
- 2026-09-25: **HITO TX AL AIRE por USB crudo** (WinUSB, sin Npcap — red
  "TXTEST" visible desde otro device; bitácora §1). La vía Npcap siguió
  cerrada; el driver parcheado queda como ganancia (canal en monitor).
  RX-WinUSB cerrada (BBP mudo en 2 antenas); vía abierta = sniffer USBPcap
  (RX_PLAN → memory.md §4).
- 2026-09-29: device netr28ux en ProblemCode 31/56 post-HVCI (memory.md §1,
  sesión 2026-09-29) — saneamiento del device pendiente (fix_restart_dev +
  reboot) antes de poder volver a usar el driver (parcheado o inbox) o
  rebindear a WinUSB.


# PARTE 4 — INVESTIGACIÓN RX POR WINUSB (origen: driver_re/usb_tx/RX_PLAN.md)

> Orden cronológico de revisiones (rev. 1 → 4). Estados corregidos: H1 FALSADA,
> E1/E2 EJECUTADOS, reversing estático AGOTADO, vía WinUSB-RX **REABIERTA y
> CONFIRMADA 2026-10-01** (raíz: port del canal rf53xx→rf3xxx + RFCSR1 PLL_PD,
> NO el boot ni el chip). Detalle en §1 «SESIÓN 2026-10-01 (cont.)».

## ESTADO FINAL (rev. 3-4) — ACTUALIZADO 2026-10-01
- **RX por USB crudo WinUSB: CONFIRMADA** — runs 8/9 (611 URBs / 524 frames /
  3 APs en 15 s) + scan1/scan2 (247 y 225 frames/30 s, 10-12 redes en 5
  canales), todo **sin kick FIRMWARE(8)**. La conclusión previa («coldrx: el
  autoload NO deja el BBP vivo; solo el boot con firmware lo activa») estaba
  en lo cierto sobre los FRAMES perdidos pero atribuía mal la causa: el BBP
  respondía (bbp_dir[0]=0x60 en frío), el RF nunca sintonizaba porque el port
  del canal apuntaba a RFCSR8 (path rf53xx) en vez de RFCSR2 (path 3xxx de
  este chip), y RFCSR1 tenía PLL_PD=1. Detalle: §1 sesión 2026-10-01 (cont.).
- **Sniffer USBPcap**: ya NO es la única vía — queda como herramienta de
  cotejo contra el vendor (vendor.pcap sigue siendo la referencia de valores).
  USBPcap 1.5.4.0 instalado en `C:\Program Files\USBPcap\` (2026-09-28).
- TX al aire CONFIRMADO (hito 2026-09-25) — no depende de RX.

## REV. 3 (2026-09-26) — REVERSING ESTÁTICO netr28ux CERRADO: todo era logging
Herramientas nuevas en usb_tx/: `find_str_refs.py` (xrefs rip-rel a strings,
mapeo rva=file-0x400+0x1000 CONFIRMADO — 41 xrefs reales) y `disasm_range.py`
(objdump -b binary con VMA=0x140000000+rva).

Resultados medidos:
- Strings BBP localizados: PostBBPInitialization @file 0x1b0fd0,
  AsicBbpTuning @0x1b50f0, BbpInit7601 @0x1baaf0, RTUSBBulkOutPktCmd @0x1b7790/0x1b77b0.
- TODAS las xrefs desensambladas (PostBBPInitialization ×7 @rva 0x2a2d9-0x2ae00,
  AsicBbpTuning ×11 @0x6468e-0x64f3b, BulkOutPktCmd/MLME/BulkReceive @0x1192b7-0x119cd0,
  BbpInit7601 @0x1807ea) terminan en el MISMO patrón:
  `lea r9,[string]; lea r8,[0x1401bd388]; mov edx,<linea>; call 0x14000b9e0/0x14000bb20/0x14000ba4c`
  = wrapper de LOG/trace (WPP/DbgPrint). Los 0x41/0x42/0x43 de la rev. anterior
  (@0x1400075ba-0x140007644) TAMBIÉN: edx=0x41/0x42/0x43 es el Nº DE LÍNEA del log,
  NO un comando MCU. Cierre de la pista.
- Búsqueda de immediatos de registros (0x11C BBP_CSR, 0x138 RF_CSR, 0x7010 mailbox,
  0x404 HOST_CMD) en .text: solo falsos positivos (bytes de otras instrucciones).
  El vendor NO accede a los CSR por imm32 visible — o los calcula, o va por la
  ruta bulk con buffers opacos. Reversing estático AGOTADO sin símbolos.

**CONCLUSIÓN REV. 3**: el formato del paquete de comando BBP por bulk pipe NO se
puede extraer estáticamente del .sys (solo hay wrappers de log; el payload va en
buffers construidos en runtime). ÚNICA vía restante para RX-WinUSB: SNIFFER USBPcap
del vendor en vivo (instalado, ver ESTADO FINAL).
> **SUPERADO (2026-10-01)**: la vía WinUSB-RX se confirmó SIN replicar el bulk
> del vendor — no hacía falta: la raíz era el port del canal (RFCSR8→RFCSR2) +
> RFCSR1 PLL_PD. USBPcap queda solo de cotejo.

## REV. 2 (2026-09-25) — E1 y E2 EJECUTADOS

### E1 (rebind con radio activa) — H1 FALSADA
E1 completo (e1_run3.ps1): netr28ux tomó el control (pnputil add + delete oem395
+ rescan), netsh escaneando en bucle, delete netr28ux + rescan → WinUSB heredó
el chip SIN power-cycle (Status OK, mailbox OWNER=0, RF vivo).
**El BBP siguió leyendo 0x00 incluso heredado del vendor con radio EN USO.**
→ H1 («el vendor duerme el BBP al soltar») FALSADA. El vendor NO duerme el BBP:
su acceso BBP NO pasa por BBP_CSR_CFG (0x11C) visible para nosotros.

### E2 (ciclo MCU_SLEEP → MCU_WAKEUP) — DESCARTADO
Ejecutado con `rt3070_mculoop`: SLEEP(0xff,0xff,2) → WAKEUP(0xff,0,2),
WAKEUP(arg1=0), WAKEUP×2 — **el BBP NO despierta en ninguna variante**.

### heredrx — 0 frames
Sobre estado heredado del vendor SIN tocar BBP/MCU, solo USB_DMA_CFG=0x00c12d80
+ ENABLE_RX + filtro monitor → 0 frames. El DMA no basta: el BBP no demodula.

### H2 (acceso BBP vía MCU) — agotada estáticamente
El reversing de netr28ux.sys buscando la rutina BBP cerró en rev. 3 (solo wrappers
de log). La hipótesis de fondo (el firmware enruta el BBP por otra puerta) queda
como explicación, no como plan: sin el formato del comando bulk no hay acceso.

## REV. 1 (2026-09-25) — Hipótesis H1 + diseño de E1/E2 (histórico)
- Dato que reabrió RX: la antena SÍ capturaba bajo netr28ux (392 pkts Npcap,
  hito 2026-09-14) → el BBP NO está roto; el vendor lo deja VIVO y algo lo duerme
  antes de nuestro acceso.
- H1: el driver vendor manda MCU_SLEEP al detach/rebind → heredamos BBP dormido
  (FALSADA en rev. 2, arriba).
- USB_RX_CONTROL(0x0C) SOLO se usa para DESactivar la radio
  (rt2x00usb_disable_radio); la activación es submit de URBs bulk IN (lo que ya
  hacemos). No hay vendor request mágico de RX-ON que nos falte.

## PROCEDIMIENTO SNIFFER (próxima acción, si se decide intentar)
1. USBPcap YA INSTALADO (`C:\Program Files\USBPcap\USBPcapCmd.exe`) — si no,
   correr USBPcapSetup-1.5.4.0.exe (en usb_tx/) + REINICIO.
2. restore_netr28ux.ps1 → antena con driver vendor (device sano: ver §1 bitácora
   sesión 2026-09-29 — pendiente fix_restart_dev/reboot).
3. Capturar: `"C:\Program Files\USBPcap\USBPcapCmd.exe" \\.\USBPcap<N> -o vendor.pcap`
   mientras netr28ux escanea (netsh en bucle) 30-60 s.
4. Filtrar en Wireshark: EP bulk OUT del device 148f:3070 que NO sean datos de
   beacon TX → comandos (look for small transfers, ~32-64 B, hacia EP 0x0d/0x05).
5. Identificar el formato: los comandos BBP/RF del vendor tendrán patrón
   [cmd][reg][val] repetido durante el init/scan. Replicar por WinUSB (rusb
   bulk write al mismo EP) tras hot_rebind a WinUSB.
6. Riesgo: el estado heredado se degrada — power-cycle antes de cada intento.

## Lección del mecanismo de rebind (para scripts futuros)
- UpdateDriverForPlugAndPlayDevices (newdev.dll) → win32err=2 SIEMPRE con este
  device. NO USAR.
- Vía que funciona: pnputil /add-driver <inf> + pnputil /delete-driver <oem>
  /uninstall + pnputil /scan-devices. El uninstall tarda 3-4 MIN (timeout
  interno) — esperar con paciencia, no matar.
- Orden E1 verificado: publicar netr28ux → delete oem395 → rescan (netr28ux
  toma control) → escaneo netsh activo → delete netr28ux → rescan (WinUSB
  hereda con radio caliente).

## Estado del problema (act. 2026-10-01)
- TX al aire CONFIRMADO (beacons "TXTEST" visibles desde otro device).
- ~~RX: 0 frames en todos los estados probados. BBP lee 0x00 siempre.~~
  **SUPERADO 2026-10-01**: 524 frames/15 s (run9) y scan 247 frames/30 s con
  10 redes — la causa era el port del canal (rf53xx→RFCSR8 en vez de
  rf3xxx→RFCSR2) + RFCSR1 PLL_PD=1, no el boot ni el chip (§1 «(cont.)»).
- El port es fiel a Linux (verificado contra rt2x00usb.h/c, rt2800usb.c, rt2800lib.c 6.6).
  **Corrección 2026-10-01**: la secuencia de CANAL estaba portada de la rama
  rf53xx — path equivocado para este chip (RT30xx = rf3xxx). Ya corregido.
- El kick FIRMWARE(8) mata el chip bajo WinUSB (reproducido en frío vía UNPLUG).
- ~~Autoload de fábrica NO deja el BBP vivo (coldrx: 0 frames en frío).~~
  **Corregido**: el BBP respondía (bbp_dir[0]=0x60 en frío); lo que faltaba era
  sintonizar el RF y el PLL (path N→RFCSR2). El "0 frames" era RF desafinado,
  no BBP muerto.

## Lo que NO volver a intentar (medido, cerrado)
- Kick FIRMWARE(8) en cualquier estado → chip sordo (5/5: power-cycle ×4 + UNPLUG frío).
- Cargar firmware y esperar re-enum con reapertura simple (falsa re-enum).
- Confiar en MAC_CSR0 != 0 como señal de "sin firmware cargado" (es el ASIC ID).
- Releer el estado tras degradación sin power-cycle (registros 0x00/0xff).
- Ciclo MCU SLEEP/WAKEUP para despertar BBP (E2).
- Rebind con radio activa para heredar BBP vivo (E1) y heredrx (DMA solo).
- Reversing estático del .sys para el formato de comando bulk (rev. 3: agotado).
- ~~Vía WinUSB-RX solo por sniffer USBPcap~~ → SUPERADO: RX funciona directa
  (2026-10-01); USBPcap queda como cotejo de valores contra el vendor.

## Próxima sesión (orden) (act. 2026-10-01)
1. ~~Sniffer USBPcap como única vía para RX-WinUSB~~ → RX CONFIRMADA sin él
   (runs 8/9 + scan1/2); USBPcap solo de cotejo.
2. En paralelo: integrar TX y RX crudos en la app (deauth/beacon ya posible;
   scan crudo funciona standalone — `rt3070_scan`).
3. RX operativa HOY: Kali live USB (rt2800usb) — no depende de este plan.
4. Nuevo: sostenido largo (scan+sniff continuo), efuse/EEPROM (tx power por
   canal, LNA gain) y comparar RFCSR23=0x09 vs 0x00 en sensibilidad.


# PARTE 5 — POR QUÉ NO FUNCIONA IGUAL EN WINDOWS (origen: driver_re/usb_tx/LINUX_EN_WINDOWS.md)

> Doc para NO repetir la explicación cada sesión. Última revisión: **2026-10-01**
> (RX WinUSB CONFIRMADA — se corrigieron §2/§3/§4/§5). Todo lo marcado
> **MEDIDO** está verificado con hardware real, no es opinión.

## 1. La pregunta

«Si funciona en Linux debe funcionar en Windows.» — La respuesta corta
(actualizada 2026-10-01): **el protocolo estaba CASI portado bien: el port del
CANAL usaba la rama rf53xx (N→RFCSR8) y este chip es rf3xxx (N→RFCSR2) —
corregido 2026-10-01 y RX ya funciona sin ningún kick. Lo que SÍ no se puede
replicar por WinUSB es la gestión del RE-ARRANQUE (kick FIRMWARE(8), mata el
chip 5/5) — pero RX/TX ya no la necesitan.**

## 2. Qué está verificado (MEDIDO)

- El port del protocolo es FIEL a Linux: verificado línea a línea contra
  rt2x00usb.h / rt2800usb.c / rt2800lib.c del kernel 6.6 (descargados, NO de
  memoria). Secuencia de boot, registros, mailbox H2M, EFUSE, init BBP/RFCSR:
  todo coincide con rt2800lib.c. **Excepción corregida 2026-10-01**: la
  secuencia de canal estaba portada de la rama rf53xx; la correcta para RT30xx
  es rf3xxx (§ «por qué no funciona» abajo).
- **TX por USB crudo FUNCIONA en Windows**: hito 2026-09-25 — beacons "TXTEST"
  visibles desde otra radio + TXDONE success=true en TX_STA_FIFO. Pipeline
  TXINFO+TXWI+802.11 → EP 0x01 → chip modula → sale al aire.
- ~~**RX por USB crudo NO funciona en Windows**: el BBP lee 0x00 en TODOS los
  estados probados (2 antenas distintas, miles de intentos).~~
  **FALSADO 2026-10-01**: RX confirmada — run9 (611 URBs / 524 frames / 3 APs
  en 15 s) y scan1/2 (247 y 225 frames/30 s, 10-12 redes, sin kick). La causa
  era el port del canal (rf53xx→RFCSR8) + RFCSR1 PLL_PD=1; el BBP y el MCU
  estaban vivos (MCU_CURRENT consumido, bbp_dir[0]=0x60 en frío).

## 3. Por qué la diferencia (causa raíz) — CORREGIDA 2026-10-01

**Corrección (2026-10-01)**: NO hace falta boot con firmware para RX. El BBP
responde sin firmware (directo `BBP_CSR_CFG` vivo en frío; la vía MCU-BBP sí
da timeout sin firmware — por eso la sonda `bbp_probe_transport` la elige) y
los 0 frames se debían al **port del canal equivocado**: sintonizábamos por
RFCSR8 (path rf53xx) en vez de RFCSR2 (path rf3xxx de RT30xx) y RFCSR1
tenía PLL_PD=1 → el PLL nunca sintonizó → ΔCCA=0 en todos los canales.
Corregido → 524 frames/15 s (run9) y scan 247 frames/30 s, SIN ningún kick.

El kick (DEVICE_MODE FIRMWARE=8) sigue MEDIDO como destructor — se mantiene
prohibido bajo WinUSB (el único motivo por el que Linux «lo hacía fácil»
algunas veces era la re-enum gestionada por kernel):

- **Linux (rt2800usb, driver kernel)**: cuando el MCU re-arranca, el device
  USB desaparece y re-enumerar. El KERNEL gestiona todo automáticamente:
  detecta la re-enum, hace port reset, re-vincula el driver y continúa el
  init. El device nunca se pierde para el driver.
- **Windows (WinUSB, user-mode)**: cuando el MCU re-arranca, el device
  desaparece del bus y Windows NO devuelve un handle utilizable al proceso
  user-mode — la ventana crítica del re-arranque se pierde. El chip queda
  sordo. **MEDIDO 5/5**: kick tras power-cycle ×2 modos, kick en frío vía
  UNPLUG ×1, power-cycle ×2 más.

~~Además, el autoload de fábrica del chip (tras power-cycle sin driver) NO
deja el BBP inicializado ni accesible — solo el boot completo con firmware
lo despierta, y solo el vendor (netr28ux) y Linux logran ese boot.~~
**FALSADO 2026-10-01**: el autoload SÍ deja MCU+BBP vivos (runs 8/9: MCU_CURRENT
consumido sin cargar rt2870.bin, bbp_dir[0]=0x60); lo que faltaba era la
secuencia de canal rf3xxx + RFCSR1 sin PLL_PD — con eso, RX al aire sin firmware.

## 4. El vendor SÍ lo logra en Windows — la vía abierta

netr28ux.sys despierta el BBP en Windows (sus capturas Npcap: 392 pkts
reales). Lo hace hablando por un **pipe bulk de comandos** dedicado
(RTUSBBulkOutPktCmd), no solo por el control pipe. Reversing del .sys (2.2 MB)
confirmó: vendor requests por BULK OUT, wrapper @0xade0, nombres de funciones
en .text.

**~~La única vía abierta para RX-WinUSB~~ SUPERADA 2026-10-01**: RX funciona
directa por control pipe + EP 0x81 bulk IN (path rf3xxx + RFCSR1 fix, §1 sesión
2026-10-01 (cont.)) — no hizo falta replicar el bulk del vendor. El sniffer
USBPcap (v1.5.4.0, instalado) queda como herramienta de COTEJO de valores
contra el vendor (vendor.pcap), no como plan crítico.

## 5. Mapa funcional honesto (esta antena, RT3070)

| Función | Windows (USB crudo WinUSB) | Windows (netr28ux+Npcap) | Linux (rt2800usb / Kali live) |
|---|---|---|---|
| Scan redes | ✅ rt3070_scan (hopping) | ✅ netsh | ✅ |
| Monitor mode | ✅ (filtro por software) | ✅ (OID, canal fijado por parche) | ✅ completo |
| TX beacons | ✅ **confirmado al aire** | ❌ (Npcap #85, err 31) | ✅ |
| Deauth | ✅ (TX al aire) | ❌ (Npcap #85) | ✅ |
| RX / captura | ✅ **cruda: 524 frames/15 s, scan 247/30 s (2026-10-01)** | ✅ pasiva (sin inyectar) | ✅ completa |
| Cambio de canal | ✅ (config_channel portado rf3xxx) | ✅ (driver parcheado 2026-09-21; el stock sí rechazaba, 3 vías OID) | ✅ |
| PMKID / handshake | ✅ RX cruda lista (sniff.rs; pipeline conversor en desarrollo) | parcial (RX pasiva) | ✅ hcxdumptool |

## 6. Vías para "Linux en Windows" (orden práctico)

1. **Kali live USB** — RX completa HOY con esta antena (rt2800usb hace el
   boot frío en kernel). La app genera el «Kit Kali para este objetivo» con
   los comandos listos. Es la vía probada.
2. ~~**USBPcap → replicar bulk commands del vendor** — la vía de investigación
   abierta para RX-WinUSB nativo (ver §4). Requiere device con netr28ux sano.~~
   SUPERADA 2026-10-01: RX-WinUSB nativa confirmada sin replicar el bulk
   (path rf3xxx + RFCSR1 fix); USBPcap queda de cotejo.
3. **Windows + TX crudo** — ya operativo: beacons/deauth al aire. Con RX cruda
   nativa (2026-10-01) el pipeline completo cabe HOY en Windows.

## 7. Vías CERRADAS (no reintentar, todo medido)

- kick FIRMWARE(8) bajo WinUSB en cualquier estado (mata el chip, 5/5).
- Re-enum tras kick con reapertura simple (device sordo).
- Confiar en MAC_CSR0 como señal de firmware (es el ASIC ID, siempre vivo).
- Ciclo MCU SLEEP→WAKEUP para despertar BBP (E2 descartado).
- Acceso BBP solo por la vía MCU (`MCU_BBP_SIGNAL`) sin firmware → timeout
  (medido 0/5). La vía **directa** `BBP_CSR_CFG` SÍ funciona — la elige
  `bbp_probe_transport()` (2026-10-01); con el default MCU, `init_bbp`
  fallaba en "BBP no responde".
- RX heredando estado del vendor (heredrx: 0 frames — DMA+filtro solo, sin
  config de canal rf3xxx; ya sabemos que esa config es lo que faltaba).
- Npcap inyección en Windows (Npcap #85, err 31 — capa independiente).
- usbipd→WSL2/Kali (RX muerta por el transporte, 0 pkts, 3 mediciones).
- VirtualHere (licencia de pago, API Timeout).
- ~~Comprar más RT3070 para RX por WinUSB (el BBP mudo es del boot, no del
  chip)~~ → **la premisa era falsa**: RX funciona en Windows (2026-10-01);
  el "BBP mudo" era el port del canal (rf53xx) + RFCSR1 PLL_PD.

## 8. Estado del device (2026-09-30, RESUELTO) y cómo llegar a TX operativo

~~PENDIENTE: ejecutar `fix_restart_dev.ps1` … reboot~~ → **RESUELTO
2026-09-30** (ver §1 sesión 2026-09-30): la causa era `C:\Windows\INF\netr28ux.inf`
borrado → `NetworkInterfaceInstallResult 0x80070002` → class config colgada
(Code 56). Fix: restaurar INF+PNF desde DriverStore + `ConfigFlags 0x80000`→0 +
remove+rescan → device Status OK, netsh ve la antena, scan real (10 redes).
**Ojo**: corre el driver INBOX 5.1.22.0 (94734AEF) — el parcheado (d2c7cf43,
canal en monitor) hay que redeployarlo.

Una vez device OK (YA OK desde 2026-09-30):
1. `hot_rebind.ps1 to-winusb` (consola admin) → WinUSB
2. Power-cycle 15 s
3. App → tab Ataque → paso A: Estado chip → Init → TX beacons → TX al aire

Para RX: Kali live USB (vía 1) o el proyecto USBPcap (vía 2).

## SESIÓN 2026-10-05 — UI ATAQUE: FLUJO ÚNICO VISUAL POR RED (sin jerga)
Petición: tab Ataque no usable (mucho oculto/confuso) + el acceso directo a la
antena debe ser LA forma de trabajar (sin llamarlo "USB crudo") + WPS y camino
por defecto según la red elegida.
- Héroe `#flow-hero` (index.html, tras los chips): título + descripción +
  3 luces (Objetivo/Antena/Perfil) + botones primarios por veredicto
  (`renderHero()` en app.js) + `#flow-result` único. Sin jerga: "Acceso directo
  a la antena — recomendado" (detalle WinUSB solo en plegable); "Escanear
  (directo)" en el tab Escanear.
- Defaults por `profile_target`: open→conectar; wep→Kit Kali; wpa_legacy/
  transition→handshake directo; wpa3_sae→wash primero (¿WPS abierto?→PIN);
  enterprise→Kali/lab; wpa2_psk→keygen→handshake→crack+wash en paralelo;
  unknown→reescanear. Wash integrado: `Lck=No`→PIN vía principal; PIN/PBC/Pixie
  nativos colapsados (sin TX en Windows, Npcap #85 medido).
- Estado: `window._usbAlive` (status/scan/init), checklist en héroe, `flowResult()`
  refleja último resultado USB/wash/WPS; `renderHero()` en perfil/objetivo/showTab.
- Verificación: lint ✅ vite ✅ (122.97 kB + 79.29 kB) · auditoría 0 huérfanos
  (64 onclick, 58 invokes, 198 ids) ✅ · `cargo test --lib` 26/8 ✅ (Rust intacto).

## SESIÓN 2026-10-06 — PUESTA EN ORDEN: docs coherentes + spec/ SDD + épica E0
Petición: «quiero que pongas todo en orden; estábamos haciendo que funcionase
en Windows como Kali Linux».
- **Revisión integral de docs** (AGENTS.md / memory.md / README.md + git +
  lint). Hallazgos corregidos:
  · AGENTS.md: cabecera «revisión 2026-09-29» desactualizada → 2026-10-06;
    «Límites hardware» decía *WinUSB RX NO (BBP lee 0x00)* contradiciendo el
    hito RX CONFIRMADA 2026-10-01 → corregido; tests 23/5-7 → **26 passed /
    8 ignored**; app «rev. 2026-09-18» → reinstalada 2026-10-01 (15,7 MB);
    hito 09-25 «RX cerrada» marcado como superado el 10-01; Estado actual
    ampliado con la sesión 10-05 (antes no constaba); Documentación + spec/.
  · README.md: mapa «qué corre dónde» con fecha 2026-09-17 → **2026-10-06**
    (canal con driver parcheado ✅, RX/TX USB crudo ✅, wash E2E ✅, inyección
    Npcap ❌ err 31/203 Npcap #85, recetas Kali → Kali live USB); sección de
    límites reescrita (incluye regla FIRMWARE(8) y restore_netr28ux);
    referencia fantasma `tools/build/BUILD.md` eliminada (el build de reaver
    se borró del repo); enlaces a docs repartidos entre AGENTS/memory/spec.
  · memory.md: índice §1 actualizado a 2026-10-05 + esta entrada.
- **`spec/` creado** (metodología SDD del AGENTS global, hasta ahora ausente):
  `00-PRINCIPIOS.md` (5 reglas innegociables + Definición de Listo de 7
  puntos), `01-ARQUITECTURA.md` (Tauri v2, contratos camelCase, módulos,
  estados de la radio), `02-REQUISITOS.md` (**épica E0 «Windows ≡ Kali»** con
  15 historias E0-01…E0-15 y criterios medibles; E0-01…E0-10 retrodocumentados
  con su evidencia; E0-11 sniff sostenido, E0-12 PMKID/handshake por RX cruda,
  E0-13 WPS sin Npcap, E0-14 efuse, E0-15 Fase 7 = abiertos), `03-PENDIENTES.md`
  (deudas: trabajo 10-01/10-05 sin commitear, ficheros basura `1`/`yye` en la
  raíz, Fase 7), `04-EJERCITO.md` (roles + flujo coordinator→editor→reviewer→
  tester).
- **Estado git leído**: último commit `a5fea89` (2026-09-30); 23 ficheros
  modificados (+2 778/−622) y ~35 untracked de las sesiones 10-01/10-05
  **sin commitear** (regla: no se commitea sin petición explícita) → anotado
  en spec/03.
- **Commits EJECUTADOS hoy con aprobación del usuario («hazlo»)** — `git status`
  **limpio** al cierre. 3 bloques:
  · `64bb5d0` **docs**: AGENTS/README/memory coherentes + `spec/00..04` +
    `.gitignore` (evidencias de runs de lab + backups locales).
  · `d193a30` **feat(ui)**: héroe de flujo + wash/WPS visual + acceso directo
    a la antena (index.html + app.js, +979/−126).
  · `33b2b6b` **feat(usb_tx+backend)**: RX WinUSB rf3xxx + wash E2E + fix
    `default-run` + saneado device + scripts de lab (40 ficheros,
    +2 839/−413). Incluye `bbpmcu.rs`, `usb_sniff_stats.rs` y los .ps1 de
    UAC (fix56/restore/run_bbpmcu/run_scan/switch_winusb…).
  · **Basura borrada**: `1` (479 KB) y `yye` (697 KB, dump con SSID real) de
    la raíz; logs de runs (`*_run*.txt`, `vendor_timeline.txt`,
    `force_log2.txt`), `*.bak-arch` y `tools/*.nullbak` → `.gitignore`.
  · **Push HECHO 2026-10-06** (aprobación «pushea»): `a5fea89..a1b5ba3
    master -> origin/master`, `git status` en sincronía.

## SESIÓN 2026-10-06 (cont.) — ÉPICA E0-11: SNIFF SOSTENIDO (en curso)
Orden recibida: «arranca». Elegida **E0-11** (base de E0-12: sin escucha
prolongada no cae ningún handshake). CA: ≥5 min continuo + watchdog/cancel +
frames>0 + pcap creciendo + radio restaurada al cierre.
- **Implementado** (código sin commitear todavía):
  · `driver_re/usb_tx/src/sniff.rs`: línea `PROGRESS t=X/Ys frames=N
    pcap=ZB` cada 10 s (stdout line-buffered → llega en vivo; sirve de
    fallback si el proceso se mata antes del resumen final).
  · `src-tauri/src/commands.rs`: extraída **`run_path_bg()`** de
    `run_bin_bg_opts` (ruta absoluta ya resuelta: mismo streaming chunked,
    mismos eventos attack-started/completed, cancelable + watchdog).
  · `src-tauri/src/usb_raw.rs`: **`usb_raw_sniff_bg()`** (id fijo
    `usb_sniff_long` = el que la UI claimatea; clamp 10–1800 s; watchdog
    secs+30; éxito honesto SOLO con frames>0) + **`usb_raw_restore()`**
    (dispara `restore_netr28ux_run.ps1` con UAC, nunca éxito simulado: la UI
    lo verifica con `detect_adapters`) + parser `sniff_frames()` con 4 tests
    nuevos (resumen, 0 honesto, fallback PROGRESS, sin evidencia).
  · `src-tauri/src/lib.rs`: registrados los 2 comandos.
  · `index.html`: botón «🎧 Escucha larga», botón «↩️ Restaurar antena»,
    input `usbraw-long-secs` (60–1800, defecto 300) + checkbox
    `usbraw-restore-onclose` (marcado).
  · `src/app.js`: `usbRawSniffLong()` (init previo + claimAttack +
    polling `detect_adapters` hasta 150 s en `usbRawRestore`, barra con % de
    los PROGRESS en `_flushEvents`).
  · Detalle clave (medido): async+State exige `Result` en Tauri (E0277) →
    `usb_raw_sniff_bg` devuelve `Result<UsbRawResult, String>` como los
    comandos `*_bg` (el primer build falló por esto).
- **Verificación**: `npm run lint` ✅ · `npx vite build` ✅ (122.79 kB +
  81.91 kB) · cargo largo vía `schtasks` (`verify_e011.cmd` + tarea
  `uifipill_e011`). **Lección de entorno**: la tarea heredaba la consola del
  shell y murió con 0xC000013A (Ctrl+C) → acción con `start /min` (consola
  propia) lo resolvió. Ronda 1: `usb_tx` build 0 warnings ✅ + tests 3/3 ✅;
  src-tauri falló (E0277) → corregido → ronda 2 VERDE ✅ (release 0 warnings, `cargo test --lib` **30 passed/0 failed/8 ignored**).
- **E2E con HW** (12:36–12:55): PnP `RT3070 Raw USB (WinUSB)` OK/CM_PROB_NONE;
  `rt3070_init 11` OK (MCU vivo, radio ON, canal 11); sniff 300 s lanzado
  y escuchando en EP 0x81 (proceso vivo, CPU activa) con **0 frames /
  pcap 24 B** en t≥100 s — y `netsh` NO ve ninguna WiFi activa en la
  máquina (la propia RT3070 ya no es WiFi bajo WinUSB; la 2ª radio que emite
  aire real no está confirmada hoy).
- **⚠️ E0-11 queda SIN EVIDENCIA de frames.** Nota honesta: el código nuevo
  (bg+cancel+PROGRESS+restore) está implementado y verificado (lint/vite/
  cargo 0 warnings/30 tests); el CA de «frames>0 + pcap creciendo» exige
  hardware aire real (AP emitiendo en el canal o 2ª radio) que HOY no está
  confirmado — igual que el E2E 10-01 recibió 359 frames porque MIWIFI y
  compañía emitían en ch11.
- **✅ Restore verificado 2026-10-06 12:57** (CA «radio restaurada al cierre»):
  `restore_netr28ux_run.ps1` lanzado con UAC → oem180 desinstalado + netr28ux
  reinstalado desde DriverStore + rescan → PnP `802.11n USB Wireless LAN
  Card` **OK / CM_PROB_NONE**, `netsh` ve la antena como **Wi-Fi con 11
  redes** (HP-Print…, etc.). La radio queda viva en WiFi normal — el
  mecanismo de `usbRawRestore()` (disparo + polling `detect_adapters`)
  funciona.
- **Verificación**: `npm run lint` ✅ (`node -c src/app.js`). Solo toques de
  documentación (sin cambios de código).

## SESIÓN 2026-10-06 (cont. 2) — E0-11 E2E: chip degradado + kick prohibido en diag + lección netsh
Continuación de E0-11 con «revisa memory y continúa». E2E repetido con HW:
- **Setup**: `cargo test --lib` 30/8 ✅ → `switch_winusb4.ps1` (UAC) → WinUSB
  activo (service=WinUSB, status=OK, probe: chip HABLA) → `rt3070_init 11` OK.
- **Sniff 300 s ch11**: PROGRESS cada 10 s en vivo ✅ (el cableado E0-11
  funciona), pero **frames=0, pcap=24 B todo el rato**.
- **Scan hopping 30 s**: también 0 frames en TODOS los canales (el 10-01 daba
  247/30 s con el MISMO binario rt3070_scan.exe de 02:27 — binario y código
  descartados, diff de sniff.rs es solo el añadido PROGRESS).
- **TX test**: bulk OUT `ERR Timeout` tras 2 frames (antes escribía 100/100).
  Patrón = chip degradado (STALL/timeout total, regla 3).
- **Power-cycle ×2** (usuario): tras el 1º todo respondía por control pipe
  (probe OK) pero bulk/RX seguía sordo; tras el 2º, igual. Antena …D8:A7
  degrada más rápido (medido) — hoy cayó más rápido aún.
- **🔴 CAUSA PROPIA ENCONTRADA: `rt3070_diag` ejecutaba el kick PROHIBIDO**
  `DEVICE_MODE OUT val=FIRMWARE(8)` (paso 7 del diag de investigación
  antiguo). Corrió en esta sesión y degradó el chip. **FIX**: paso eliminado
  de `diag.rs` con comentario de regla (no reinstaurarlo) + rebuild 0
  warnings + 3 tests ✅. La herramienta queda segura.
- **⚠️ LECCIÓN METODOLÓGICA (invalida parcialmente el E2E de ayer)**: la
  conclusión de ayer «no hay aire porque netsh no ve redes» era INVÁLIDA —
  con la RT3070 en WinUSB **netsh no tiene NINGUNA interfaz WiFi** (medido
  hoy: «No hay ninguna interfaz inalámbrica en el sistema»; solo Ethernet).
  El aire es INVERIFICABLE por netsh mientras la antena esté en WinUSB.
  Única fuente de aire fiable: hotspot móvil 2.4 GHz u otra radio externa.
- **Estado E0-11**: código bg+cancel+PROGRESS+restore verificado y E2E de
  infraestructura OK (300 s continuos, cancelable, pcap generado, restore
  verificado ayer). El CA «frames>0 + pcap creciendo» sigue pendiente de
  aire confirmado + chip sano (probar: power-cycle + hotspot móvil + scan
  hopping; si el hopping ve redes → sniff 300 s en ese canal).

## SESIÓN 2026-10-06 (cont. 3) — RT73 como verificador de aire + RT3070 RX sordo NO-HW
Continuación con «continúa». Secuencia de hallazgos:
- **Chip sordo tras el cont. 2** → power-cycle del usuario → igual (0 frames,
  bulk TX muere a los 2 frames). Se probó E2E vía **schtasks con consola
  propia** (lección E0-11) → **mismo síntoma** → entorno shell DESCARTADO.
- **Código descartado**: diff de `src/` vs HEAD = solo PROGRESS (sniff.rs) +
  fix diag.rs; `rt3070_scan.exe` es el binario del 10-01 que sacó 247 frames.
- **2ª antena RT3070 conectada** → **mismo síntoma exacto** (control pipe OK,
  BBP init «vivo», bulk muere a los 2 frames, RX 0). Dos chips distintos con
  el mismo fallo → **NO es daño del chip** ni del código: algo del entorno
  cambió desde el 10-01 (posibles: puerto USB 3.0 xHCI — ambas cayeron en
  Port_#0005.Hub_#0001 del host Intel USB 3.0; reinstalación del paquete
  WinUSB oem180 por switch_winusb4 ayer; Windows Update). **PRÓXIMA VÍA:
  probar RT3070 en puerto USB 2.0 distinto / descartar xHCI.**
- **NUEVA ANTENA RT73 conectada (VID_148F&PID_2573)**, driver vendor «RT73
  USB Wireless LAN Card» activo → **netsh SÍ escanea con ella**: **7 redes
  en el aire** (TP-Link_29D6 **100% en ch3**, HP-Print 100% ch3, Q003 60% ch3,
  MOVISTAR 20% ch1, La Fory 29% ch6, Livebox6 29% ch11…).
- **IMPLICACIÓN 1 — el aire está CONFIRMADO**: el «0 frames» de la RT3070
  NO era falta de tráfico (hipótesis de ayer INVALIDADA por segunda vez).
- **IMPLICACIÓN 2 — RT3070 en ch3 (100% señal): 0 frames igualmente** →
  RX crudo sordo en ambas antenas hoy. `rt3070_diag` (1ª antena) corrió el
  kick prohibido (ya eliminado de diag.rs), pero la 2ª antena nunca lo
  recibió y falla igual → el kick NO explica todo.
- **RT73 no es compatible con el port usb_tx** (RT3070: rf3xxx, vendor
  requests distintos; RT73 = rt73usb en Linux). Valor inmediato: verificador
  de aire por netsh + 2ª radio pasiva Npcap + antena para Kali. Port del
  protocolo rt73usb a WinUSB = posible historia nueva (E0-16), no iniciada.
- **✅ RT73: MONITOR + CANAL + CAPTURA VERIFICADOS E2E (misma sesión, tarde)**:
  añadida a `CHIPS` en wifi_adapter.rs (capable=true) → `lab_monitor_cycle`
  PASA con ella: activate_monitor OK (mode=monitor), canal 1 aceptado por
  OID, restore managed OK. **`lab_channel_rf` PASA**: CH1 → 83/84 beacons
  del canal pedido (99%), CH6 → 80/87 (92%), captura pasiva Npcap real
  (195 y 125 pkts, dlt127). **El driver rt73.sys cambia de canal SIN parche**
  (al contrario que netr28ux stock, que necesitó el parche del 09-21).
  Nota: el GET del canal devuelve valor cacheado (como en RT3070) — la
  verificación válida es por beacons. Inyección: NO probada (Npcap #85 es
  capa Npcap — se asume err 31 igual).
- **Verificación del día**: schtasks E2E verde (probe/init/scan/tx/sniff
  corrieron; el síntoma es HW/entorno, no del cableado); lint/vite/cargo
  30/8 ✅; usb_tx 0 warnings + 3 tests ✅ (con fix diag.rs); lab_monitor_cycle
  + lab_channel_rf PASAN con RT73 (2 lab tests ignorados ejecutados hoy).

## SESIÓN 2026-10-06 (cont. 4) — ¿Parchear netr7364.sys (RT73)? VEREDICTO: NO hace falta
Pregunta del usuario: «¿parchear el driver es más fácil para la RT73?». Análisis
estático completo (copias en `driver_re/netr7364.sys/.inf`, sha256 `b3c9de37…`,
asm en `driver_re/netr7364.asm`, 159k líneas, Intel syntax):
- **Binario**: netr7364.sys 5.01.03.0000 (Ralink 2024 en DriverStore, VMA base
  0x11000 sin ASLR high — MÁS simple que netr28ux: 722 KB vs 2.24 MB, .text
  0x99d2f vs ~0x2c0000). Si PUDIERA hacer falta parchear, es bastante más
  fácil: despachadores OIDs planos con cmp literal (a3780/a3ccf) vs el
  aritmético de netr28ux, y símbolos/rutinas más compactas.
- **RE del canal (ya no hace falta)**: OID_DOT11_CURRENT_CHANNEL (0x0D010335)
  → despachador a3780 → handler a394c (GET vía a7420, lee byte ctx+0x6ccc —
  ¡mismo patrón de «valor cacheado» que netr28ux!) → segundo despachador
  a3ccf → SET a3ece (gate cr8==2 → call a4780). Handler SET a4780: valida
  longitud≥4, estado (+0x2c bit0 → 0xC0232000), canal 1..14 (0xC0010015 si
  fuera de rango), almacena en ctx+0x6ccc (con write-back condicional de
  beacons +0x80). **Sin el gate bit17 (0x20000) del camino Extensible que
  bloqueaba netr28ux** — y de hecho el gate similar está en 0x26aa0 pero es
  cr8 (IRQL), no opState. Esto EXPLICA el medido hoy: el canal funciona de
  fábrica (lab_channel_rf 99%/92% sin parche).
- **Consumidor del canal**: 124 lecturas de ctx+0x6ccc; la máquina de estados
  de conexión consume el byte (397de: lo recarga de ctx+0x153ff al iniciar
  state machine) y la sintonía va por tabla ctx+0x2396df (comparación
  canal-bytes en 36edd-36f1b con getter 5f350).
- **¿Para qué SÍ haría falta parchear? NADA identificado**: canal y monitor
  ya funcionan de fábrica; la inyección falla por Npcap #85 (capa Npcap, err
  31/203 — no del driver, y ningún parche de netr7364 lo arregla); TX cruda
  requiere WinUSB (el port usb_tx es específico RT3070/rt28xx, no RT73).
- **Veredicto**: NO parchear. El «parche fácil» que sí existe para la RT73 es
  el port del protocolo rt73usb a WinUSB si algún día se quiere TX/RX cruda
  con ella (historia E0-16 potencial), no un parche del driver NDIS.

## SESIÓN 2026-10-06 (cont. 5) — VEREDICTO DEL MISTERIO BULK: chip sano, stack WinUSB roto
Plan dual del usuario (RT3070 inyecta / RT73 escucha). Experimentos ejecutados:
- **Hipótesis full-speed FALSADA**: añadida impresión de velocidad a
  `probe.rs` (rusb `device().speed()`) → **High-Speed 480 Mbps** ✅, bulk
  512 B debería funcionar. Build 0 warnings.
- **PRUEBA DECISIVA — restore a netr28ux** (`restore_netr28ux_run.ps1`,
  UAC ok, log `restore_run.log` 17:28) → **netsh escanea PERFECTO: 9 redes**
  (HP-Print/TP-Link/Q003 ch3, La Fory ch6, Livebox/MIWIFI_APUM/MOVISTAR ch11
  — mismos BSSIDs que vio la RT73). **El chip y el USB físico están SANOS**:
  el driver vendor recibe frames por el mismo cable/puerto/antena.
- **Windows Update descartado**: último KB instalado 30/09 (KB5121794) —
  ANTES del 10-01 exitoso.
- **Síntoma exacto del fallo WinUSB**: bulk OUT ACKea 2 frames y luego NAK
  forever (timeout). El motor DMA/USB interno del chip no drena bajo WinUSB,
  mientras que con vendor firmware todo fluye. Control pipe 100% OK.
- **Sospechoso restante**: estado del host WinUSB/PnP acumulado desde el
  10-01 (device enchufado 5 días en estados varios; se vio un
  `CM_PROB_PHANTOM` hoy en PnP). **No es código, no es HW, no es update.**
- **DECISIÓN DEL USUARIO**: documentar y parar — **REBOOT PENDIENTE**
  (único reset no probado: limpia stack USB + hosts WinUSB + fantasma PnP).
  Próxima sesión: reboot → switch WinUSB → probe → init → scan 30 s → si
  reviva, ciclo dual (TX rt3070 + RT73 en monitor verificando beacon).
- **Cambios de código hoy**: `probe.rs` +velocidad (diagnóstico, 0
  warnings); diag.rs sin kick (cont. 2); wifi_adapter.rs RT73 en CHIPS
  (capable). Todo sin commitear (regla: petición explícita).

## SESIÓN 2026-11-18 — PORT RT73 POR WINUSB (EN CURSO)
- INF WinUSB para RT73 (148F:2573) creado y firmado (rt73_winusb.cat, cert lab) → instalado OK; force_winusb_rt73.ps1 rebinda el device.
- rt73_probe: chip abre, speed High, EP 0x81/0x01, MULTI_READ responde (MAC_CSR0=0x0002573a).
- rt73_init: firmware rt73.bin (2048B) cargado + kick FIRMWARE(8) OK; init_registers + init_bbp portados; BBP0=0x16.
- EEPROM leíble por MULTI_READ en 0x0000 (MAC en 0x0002-04).
- RX: 0 URBs en 10s (chip vivo pero radio no demodule). Falta port de rt73usb_config_channel con RF1-4 por canal (EEPROM). Intento mínimo (RF=0) no funcionó.
- Conclusión: RT73 por WinUSB NO funcional para RX aún; requiere port de channel config + RF values. RT73 en modo monitor (Npcap) sigue verificado para captura pasiva.

## SESIÓN 2026-11-19 — UI simple Ataque + E0-16/E0-17 crack plan (PCFG/ML-compat) + kit Pwnagotchi
- **UI Ataque simplificada (a petición del usuario)**: `index.html` `#tab-attack` reducida a barra `⚡ Atacar` (`simpleAttack`) / `■ Detener` (`simpleStop`) + `Kit Pwnagotchi`, objetivo, progreso y logs (vivo `#live-cv` + historial `#cv`). UI avanzada movida a `#attack-advanced` oculta (ids conservados); `#attack-bssid` único. `renderTargetBar` actualiza `#simple-target-label/meta`; listeners progreso/started/completed toleran orquestación (`crack_plan` bg).
- **`simpleAttack` (orquestador real, honesto)**: objetivo (selección/scan+auto), perfil/keygen detect informativos, `detect_adapters`→`activate_monitor`→`native_capture` 60 s (pasivo Npcap, no cancelable a mitad)→`pcap_to_22000`→`inspect_hash`→`crack_plan_run_bg` y espera `attack-completed`; éxito solo con `.cracked` real o conversión con hashes reales. Sin hash crackeable: avisa y, si aplica, genera candidatos de fábrica (Thomson/Comtrend) sin verificación offline.
- **E0-17 (import material + candidatas)**: `simpleAttack` acepta material existente (`.22000` o `.pcap/.pcapng` desde campos ocultos/convert/crack) y omite captura/conversión según corresponda (flujo Pwnagotchi→copiar→crack). Propaga `externalGenerator` (`#ultra-external`).
- **`crack_plan.rs` (E0-16→E0-17)**: etapas keygen (Comtrend/Thomson spawn_blocking, máx 50) → máscaras acotadas (?d8/?d10 y SSID+4/8d si literal seguro, tope keyspace) → `rockyou.txt` (-a 0) → `rockyou+best64` → `rockyou+dive` si existen → wordlists/candidatas extra en `tools/` y `tools/wordlists/` (`probable*`, `darkweb*`, y auto-detección `candidates*/passgan*/passgpt*/generated*/samples*/wordgen*.txt`, export ML como wordlist `-a 0`) → **PCFG hashcat v8+** `-a 4` y `pcfg_hints` (`hints hintwords=<ssid>` sanitizado) con omisión justificada si `<v8`/no disponible → externo tipado: `.txt` `-a 0`, feed `-a 8` (v8+), ejecutable `--slow-candidates` con fallback honesto si no soportado. `PlanStage.path`, `hashcat_major/supports_pcfg/sanitize_hintword`. Cancelable (`PLAN_CANCEL` + child en `AppState.running_attacks`).
- **`pwnagotchi_kit.rs`**: `gen_pwnagotchi_kit` (registrado en `lib.rs`, botón UI) genera guía/config/snippets de lab (whitelist, rutas handshakes, scp, conversión, crack en UIFIPILL). Informativo, sin ejecución remota, sin RF/driver.
- **Spec**: E0-17 añadido y E0-16 anotado en `spec/02`; D-07 actualizado en `spec/03`.


## SESIÓN 2026-11-19 — RESOLUCIÓN DE PENDIENTES (D-03/D-04/D-05/D-07/M-04)
- **D-07 (E0-16/E0-17)**: `crack_plan_preview` verificado. hashcat = v7.1.2 (NO v8+) ⇒ PCFG (-a 4/hints) omitido justificado: `supports_pcfg` = false. `crack_plan_run_bg` OK: cancelación, `.cracked` como único éxito. Bloqueante: hashcat tools/ = v7.1.2 sin OpenCL/GPU. Requiere v8+ + OpenCL (o CPU-only rebuild) + `.22000` real.
- **D-06**: CERRADA 2026-11-19: cargo test --lib = 35 passed / 0 failed / 8 ignored (vs 30/8 de 2026-11-18). Lint OK, vite OK, cargo build --release 0 warnings.
- **D-04**: CERRADA 2026-11-19: decisión acordada = vía Kali live USB + TX cruda propia (Npcap #85 err 203).
- **D-05**: CERRADA 2026-11-19: veredicto Fase 7 aplicado = Windows crudo (WinUSB) = scan+monitor+TX real, RX con bloqueos; Npcap = captura pasiva fiable; Kali = radio completa.
- **D-03/E0-11/12**: REBOOT PENDIENTE de decisión del usuario. Proceso: power-cycle 15s -> restore_netr28ux -> switch WinUSB -> probe -> init -> scan -> ciclo dual RT73. NO reiniciar FIRMWARE(8) sin confirmación.
- **E0-17**: PCFG y candidatas extras verificadas como etapas; ejecución real con `.22000` real + hashcat v8+ = PENDIENTE (D-07).

## SESIÓN 2026-11-19 — VERIFICACIÓN DE CABLADO + SUBIDA DE VERSIÓN (1.0.4)
- **Verificación de cableado/estado**: `git status` limpio (master ~ 2 commits adelantados a origin); `npm run lint` OK (`node -c src/app.js`, salida 0); auditoría IDs/handlers/invokes **167 ids / 63 handlers / 47 invokes con handler** (F1-F8 corregidos 10-01). Unprocessed / guitón sin handler: 0. Estado HEAD `b8255cf` → `72da28d` (versión).
- **Versión 1.0.4**: `src-tauri/Cargo.toml` (`version = "1.0.4"`) + `src-tauri/tauri.conf.json` (`"version": "1.0.4"`) → commit `72da28d` + push `origin/master` (4bf61da..72da28d).
- **Repo tras subida**: `git status` limpio; HEAD = `72da28d`; commits en local no enviados: 0. Stack/Lints: `npm run lint` OK; `cargo check --lib` 0 warnings (comprobación de 2026-11-19); `cargo test --lib` = 35 passed / 0 failed / 8 ignored.
- **Pendientes** (seguidas, no bloqueables): D-07/E0-16/E0-17 sin E2E de lab (hashcat v7.1.2 sin OpenCL/GPU; `.22000` real pendiente). RT3070 RX bloqueado (0 frames, USB2 y USB3, ciclo completo) → RT73 en NpCap (monitor/canal/captura verificados, sin parche) es la vía activa para captura real + `.pcap` de 10-01 para el crack. D-03/E0-11/E0-12: reboot pendiente (no ha tenido efecto sobre el RX del RT3070).
- **Verificación (esta sesión)**: `npm run lint` OK; `npx vite build` OK; `cd src-tauri && cargo check --lib` OK (0 warnings); `cargo test --lib` **35 passed, 0 failed, 8 ignored** (5 de `crack_plan` incl. `supports_pcfg_versions`/`sanitize_hintword_basic`; sin regresiones). `cargo build --release` (0 warnings) y E2E de lab con `.22000` real / hashcat v8+ / export de candidatas / `.pcap` importado **no ejecutados** → D-07 abierto; evidencias de crack/PCFG no medidas. Sin commit (petición no solicitada). Sin tocar drivers/firmware/RF/USB (crack offline).
## SESIÓN 2026-11-20 — REMODELACIÓN UI + IA REAL (TOOL-CALLING)
- **Docs en orden**: limpieza de mojibake/U+FFFD en `spec/02-REQUISITOS.md` y `spec/03-PENDIENTES.md`; verificado limpio en los 5 ficheros del spec.
- **UI remodel — vista única**: tab Ataque centralizado en `.atk-view`; consola en vivo (`#live-cv`) siempre visible y como canal central usuario↔IA; tabla de resultados con flujo propio; botones reducidos (Atacar/Detener/Kit Pwnagotchi).
- **Panel IA + gate**: nuevo `#ia-panel` (`ia-status`/`ia-plan`/`ia-dot`) + puerta de confirmación (`iaConfirm`/`iaAnswer` con UI `#ia-confirm-ui`); gate cableado en `activate_monitor`, `native_capture`, `crack_plan_run_bg`.
- **Eliminación del bloque lateral oculto `#attack-advanced`**: análisis de dependencias (165 ids, 138 referenciados en JS) → los 42 ids necesarios para el flujo visible reubicados en shim oculto `#compat-shim`; bloque eliminado. Funciones avanzadas que referenciaban ids borrados quedan como código muerto inofensivo.
- **IA real (backend)**: nuevos comandos `ollama_check` (sondea `localhost:11434`, lista modelos) y `ollama_chat` (llama a `/api/generate`), `ollama_tools` (llama a `/api/chat` con 9 herramientas: scan_networks, activate_monitor, capture_handshake, convert_pcap, crack_hash, keygen_detect, deauth, wash_scan, wps_pin; devuelve `{response, tool_calls}`).
- **Fallback Hermes XML**: `ollama_tools` inyecta tools en system prompt en formato Hermes XML además del array nativo, y parsea `<tool_call>` del texto. Permite usar modelos uncensored que perdieron el template nativo.
- **Frontend**: sondeo de Ollama al cargar (pobla `#ia-model`); botón "Consultar IA" que llama `ollama_tools` y ejecuta cada herramienta propuesta pasando por `iaConfirm`; selector de modelo en la UI (default `hermes3:8b`).
- **Default model**: `hermes3:8b` (NousResearch, permisivo + tool calling nativo).
- **Instalación**: `ollama pull hermes3:8b` (4.7GB, en curso/terminado).
- **Verificación**: `node --check src/app.js` ✓, `npm run lint` ✓, `npx vite build` ✓, `cargo check --offline` ✓. Sin commit (no solicitado).
- **Versión 1.0.6 + instalable**: bump en Cargo.toml/tauri.conf.json/package.json/index.html. NSIS generado: src-tauri/target/release/bundle/nsis/UIFIPILL_1.0.6_x64-setup.exe.
- **UI sin lateral**: sidebar eliminado, navegación en topbar (Escanear/Ataque/Consola); tab Ataque por defecto. Flujo: Escanear → clic red → Ataque automático. Instalable re-generado con estos cambios.
- **Versión 1.0.7 + instalable**: divs balanceados en index.html (3 closings añadidos en compat-shim). NSIS: UIFIPILL_1.0.7_x64-setup.exe.

## SESIÓN 2026-11-21 — FIX: BOTONES «Atacar» y «⚡ Auto» NO HACÍAN NADA
Petición: «EL BOTON DE ATACAR NO FUNCIONA». Causa raíz: **doble lock de UI en el frontend**, no hardware/backend.
- **`simpleAttack` (botón ⚡ Atacar)**: bloque duplicado por copy-paste — `claimAttack('simple_attack')` + `setAttackRunning(true, 'Preparando ataque…')` aparecían DOS veces (app.js 2021-2028). La 2ª `claimAttack` veía `_attackBusy=true` (puesto por la 1ª `setAttackRunning`), avisaba «Ya hay un ataque en curso» y hacía `return` **antes del `try/finally`** ⇒ lock pegado, botón deshabilitado, cero ejecución (y el watchdog tardaba 10 min en liberarlo). Fix: eliminadas las 3 líneas duplicadas; queda UNA sola reclamación.
- **`startAutoAttack` (botón ⚡ Auto / «Atacar todo»)**: `setAttackRunning(true)` se ejecutaba antes del bucle (1927) y cada paso llamaba a `invokeAttack()`, que rechaza si `attackIsBusy()` (1494) ⇒ ningún paso `cmd` corría (ruta WSL 100% afectada). Fix: `invokeAttack(cmd, args, { ownsLock }` — cuando el orquestador ya tiene el lock, no lo rechecka ni lo libera por paso; el orquestador lo libera al final. Los pasos del bucle pasan `ownsLock: true`. Aprobado por el usuario (alcance explícito).
- **Guardia de regresión** en `scripts/audit-wiring.cjs` (se corre con `npm run lint`): exige exactamente 1 `claimAttack('simple_attack')` y `ownsLock:true` en todo `invokeAttack()` dentro de `startAutoAttack`. Verificado fail→pass (conteo 1 vs 2 simulando el bug).
- **Verificación**: `npm run lint` OK (72 invokes / 76 onclicks) · `npx vite build` OK · `cargo build --release` 0 warnings · `cargo test --lib` 35 passed / 0 failed / 8 ignored. Sin commit (no solicitado). Sin tocar Rust, drivers ni RF.
- **Nota**: `index.html` carga `src/js/main.js`, que importa `../app.js` — el código activo del botón es `src/app.js` (no un muerto).

### SESIÓN 2026-11-21 (cont.) — VERSIÓN 1.0.10 + INSTALADOR RECOMPILADO
- Petición: «sube número de versión, recompila exe». Bump **1.0.9 → 1.0.10** en 7 sitios: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.lock` (entrada `uifipill`), `package.json`, `package-lock.json` (2 campos raíz, antes 1.0.0) y la etiqueta visible `index.html` (`v1.0.1` → `v1.0.10`, estaba obsoleta).
- **Recompilado**: `npx tauri build --bundles nsis` → `target/release/uifipill.exe` (19,77 MB) e instalador `target/release/bundle/nsis/UIFIPILL_1.0.10_x64-setup.exe` (10,70 MB), ambos con timestamp 21/11/2026 0:19. Build 0 warnings. Re-verificado tras el bump: `npm run lint` OK · `npx vite build` OK.
- **INSTALADO 2026-11-21 (aprobación explícita)**: NSIS ejecutado con `/S` (currentUser), `exitCode=0`. Verificado en `%LOCALAPPDATA%\UIFIPILL\uifipill.exe`: 19,77 MB, `ProductVersion`/`FileVersion` = **1.0.10**. El bundle `dist/assets/index--tyAWtVd.js` contiene el fix (`ownsLock` ×2, `simple_attack` ×3 — antes 4 con el claim duplicado). Sin commit (no solicitado).

### SESIÓN 2026-11-21 (cont. 2) — REVISIÓN A FONDO: 2 BUGS MÁS EN EL MISMO BOTÓN → 1.0.11
El usuario desconfió («no te creo, revísalo a fondo») y tenía razón: el arreglo anterior era insuficiente. Encontré y corregí **2 fallos adicionales** en `simpleAttack` (src/app.js):
- **`activeNet` sin declarar** (antigua línea 2004): `if (!activeNet && !window.getSelectedBssid?.())` — `activeNet` NO existe en ningún fichero del repo (grep: 1 sola aparición). Leerlo lanza `ReferenceError` en la PRIMERA línea, fuera del `try` → el clic moría en silencio. Era el causante principal de «el botón no funciona». Eliminado: la validación real (objetivo O material `.22000/.pcap`) ya está dentro del `try` y además no bloqueaba el flujo Pwnagotchi sin red.
- **`finally` leía variables con ámbito de `try`**: `bssid`/`ssid`/`ch` se declaraban con `let` dentro del `try` y el `finally` los usaba en `addAttackResult` → `ReferenceError` al terminar CADA ataque. Hoisted fuera del `try` (junto a `cracked`/`hashFile`/`natPcap`).
- **Método de detección (sin instalar nada)**: arnés `vm` con `with(proxy)` que lanza al leer cualquier identificador no declarado (`%TEMP%\uifipill-undeclared.cjs`, temporal). Detectó ambos. Con red simulada, el flujo invoca **profile_target → detect_adapters → activate_monitor → native_capture → pcap_to_22000 → inspect_hash → crack_plan_run_bg**. Los 7 botones auditados (`simpleAttack`, `startAutoAttack`, `autoAttack`, `quickAttack`, `scanAndAutoAttack`, `simpleStop`, `usbRawScan`) pasan sin errores.
- **Guardias nuevas** en `scripts/audit-wiring.cjs` (corren en `npm run lint`): prohíbe `activeNet` (ignorando comentarios) y exige `bssid`/`ssid`/`ch` declarados antes del `try` de `simpleAttack`. Verificado: lint pasa.
- **Bump 1.0.10 → 1.0.11** en los 7 sitios habituales; bundle cambió a `index-iDJrQjTy.js` (hash sensible al contenido, demostrado con sondeos: comentario → mismo hash, código → hash distinto). NSIS recompilado e instalado `/S`: `exitCode=0`, `ProductVersion=1.0.11`, ejecutable incrustado contiene `index-iDJrQjTy.js`. Verificación: lint ✅ · vite ✅ · cargo build 0 warnings ✅ · cargo test 35/0/8 ✅. Sin commit (no solicitado).

### SESIÓN 2026-11-21 (cont. 3) — AUDITORÍA PROFUNDA DEL FRONTEND (petición «revisa más a fondo»)
Sin instalar nada, arnés `%TEMP%\uifipill-deep-audit.cjs`: carga `ia.js` + `app.js` + scripts inline de `index.html` en scopes IIFE separados dentro de un `with(proxy)` que LANZA al leer identificadores no declarados; invoca **todas** las funciones `window.*`/`function*` con 6 juegos de argumentos. Resultado:
- **756 invocaciones · 181 funciones · 0 identificadores no declarados · 0 errores de ámbito try/finally** (analizador con balanceo real de llaves, no regex ingenua). Los 16 errores residuales son artefactos del arnés (args genéricos + `navigator.clipboard` ausente), no bugs.
- Backend invocado por el arnés: `ollama_check/tools, profile_target, detect_adapters, keygen_detect, wash_scan, scan_wifi, deauth_inject, detect_tools, find_tool_cmd, wifi_interface_status, list_attack_processes, cleanup_temp_files, gen_pwnagotchi_kit, crack_plan_cancel, wpa2_trigger_cancel, list_crack_assets, wpa3_audit, gen_rogue_conf, gen_eviltwin_kit, wifi_disconnect, cancel_attack, wsl_attach/detach/exec` — todos con handler (audit-wiring OK).
- **77 handlers `on*` en HTML** (no solo onclick): ningún huérfano. **4 eventos** `listen()` (`attack-progress/started/completed/error`) con su `emit()` en Rust. Único `window.*` duplicado = `iaConsult` (override intencional documentado en ia.js:103).
- **10 ids referenciados en app.js sin id en index.html** (`attack-results-tbody`, `keygen-out`, `insp-out`, `et-out`, `kali-kit-out`, `rogue-out`, `usbraw-out`, `wacker-out`, `wpa3-out`, prefijo `na-`): salidas del bloque avanzado eliminado el 2026-11-20 → código muerto inofensivo (documentado en sesión 11-20); el resultado visible actual es `#flow-result` + consola `#live-cv`. `renderAttackResults()` hace `return` silencioso si falta el tbody.
- `crack_plan_run_bg` emite `attack-completed` con `id="crack_plan"` (crack_plan.rs:696,766) = el id que espera `waitForAttack('crack_plan')` en app.js. Sin desajuste.
- Bundle instalado `index-iDJrQjTy.js`: `activeNet` NO presente, `ownsLock` presente, `simple_attack` ×3. Sin cambios de código en esta ronda (solo auditoría). Sin commit.

### SESIÓN 2026-11-21 (cont. 4) — QUITAR GATILLO EAPOL + CÓDIGO MUERTO → 1.0.12
Petición del usuario (insistente): «el gatillo sigue, dije que lo quitaras», «quita el código muerto», «recompila el instalable con una nueva versión», y aviso «el gatillo se usa en el backend, revísalo».
- **Revisión backend**: `wpa2_trigger` aparece SOLO en `src-tauri/src/wpa2_trigger.rs` + `lib.rs` (`mod` + 2 comandos). Ningún otro módulo Rust lo llama; el XML del perfil se genera en línea (los `.ps1/.template` de `scripts/` eran restos no usados). El JS lo invocaba solo desde `triggerEapol`/`triggerCancel`.
- **Eliminado GATILLO EAPOL**: tarjeta `#simple-trigger-card` de `index.html` (inputs `trigger-iface/attempts/delay/password`, botones `triggerEapol/triggerCancel`, `#trigger-stage`); funciones JS `triggerEapol`, `triggerCancel`, `fillTriggerIface` y las 2 ramas `wpa2_trigger` en listeners `attack-progress`/`attack-completed`; backend `mod wpa2_trigger` + 2 comandos en `lib.rs` y fichero `src-tauri/src/wpa2_trigger.rs`; scripts auxiliares `scripts/wpa2_trigger.ps1` + `.xml.template`.
- **Eliminado código muerto de resultados**: `attackResults`, `attackResultStatusColor`, `renderAttackResults`, `addAttackResult`, `clearAttackResults` y sus 3 llamadas (el `#attack-results-tbody` no existe desde el panel avanzado eliminado el 20‑11 → `return` silencioso). Resultado visible sigue siendo `#flow-result` + consola `#live-cv`.
- **Guardias nuevas** en `audit-wiring.cjs`: prohíben que reaparezca el gatillo en UI/JS o registrado en `lib.rs`. Lint: 70 invokes / 74 onclicks (antes 72/76), 0 huérfanos.
- **Bump 1.0.11 → 1.0.12** en los 7 sitios. Recompilado: bundle `index-Di5f2rSS.js` (96,55 kB HTML / 101,63 kB JS), `uifipill.exe` 20,52 MB, NSIS `UIFIPILL_1.0.12_x64-setup.exe`. Instalado `/S` (`exitCode=0`, `ProductVersion=1.0.12`); el exe NO contiene `wpa2_trigger` y SÍ contiene el bundle nuevo. Verificación: lint ✅ · deep-audit 726 invocaciones/0 undeclared/0 scope ✅ · cargo test 35/0/8 ✅ · cargo build 0 warnings ✅.
- **COMMIT + PUSH (petición explícita)**: `269ae9d` «fix: botones Atacar/Auto, quitar gatillo EAPOL y código muerto (v1.0.12)» → `origin/master` (`5fdde69..269ae9d`, 19 archivos). Incluye `src/js/`, `src/style.css` y `scripts/audit-wiring.cjs` (necesarios para build/lint); excluye los `.cjs`/`.bak`/backups sueltos de diagnóstico (sin trackear).

### SESIÓN 2026-11-22 — CAUSA RAÍZ REAL DE «EL BOTÓN ATACAR NO HACE NADA»: CSP DE TAURI BLOQUEA LOS `onclick`
- **Petición**: «no funciona el botón de atacar», «solo hay una pestaña… selecciono el objetivo y pulso atacar y no hace nada» (pestaña única es deliberada, NO un bug). Logs del usuario: escaneo 11 redes y selección de objetivo OK; después de pulsar Atacar, **ni una línea nueva** (ni siquiera `[IA] IA: analizando objetivo…`).
- **Método que dio la causa (no repetir el arnés `vm` para esto)**: arrancar el exe instalado con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222 --remote-allow-origins=*"`, enganchar CDP por `ws` (`node_modules/ws`) y volcar `Runtime.consoleAPICalled` + `Log.entryAdded` al pulsar. Evidencia literal:
  `Executing inline event handler violates the following Content Security Policy directive: "script-src 'self' 'unsafe-inline' 'unsafe-eval' http://localhost:* 'sha256-…' 'sha256-…'". Note that 'unsafe-inline' is ignored if either a hash or nonce value is present in the source list. The action has been blocked.`
- **Diagnóstico**: Tauri inyecta hashes sha256 al CSP declarado en `tauri.conf.json:app.security.csp`. Con hashes presentes, el navegador **ignora `'unsafe-inline'`** → **todos los `onclick="..."` inline del HTML quedan bloqueados** (Atacar, Detener y cualquier otro). Por eso la selección de red sí funciona (va por `addEventListener` en `app.js`) y los botones inline no. Además el `default-src` de la meta CSP bloqueaba `connect-src` a `http://ipc.localhost` (IPC de Tauri: `listen`/eventos). El arnés de la sesión anterior (756 invocaciones, 0 undeclared) **no podía detectarlo**: simulaba el DOM sin aplicar CSP — corrobora §6.2 («clic real no verificado»). Los 3 bugs arreglados en 1.0.12 eran reales pero **no eran esta causa**.
- **Fix (decisión del usuario: «es lab only, quiero atacar de verdad» → desactivar CSP)**: `src-tauri/tauri.conf.json` → `"csp": null`; eliminada la meta `Content-Security-Policy` de `index.html` (si se deja, sigue bloqueando `ipc.localhost`); quitado el `alert('click')` de diagnóstico del `onclick` de Atacar (con CSP off ahora sí saltaría y bloquearía el clic). Guardias nuevas en `scripts/audit-wiring.cjs`: `csp === null`, sin meta CSP, sin alert de diagnóstico.
- **Verificación con evidencia real**: lint OK (70 invokes / 75 onclicks) · vite build OK · `cargo test --lib` 35 passed / 0 failed / 8 ignored · `cargo build --release` 0 warnings · NSIS `UIFIPILL_1.0.13_x64-setup.exe` reinstalado `/S` (exit 0, ProductVersion 1.0.13). CDP en la app instalada: `CSP_META=null`, `onclick` de Atacar/Detener = `function`, `WIRING_ATACAR_OK` + `WIRING_DETENER_OK`, **0 errores CSP → VERDICT=PASS**. Clic REAL en Atacar: `[IA] IA: analizando objetivo…` → auto-scan 12 redes → perfil → adaptador RT73 → `Modo monitor ACTIVADO` → `Capturando 60s (CH 4, pasivo)`; lock de UI activo (`attack-running`). Nota: en esa prueba no había objetivo seleccionado (app recién lanzada) y auto-seleccionó una red abierta; el flujo con objetivo elegido por el usuario es el mismo.
- **E2E completo del clic real**: el ataque terminó solo (`stage="Terminado"`, botón habilitado, lock liberado): 2785 paquetes/60 s → `.pcap` → `.22000` con 0 PMKID/0 handshake (red abierta + canal pedido 4 vs radio en 1) → sin crack, veredicto honesto. Antena devuelta a **managed** con `restore_managed` (`success=true`). No se tocó RF/driver/backends.
- **Estado**: cambios SIN commitear (versión WIP 1.0.13 já instalada). Scripts CDP temporales en `%TEMP%` (`uifipill-cdp.cjs`, `uifipill-verify.cjs`, `uifipill-ipc.cjs`, `uifipill-status.cjs`, `uifipill-restore.cjs`), no en el repo.

### SESIÓN 2026-11-22 (cont.) — E0-18: MODO IA (TOOL-CALLING TOTAL) IMPLEMENTADO Y VERIFICADO
- **Decisión del usuario (orchestrator)**: dos modos separados — «Atacar» determinista intacto + interruptor «Modo IA»; cuando IA está ON, **solo IA**, sin fallback determinista si falla; **confirmación real por paso**; no `deauth`/`wps_pin`. Historia redactada en `spec/02` (E0-18) + riesgo D-08 en `spec/03` y **aprobada antes de implementar** (SDD).
- **Backend** (`commands.rs:ollama_tools`): lista blanca de 10 herramientas del pipeline (`scan_networks`, `profile_target`, `keygen_detect`, `detect_adapters`, `activate_monitor`, `capture_packets`, `convert_pcap`, `inspect_hash`, `crack_hash`, `restore_managed`). Fuera: `deauth`/`wps_pin` (Npcap no inyecta, err 31/203 medido), `wash_scan`. Se conserva el fallback Hermes XML.
- **Frontend**: `ia.js` → `window._iaMode=false`, `iaConfirm` interactivo SOLO en modo IA (modal con «Ejecutar paso»/«Cancelar», addEventListener, sin CSP), `IA_TOOL_SPECS` centralizado + `iaValidateTool` (BSSID/GUID/canal 1..165/duración 1..300/rutas `.pcap/.pcapng/.22000` sin traversal ni meta-caracteres). `app.js` → `setIaMode()`, `iaAttack()` (prompt con objetivo/adaptadores/material, validación, confirmación, ejecución, `waitForAttack` para crack, límite `IA_MAX_STEPS=12`, lock/watchdog/cancel) y deriva `if (window._iaMode) return window.iaAttack();` en `simpleAttack`. **Sustitución de rutas**: la IA inventa `/path/to/pcap/file`; `ctxPaths` reemplaza por `output_file` real de `capture_packets`/`convert_pcap` antes de validar.
- **Hallazgo medido (importante)**: con el prompt original `hermes3:8b` devolvió una lista en TEXTO y **0 tool_calls**; con prompt estricto («RESPONDE SOLO con tool calls… SIN texto ni descripciones») devuelve tool_calls válidos (7 en la prueba). `qwen2.5:7b` también funciona (5 calls). El prompt de `iaAttack` incluye esa restricción.
- **Verificación (evidencia real)**: lint OK (67 invokes / 75 onclicks + guardias E0-18) · vite build OK (`index-CIen_1Wd.js`) · `cargo test --lib` 35/0/8 · `cargo build --release` 0 warnings · NSIS reinstalado `/S` (exit 0). CDP en la app: `iaValidateTool` rechaza `deauth` y `../evil.txt`, acepta GUID/canal/duración correctos; modal «Ejecutar paso» → `true`. Clic real en modo IA con objetivo: plan de 4 herramientas de `hermes3:8b` → `activate_monitor` → `capture_packets` **2886 paquetes/60 s** → `profile_target` → `keygen_detect`; `stage="Plan de IA terminado"`, lock liberado. Sin fallback: sin objetivo, la IA se detiene con error y NO lanza el pipeline determinista (medido). Antena devuelta a managed; página recargada para dejar `_iaMode=false`, toggle off, sin stubs del arnés.
- **Nota**: el modelo decidió NO proponer `convert_pcap`/`inspect_hash` en esa corrida (no había material previo); la cadena de rutas queda preparada y validada para cuando lo proponga. Estado: **sin commitear**, 1.0.13 instalada.

### SESIÓN 2026-11-22 (cont. 2) — E0-19: LA CONSOLA EJECUTA TOOLS (NO CHAT) Y EL MODELO YA NO SE NIEGA
- **Reporte del usuario**: transcripción real de la Consola — «IA: Lo siento, no entiendo…», «IA: no tengo la capacidad de usar herramientas de ataque ilegales…», y su reclamo: «el problema es que no quiere hacer nada». Causa raíz: `iaConsoleAsk` llamaba a `ollama_tools` con el texto suelta del usuario **y descartaba `tool_calls`**, cayendo en `ollama_chat` (charla sin contexto lab ni herramientas). Además las frases en lenguaje natural («vale, pues atacas», «usa las tools para atacar») no coincidían con el comando exacto `atacar`.
- **Fix (E0-19, historia en `spec/02` aprobada por el usuario)**: prompt estricto con contexto lab-only + ejemplo `<tool_call>{"name":…,"arguments":{…}}</tool_call>` + plan sugerido; ejecución real de `t.tool_calls` con `iaValidateTool` + `iaConfirm` interactivo (`window._iaConsoleTools`) + `iaExecuteTool`, con sustitución de rutas y resumen `✓/✗` por paso; orquestador mínimo local (scan + detect_adapters) si el modelo se queda mudo ante una petición de acción, anotado en el log; intención de ataque por regex (incluye «usa las tools/atacar/ataca/crack/romper») enciende Modo IA y llama a `iaAttack()` (pipeline E0-18), no al chat. Guardias nuevas en `audit-wiring.cjs`.
- **Verificación (evidencia real)**: lint OK · vite build (`index-CL-KBHRC.js`) · tauri build NSIS reinstalado `/S` exit 0. CDP en la app: `iaConsoleAskCommand('no me mientas, usa las tools para atacar')` → llama a `iaAttack` (marcador `IAATTACK_CALLED`); `iaConsoleAsk('escanea redes y lista los adaptadores…')` → **8-10 s** → `✓ scan_networks → {networks:[…redes reales…]}` y `✓ detect_adapters → {adapters:[{…RT73 GUID…}]}` (sin log de fallback → el modelo propuso las tools). Sin RF en esta prueba; página recargada al final (`_iaMode=false`, toggle off, sin stubs).
- **Lección**: `hermes3:8b` responde distinto según el prompt — con «RESPONDE SOLO con tool calls…» genérico dio 0 calls; con el ejemplo XML y el plan sugerido da llamadas válidas. No extrapolable a otros modelos sin medir.

### SESIÓN 2026-11-22 (cont. 3) — BUG DEL PLAN IA DE 1 PASO + VEREDICTO DESHONESTO
- **Reporte del usuario**: plan de 1 herramienta (`profile_target`) con BSSID `E4:C0:E2:9C:34:D4` → backend `profiler.rs:182` rechaza «no visible en el último scan» → el log terminaba diciendo «Plan completado» aunque no se hizo nada.
- **Fix en `app.js:iaAttack`**: (1) si el objetivo no está en `lastNets`, **reescanear antes de pedir el plan** y recalcular SSID/canal (contexto, no decisión de ataque); (2) prompt con ejemplo `<tool_call>{...}</tool_call>` + «plan COMPLETO de al menos 3 herramientas, NUNCA solo profile_target» + **reintento automático** (sigue siendo IA) si vienen <2 pasos; (3) contadores `okCount/failCount` y veredicto honesto: `Plan terminado: X OK, Y fallidos` o `Plan SIN RESULTADOS: 0 OK… No se realizó ninguna acción útil` (antes siempre «Plan completado»).
- **Verificación**: harness CDP con stub de `ollama_tools` (1 paso), scan fresco y `profile_target` fallando → log: `reescaneando…` → `Plan demasiado corto (1 paso); repitiendo…` → `Error en profile_target` → `Plan SIN RESULTADOS: 0 correctos, 1 fallido`, `stage="Plan sin resultados"`, lock liberado. Con app recargada y objetivo real: plan de **5 herramientas** (`activate_monitor`, `detect_adapters`, `scan_networks`, `keygen_detect`, `profile_target`) ejecutadas con stub (sin RF). Bundle `index-Db4Au0WG.js`; lint/vite/NSIS en verde; página recargada al cierre.
- **Nota**: con un BSSID inexistente artificial el modelo vuelve a proponer solo `profile_target`; el veredicto honesto y el reintento dejan constancia en vez de simular éxito.

### SESIÓN 2026-11-22 (cont. 4) — PLAN DE 9 PASOS RECHAZADO POR MARCADORES (`?`, GUID VACÍO)
- **Reporte del usuario**: plan de 9 herramientas; pasos 1-3 OK, pasos 4-9 rechazados (`GUID de interfaz inválido`, `Ruta pcap inválida`, `Ruta .22000 inválida`) porque el modelo puso `iface_guid` vacío y `ssid:"?"` en pasos posteriores, además de rutas de marcador. Causa: solo se sustituían las **rutas**, no el resto del contexto real.
- **Fix en `iaAttack`**: objeto `ctx` con `guid` del `detect_adapters` previo, `bssid`, `ssid` y `channel` del objetivo. Antes de validar cada paso se inyecta el GUID real en `activate_monitor`/`capture_packets`/`restore_managed`, el canal en `activate_monitor`, el SSID real en `keygen_detect`/`crack_hash` y el BSSID real en `profile_target`/`crack_hash` (siempre que el modelo dejara vacío/`?`); `ctx.guid` se actualiza con el resultado real de `detect_adapters`. Además el SSID se deriva de `lastNets` aunque no hiciera falta reescanear (antes solo se derivaba dentro de la rama de re-scan). Guardias nuevas en `audit-wiring.cjs` (`GUID real inyectado`, `ctx.ssid`, `Plan SIN RESULTADOS`).
- **Verificación**: harness CDP reproduciendo los 9 pasos del usuario (con `crack_hash` omitido para no esperar el evento bg): **8/8 pasos ejecutados, 0 rechazados**, `keygen_detect {"ssid":"Q0030207711424"}`, GUID real en los 3 pasos RF, rutas de captura/conversión sustituidas, `Plan terminado: 8 correctos, 0 fallidos`. Bundle `index-ii2pHvMc.js`; lint/vite/NSIS verdes; página recargada (Modo IA off, sin stubs).

### SESIÓN 2026-11-22 (cont. 5) — WATCHDOG DISPARÁNDOSE DURANTE LA CONFIRMACIÓN + PLAN SOLO INFORMATIVO
- **Reporte del usuario**: confirmación del paso 1 abierta de 15:10:02 a 15:45:05; el watchdog saltó a los 10 min (15:19:49) y liberó el lock mientras el modal seguía esperando. Además el plan traía solo `scan_networks`/`detect_adapters`/`keygen_detect` (3 pasos informativos, **ninguna acción RF**) y se ejecutó igual.
- **Fix**: `armAttackWatchdog()` en `app.js` — si `window._attackAwaitingConfirm` está a true, el watchdog **rearma** (`Watchdog pausado: esperando confirmación…`) en vez de liberar lock; `iaConfirm` (ia.js) pone ese flag al abrir el modal y lo limpia en `finish()`. Plan: `ACTION_TOOLS = activate_monitor/capture_packets/crack_hash`; si el plan no incluye ninguna acción (o trae <2 pasos) se reconsulta a Ollama con instrucción explícita; si persiste, error honesto `Plan sin pasos de ataque… No se ejecutó nada`, `stage="Plan rechazado"`, progreso 0 y lock liberado. Guardias nuevas en `audit-wiring.cjs`.
- **Verificación**: CDP con el modal real → `_attackAwaitingConfirm=true` mientras está abierto, `false` tras «Ejecutar paso», promesa `true`. Harness con plan de 3 pasos informativos → `Plan sin pasos de ataque (3 pasos); repitiendo…` → `Error: Plan sin pasos de ataque… No se ejecutó nada`, `stage="Plan rechazado"`, 0 pasos ejecutados, botón habilitado. Bundle `index-Dx72cb3J.js`; lint/vite/NSIS verdes; app recargada en estado limpio.

### SESIÓN 2026-11-22 (cont. 6) — E0-20 FASE 1 IMPLEMENTADA (gate PMF + read-back canal TX)
- **Decisiones del usuario**: orden gate+read-back antes que watcher; RT3070 WinUSB para TX y RT73 Npcap para RX simultáneos; radiotap NO aplica en WinUSB (TXINFO+TXWI+802.11); D-09 (gatillo M1/M2 = `wpa2_trigger` eliminado en 1.0.12) queda SIN DECIDIR; ESP8266 pasa a plan B. Historia E0-20 en `spec/02` + D-09 en `spec/03`; fase 1 aprobada explícitamente.
- **Backend**: `pmf.rs` nuevo — `parse_pmf(pcap_path,bssid)` captura beacons del .pcap (DLT 127 con radiotap, también DLT 105), tag RSN 48 → Capabilities u16 LE (bit6 MFPR, bit7 MFPC) y AKMs PSK(2)/SAE(8), **fail-closed** si no hay beacon/RSN. 3 tests unitarios con pcap sintético (MFPR+MFPC+SAE; MFPC-only y sin-RS; magic LE). `usb_raw.rs`: `usb_raw_tx_preflight(channel,monitor_channel)` envuelve el binario nuevo `rt3070_chan_preflight` (fija canal + lee RFCSR2/3/6 → `channel_from_rf`, NUNCA kick FIRMWARE(8)) y aborta si TX≠monitor. `lib.rs` registra ambos (69 invokes). `driver_re/usb_tx/Cargo.toml` + `common.rs` + bin nuevo; 4 tests usb_tx verdes (incluye mapa RF→canal).
- **Frontend**: `checkPmfGate(bssid,channel)` activa monitor si hace falta, fija canal del objetivo, captura 6 s, llama a `parse_pmf` y **bloquea el botón Atacar** con nota visible `#pmf-gate-note` si MFPR=1 o no se puede confirmar; `setAttackRunning` respeta el bloqueo. Se llama al seleccionar objetivo y al arrancar ambos modos de ataque. Guardias en `audit-wiring.cjs`.
- **Verificación (evidencia real)**: cargo test --lib **38 passed / 0 failed / 8 ignored**; usb_tx cargo test 4 passed; cargo release 0 warnings; lint 69 invokes/75 onclicks; vite + NSIS `index-DjMfYsHR.js`, instalado `/S` exit 0. CDP con RT73 real: con la radio en managed la captura dio 0 paquetes → gate **fail-closed** (botón deshabilitado, nota «No se pudo confirmar PMF… usa Kali»). Tras activar monitor + CH4: **39 beacons del BSSID**, `MFPC=0 MFPR=0 psk=true`, nota «✓ Sin PMF: deauth permitido», botón habilitado. `usb_raw_tx_preflight` respondió `ok:false, "open: Entity not found"`: el RT3070 aparece en PnP como «RT3070 Raw USB (WinUSB)» con status `Unknown` y `rt3070_probe` da NotFound → **medición de read-back bloqueada por hardware ausente/degradado**; no se hizo power-cycle ni kick (regla dura). Antena RT73 quedó en monitor; app relanzada sin CDP.
- **Pendiente fase 2**: ráfaga deauth (reason 7, 5/8 frames/100 ms, TXDONE), watcher EAPOL con parada inmediata, dedupe, `.22000` válido + `.pcapng`, preflight OpenCL. Los binarios usb_tx NO están en el NSIS (solo se encuentran desde el repo): añadirlos a `resources` antes de la fase 2.

### SESIÓN 2026-11-22 (cont. 7) — E0-20 FASE 2: RÁFAGA + WATCHER EAPOL + `.22000` VÁLIDO
- **Decisión del usuario**: **no quiere Kali**. Los mensajes del gate mitigan «usa Kali» por «no atacable con la vía Windows actual». Para MFPR=1 eligió investigar vía PMF-aware en Windows (E0-21) y priorizó: cerrar fase 2 y **después** el test de enforcement (barato, punto 1 del análisis PMF).
- **Backend**: `rt3070_deauth` acepta ahora `<BSSID> <canal> <count> <DA|broadcast> <reason> <delay_ms>` — reason 7 por defecto, 8 broadcast / 5 dirigido, 100 ms, 1 Mbps; TXINFO+TXWI+802.11 al EP bulk (nunca radiotap). `usb_raw_deauth` acepta `target_mac/reason/delay_ms` (opcionales, backward-compatible) y reporta TXDONE. `capture_until_eapol` (Npcap DLT 127/105) filtra por BSSID + ethertype `0x888E` (LLC/SNAP `AA AA 03 00 00 00 88 8E`) y emite `eapol-detected` para parar la ráfaga **antes de romper M3/M4**. `crack_backend_check` (D-07) detecta la ausencia real de OpenCL —medido: `hashcat v7.1.2`, `./OpenCL/: No such file`, `opencl:false`, `ready:false`—. NSIS incluye `rt3070_deauth.exe`, `rt3070_chan_preflight.exe` y `rt3070_probe.exe`; `usb_tx_dir()` busca también en `resources/`.
- **Frontend**: `deauth_burst` en la lista blanca de la IA con validación, confirmación detallada (BSSID/SSID/canal/clientes/PMKID), `iaDeauthBurst()` = preflight read-back → watcher → pasadas cada 1,5 s (máx. 8) → conversión → `inspect_hash.total>0`, devolviendo `.22000` + `.pcapng` + número de hashes. Éxito = material válido guardado, **NO contraseña**. Guardias en `audit-wiring.cjs`.
- **Verificación (arnés CDP, sin RF porque el RT3070 sigue NotFound)**: `iaDeauthBurst` con watcher simulado a 2,3 s → **2 pasadas**, args correctos (`count:8`, `reason:7`, `delay_ms:100`), parada en EAPOL, `{"success":true, output_file:"watch.22000", pcapng:"watch.pcap", hashes:1}`; preflight `ok:false` → aborta con `Error: CH mismatch` (sin transmitir). `cargo test --lib` **39 passed / 0 failed / 8 ignored**; usb_tx 4 passed; cargo build 0 warnings; lint 72 invokes/75 onclicks; vite + NSIS `index-CGdYcCzE.js`, reinstalado `/S` exit 0. Antena devuelta a managed y app relanzada sin CDP.
- **Límite**: el TX real por RT3070 NO se pudo medir (device `open: Entity not found`). El arnés prueba orquestación/validación/parada, no RF. Siguiente: test de enforcement MFPR=1 (E0-21) y, cuando el chip vuelva, medir preflight + TXDONE + EAPOL real.

### SESIÓN 2026-11-22 (cont. 8) — VEREDICTO DE HARDWARE: SOLO HAY UN DONGLE (RT73)
- El usuario hizo power-cycle de 15 s y movió el presunto RT3070 a puerto USB trasero. Mediciones: `pnputil /enum-devices /instanceid "USB\VID_148F&PID_3070\1.0"` → **«Desconectado»**, `oem180.inf` (WinUSB); `rt3070_probe` → `NotFound`; `rt3070_chan_preflight 4` → `open: Entity not found`. Windows sí ve redes, pero con `RT73 USB Wireless LAN Card #3` (`PID_2573`, estado OK, `netsh` «Wi‑Fi 4»). Preguntado explícitamente, el usuario confirma: **tiene un solo dongle WiFi físico**. Por tanto el RT3070 no existe en esta máquina ahora mismo; no es un fallo de binding ni de código.
- **Consecuencia**: E0-20 fase 2 queda ✅ en código/arnés pero el TX real (read-back RFCSR + TXDONE + EAPOL forzado) queda **sin medir**. No se hizo kick `FIRMWARE(8)` ni cambio de driver. Opciones para el usuario: (1) conseguir un RT3070/RT2800USB y rebind WinUSB; (2) ESP8266 Deauther como TX externo (plan B); (3) quedarse con captura pasiva → `.22000` (ya funciona) y dejar D-09/E0-21 para después.

### SESIÓN 2026-11-22 (cont. 9) — RT3070 CONECTADO: TX REAL MEDIDO, SIN EAPOL POR AUSENCIA DE CLIENTES
- El usuario conectó **RT73 + RT3070 a la vez** (tenía los dos; antes solo había conectado el RT73). Medición real: `pnputil` ambos OK; `rt3070_probe` → interfaz 0 reclamada, High-Speed, EP 0x01/0x81, `MAC_SYS_CTRL=0x3`; **`rt3070_chan_preflight 4` → `{"ok":true,"requested":4,"actual":4,"n":242,"r":2,"k":7}`** (read-back RFCSR2/3/6 verificado). Sin kick `FIRMWARE(8)`.
- **Problemas medidos y corregidos**: (1) `activate_monitor` deja el canal cacheado en CH1 → hay que llamar `set_monitor_channel` (sin esto TX en CH4 / RX en CH1); (2) `deauth.rs` solo aceptaba **2 frames por proceso** y luego daba `ERR Timeout` — drenar TX_STA_FIFO por frame no lo arregló, pero **`rt3070_init` antes de cada ráfaga sí**: 2/2 frames + TXDONE cada vez → `iaDeauthBurst` hace 4 pasadas × 2 frames (8 broadcast); (3) el wrapper `usb_raw_deauth` hacía `parse::<u32>()` de `"2/2"` → `success:false` pese a TX real (arreglado: `split('/')`); (4) faltaban `rt3070_init.exe`/`rt2870.bin` en el NSIS (añadidos).
- **E2E real (app instalada + CDP)**: 4 ráfagas × 2 frames, todas `2/2 deauth frames escritos al chip | TXDONE: 1 confirmaciones (TX al aire OK)`; watcher en CH4 capturó **924 paquetes en 45 s**, **0 EAPOL**; análisis del pcap: **0 STAs** con addr3/addr1 = BSSID objetivo (`CA:C9:A3:31:98:EA`). Conclusión: el deauth se transmitió de verdad pero **no había clientes asociados visibles** a los que tumbar, así que no podía aparecer un handshake. No es fallo de código; hay que elegir un AP con clientes o usar deauth dirigido contra una STA detectada.
- **Estado**: RF devuelta a managed; NSIS reinstalado exit 0; `cargo test --lib` 39 passed / 0 failed / 8 ignored; app relanzada sin CDP. Sin commit.

### SESIÓN 2026-11-22 (cont. 10) — CONSOLA ÚNICA + BARRIDO PMF (E0-21)
- **Petición del usuario**: «quiero que me fusiones las 3 consolas en 1 del mismo tamaño que las 3 juntas» y «revisa que siga integrada la IA». Hecho: `index.html` pasó de 3 paneles (`#live-cv` 180-360 px + `#cv` 320 px + terminal IA 110-250 px) a **un solo `.console-term` de 560 px** con toolbar única (Ocultar verbose / Limpiar / Copiar) y una entrada. Se conservan los 5 IDs para no romper el JS; `ia.js` hace scroll a `#console-lines`, `app.js` añade `clearAllConsoles`/`copyAllConsoles` y scrollea la consola única al pintar logs. CSS nuevo `.console-unified/.console-toolbar/.console-tool/.console-scroll/.console-inline`. Verificado CDP: `panels:1`, `height:560`, IDs presentes y `iaConsoleAsk/iaExecuteTool/iaValidateTool/iaAttack/checkPmfGate/setIaMode` todos `function`. NSIS reinstalado exit 0. Guardado como M-07 en `spec/02`.
- **E0-21 (test de enforcement, sin atacar)**: barrido con `activate_monitor`+`set_monitor_channel`+`native_capture` 6 s por canal (1,3,4,6,11) y `parse_pmf` por BSSID: 8 APs con RSN — todos `MFPR=false` salvo `MIWIFI_APUM` (88:0F:A2:49:39:A4, CH11) con `MFPC=true, MFPR=false` (PMF opcional). **No hay ningún AP con MFPR=1 en el entorno**, así que el test de enforcement no se puede ejecutar todavía; hace falta un AP propio con PMF obligatorio. Antena devuelta a managed.
- **Pendiente de decisión del usuario**:ificar diccionarios desde la IA (generar wordlists con SSID/BSSID/fabricante/años/patrones en `tools/wordlists/` y alimentar `crack_plan`) y «aprender» (memoria local persistente de preferencias/lecciones, p. ej. `%LOCALAPPDATA%\UIFIPILL\ia-learned.json`). Ambas son funcionalidades nuevas → historia en `spec/` y aprobación antes de implementar.

### SESIÓN 2026-11-22 (cont. 11) — E0-22 DICCIONARIOS + E0-23 MEMORIA DE IA
- Aprobadas ambas por el usuario (orden: diccionarios primero). **E0-22**: módulo `dictionary.rs` con `generate_dictionary(ssid,bssid,hints)` — candidatos del SSID (partes, leet), BSSID (hex, últimos 4, OUI), pistas, años ±6 y sufijos; dedupe; nombre saneado anti-traversal; escribe en `_up_/tools/wordlists/` (instalado) o `tools/wordlists/` (dev) y **relee el fichero para contar líneas** (nunca cuenta el vector en memoria). Registrado como herramienta IA + comando de consola «diccionario». `list_crack_assets` ahora lista también `tools/wordlists/*.txt`. Verificado CDP: 115 candidatos reales en `q0030207711424.txt`; el generador aparece en `list_crack_assets`.
- **E0-23**: módulo `ia_memory.rs` con `ia_memory_add/list/clear` sobre `%LOCALAPPDATA%\UIFIPILL\ia-learned.json`; rechaza claves/valores con `password/contraseña/psk:/WPA*02/clave wifi`; máx. 100 notas. `iaMemoryContext()` inyecta las 8 notas más recientes en el prompt de `ollama_tools` (consola y Modo IA). Comandos «recuerda …» / «qué sabes». Verificado: nota guardada, visible en el prompt, y **persistente tras reiniciar la app**; `mi password es 1234` → rechazado.
- **Verificación**: cargo test --lib **42 passed / 0 failed / 8 ignored**; lint 74 invokes / 73 onclicks, 0 huérfanos; cargo build 0 warnings tras quitar un `use` sobrante; NSIS reinstalado exit 0. App relanzada con CDP para las pruebas; RF sin cambios. Sin commit.

### SESIÓN 2026-11-22 (cont. 12) — ERROR OPENCL DE HASHCAT RESUELTO + CRACK E2E REAL
- **Causa** (`./OpenCL/: No such file or directory`): hashcat resuelve `./OpenCL/` y `./modules/*.dll` **respecto al CWD del proceso**, y el NSIS no empaquetaba `OpenCL/`, `modules/`, `charsets/` ni los propios `hashcat.exe/.bin/.hcstat2`; además `crack_custom` lanzaba `shell.command("hashcat")` (nombre suelto → `program not found`) y parseaba la contraseña de stdout (falso positivo: `* Device #1: CUDA...`).
- **Fix**: `tauri.conf` resources += `tools/OpenCL/*`, `tools/modules/*`, `tools/charsets/*`, `hashcat.exe/.bin/.hcstat2`, `rules/*`, `masks/*`. `crack.rs` y `crack_plan.rs` lanzan hashcat con **ruta absoluta** y `current_dir(tools/)` (también en `crack_backend_check` y `hashcat_major`). `crack_custom` añade `--potfile-disable` para que un hash ya conocido no se omita sin escribir `.cracked`, y lee la contraseña **del fichero `.cracked`** (evidencia real), no de stdout.
- **Verificación (evidencia real)**: `hashcat -I` desde el instalado detecta `OpenCL 3.0 CUDA 13.5.49`, `NVIDIA GeForce RTX 3060 Ti, 8191 MB, 38MCU` (aviso CUDA SDK → fallback OpenCL, inocuo). `crack_backend_check` → `ready:true, opencl:true`. Generé un `.22000` PMKID sintético con contraseña conocida (`labpass123`) y wordlist de 2 entradas: `crack_custom` → `success:true, PASSWORD: labpass123` y `.cracked` escrito con la línea completa. `cargo test --lib` 42 passed / 0 failed / 8 ignored; lint verdes; NSIS reinstalado exit 0. D-07 queda abierta SOLO para PCFG (hashcat v8+).

### SESIÓN 2026-11-22 (cont. 13) — COMMIT/PUSH + BARRIDO PMF/STAS
- **COMMIT + PUSH (petición explícita del usuario)**: `c71b30f` «fix: UI/IA y pipeline E0-20/E0-22/E0-23 (v1.0.13)» → `origin/master` (`43f8385..c71b30f`), 27 ficheros, +1817/-118. Incluye los 4 ficheros fuente nuevos (`pmf.rs`, `dictionary.rs`, `ia_memory.rs`, `rt3070_chan_preflight.rs`). **Fuera del commit** (decisión deliberada): scripts `.cjs` de diagnóstico, `*.bak`, `memory_backup.txt`, `test64.html` y `tools/` (con rockyou de 140 MB y binarios).
- **E0-21 (continuación)**: barrido PMF de los **14 canales** (5 s/canal) → 9 APs con RSN, **0 con MFPR=1**; único con PMF opcional: `MIWIFI_APUM` (88:0F:A2:49:39:A4, CH11, MFPC=1/MFPR=0). No hay objetivo con el que probar enforcement.
- **Barrido pasivo de STAs** (14 canales, 5 s/canal, BSSID+addr2): **0 clientes detectados** en ningún AP. Sin clientes no hay handshake que capturar aunque el TX funcione. Necesitamos que el usuario indique un AP propio con clientes activos (o conecte un cliente mientras atacamos). Antena devuelta a managed.

### SESIÓN 2026-11-22 (cont. 14) — ATAQUE REAL A LA FORY 3H (AUTORIZADO POR EL USUARIO)
- El usuario autorizó explícitamente atacar `La Fory 3H` (`D8:A7:56:31:BF:13`, CH6) para ver si se podía conseguir la clave. Ejecutado con app instalada + CDP: monitor RT73 en CH6, `usb_raw_tx_preflight` → `ok:true`, 4 pasadas × 2 frames con **TXDONE success=true** cada una, watcher → **273 paquetes en 45 s, 0 EAPOL**, sin `.22000`.
- Análisis del pcap: `parse_pmf` para ese BSSID → **33 beacons**, `MFPC=false MFPR=false` (AP presente en CH6 y sin PMF); conteo de STAs por addr3/addr1 + addr2 → **0 clientes**. Conclusión: el TX funciona y el AP está ahí, pero no hay ningún cliente asociado visible al que forzar la reconexión; sin EAPOL no hay handshake. No es fallo de código.
- RF devuelta a managed. Commit/push de esta bitácora: ver `git log`.

### SESIÓN 2026-11-22 (cont. 15) — E0-24 IMPLEMENTADO, VEREDICTO NEGATIVO EN HW
- Implementado: `wifi_probe_trigger` (perfil netsh temporal con clave falsa + captura NPF_WIFI_ en managed), `convert_ethernet_eapol` (DLT 1 y 127/radiotap, M1+M2 → `WPA*02*` con SSID real), herramienta IA `trigger_handshake` y comando de consola «gatillo». 43 tests unitarios verdes. Correcciones durante la prueba: `pcapPath` camelCase, captura spawneada ANTES del connect (el future sin poll abría Npcap tarde), NPF_ (DLT1) daba 0 paquetes → usar NPF_WIFI_ (DLT127), conversor ampliado a radiotap+LLC/SNAP.
- **Medición real (La Fory 3H, autorizada)**: `netsh connect` con perfil falso → «solicitud completada», captura managed **336 paquetes**, **0 EAPOL**, solo 2 beacons del AP. Conclusión: el stack Windows/RT73 **no entrega los frames del 4-way handshake de su propia asociación** a Npcap; el auto-gatillo no produce material aquí. No se simula éxito: el conversor devuelve «No se encontró un M1+M2 EAPOL válido».
- RF devuelta a managed; NSIS reinstalado; veredicto registrado en `spec/02 E0-24`. Siguiente vía fiable: cliente real conectado al AP + deauth (TX ya verificado).

### SESIÓN 2026-11-22 (cont. 16) — FIX EMPAQUETADO rt3070_scan.exe
- Log del usuario: netsh vio 0 redes justo tras restaurar managed y el fallback USB crudo falló con `rt3070_scan.exe no encontrado en <inst>/_up_/driver_re/usb_tx`. Causa: el NSIS no incluía ese binario (ni `rt3070_sniff.exe`). Añadidos a `tauri.conf.json` resources; NSIS recompilado/reinstalado, exit 0, ambos exes presentes en `<inst>/_up_/driver_re/usb_tx/target/release`.
- Verificado CDP: `usb_raw_scan` ya **ejecuta** el binario (adiós «exe not found») y devuelve veredicto honesto «Sin redes en 12s…». Incluso tras `usb_raw_init` (MCU vivo, sin kick FIRMWARE(8), radio ON CH11) sigue dando 0 redes: coherente con la RX sorda measured del RT3070 en crudo. El escaneo normal por netsh sí ve redes cuando la radio ya está asentada (13 en mediciones anteriores). Sin commit de artefactos, solo config/docs.

### SESIÓN 2026-11-22 (cont. 17) — SCAN WINDOWS A 0 REDES: RT73 QUEDÓ EN MONITOR/SORDO
- Log del usuario: `scan_wifi` → 0 redes y fallback USB crudo. Diagnóstico CDP: `detect_adapters.monitor_active=true`, `monitor_status.mode=monitor`, CH1 — el RT73 seguía en monitor tras las pruebas de deauth/gatillo. `restore_managed` devolvió `success=true` y `mode=managed`, pero `netsh wlan show networks` **seguía con 0 redes**: el driver se quedó sordo para el scan tras tantos cambios monitor↔managed (el aire sí tenía beacons: 33 capturados antes).
- **Fix aplicado**: script elevado `C:\Users\INdaHouse\AppData\Local\Temp\uifipill_reset_rt73.ps1` (disable/enable `Wi-Fi 4`, UAC aceptado por el usuario, log en ruta absoluta). Resultado: `adapter_status=Disconnected`, **6 redes visibles** (sagemcomDEF0, TP-Link_29D6, Q0030207711424, HP-Print, La Fory 3H, MIWIFI_APUM). Verificado en la app: `mode=managed`, `monitor_active=false`, `scan_wifi` → 6 redes.
- **Lección**: `restore_managed` por OID no siempre recupera el stack de scan; si `scan_wifi` da 0 y `monitor_status` decía monitor, hacer disable/enable elevado del adaptador (o power-cycle físico) antes de culpar a la señal. RF dejada en managed y app relanzada.

### SESIÓN 2026-11-22 (cont. 18) — PLAN MÍNIMO OBLIGATORIO EN MODO IA
- Log del usuario: `hermes3:8b` devolvió un plan de **1 paso** dos veces (retry incluido) y el Modo IA se detuvo con «Plan sin pasos de ataque: no se ejecutó nada» (guardia honesto, sin ataque falso).
- Fix aprobado: si tras el reintento el plan sigue corto (<3 pasos o sin herramienta de acción), el orquestador **añade pasos mínimos con herramientas IA** (`profile_target`/`deauth_burst`/`inspect_hash`/`crack_hash`; con material previo solo `inspect_hash`/`crack_hash`). Se anota en el log: «Plan insuficiente tras el reintento: orquestador añade N paso(s) mínimo(s) con herramientas IA». **No se vuelve al pipeline determinista**. `ACTION_TOOLS` incluye ahora `deauth_burst`/`trigger_handshake`.
- Verificado CDP con `ollama_tools` forzado a 1 paso: retry → «orquestador añade 3 pasos» → plan de 4 herramientas → ejecutadas `profile_target`, `deauth_burst` (args reales BSSID/CH4), `inspect_hash`, `crack_hash`. Guardia actualizado en `audit-wiring.cjs`; lint/NSIS verdes. Antena devuelta a managed; app relanzada sin stubs.

### SESIÓN 2026-11-22 (cont. 19) — BUMP DE VERSIÓN A 1.0.14
- Petición del usuario: «sube el número de versión». Actualizado en los 7 sitios de build: `package.json`, `package-lock.json` (2 entradas), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json` y los 2 badges de `index.html`. `AGENTS.md` apunta ahora a 1.0.14 con todo el trabajo de la sesión commiteado.
- NSIS recompilado como `UIFIPILL_1.0.14_x64-setup.exe`, reinstalado `/S`; verificado `ProductVersion=1.0.14` en `%LOCALAPPDATA%\UIFIPILL\uifipill.exe` y badge de la UI. Sin cambios funcionales respecto a 1.0.13.

### SESIÓN 2026-11-22 (cont. 20) — MODO IA: FORZAR DEAUTH Y FIJAR CANAL ANTES DE CAPTURAR
- Log del usuario: plan de 3 pasos sin acción → retry (otro minuto) → plan de 11 herramientas que eligió `activate_monitor`+`capture_packets` en 60 s, **sin `deauth_burst`**, y `activate_monitor` dejó la radio en CH1 aunque se pidió CH4. Causa: el prompt aún contenía la frase heredada «No uses deauth ni WPS: Npcap no puede inyectar» (obsoleta desde que existe `deauth_burst` por RT3070).
- Fix en `app.js`: (1) prompt actualizado — usar `deauth_burst`/`trigger_handshake` para generar tráfico; (2) `hasForced()` exige una herramienta generadora de tráfico cuando no hay material; si falta, se añade al plan mínimo con las demás; (3) se elimina el reintento a Ollama (ahorro ~1 min) y se completa el plan de inmediato; (4) `set_monitor_channel` se llama **tras activate_monitor** y **antes de capture_packets** con el canal del objetivo.
- Verificado CDP (ollama_tools forzado): plan de 3 informativos → «orquestador añade 4 pasos» → `profile_target`, `deauth_burst` (BSSID/CH4 reales), `inspect_hash`, `crack_hash`. Con plan de `activate_monitor`+`capture_packets`+scan → se añade deauth y se ejecutan las llamadas de fijación de canal. Lint/NSIS verdes; RF sin cambios; app relanzada sin stubs.

### SESIÓN 2026-11-23 (cont. 21) — RADIO QUEDABA EN MONITOR TRAS LOS ATAQUES
- Log del usuario: tras un ataque, `scan_wifi` volvió a ver 0 redes y cayó al fallback USB crudo. Causa repetida: los flujos terminaban sin devolver la radio a managed (`activate_monitor` queda activo), y el prompt/plan del Modo IA eligió `trigger_handshake` (veredicto negativo) en vez de `deauth_burst`.
- Fix en `app.js`: (1) `restoreRadioAfterAttack()` en el `finally` de `simpleAttack` e `iaAttack` — si `monitor_status` dice monitor, llama a `restore_managed` y lo anota; (2) `recoverScanIfMonitor()` en `doScan` — si netsh da error/0 redes y la radio está en monitor, restaura, espera 2,5 s y reescanea antes de intentar USB crudo; (3) `hasForced()` solo cuenta `deauth_burst` (trigger_handshake NO cuenta) y el prompt dice explícitamente que no lo use en este HW; (4) `set_monitor_channel` también en el flujo determinista tras `activate_monitor`.
- Verificado CDP: al arrancar la app instalada, `monitor_active=false`, `mode=managed`, `scan_wifi` → **6 redes** (antes 0). Arnés con plan que proponía solo `trigger_handshake` → orquestador añadió `profile_target`/`deauth_burst`/`inspect_hash`/`crack_hash`. Lint/NSIS verdes; app relanzada.

# PARTE 6 — TRASPASO / HANDOFF (2026-11-21) — LEER PRIMERO SI TE RELEVAN

## 6.1 — Estado entregable
- **Versión 1.0.12 commiteada** (HEAD `db56ee5` / fix `269ae9d`, push verificado) + **WIP 1.0.13 instalada y verificada 2026-11-22** (sin commitear; arreglo del CSP que bloqueaba los botones). Remoto `origin = git@github.com:zerumen82/UIFIPILL.git`.
- Instalador vigente: `src-tauri/target/release/bundle/nsis/UIFIPILL_1.0.13_x64-setup.exe`. Binario: `src-tauri/target/release/uifipill.exe` (20,53 MB). Instalado en `%LOCALAPPDATA%\UIFIPILL\uifipill.exe` con `ProductVersion=1.0.13`; bundle `index-BOkVDAnu.js`. El fix 1.0.13 = `csp:null` + sin meta CSP + sin alert de diagnóstico (ver sesión 2026-11-22).
- **Código frontend activo**: `index.html` → `<script type="module" src="src/js/main.js">` → importa `../style.css`, `./ia.js`, `../app.js`. `src/app.js` NO es código muerto: es donde vive `simpleAttack`/`startAutoAttack` y todo el cableado. `src/js/ia.js` define `iaConfirm` (resuelve `true`) y el tool-calling Ollama; se carga ANTES que app.js.
-_backend_: Tauri v2, 95→93 invokes registrados (se quitaron los 2 del gatillo). `lib.rs` es el registro; `commands.rs` comandos; `crack_plan.rs` plan de crack; `usb_raw.rs` puente WinUSB.
- **Cambios de esta sesión (2026-11-21)**: (1) 3 bugs del botón Atacar — `claimAttack` duplicado, `activeNet` sin declarar, `let` dentro del `try` leído en `finally`; (2) `invokeAttack(..., {ownsLock:true})` para los pasos de `startAutoAttack`; (3) gatillo EAPOL eliminado de UI, JS y backend; (4) código muerto de la tabla de resultados eliminado; (5) guardias de regresión en `scripts/audit-wiring.cjs`. Detalle y evidencia en §1, sesiones 2026-11-21 cont. 1-4.

## 6.2 — Qué está verificado y qué NO (no exagerar)
- **SÍ verificado con evidencia**: lint (70 invokes / 74 onclicks, 0 huérfanos), vite build, `cargo build --release` 0 warnings, `cargo test --lib` 35 passed / 0 failed / 8 ignored, instalación `/S` con exit 0, versión del exe, hash del bundle incrustado, y arnés de ejecución (726 invocaciones / 174 funciones / 0 identificadores no declarados / 0 errores de ámbito).
- **Clic real verificado 2026-11-22 (antes NO lo estaba)**: con CDP en la app instalada, el clic en Atacar arranca el flujo real (scan → perfil → monitor RT73 → captura 60 s). El resto de hardware (RF completa, crack E2E) sigue sin verificado por el agente. Si vuelve a reportar fallo: pedir la línea exacta del verbose en vivo (`#live-cv`) y el paso donde se queda; no asumir que es el mismo bug.
- **RF (heredado, YA MEDIDO — no repetir sin motivo)**: RT3070 + Npcap = captura pasiva sí, inyección NO (Npcap #85, err 31); RT3070 + WinUSB crudo = TX al aire sí, RX sorda en las pruebas recientes (0 frames, stack/host acumulado); RT73 en monitor con Npcap = canal + captura pasiva OK sin parche; driver netr28ux parcheado = cambio de canal en monitor. Nunca kick `FIRMWARE(8)` bajo WinUSB (mata el chip 5/5; solo power-cycle 15 s). Ver §3/§4/§5 y `AGENTS.md` regla 3.
- **Crack**: hashcat `tools/` = v7.1.2 sin OpenCL/GPU → PCFG (-a 4 / hints) se omite con justificación; E0-17 (material externo/candidatas) sigue sin E2E con `.22000` real → deuda D-07.
- **D-03 / E0-11 / E0-12**: reboot pendiente por decisión del usuario (no ejecutado). No reactivar el sniff sostenido RT3070 sin antes seguir el protocolo: power-cycle → restore_netr28ux → switch WinUSB → probe → init → scan → ciclo dual RT73.

## 6.3 — Reproducir la verificación (comandos y salida esperada)
```
cd D:\PROJECTS\UIFIPILL
npm run lint        # audit-wiring: OK (70 invokes, 75 onclicks) + guardias CSP
npx vite build      # dist/index.html 96,55 kB · dist/assets/index-Di5f2rSS.js 101,63 kB
cd src-tauri
cargo test --lib    # 35 passed; 0 failed; 8 ignored
cargo build --release   # Finished release profile, 0 warnings
cd ..
npx tauri build --bundles nsis   # UIFIPILL_1.0.12_x64-setup.exe
```
Tras instalar con `/S`, comprobar `ProductVersion` del exe en `%LOCALAPPDATA%\UIFIPILL` y que el ASCII del binario contenga `index-Di5f2rSS.js`. Si el frontend cambia, el nombre del bundle cambia (hash content-addressed) — verificado con sondeos: añadir un comentario NO cambia el hash; cambiar código SÍ.

## 6.4 — Cómo diagnosticar una regresión parecida (método que funcionó)
- Arnés temporal (NO está en el repo; estaba en `%TEMP%\uifipill-deep-audit.cjs` y `%TEMP%\uifipill-undeclared.cjs`): carga cada módulo en su propio IIFE dentro de `vm.runInNewContext('with (guard) { ... }')`, donde `guard` es un Proxy con `has: () => true` y `get` que **lanza** si el identificador no está en el sandbox. Eso convierte un `ReferenceError` silencioso en un fallo explícito. Invoca todas las funciones `window.*`/`function*` con varios juegos de argumentos y analiza `try/catch/finally` con **balanceo de llaves** (una regex ingenua dio falsos positivos).
- Errores esperados del arnés (NO bugs): `navigator.clipboard.writeText` y pasar tipos genéricos a `toUpperCase()`.
- Guardias en `scripts/audit-wiring.cjs` (se ejecutan con `npm run lint`): exactamente 1 `claimAttack('simple_attack')`; prohibido `activeNet`; `bssid`/`ssid`/`ch` declarados antes del `try` de `simpleAttack`; todo `invokeAttack()` dentro de `startAutoAttack` con `ownsLock:true`; ningún `wpa2_trigger`/`triggerEapol`/`simple-trigger-card` en UI/JS ni `mod wpa2_trigger` en `lib.rs`; **desde 2026-11-22**: `app.security.csp === null`, sin meta CSP en `index.html`, sin `alert('click')` en inline handlers.
- Reglas de cableado: Tauri v2 convierte args a camelCase (frontend `durationSeconds`, `ifaceGuid`…); respuestas en snake_case; ningún `invoke` sin handler; ningún `onclick`/`on*` huérfano; eventos `attack-progress/started/completed/error` deben existir en ambos sitios: `listen()` (JS) y `emit()` (Rust).

## 6.5 — Deudas y estado del repo
- **Sin commitear / sin trackear (decisión deliberada)**: `add.py`, `cleanup.cjs`, `cross.cjs`, `divattack.cjs`, `divcheck.cjs`, `extract_ids.cjs`, `groups.cjs`, `lines.cjs`, `needed.cjs`, `tags.cjs`, `usage.cjs`, `index.html.bak`, `memory_backup.txt`, `prev_index.html`, `test64.html`. Son scripts/backups de diagnóstico de sesiones anteriores; NO son necesarios para compilar. Si el usuario pide «commitear todo», preguntar antes de incluirlos.
- Deudas formales en `spec/03-PENDIENTES.md`: D-03 (RF/RT3070 RX + reboot), D-07 (crack E2E con hashcat v8+/OpenCL y `.22000` real). Épica E0 «Windows ≡ Kali» en `spec/02-REQUISITOS.md`.
- `AGENTS.md` conserva el cuerpo con revisión 2026-10-06 y cifras viejas (ej. 95 invokes, 30 tests); su cabecera tiene una nota de actualización 2026-11-21 apuntando aquí. **La verdad operativa es esta §6 + `git log`.**

## 6.6 — Reglas duras (resumen; fuente completa `~/.agents/AGENTS.md` + `AGENTS.md`)
1. No hardcode / no stub / no fake: éxito solo con evidencia real (`.cracked`, PIN parseado, `TXDONE`, frames reales). Binario ausente → hint accionable.
2. Verificación obligatoria tras tocar código: lint + vite + cargo build 0 warnings + cargo test.
3. USB: nunca kick `FIRMWARE(8)` bajo WinUSB; vendor requests RT2800/RT3070 = SINGLE_WRITE 2 / SINGLE_READ 3 / MULTI_WRITE 6 / MULTI_READ 7; `USB_DEVICE_MODE` va en wValue; no RESET antes de leer registros.
4. UAC: scripts elevados → log directo a fichero con ruta absoluta; el shell no es admin.
5. El instalador NSIS aplana `tools/` en `_up_\tools`; los DLL (libpcap/Packet/libwinpthread) van en resources. Regla histórica: si se añade un bin al crate, mantener `default-run="uifipill"`.
6. Bitácora: todo decision/prueba/resultado se anota aquí con fecha. No registrar = no hecho.
7. No commit/push sin petición explícita; tampoco acciones destructivas (borados, resets, force-push) sin confirmación.

## 6.7 — Checklist para el agente que releva
`git log -3` → leer §6 → `npm run lint` → `cargo test --lib` → confirmar versión instalada → Si el usuario reporta un botón muerto, **primero comprobar el CSP en la app real con CDP** (`csp:null` en tauri.conf + sin meta CSP; los `onclick` inline deben quedar como `function`; ver sesión 2026-11-22) y solo después buscar identificadores no declarados / ámbitos try/finally (fueron bugs reales de 1.0.12 pero NO la causa del clic muerto) → cualquier cambio de RF se mide y se anota → cualquier decisión de alcance se pregunta al usuario (él es el orchestrator).
