# WSL2 + Kali — Hoja de ruta (documento vivo)

Objetivo: motor dual. UI Tauri en Windows + ejecución RF en Kali-WSL2 (mismo RT3070
vía usbipd). Lo offline sigue nativo Windows. Cada fase tiene criterio de "hecho"
verificable; no se avanza sin marcarlo. Estado actual arriba del todo.

## ESTADO ACTUAL (2026-09-15)
- Fases 0-5 completas. Fase 6 PARCIAL + Fase 6b NUEVA (UI VirtualHere automatizada,
  ver abajo). Post-reboot: Kali OK con kernel `bzImage-84test` (6.6.84.1+, rt2800usb
  + rt2870 builtin + USBIP_VHCI verificados), RT3070 en busid **1-7** (era 1-5),
  servidor VH 4.8.8 como servicio SYSTEM en 7575.
- Veredicto VirtualHere 2026-09-15: LIST ve el RT3070 (`802.11 n WLAN`), pero USE
  → `FAILED: API Timeout`. Causa oficial del desarrollador (foro #4683): el cliente
  consola EXIGE licencia de pago; con trial el USE se rechaza por diseño. NO es
  problema de elevación/driver/red (puerto accesible desde Kali, vhci cargado).
- Siguiente (a elegir): (a) comprar licencia VH (~49 USD) y RF-check; (b) probar
  gratis el cliente GUI Linux bajo WSLg contra el trial; (c) depurar RX usbipd;
  (d) Kali bare-metal (cero coste, RF garantizada).
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
