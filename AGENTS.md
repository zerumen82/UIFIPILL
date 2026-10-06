# AGENTS.md — UIFIPILL Project

> **Reglas, objetivos, arquitectura y estado VIGENTE** (última revisión 2026-10-06).
> La historia vive en **`memory.md`**: §1 bitácora de sesiones (2026-09-13 → 2026-10-05),
> §2 roadmap motor dual WSL2/Kali, §3 RE de netr28ux.sys, §4 investigación RX WinUSB,
> §5 por qué no funciona igual que en Windows-vs-Linux.
> README.md = overview para usuario. Lab-only; no commit sin petición explícita.

## Objetivo (Goal)
Windows desktop WiFi suite: scan + modo monitor + PMKID capture/convert/crack +
WPS PIN bruteforce + keygen. **Solo laboratorio** (redes propias o con
autorización explícita).

**Meta en curso (épica E0 de `spec/02-REQUISITOS.md`): que todo el pipeline RF
funcione en Windows como en Kali** — scan → monitor → RX → TX → captura → crack
sin salir de la máquina. Ganado hasta hoy: TX cruda al aire (09-25), RX cruda
(10-01), canal con driver parcheado (09-21), wash E2E (10-01), UI de flujo
(10-05). Pendiente: sniff sostenido, PMKID/handshake por RX cruda y veredicto
dual (Fase 7).

## Reglas del proyecto
1. **Verificación obligatoria tras tocar código**: `npm run lint` + `npx vite build`
   + `cd src-tauri && cargo build --release` (**0 warnings**) + `cargo test --lib`
   (26 passed / 8 ignored desde 2026-10-05 — los `lab_*` exigen HW). Entorno: el runner corta a
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
    - **RX por USB crudo WinUSB: CONFIRMADA 2026-10-01** (611 URBs / 524 frames /
      3 APs en 15 s, sin kick FIRMWARE(8)). La raíz del "BBP mudo/RF sordo" era
      **nuestro port del canal**: path rf53xx (N→RFCSR8, que es el ID de versión
      del RF) en vez del path 3xxx de este chip (**N→RFCSR2, K→RFCSR3[3:0],
      R→RFCSR6[1:0]**) + RFCSR1 con PLL_PD=1 (RMW heredado) + calib BW20 no
      aplicada en RFCSR24/31. Detalle: memory §1 sesión 2026-10-01 (cont.).
      **TX por USB crudo: CONFIRMADO al aire** (hito 2026-09-25). Sniffer
      USBPcap sigue útil para cotejar contra el vendor.
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

## Estado actual (act. 2026-10-05)
- **UI ATAQUE: FLUJO ÚNICO VISUAL POR RED (2026-10-05)**: héroe `#flow-hero`
  con luces Objetivo/Antena/Perfil + botones primarios por veredicto
  (`renderHero()`), defaults por `profile_target` (open→conectar; wpa2_psk→
  keygen→handshake→crack; wpa3_sae→wash primero; enterprise→Kali/lab…),
  checklist `window._usbAlive`, panel único `#flow-result`, «USB crudo»
  renombrado **«Acceso directo a la antena — recomendado»** (detalle WinUSB
  solo en plegable). Verificado: lint/vite/cargo 0 warnings/26 tests/0
  huérfanos. Detalle: memory §1 sesión 2026-10-05.
- **INSTALADOR FIXEADO «se abre y se cierra» (2026-10-01, cont. 5)**: causa raíz
  = **2 bins en el crate** (`src/bin/send_assoc.rs`) + sin `default-run` ni
  `[[bin]]` → tauri-cli 2.11.2 empaquetaba **send_assoc.exe** (224 KB, la sonda
  TX) como main (`MAINBINARYNAME "send_assoc"`). `mainBinaryName` en
  tauri.conf **solo renombra el binario elegido** (intento insuficiente,
  medido: pisaba uifipill.exe con la sonda). Fix: `Cargo.toml` con
  `default-run = "uifipill"` + `[[bin]]` explícitos → installer con
  `uifipill.exe` 15,7 MB; reinstalado en silencio (`/S`, currentUser) y app
  verificada **viva** (título UIFIPILL). `send_assoc.exe` se instala al lado
  (inofensivo). Regla: **si se añade un bin, mantener `default-run`**. Detalle:
  memory §1 sesión 2026-10-01 (cont. 5).
