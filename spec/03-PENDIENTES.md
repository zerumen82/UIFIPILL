# spec/03 — PENDIENTES (deudas declaradas y riesgos)

> Regla global: las deudas NO se ocultan — van aquí y se revisan en cada
> historia. Última revisión: 2026-10-06.

## Deudas abiertas (bloqueantes o con riesgo)

| # | Deuda | Riesgo | Siguiente paso |
|---|---|---|---|
| ~~D-01~~ | ~~23 ficheros modificados + ~35 untracked SIN COMMITEAR~~ | ~~pérdida de ~1 semana~~ | **CERRADA 2026-10-06** con aprobación del usuario: commits `64bb5d0` (docs+spec), `d193a30` (ui), `33b2b6b` (usb_tx+backend); `git status` limpio; **push hecho** `a5fea89..a1b5ba3`. |
| ~~D-02~~ | ~~Basura `1` (479 KB) y `yye` (697 KB) en la raíz~~ | ~~suciedad + datos de captura~~ | **CERRADA 2026-10-06**: borrados; logs de run y backups → `.gitignore` |
| D-03 | E0-11/E0-12 (sniff sostenido + PMKID/handshake por RX cruda RT3070) | El pipeline Windows=Kali no cierra captura=crack sin Npcap | **CERRADA 2026-11-19** con decisión de usuario: REBOOT PENDIENTE. Proceso documentado en driver_re/usb_tx/e011_e2e.cmd y memory.md: power-cycle 15s -> restore_netr28ux (runner) -> switch WinUSB -> probe -> init -> scan -> ciclo dual RT73 en monitor.??? NO reinicia FIRMWARE(8) sin confirmación explícitoo del usuario. NO hay dato nuevo que excuda el reboot; el chip es SANO.
| D-04 | E0-13 WPS: reaver/bully no asocian en Windows (Npcap #85, err 203 medido) | WPS PIN real solo en Kali live USB | **CERRADA 2026-11-19**: decisión de usuario: vía Kali live USB (reaver real) documentada en la app. TX cruda propia opcional. No se reinvierte en TX Windows para WPS. Decision tomada y acordada.
| D-05 | Fase 7 roadmap dual sin cerrar (veredicto funcional WSL2/Kali/Windows) | Roadmap con fase pendiente desde 2026-09-14 | **CERRADA 2026-11-19**: Veredicto Fase 7 aplicado: Windows crudo (WinUSB) = scan+monitor+TX real, RX con bloqueos; Npcap = captura pasiva fiable; Kali = radio completa. Tabla en memory.md sección 2 Fase 7.
| D-07 | E0-16/E0-17 sin verificación E2E de lab (PCFG/candidatas/import) | Pipeline extendido (PCFG -a 4/hints v8+, auto-detección de wordlists/candidatas, externo tipado, import .22000/.pcap, kit Pwnagotchi) pero sin prueba con .22000 real/hardware ni hashcat v8+ en esta sesi$n | **2026-11-19 (verificado)**: crack_plan_preview genera etapas correctas (keygen -> máscaras -> rockyou -> skip PCFG por hashcat v7.1.2 < v8). supports_pcfg = false para v7.1.2. crack_plan_run_bg: cancelación OK, .cracked escrito por hashcat como unico exito. **Bloqueante**: hashcat en tools/ = v7.1.2 sin OpenCL/GPU -> no ejecuta ataques. Requiere hashcat v8+ + OpenCL (o CPU-only recompilado) + .22000 real.
| ~~D-06~~ | ~~verificación completa post-docs pendiente~~ | ~~DoD spec/00 incompleto~~ | **CERRADA 2026-11-19**: lint OK, vite build OK, cargo build --release 0 warnings, cargo test --lib **35 passed / 0 failed / 8 ignored** (vs 30/8 de 2026-11-18).

## Riesgos operativos (no deudas, pero vigentes)

- **Hardware**: kick `FIRMWARE(8)` bajo WinUSB mata el chip (5/5) → solo
  power-cycle físico 15 s; antena …D8:A7 se degrada más rápido.
- **Driver**: testsigning ON + netr28ux parcheado (hash `d2c7cf43`); rollback =
  copia inbox `94734AEF` desde DriverStore. HVCI debe permanecer OFF.
- **WinUSB**: mientras esté activo, la antena NO es WiFi para netsh →
  `restore_netr28ux.ps1` antes de flujos netsh/wash con Npcap.
- **Entorno**: shell NO admin (UAC) — scripts con log a ruta absoluta;
  `Start-Transcript`+pnputil = deadlock en PS 5.1.
- **Repo**: «no commit sin petición explícita» — D-01 cerrada 2026-10-06
  (3 commits aprobados + push `a5fea89..a1b5ba3`).

## Cerradas (histórico reciente)

- ~~spec/ ausente~~ → creado 2026-10-06 (M-01).
- ~~Device ProblemCode 56~~ → INF restaurado 2026-09-30.
- ~~Instalador «se abre y se cierra»~~ → `default-run` fixeado 2026-10-01.
- ~~RX WinUSB imposible~~ → falsada 2026-10-01 (path rf3xxx).
- ~~README con mapa de 2026-09-17~~ → actualizado 2026-10-06.
- ~~Trabajo 10-01/10-05 sin commitear (D-01)~~ → 3 commits 2026-10-06.
- ~~Basura `1`/`yye` (D-02)~~ → borrada 2026-10-06.


