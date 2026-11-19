# spec/02 â€” REQUISITOS (historias con criterios verificables)

> Ã‰pica principal **E0 Â«Windows â‰¡ KaliÂ»**: que en Windows funcione el pipeline
> RF completo como en Kali (scan â†’ monitor â†’ RX â†’ TX â†’ captura â†’ crack).
> Formato: historia â†’ criterios de aceptaciÃ³n (CA) medibles â†’ estado.
> Evidencia de cada historia cerrada: sesiÃ³n correspondiente en `../memory.md` Â§1.

## Ã‰pica E0 â€” Windows â‰¡ Kali (pipeline RF completo)

| ID | Historia | CA (criterios de aceptaciÃ³n) | Estado |
|---|---|---|---|
| E0-01 | Scan de redes sin Kali | `scan_wifi` (netsh) devuelve SSID/BSSID/CH/seuridad reales; fallback automÃ¡tico a scan USB crudo si netsh da 0 | âœ… 2026-10-01 (11 redes + RSN/SAE parseado) |
| E0-02 | Modo monitor con cambio de canal | `activate_monitor` + `set_monitor_channel` muevan la radio (beacons SOLO del canal pedido en captura) | âœ… 2026-09-21 (RT3070, CH1 vs CH6) Â· âœ… 2026-10-06 (RT73: CH1 99% / CH6 92%, SIN parche de driver â€” `lab_channel_rf`) |
| E0-03 | RX cruda por WinUSB | â‰¥500 frames/15 s y â‰¥3 APs por EP 0x81 con parser FC@20, sin kick FIRMWARE(8) | âœ… 2026-10-01 (524/3) |
| E0-04 | TX cruda al aire | Beacon/deauth con `TXDONE` y beacon visible desde otro dispositivo | âœ… 2026-09-25 |
| E0-05 | Scan hopping sostenido | â‰¥100 frames/30 s en â‰¥5 canales con â‰¥10 redes, sin degradaciÃ³n del chip | âœ… 2026-10-01 (247/10 y 225/12) |
| E0-06 | Captura pasiva nativa Npcap + `.22000` | `native_capture` â‰¥1 pkt real; `pcap_to_22000` extrae PMKID/EAPOL sin hcxpcapngtool | âœ… 2026-09-14 |
| E0-07 | Crack end-to-end en Windows | hashcat escribe `.cracked` (no parseo de stdout); `wifi_connect` conecta con perfil netsh | âœ… 2026-09-17 |
| E0-08 | wash E2E en Windows | `wash -i \Device\NPF_WIFI_{GUID} -c N -F` en monitor â‰¥1 fila WPS real; `parse_wash` laclava | âœ… 2026-10-01 (7 filas) |
| E0-09 | UI Â«flujo por redÂ» sin jerga | HÃ©roe con veredicto por `profile_target`; 0 ids/handlers/invokes huÃ©rfanos; lint+vite+26 tests | âœ… 2026-10-05 |
| E0-10 | Instalador abre la app | NSIS instala `uifipill.exe` (15,7 MB) y la app queda viva (tÃ­tulo UIFIPILL) | âœ… 2026-10-01 |
| E0-11 | Sniff sostenido largo (USB crudo) | â‰¥5 min de sniff continuo con watchdog/cancel, frames contados >0 y pcap creciendo; radio restaurada al cierre | â¬œ infraestructura âœ… (300 s, PROGRESS, restore, schtasks). Bloqueo medido 2026-10-06: RT3070 RX sordo en AMBAS antenas (0 frames con aire confirmado por RT73: 7 redes, ch3 100%). Sospecha: puerto USB 3.0 xHCI / paquete WinUSB oem180 â†’ probar puerto USB 2.0 |
| E0-12 | PMKID/handshake por RX cruda | Sniff crudo produce pcap con EAPOL/PMKID â†’ `pcap_to_22000` â†’ hashcat `.cracked`, todo en Windows | â¬œ abierto (RX ya dada; falta pipeline) |
| E0-13 | WPS sin Npcap | vÃ­a Kali live USB (reaver real) documentada en la app O TX cruda propia; veredicto honesto si Windows no puede asociar | â¬œ decisiÃ³n: Npcap #85 cerrado (err 203 medido) |
| E0-14 | efuse/EEPROM (tx power/LNA por canal) | lectura/escritura con verificaciÃ³n de valor; comparaciÃ³n RFCSR23=0x09 vs 0x00 | â¬œ opcional (idea de sesiÃ³n 10-01) |
| E0-15 | Veredicto dual (Fase 7) | Tabla final Windows crudo vs Npcap vs Kali con evidencia por flujo; marcada en memory Â§2 Fase 7 | â¬œ |
| E0-16 | Pipeline de candidatas dirigido (offline) | `crack_plan_preview` devuelve etapas ordenadas (keygenâ†’mÃ¡scarasâ†’rockyouâ†’reglasâ†’externo) con keyspace/motivo; `crack_plan_run_bg` ejecuta en orden, detiene en `.cracked` real (hashcat), salta etapas no disponibles con hint, cancelable; UI ultra-simple en pestaÃ±a Ataque | â¬œ 2026-11-19 (infra implementada y extendida por E0-17; verificaciÃ³n E2E de lab pendiente D-07) |
| E0-17 | Candidatas PCFG/ML-compatibles + flujo Pwnagotchi->crack | `crack_plan`: PCFG nativo hashcat v8+ (-a 4) + `hints` con SSID seguro (omitido con motivo si <v8), diccionarios extra (`tools/`, `tools/wordlists/`, `dive.rule`) y candidatas auto-detectadas (`candidates*/passgan*/passgpt*/generated*/samples*/wordgen*.txt`), externo tipado (`.txt`->`-a 0`, feed `-a 8` v8+, ejecutable `--slow-candidates` con fallback honesto). `simpleAttack` acepta `.22000` o `.pcap/.pcapng` existente (omite captura/conversi¢n segun material, p.ej. Pwnagotchi). `gen_pwnagotchi_kit` genera guia/config/snippets de lab sin ejecuci¢n remota ni cambios de driver/RF. 0 huérfanos UI<->backend. **2026-11-19**: implementado; lint/vite/cargo check+test lib verdes; hashcat v7.1.2 => PCFG skip justificado; E2E con .22000 real/hashcat v8+ PENDIENTE D-07.

