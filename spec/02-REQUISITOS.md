# spec/02 — REQUISITOS (historias con criterios verificables)

> Épica principal **E0 «Windows ≡ Kali»**: que en Windows funcione el pipeline
> RF completo como en Kali (scan → monitor → RX → TX → captura → crack).
> Formato: historia → criterios de aceptación (CA) medibles → estado.
> Evidencia de cada historia cerrada: sesión correspondiente en `../memory.md` §1.

## Épica E0 — Windows ≡ Kali (pipeline RF completo)

| ID | Historia | CA (criterios de aceptación) | Estado |
|---|---|---|---|
| E0-01 | Scan de redes sin Kali | `scan_wifi` (netsh) devuelve SSID/BSSID/CH/seuridad reales; fallback automático a scan USB crudo si netsh da 0 | ✅ 2026-10-01 (11 redes + RSN/SAE parseado) |
| E0-02 | Modo monitor con cambio de canal | `activate_monitor` + `set_monitor_channel` muevan la radio (beacons SOLO del canal pedido en captura) | ✅ 2026-09-21 (RT3070, CH1 vs CH6) · ✅ 2026-10-06 (RT73: CH1 99% / CH6 92%, SIN parche de driver — `lab_channel_rf`) |
| E0-03 | RX cruda por WinUSB | ≥500 frames/15 s y ≥3 APs por EP 0x81 con parser FC@20, sin kick FIRMWARE(8) | ✅ 2026-10-01 (524/3) |
| E0-04 | TX cruda al aire | Beacon/deauth con `TXDONE` y beacon visible desde otro dispositivo | ✅ 2026-09-25 |
| E0-05 | Scan hopping sostenido | ≥100 frames/30 s en ≥5 canales con ≥10 redes, sin degradación del chip | ✅ 2026-10-01 (247/10 y 225/12) |
| E0-06 | Captura pasiva nativa Npcap + `.22000` | `native_capture` ≥1 pkt real; `pcap_to_22000` extrae PMKID/EAPOL sin hcxpcapngtool | ✅ 2026-09-14 |
| E0-07 | Crack end-to-end en Windows | hashcat escribe `.cracked` (no parseo de stdout); `wifi_connect` conecta con perfil netsh | ✅ 2026-09-17 |
| E0-08 | wash E2E en Windows | `wash -i \Device\NPF_WIFI_{GUID} -c N -F` en monitor ≥1 fila WPS real; `parse_wash` laclava | ✅ 2026-10-01 (7 filas) |
| E0-09 | UI «flujo por red» sin jerga | Héroe con veredicto por `profile_target`; 0 ids/handlers/invokes huérfanos; lint+vite+26 tests | ✅ 2026-10-05 |
| E0-10 | Instalador abre la app | NSIS instala `uifipill.exe` (15,7 MB) y la app queda viva (título UIFIPILL) | ✅ 2026-10-01 |
| E0-11 | Sniff sostenido largo (USB crudo) | ≥5 min de sniff continuo con watchdog/cancel, frames contados >0 y pcap creciendo; radio restaurada al cierre | ⬜ infraestructura ✅ (300 s, PROGRESS, restore, schtasks). Bloqueo medido 2026-10-06: RT3070 RX sordo en AMBAS antenas (0 frames con aire confirmado por RT73: 7 redes, ch3 100%). Sospecha: puerto USB 3.0 xHCI / paquete WinUSB oem180 → probar puerto USB 2.0 |
| E0-12 | PMKID/handshake por RX cruda | Sniff crudo produce pcap con EAPOL/PMKID → `pcap_to_22000` → hashcat `.cracked`, todo en Windows | ⬜ abierto (RX ya dada; falta pipeline) |
| E0-13 | WPS sin Npcap | vía Kali live USB (reaver real) documentada en la app O TX cruda propia; veredicto honesto si Windows no puede asociar | ⬜ decisión: Npcap #85 cerrado (err 203 medido) |
| E0-14 | efuse/EEPROM (tx power/LNA por canal) | lectura/escritura con verificación de valor; comparación RFCSR23=0x09 vs 0x00 | ⬜ opcional (idea de sesión 10-01) |
| E0-15 | Veredicto dual (Fase 7) | Tabla final Windows crudo vs Npcap vs Kali con evidencia por flujo; marcada en memory §2 Fase 7 | ⬜ |
| E0-16 | Pipeline de candidatas dirigido (offline) | `crack_plan_preview` devuelve etapas ordenadas (keygen→máscaras→rockyou→reglas→externo) con keyspace/motivo; `crack_plan_run_bg` ejecuta en orden, detiene en `.cracked` real (hashcat), salta etapas no disponibles con hint, cancelable; UI ultra-simple en pestaña Ataque | ⬜ 2026-11-19 (infra implementada y extendida por E0-17; verificación E2E de lab pendiente D-07) |
| E0-17 | Candidatas PCFG/ML-compatibles + flujo Pwnagotchi->crack | `crack_plan`: PCFG nativo hashcat v8+ (-a 4) + `hints` con SSID seguro (omitido con motivo si <v8), diccionarios extra (`tools/`, `tools/wordlists/`, `dive.rule`) y candidatas auto-detectadas (`candidates*/passgan*/passgpt*/generated*/samples*/wordgen*.txt`), externo tipado (`.txt`->`-a 0`, feed `-a 8` v8+, ejecutable `--slow-candidates` con fallback honesto). `simpleAttack` acepta `.22000` o `.pcap/.pcapng` existente (omite captura/conversión según material, p.ej. Pwnagotchi). `gen_pwnagotchi_kit` genera guia/config/snippets de lab sin ejecución remota ni cambios de driver/RF. 0 huérfanos UI<->backend. **2026-11-19**: implementado; lint/vite/cargo check+test lib verdes; hashcat v7.1.2 => PCFG skip justificado; E2E con .22000 real/hashcat v8+ PENDIENTE D-07.

