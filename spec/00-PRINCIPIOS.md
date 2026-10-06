# spec/00 — PRINCIPIOS (constitución + Definición de Listo)

> UIFIPILL — suite WiFi de laboratorio para Windows. Este documento es la
> constitución del proyecto: sobreescribe cualquier práctica no escrita.
> Reglas operativas y estado vigente: `../AGENTS.md`. Historia: `../memory.md`.

## Principios

1. **Laboratorio únicamente** — redes propias o con autorización explícita.
   Nada de ofensiva contra terceros.
2. **Evidencia sobre afirmaciones** — todo «funciona» exige prueba ejecutada
   y pegada en la bitácora (`memory.md`). Fallo → se documenta, no se maquilla.
3. **Sin fakes ni stubs** — ningún comando devuelve éxito simulado; binario
   ausente → hint accionable.
4. **Config, no hardcode** — parámetros en `tauri.conf.json`/config del
   sistema; cero valores mágicos en el código.
5. **Local-first** — el cómputo vive en la máquina del usuario (hashcat,
   parsers, keygens nativos); todo recurso de red se declara.
6. **Spec antes que código** — toda funcionalidad nueva nace aquí
   (`02-REQUISITOS.md`) con criterios de aceptación verificables; decidir
   alcance/designio sin preguntar al usuario = violación.
7. **Vínculo Windows≡Kali** — la meta es que el pipeline RF opere en Windows
   como en Kali (épica E0); cualquier límite medido se documenta y NO se
   reintenta sin datos nuevos.

## Definición de Listo (DoD)

Una historia está HECHA solo si **todo** esto es verde y referenciado en
`memory.md` con fecha:

- [ ] `npm run lint` sin errores.
- [ ] `npx vite build` OK (dist/ regenerado).
- [ ] `cd src-tauri && cargo build --release` con **0 warnings**.
- [ ] `cargo test --lib` completo verde (referencia actual: **26 passed /
      8 ignored**; los `lab_*` exigen hardware y se ignoran fuera de lab).
- [ ] Evidencia real del comportamiento (ej.: frames RX contados, `TXDONE`,
      wash E2E, `.cracked` escrito por hashcat, app instalada viva).
- [ ] Auditoría de cableado UI: 0 `invoke` sin handler, 0 `onclick`/id
      huérfanos en `index.html`/`app.js`.
- [ ] Historia marcada en `02-REQUISITOS.md`; deudas nuevas, si las hay, en
      `03-PENDIENTES.md`; sesión añadida en `memory.md` §1.

Nota entorno: el runner de comandos corta a ~30 s → builds largos se lanzan
con `schtasks /create + /run` y sondeo del log.