- **UI WPS/WASH VISUAL + AUDITORÍA (2026-10-01, cont. 4)**: tabla de resultados
  wash con **clic → objetivo** (`selectWashTarget` fusiona el AP en `lastNets` y
  sincroniza todas las tarjetas), objetivo visible en la tarjeta WPS PIN,
  panel de resultado WPS que parsea SOLO stdout real (reaver `WPS PIN:`/`WPA
  PSK:`; bully `Pin is`/`Key is`), `claimAttack()` cierra el bug del lock de UI
  pegado (`currentAttackId` fijado antes del busy-check), `#autoAttackBtn` que
  faltaba, placeholders `\Device\NPF_WIFI_{GUID}`, progreso con segundos en el
  survey. **Hallazgo medido: wash solo ve frames en MODO MONITOR** (managed =
  0 filas con canal fijado; monitor = 27 filas ch11) → `washModeHint()` pinta
  hint accionable si `monitor_status`=managed. Auditoría: 0 ids/handlers/invokes
  huérfanos. Verificado: lint/vite/cargo 0 warnings/26 tests + wash E2E real +
  instalador NSIS recompilado; radio restaurada a managed al cierre. Detalle:
  memory §1 sesión 2026-10-01 (cont. 4).
- **WPS/WASH E2E + RESTORE DRIVER (2026-10-01, tarde)**: driver restaurado a
  **netr28ux parcheado** (`restore_run3.ps1`, 1 UAC; `Start-Transcript`+pnputil
  deadlock → `Start-Process` con redirect) → device OK, netsh 18 SSIDs, loaded
  hash `D2C7CF43`; managed restaurado al cierre. **wash FUNCIONA E2E** (7 filas,
  ch11, args `-i \Device\NPF_WIFI_{GUID} -c N -F`): MIWIFI/Livebox LOCKED,
  **sagemcomDEF0_Plus desbloqueado**. 4 causas raíz fixeadas: `libpcap.dll`
  backend-null → **swap wpcap.dll de Npcap + Packet.dll** (añadido a resources
  NSIS; backup `libpcap.dll.nullbak`), nombre **`NPF_WIFI_`** (dlt127, NO
  `NPF_`=dlt1), flag **`-F`** (FCS) en wash+reaver, survey acotado
  (`run_bin_bg_opts` watchdog 30/60 s, cancelable). `parse_wash` reescrito
  (Lck=Yes/No, vendor truncado→prefijo, dedupe) + `normalize_wps_iface` en los
  8 cmds WPS; 3 tests nuevos (**26 passed**). **REAVER: veredicto honesto** —
  binario/cableado/args OK pero `pcap_sendpacket rc=-1` (Npcap #85 sigue
  cerrado) → **no puede TX en Windows nativo**; E2E WPS real = Kali live USB.
  Lección: set OID modo monitor = data 8 bytes (modo en offset 4); `WlanHelper
  mode` (WLAN API) NO refleja el modo raw. Detalle: memory §1 sesión 2026-10-01
  (cont. 3).