## Historias de mantenimiento (M)

| ID | Historia | CA | Estado |
|---|---|---|---|
| M-01 | Puesta en orden de docs + spec/ | Docs sin contradicciones; `spec/00..04` existen; sesión 2026-10-06 en memory | ✅ 2026-10-06 |
| M-02 | Trabajo 10-01/10-05 commiteado | `git status` limpio (o solo ignorados) tras aprobación explícita del usuario | ✅ 2026-10-06 (`64bb5d0`, `d193a30`, `33b2b6b`, push `a1b5ba3`) |
| M-03 | Basura de raíz eliminada | `1` y `yye` fuera del repo (eran dumps de captura, ~1,2 MB) | ✅ 2026-10-06 (borrados; logs → `.gitignore`) |
| M-04 | Verificación completa post-docs | lint + vite + cargo build 0 warnings + cargo test --lib 35 passed / 0 failed / 8 ignored | **CERRADA 2026-11-19**: cargo test --lib = 35 passed (vs 30/8 de 2026-11-18). E0-16/E0-17 implementadas. M-04 queda verde.
| M-05 | UI Ataque minimalista con tabla de resultados | Tab Ataque reducido a barra (objetivo + Iniciar/Detener + Kit Pwnagotchi) + tabla de resultados (flex:1) con columnas Red/BSSID/Canal/Estado/Evidencia/Clave; logs colapsables; bloque avanzado oculto intacto; 0 ids/handlers/invokes huérfanos | En curso 2026-11-20 |
| M-06 | Logs en tiempo real + anti-bloqueo en ataques automatizados | Flush inmediato con tab activo; acumulación si oculto y pintado al volver; techo de cola; progreso granular por paso; watchdog 10 min libera lock | En curso 2026-11-20 |

## Reglas de aceptación transversales
- Cualquier CA hardware exige salida real pegada en `memory.md` (fechas arriba).
- Nueva historia → añadir fila aquí ANTES de implementar (SDD).
- CA imposible en Windows (Npcap #85, etc.) → se documenta el límite medido y
  la historia redirige a Kali; no se declara ✅ con workaround no probado.
