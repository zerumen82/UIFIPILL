# spec/03 — PENDIENTES (deudas declaradas y riesgos)

> Regla global: las deudas NO se ocultan — van aquí y se revisan en cada
> historia. Última revisión: 2026-10-06.

## Deudas abiertas (bloqueantes o con riesgo)

| # | Deuda | Riesgo | Siguiente paso |
|---|---|---|---|
| ~~D-01~~ | ~~23 ficheros modificados + ~35 untracked SIN COMMITEAR~~ | ~~pérdida de ~1 semana~~ | **CERRADA 2026-10-06** con aprobación del usuario: commits `64bb5d0` (docs+spec), `d193a30` (ui), `33b2b6b` (usb_tx+backend); `git status` limpio. Push aún pendiente de petición explícita. |
| ~~D-02~~ | ~~Basura `1` (479 KB) y `yye` (697 KB) en la raíz~~ | ~~suciedad + datos de captura~~ | **CERRADA 2026-10-06**: borrados; logs de run y backups → `.gitignore` |
| D-03 | E0-11/E0-12 (sniff sostenido + PMKID/handshake por RX cruda) sin implementar | El pipeline Windows≡Kali no cierra captura→crack sin Npcap | Historia siguiente de la épica E0 |
| D-04 | E0-13 WPS: reaver/bully no asocian en Windows (Npcap #85, err 203 medido) | WPS PIN real solo en Kali live USB | Decisión de usuario: documentar vía Kali en la app vs investigar TX cruda propia |
| D-05 | Fase 7 roadmap dual sin cerrar (veredicto funcional WSL2/Kali/Windows) | Roadmap con fase pendiente desde 2026-09-14 | Cerrar con E0-15 |
| D-06 | Verificación completa post-docs pendiente (solo `npm run lint` hoy) | DoD §spec/00 incompleto | `npx vite build` + `cargo build --release` 0 warnings + `cargo test --lib` (26/8) |

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
  (3 commits aprobados); **push pendiente** de petición explícita.

## Cerradas (histórico reciente)

- ~~spec/ ausente~~ → creado 2026-10-06 (M-01).
- ~~Device ProblemCode 56~~ → INF restaurado 2026-09-30.
- ~~Instalador «se abre y se cierra»~~ → `default-run` fixeado 2026-10-01.
- ~~RX WinUSB imposible~~ → falsada 2026-10-01 (path rf3xxx).
- ~~README con mapa de 2026-09-17~~ → actualizado 2026-10-06.
- ~~Trabajo 10-01/10-05 sin commitear (D-01)~~ → 3 commits 2026-10-06.
- ~~Basura `1`/`yye` (D-02)~~ → borrada 2026-10-06.