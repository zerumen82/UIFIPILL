# spec/04 — EJERCITO (agentes y flujo)

> Roles del ejército global (AGENTS global) aplicados a UIFIPILL. El usuario
> es el ORCHESTRATOR SUPREMO: nada de decisiones de alcance/designio/prioridad
> sin preguntarle primero (con opciones concretas + recomendación marcada).

## Roles

| Agente | Rol en UIFIPILL |
|---|---|
| **sd-coordinator** | Punto de entrada. Lee `spec/` + `memory.md` antes de tocar nada. Si la petición NO está especificada → redacta historia en `02-REQUISITOS.md` y **pregunta al usuario**. |
| **sd-editor** | Implementa historias aprobadas sin decidir alcance. Respeta las reglas duras USB (AGENTS.md regla 3) — un mal kick puede matar el chip. |
| **sd-tester** | Verifica CA con evidencia real (DoD de `00-PRINCIPIOS.md`). Su PASS/FAIL es la última palabra; hardware = captura/log pegado en memory. |
| **sd-reviewer** | Calidad: sin fakes/stubs, sin hardcode, cableado UI↔backend intacto, seguridad (lab-only, anti-inyección en WSL). APPROVE / CHANGES_REQUESTED. |
| **sd-scout** | Antes de reinventar: `npx skills find`. **Instalar solo con aprobación explícita del usuario.** |
| **sd-researcher** | Investigación previa con fuentes (versiones Npcap/tauri-cli, upstream reaver, drivers) antes de implementar. |

## Flujo obligatorio de una historia

```
petición → coordinator lee spec/ + memory.md
   → ¿está especificada en 02? NO → redacta historia → PREGUNTA al usuario
   → sd-scout (¿hay skill?) + sd-researcher (¿qué hay que saber?)
   → sd-editor implementa
   → sd-reviewer revisa (APPROVE / CHANGES_REQUESTED)
        → CHANGES_REQUESTED: sd-editor corrige → re-review
   → sd-tester verifica CA con evidencia (PASS / FAIL)
        → FAIL: diagnostica; si hay varias vías → PREGUNTA al usuario
   → PASS + APPROVE: actualiza spec/02 + memory.md → HECHO
```

## Reglas de cierre

- Declarar HECHO sin PASS del tester **y** APPROVE del reviewer = rechazable.
- Ocultar deudas → van a `03-PENDIENTES.md`; fallos → a la bitácora.
- Commits, borrados, resets, force-push: **solo con petición explícita**.
- Después de cada sesión: entrada en `memory.md` §1 con fecha + evidencia.

## Estado del ejército (2026-10-06)

- Especificación creada hoy (00-04) — el proyecto queda puesto en orden SDD.
- Primeras historias activas de la épica E0: **E0-11** (sniff sostenido) y
  **E0-12** (PMKID/handshake por RX cruda) + deuda D-01 (commits) pendiente
  de aprobación del usuario.