## Historias de mantenimiento (M)

| ID | Historia | CA | Estado |
|---|---|---|---|
| M-01 | Puesta en orden de docs + spec/ | Docs sin contradicciones; `spec/00..04` existen; sesiÃ³n 2026-10-06 en memory | âœ… 2026-10-06 |
| M-02 | Trabajo 10-01/10-05 commiteado | `git status` limpio (o solo ignorados) tras aprobaciÃ³n explÃ­cita del usuario | âœ… 2026-10-06 (`64bb5d0`, `d193a30`, `33b2b6b`, push `a1b5ba3`) |
| M-03 | Basura de raÃ­z eliminada | `1` y `yye` fuera del repo (eran dumps de captura, ~1,2 MB) | âœ… 2026-10-06 (borrados; logs â†’ `.gitignore`) |
| M-04 | Verificaci¢n completa post-docs | lint + vite + cargo build 0 warnings + cargo test --lib 35 passed / 0 failed / 8 ignored | **CERRADA 2026-11-19**: cargo test --lib = 35 passed (vs 30/8 de 2026-11-18). E0-16/E0-17 implementadas. M-04 queda verde.

## Reglas de aceptaciÃ³n transversales
- Cualquier CA hardware exige salida real pegada en `memory.md` (fechas arriba).
- Nueva historia â†’ aÃ±adir fila aquÃ­ ANTES de implementar (SDD).
- CA imposible en Windows (Npcap #85, etc.) â†’ se documenta el lÃ­mite medido y
  la historia redirige a Kali; no se declara âœ… con workaround no probado.
