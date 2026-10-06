# spec/02 — REQUISITOS (historias con criterios verificables)

> Épica principal **E0 «Windows ≡ Kali»**: que en Windows funcione el pipeline
> RF completo como en Kali (scan → monitor → RX → TX → captura → crack).
> Formato: historia → criterios de aceptación (CA) medibles → estado.
> Evidencia de cada historia cerrada: sesión correspondiente en `../memory.md` §1.

## Épica E0 — Windows ≡ Kali (pipeline RF completo)

| ID | Historia | CA (criterios de aceptación) | Estado |
|---|---|---|---|
| E0-01 | Scan de redes sin Kali | `scan_wifi` (netsh) devuelve SSID/BSSID/CH/seuridad reales; fallback automático a scan USB crudo si netsh da 0 | ✅ 2026-10-01 (11 redes + RSN/SAE parseado) |
| E0-02 | Modo monitor con cambio de canal | `activate_monitor` + `set_monitor_channel` muevan la radio (beacons SOLO del canal pedido en captura) | ✅ 2026-09-21 (CH1 vs CH6) |
| E0-03 | RX cruda por WinUSB | ≥500 frames/15 s y ≥3 APs por EP 0x81 con parser FC@20, sin kick FIRMWARE(8) | ✅ 2026-10-01 (524/3) |
| E0-04 | TX cruda al aire | Beacon/deauth con `TXDONE` y beacon visible desde otro dispositivo | ✅ 2026-09-25 |
| E0-05 | Scan hopping sostenido | ≥100 frames/30 s en ≥5 canales con ≥10 redes, sin degradación del chip | ✅ 2026-10-01 (247/10 y 225/12) |
| E0-06 | Captura pasiva nativa Npcap + `.22000` | `native_capture` ≥1 pkt real; `pcap_to_22000` extrae PMKID/EAPOL sin hcxpcapngtool | ✅ 2026-09-14 |
| E0-07 | Crack end-to-end en Windows | hashcat escribe `.cracked` (no parseo de stdout); `wifi_connect` conecta con perfil netsh | ✅ 2026-09-17 |
| E0-08 | wash E2E en Windows | `wash -i \Device\NPF_WIFI_{GUID} -c N -F` en monitor ≥1 fila WPS real; `parse_wash` laclava | ✅ 2026-10-01 (7 filas) |
| E0-09 | UI «flujo por red» sin jerga | Héroe con veredicto por `profile_target`; 0 ids/handlers/invokes huérfanos; lint+vite+26 tests | ✅ 2026-10-05 |
| E0-10 | Instalador abre la app | NSIS instala `uifipill.exe` (15,7 MB) y la app queda viva (título UIFIPILL) | ✅ 2026-10-01 |
| E0-11 | Sniff sostenido largo (USB crudo) | ≥5 min de sniff continuo con watchdog/cancel, frames contados >0 y pcap creciendo; radio restaurada al cierre | ⬜ abierto (siguiente paso natural) |
| E0-12 | PMKID/handshake por RX cruda | Sniff crudo produce pcap con EAPOL/PMKID → `pcap_to_22000` → hashcat `.cracked`, todo en Windows | ⬜ abierto (RX ya dada; falta pipeline) |
| E0-13 | WPS sin Npcap | vía Kali live USB (reaver real) documentada en la app O TX cruda propia; veredicto honesto si Windows no puede asociar | ⬜ decisión: Npcap #85 cerrado (err 203 medido) |
| E0-14 | efuse/EEPROM (tx power/LNA por canal) | lectura/escritura con verificación de valor; comparación RFCSR23=0x09 vs 0x00 | ⬜ opcional (idea de sesión 10-01) |
| E0-15 | Veredicto dual (Fase 7) | Tabla final Windows crudo vs Npcap vs Kali con evidencia por flujo; marcada en memory §2 Fase 7 | ⬜ |

## Historias de mantenimiento (M)

| ID | Historia | CA | Estado |
|---|---|---|---|
| M-01 | Puesta en orden de docs + spec/ | Docs sin contradicciones; `spec/00..04` existen; sesión 2026-10-06 en memory | ✅ 2026-10-06 |
| M-02 | Trabajo 10-01/10-05 commiteado | `git status` limpio (o solo ignorados) tras aprobación explícita del usuario | ✅ 2026-10-06 (`64bb5d0`, `d193a30`, `33b2b6b`; push pendiente) |
| M-03 | Basura de raíz eliminada | `1` y `yye` fuera del repo (eran dumps de captura, ~1,2 MB) | ✅ 2026-10-06 (borrados; logs → `.gitignore`) |
| M-04 | Verificación completa post-docs | lint + vite + cargo build 0 warnings + cargo test 26/8 | ⬜ (hoy solo lint; docs no tocan código) |

## Reglas de aceptación transversales
- Cualquier CA hardware exige salida real pegada en `memory.md` (fechas arriba).
- Nueva historia → añadir fila aquí ANTES de implementar (SDD).
- CA imposible en Windows (Npcap #85, etc.) → se documenta el límite medido y
  la historia redirige a Kali; no se declara ✅ con workaround no probado.