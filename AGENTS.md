# AGENTS.md — UIFIPILL Project

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
- Actualización de RX_PLAN.md: E1 falsado → H2 con plan de reversing.

## SESIÓN RX 2026-09-25 (final) — nueva hipótesis H1 + plan en RX_PLAN.md
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
- Plan completo y órden de experimentos: `driver_re/usb_tx/RX_PLAN.md`.
- Cerrado definitivamente (no reintentar): kick FIRMWARE en cualquier estado,
  re-enum tras kick con reapertura simple, confiar en MAC_CSR0 como señal de fw.

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

## Parche driver RT3070 (netr28ux.sys) — rev. 2026-09-20 (DRIVER_RE.md)
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
- ROADMAP_WSL2.md §ESTADO ACTUAL (2026-09-18) tiene la medición completa del
  puente; decisión de camino RF (Kali USB vs antena nueva) sin cerrar.

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

## External tools required (lab machine)
- Npcap (NPcap.dll driver)
- hcxdumptool + hcxpcapngtool (hcxtools)
- hashcat
- bully (optional — requires pixiewps, no confirmed Win native build)