- **HITO RX-WinUSB (2026-10-01)**: `rt3070_bbpmcu` (runs 8/9 vía `run_bbpmcu*.ps1`
  elevado) → **611 URBs / 524 frames / 3 APs reales (ch11) en 15 s** por EP 0x81,
  parser FC@20, sin kick `FIRMWARE(8)`. ΔCCA>0 y ΔCRC>0 en 1..14 (antes 0).
  Causa raíz: path de canal rf53xx→rf3xxx (N→RFCSR2), RFCSR1=0xf1, calib BW20
  en RFCSR24/31, VCOCAL por RFCSR30 bit7. Ver memory §1 sesión 2026-10-01 (cont.).
  **Continuado el mismo día**: scan sostenido OK — `rt3070_scan` (runs scan1/2 vía
  `run_scan*.ps1`, 30 s, sin kick) → **247 frames/10 redes y 225 frames/12 redes**
  en 5 canales; `init_radio` ya nunca kickea (gate MCU vivo → si muerto: error
  accionable) + sonda `bbp_probe_transport` (default MCU-BBP daba timeout →
  directa `BBP_CSR_CFG`); RFCSR23=**0x09** (valor vendor) aplicado y comparado
  (scan2 sin degradación). memory §4/§5 al día.
- **INTEGRACIÓN UI USB-CRUDO E2E (2026-10-01, mismo día)**: botón «📡 Escanear
  (USB crudo)» en la pestaña Escanear + **fallback automático** cuando netsh
  falla o ve 0 redes (y en auto-attack); `usbRawSniff` con init previo y canal
  heredado (regla 8); selección de red rellena el panel USB (bssid/ssid/canal);
  **handshake auto-convierte** pcap→`.22000` y prellena el crackeador; sniff
  éxito honesto (0 frames → exit 2 + `success=false`, regla 2) y ruta pcap real
  (`%TEMP%` literal no expandía — medido); input `wpa3-bssid` que faltaba
  (orphan que rompía `wpa3Audit`); auditoría cableado: 0 onclick/invoke/ids
  huérfanos. `usb_raw_init` ya no pasa firmware (init_radio no lo usa).
  E2E no-admin medido: scan 11 redes con seguridad del beacon (parser RSN SAE→
  WPA3), sniff **359 frames/10s** (sniff ya NO pisa `USB_DMA_CFG` — antes 3;
  con 0 frames sale exit 2), pcap real en `%TEMP%` (antes ruta literal).
- **DEVICE SANEDADO (2026-09-30)**: causa raíz del ProblemCode 56 = `C:\Windows\INF\netr28ux.inf`
  BORRADO → `NetworkInterfaceInstallResult 0x80070002 (ERROR_FILE_NOT_FOUND)` → la class config
  de Net nunca completaba. Fix (`fix56_inf_restore.ps1`): restaurar INF+PNF desde DriverStore +
  limpiar `ConfigFlags 0x80000` + remove+rescan → **device Status OK**, `netsh` ve la antena
  (Wi-Fi 802.11n, MAC 00:c0:ca:59:f8:b5) y **scan real: 10 redes**. HVCI OFF confirmado.
  Detalle: memory §1, sesión 2026-09-30.
- ~~PENDIENTE: UN REBOOT~~ → **REBOOT HECHO (2026-09-30 21:00) y verificado
  2026-10-01**: `testsigning Yes` tras arrancar, device OK, `lab_monitor_cycle`
  pasa (CH6→CH1→restore con beacons). Preflight del 09-30 OK: Secure Boot off,
  cert `labtest.cer` confiado (Root+TrustedPublisher), HVCI off; hot-swap del
  driver **PARCHEADO** (`d2c7cf43…` en `System32\drivers`) via
  `hotswap_driver.ps1`. Rollback posible: copia inbox `94734AEF` desde
  DriverStore. (Nota histórica: bug del wrapper `if ($tsLine -notmatch
  'Yes|S[ií]')` casaba con el "si" de "testsigning" — regex anclada; det.
  memory §1 sesión 2026-09-30.)
- App instalada en `%LOCALAPPDATA%\UIFIPILL\uifipill.exe` (instalador NSIS
  recompilado y reinstalado 2026-10-01 — `uifipill.exe` 15,7 MB; master con
  commits sin push + trabajo 10-01/10-05 **aún sin commitear** — re-verificar
  tras el próximo build; ver `spec/03-PENDIENTES.md`).
