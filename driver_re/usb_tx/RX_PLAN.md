# RX_PLAN.md — Investigación RX por WinUSB (2026-09-25)

## ✅ E1 EJECUTADO (rev. 2) — H1 FALSADA, H2 abierta
E1 completo (e1_run3.ps1): netr28ux tomó el control (pnputil add + delete oem395
+ rescan), netsh escaneando en bucle, delete netr28ux + rescan → WinUSB heredó
el chip SIN power-cycle (Status OK, mailbox OWNER=0, RF vivo).
**El BBP siguió leyendo 0x00 incluso heredado del vendor con radio EN USO.**
→ El vendor NO duerme el BBP al soltar: es que su acceso BBP NO pasa por
BBP_CSR_CFG (0x11C) visible para nosotros.

## H2 (ganadora provisional): acceso BBP vía MCU
El firmware del MCU atiende el BBP por comandos mailbox (no por el registro
0x11C del bus CSR, o además de él). Nuestro write+readback en 0x11C devuelve
vacío porque el arbiter del firmware enruta el BBP de otra forma.
INVESTIGAR:
1. Reversing de netr28ux.sys (driver_re/netr28ux.sys, 2.2 MB — objdump/Ghidra):
   buscar la rutina que escribe BBP y ver si usa 0x11C o comandos MCU.
2. Reversing del firmware rt2870.bin (4 KB del offset 0, 8051): buscar accesos
   al arbiter BBP y comandos mailbox no documentados.
3. Probar comandos MCU con tokens/args de la familia BBP ( FirmwareRBBP,
   MCU_RBBP=0x41? etc. — ver tablas de comandos del vendor SDK Ralink).
4. Nota: el firmware responde MCU_CURRENT (vivo) — hay canal de comandos
   activo; solo falta encontrar el comando BBP correcto.

## Lección del mecanismo de rebind (para scripts futuros)
- UpdateDriverForPlugAndPlayDevices (newdev.dll) → win32err=2 SIEMPRE con este
  device. NO USAR.
- Vía que funciona: pnputil /add-driver <inf> + pnputil /delete-driver <oem>
  /uninstall + pnputil /scan-devices. El uninstall tarda 3-4 MIN (timeout
  interno) — esperar con paciencia, no matar.
- Orden E1 verificado: publicar netr28ux → delete oem395 → rescan (netr28ux
  toma control) → escaneo netsh activo → delete netr28ux → rescan (WinUSB
  hereda con radio caliente).

## Estado del problema
- TX al aire CONFIRMADO (beacons "TXTEST" visibles desde otro device).
- RX: 0 frames en todos los estados probados. BBP lee 0x00 siempre.
- El port es fiel a Linux (verificado contra rt2x00usb.h/c, rt2800usb.c, rt2800lib.c 6.6).
- El kick FIRMWARE(8) mata el chip bajo WinUSB (reproducido en frío vía UNPLUG).

## DATO CLAVE QUE REABRE LA INVESTIGACIÓN
**La antena SÍ capturaba bajo netr28ux** (392 pkts reales Npcap, hito 2026-09-14).
El vendor deja el BBP VIVO. Por tanto el BBP no está roto: **algo lo duerme entre
el vendor y nuestro acceso**.

## Hipótesis principal (H1): MCU_SLEEP al soltar el device
rt2800usb_set_device_state(STATE_RADIO_OFF) manda `MCU_SLEEP(0xff, 0xff, 2)` al
desactivar la radio. El driver vendor netr28ux probablemente hace lo mismo en su
IRP_MN_REMOVE / detach (o suspend): duerme el MCU/BBP antes de soltar el device.
Cuando hacemos el rebind, heredamos un BBP DORMIDO, y MCU_WAKEUP no lo despierta
(medido: se consume pero bbp sigue 0x00).

Contra-evidencia a descartar: el vendor quizá no usa rt2x00 (es binario cerrado
MediaTek) — pero el MCU_SLEEP/WAKEUP es un mecanismo del firmware RT3070, no del
driver, así que CUALQUIER driver lo puede mandar.

## Experimento E1 — rebind a WinUSB con la radio EN USO (sin dejar dormir el BBP)
1. Arrancar con netr28ux Y la radio activa escaneando (netsh wlan show networks
   en bucle, o mejor: captura Npcap activa en monitor CH11).
2. hot_rebind.ps1 to-winusb SIN pausa larga (el rebind es instantáneo).
3. INMEDIATAMENTE (<2 s) rt3070_hotread: ¿BBP vivo?
4. Si BBP vivo → vendorradio directo (sin kick) → RX test.
   Si sigue mudo → H1 muere, pasar a E2.

Requisitos: antena con netr28ux activo (power-cycle + restore_netr28ux.ps1 si
hace falta), admin, y ejecutar el rebind mientras netsh escanea.

## Experimento E2 — MCU_SLEEP explícito y WAKEUP en el orden exacto de rt2x00
Si el vendor duerme el BBP con SLEEP, el WAKEUP debería despertarlo... salvo que
el orden/argumentos no sean los de este firmware. rt2800usb_set_state:
    AWAKE: MCU_WAKEUP, token 0xff, arg0=0, arg1=2
    SLEEP: MCU_SLEEP,   token 0xff, arg0=0xff, arg1=2
Probar: mandar MCU_SLEEP y LUEGO MCU_WAKEUP (ciclo sleep/wake completo) — el
firmware puede requerir el ciclo completo para re-armar el BBP. También probar
arg1=0 vs arg1=2.

## Experimento E3 — suspend/resume del USB como «boot frío» barato
El firmware RT3070 re-inicia el BBP en el resume. Windows: Disable/Enable PnP del
device SIN desenchufar (port_reset.ps1 ya existe) — pero eso ya se probó (deja el
chip con autoload). Queda probar: suspend (selective suspend) via powercfg /
request o deshabilitando USB selective suspend — bajo prioridad.

## Experimento E4 — pedir al MCU el estado (MCU_CURRENT con argumentos variantes)
El mailbox STATUS=0xf00f1587 heredado no es 0 → el MCU está en un estado concreto.
Probar MCU_CURRENT con distintos tokens/args y ver qué responde el firmware (mapa
de comandos del MCU: BOOT_SIGNAL/WAKEUP/SLEEP/CURRENT/LED...).

## Lo que NO volver a intentar (medido, cerrado)
- Kick FIRMWARE(8) en cualquier estado → chip sordo (4/4 + frío).
- Cargar firmware y esperar re-enum con reapertura simple (falsa re-enum).
- confiar en MAC_CSR0 != 0 como señal de "sin firmware cargado".
- Releer el estado tras degradación sin power-cycle (registros 0x00/0xff).

## Próxima sesión (orden)
1. E1 (rebind con radio activa) — el más prometedor y barato.
2. Si E1 falla: E2 (ciclo SLEEP/WAKEUP completo).
3. En paralelo: integrar TX en la app (deauth por USB crudo ya es posible).