- Motor dual WSL2/VirtualHere: código completo; RF bloqueada (usbipd RX muerta
  en 3 mediciones, VirtualHere = licencia de pago). **Plan A de RF = Kali live
  USB** + TX crudo Windows (decisión cerrada 2026-09-25).

## Límites hardware medidos (resumen; detalle memory §2/§5)
- **RT3070 + Windows/Npcap**: captura pasiva SÍ (392 pkts reales), crack/keygen/
  scan SÍ; inyección NO (err 31, Npcap #85). Canal en monitor: SÍ con el driver
  parcheado; el stock y las 3 vías OID fallaban.
- **RT3070 + WinUSB crudo (usb_tx)**: TX al aire SÍ (beacons "TXTEST" vistos desde
  otro device, TXDONE confirmado); scan hopping SÍ; **RX SÍ desde 2026-10-01**
  (524 frames/3 APs/15 s + scan 247 frames/30 s — el «BBP mudo» era el port del
  canal rf53xx→rf3xxx + RFCSR1 PLL_PD; nunca kick `FIRMWARE(8)`).
- **usbipd → WSL2/Kali**: driver perfecto pero RX muerta por el transporte.
  **VirtualHere**: USE rechazado con trial (licencia). **Kali live USB**: RX
  completa — vía probada.

## Hitos clave (detalle en memory §1)
- 2026-09-14: captura Npcap nativa + conversor `.22000`; monitor↔managed OK.
- 2026-09-21: driver netr28ux parcheado → **la radio cambia de canal en monitor**.
- 2026-09-22: **TX por USB crudo** (WinUSB) pipeline end-to-end; RE TX del driver
  medida y cerrada (Npcap #85).
- 2026-09-23: TXWI/TXINFO reales + feedback TX_STA_FIFO + scan hopping crudo.
- 2026-09-25: **TX AL AIRE confirmado**; RX-WinUSB cerrada entonces (coldrx)
  — *superado el 2026-10-01, ver fila siguiente*; 2ª antena
  comparada (BBP mudo igual → es del boot, no del chip).
- 2026-09-28/29: reset PC → HVCI bloqueó netr28ux → HVCI off; device en
  saneamiento (31/56).
- 2026-10-01: **RX AL AIRE por USB crudo WinUSB** (524 frames, 3 APs, sin
  kick FIRMWARE(8)) — la raíz del "BBP mudo" era N→RFCSR8 (path rf53xx) +
  RFCSR1 PLL_PD; path correcto 3xxx N→RFCSR2.

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
- **`tools/libpcap.dll` = copia del wpcap.dll de Npcap** (el msys64 original
  tiene backend `pcap-null` → wash/reaver no abren captura; backup en
  `libpcap.dll.nullbak`; `tools/Packet.dll` es obligatorio al lado y va en
  resources del NSIS). wash/reaver en Windows: iface `\Device\NPF_WIFI_{GUID}`
  (dlt127) + flag `-F`.
- Nota: el sistema de build de reaver (`tools/build/`) se eliminó del repo —
  recompilar con upstream + parche portable fuera de este repo.

## Documentación
- **`memory.md`** — §1 bitácora completa · §2 roadmap WSL2/Kali · §3 RE de
  netr28ux.sys (parche canal, TX cerrada) · §4 investigación RX WinUSB (plan y
  veredictos) · §5 explicación canónica Windows-vs-Linux + mapa funcional.
- `README.md` — overview, requisitos y mapa «qué corre dónde» para el usuario.
- `spec/` — especificación SDD creada 2026-10-06: `00-PRINCIPIOS` (constitución
  + Definición de Listo), `01-ARQUITECTURA`, `02-REQUISITOS` (épica E0
  «Windows ≡ Kali», E0-01…E0-15), `03-PENDIENTES` (deudas declaradas),
  `04-EJERCITO` (flujo de agentes). Toda historia nueva pasa por `spec/02`
  antes de implementarse.
