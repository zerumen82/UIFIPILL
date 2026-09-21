# DRIVER_RE.md — Ingeniería inversa de `netr28ux.sys` (RT3070)

> Bitácora viva. Objetivo: hacer que el driver MediaTek del RT3070 acepte el cambio
> de canal en modo monitor (y a medio plazo, TX) — igualar lo que `rt2800usb`
> hace en Linux. Lab-only.

## 0. Objetivo de análisis

| Dato | Valor |
|---|---|
| Binario | `C:\Windows\System32\drivers\netr28ux.sys` |
| Copia de trabajo | `driver_re/netr28ux.sys` (2.244.952 B) |
| Versión | 5.01.25.0 (MediaTek/Ralink, 2015), x64, NDIS 6.x, WDF |
| Desensamblador | `objdump -d` (Cygwin, AT&T syntax) → `driver_re/netr28ux.asm` (~500k líneas) |
| Imagen base | `0x140000000` (formato: `140xxxxxx: VA`) |

## 1. Hallazgos fase 1 (mapeo del despachador de OIDs) — 2026-09-20

### 1.1 Despachador principal: función en `140215788`

Es el switch gigante de OIDs SET. Estructura (VA):

```
140216070  cmp/sub por rangos de OID:
           0x0D01031A, 0x0D01031B(?), 0x0D01031C, 0x0D01031D
140216260  rango 0x0D010327 / 0x0D010335 / 0x0D010342 / 0x0D01034B-4C
           ├─ 0x0D010335 (OID_DOT11_CURRENT_CHANNEL) → 1402163A1
           ├─ 0x0D010336 (frecuencia) — NO aparece como cmp literal;
           │   se alcanza por aritmética (sub/dec) en el rango
           └─ 0x0D01034B/4C → cae en 1402162A6 (≠ caso canal)
140216535  rango vendor 0xE010178, 0xE010179(?)... y default → 1402192A0
           (default = STATUS_INVALID_DEVICE_REQUEST)
```

**Dato crítico: el OID de canal NO es un stub de "no soportado".** Tiene
handler real con validación y almacenamiento.

### 1.2 Handler del OID canal — dos caminos según `opState`

Entrada en `1402163A1` (validación básica: requiere algo == 1 en
`0x28(%r13)` = InformationBuffer length, si no → `0xC0000184`
STATUS_INVALID_PARAMETER... err. `0xC0232002` en el otro caso):

```
1402163D5   testl $0x20000, 0x32D468(%rsi)      ; opState bit 17
            ├── bit SET (modo ExtENSIBLE/ExtSTA AP?) → 140216413:
            │     registra evento y devuelve 0xC0232002
            │     (STATUS_INVALID_DEVICE_STATE)  ← el rechazo que ve WlanHelper
            └── bit CLEAR → 140216420:
                  14021642E   cmp %r12d, 0x230(%rbp)   ; valida longitud buffer
                  14021643B   mov (%r14), %ebx         ; lee canal pedido
                  14021645B   mov 0x68(%rsp), %rax
                  140216460   mov %r12d, (%rax)        ; bytesWritten
                  ; ... logs WPP 0x258 ...
                  → STATUS_SUCCESS (por el camino de 140219F8F)
```

**Por eso Npcap/WlanHelper "acepta" el OID pero la radio no se mueve:**
el handler de canal en modo no-Extensible SOLO ALMACENA el valor pedido
(no programa el PHY en este handler). La programación real del canal debe
ocurrir después, en la transición de estado del adaptador que consume ese
valor almacenado. En modo Extensible directamente rechaza.

### 1.3 Estado del adaptador (offsets sobre contexto en %rsi/%rbx)

| Offset | Significado |
|---|---|
| `+0x32D468` | `opState` (bitmask). Bit 17 (0x20000) = "Extensible/extSTA activo" — gate del OID canal. Bit 14 (`0x4000`, btsl/btrl) aparece como otro estado de operación |
| `+0x332390` | flags de "opzone/halted" — si bit (== `%cl`=1) → `0xC0232000` (STATUS_INVALID_DEVICE_STATE otro) |
| `+0x230` (`%rbp`) | longitud del InformationBuffer |
| `+0xCDF2` | WORD: se escribe `0x92B`-capped (función `140216096`); valor con tope 0x92B, `shl $8` etc. (probable PHY/txpower relacionado) |
| `+0xCD80` | WORD: canal anterior pedido (función `1402161CF`, OID 0x0D01031C, escribe `mov %r9w,0xCD80(%rsi)`) |
| `+0x3143E9` | BYTE: valor OID 0x0D01031D (función `14021613D`) |

### 1.4 OIDs que sí se muestran como literales en el binario

```
0x0D010308, 0x0D01030B, 0x0D010310, 0x0D010311   (1400406E7..140040BBD)
0x0D01031B, 0x0D01031C, 0x0D010326, 0x0D010327   (1400435AE..14004396A)
0x0D010338, 0x0D01033D, 0x0D010342, 0x0D010344   (1400439F0..140043A9B)
0x0D01034B, 0x0D01034C, 0x0D01034B-4C default
0x0D010704                                       (140216260)
0xFF710335                                       (14004C594) — vendor Ralink
```

Nota: `0x0D010336` (frecuencia) no aparece como literal de 4 bytes con
`cmp` — se procesa por aritmética de rangos. El OID canal sí aparece
explícito: `cmp $0xd010335, %ecx` en `14021627F` → `1402163A1`.

## 2. Estado / conclusiones

1. **El driver no "no soporta" el canal** — lo acepta y lo almacena, pero
   **no lo propaga al hardware** en modo no-Extensible (el modo en el que
   Npcap trabaja con el adaptador en monitor). Hay que encontrar el
   consumidor de ese valor almacenado y ver por qué no programa el PHY.
2. Hipótesis de trabajo: la programación del canal al PHY ocurre en la
   **máquina de estados de conexión/scan** (que nunca corre en monitor) o
   vía **comandos USB al firmware** que solo se disparan desde esa
   transición. El valor queda en memoria pero nunca llega al chip.
3. Bit 17 de `opState` (0x20000): si se pudiese SETEAR artificialmente,
   el handler pasaría por el otro camino (Extensible) — pero ese camino
   rechaza con 0xC0232002, así que no es la vía. La vía es la contraria:
   encontrar el consumidor del canal almacenado.

## 2b. Fase 2 parcial — rutina de sintonía RF identificada (2026-09-20)

La llamada misteriosa del handler de canal (`14006a7f4`) **ES la rutina de
cambio de canal real** (63 call sites — es la central de sintonía).

### 2b.1 `14006a7f4` — SwitchChannel(ctx, channel en dl, flag en r8b)

1. **Gates de entrada** (si fallan → epígono `14006e000`, sin tocar RF):
   - `0x3335d1(ctx) != 0 || 0x3335d4(ctx) == 0` → sale
   - `opState bit17 (0x20000)` SET → **sale** (mismo gate que el OID)
   - `0xCD48(ctx)>>16 == 0x76xx` (otros chips MT7601/7612/…) → sale (RT3070 sigue)
2. **Programación PHY real** (helpers identificados):
   - `1400b5690(ctx, dl=reg, r8b=val)` → **escritura de registro RF**
   - `1400a6d78(ctx, dl=reg, r8=ptr)` → **lectura BBP**
   - `1400a7020(ctx, dl=reg, r8b=val)` → **escritura BBP**
   - `1400b0728(ctx, edx=cmdId, r8=ptr)` → **envío de comando USB al firmware**
     (id 0x5D4 en el final de la sintonía; 0x1344 en el OID 0x0D01031B)
3. Tabla de frecuencias implícita en el código (cmp %dl con umbrales
   0x0E = canal 14 separado; ramales por canal/banda).

### 2b.2 Conclusión del fallo

El driver **SÍ sabe sintonizar** (la rutina existe y es la misma que usa
el scan/conexión). El problema es de **gating**: cuando el adaptador pasa
a modo monitor/extensible, `opState` bit17 se pone a 1 y:

- el handler del OID canal rechaza con `0xC0232002`, y
- aunque llegara a llamarse, SwitchChannel **también** se corta en su
  gate `14006a8c2` (bit17) → doble bloqueo.

Quién SETEA el bit17: `btsl $0x11, 0x32D468` en `140020e90` y `14008b5f8`
(esas dos rutinas son la transición a modo extensible/monitor).

## 3. Plan de parche (fases)

### Fase 2 — Encontrar el consumidor del canal (pendiente)
- Rastrear el offset donde el handler almacena el canal pedido (el store
  final del handler de canal en el camino no-Extensible) y buscar todos
  los readers de ese offset.
- Identificar la rutina de "sintonizar PHY" (probablemente cerca de
  comandos USB con el canal como parámetro: buscar constantes 2412/5000
  kHz·5 → `0x96C` para 2412 MHz, o la tabla de canales del RT3070).
- Verificar qué dispara esa rutina (scan start? OID 0x0D010311? media
  connect?) y por qué nunca corre en monitor.

### Fase 3 — Parche binario (siguiente paso)

El parche mínimo y más prometedor ahora que conocemos los gates:

- **NOPear el gate del bit17 en SwitchChannel** (`14006a8c2`, 6 bytes:
  `f7 86 68 d4 32 00 00 00 02 00` + `jne` → NOP del test y del salto).
  Con eso SwitchChannel sintoniza aunque estemos en modo extensible.
- **Redirigir el handler del OID canal** (`140216420`, camino no-Extensible)
  para que tras validar llame a `14006a7f4(ctx, canal, 0)` en vez de solo
  loguear y devolver SUCCESS. Alternativa más simple: parchear el camino
  Extensible (`140216413`) para que en lugar de devolver `0xC0232002`
  caiga en el camino no-Extensible (cambiar el `je 140216420` por
  incondicional), y ahí añadir la llamada a SwitchChannel.
- También hay que revisar el gate del bit14 (btsl/btrl `$0xe`) por si la
  transición monitor usa otro bit que bloquee más rutas.
- Firma: `bcdedit /set testsigning on` + Secure Boot off + cert propio
  (`signtool`). Probar SIEMPRE en VM snapshot primero.
- Herramientas: parcheo con Python (pefile para VA↔file offset),
  verificación con objdump del binario parcheado, re-firma con signtool.

## 5. Parche construido y firmado (2026-09-20)

Artefactos en `driver_re/`:

| Fichero | Qué es |
|---|---|
| `netr28ux_patched.sys` | binario con los 3 parches (17 bytes), firma vieja aún dentro |
| `netr28ux_patched_clean.sys` | **ENTREGABLE**: firma vieja eliminada, overlay truncado, checksum recalculado (0x22DC3C), firmado Authenticode SHA-256 con `CN=UIFIPILL Lab Test` |
| `labtest.cer` | cert self-signed del laboratorio (makecert, EKU CodeSigning) — clave privada en el store personal del usuario |
| `deploy_driver.ps1` | instalación admin: backup del original → confiar cert → testsigning on → copiar a System32\drivers → re-scan. Requiere reinicio |
| `restore_driver.ps1` | rollback completo (driver original + testsigning off) |

### Parches aplicados (verificados con objdump)

1. **P1a** (`14006a8c2`, fileoff `0x69CC2`, 10 B): `testl $0x20000,0x32D468(%r14)` → NOP×10
2. **P1a-fix** (`14006a8cc`, fileoff `0x69CCC`, 1 B): byte alto del imm32 → NOP (la instrucción era de 11 B, no 10)
3. **P1b** (`14006a8cd`, fileoff `0x69CCD`, 6 B): `jne 0x14006e000` → NOP×6
4. **P2** (`1402163df`, fileoff `0x20D7DF`, 2 B): `je 0x140216420` → `jmp 0x140216420`

Resultado: SwitchChannel ya no se corta por el bit17 y el OID canal
acepta el SET también en modo ExtSTA, delegando en SwitchChannel.

### Descubrimiento posterior que simplifica el parche

Releyendo `14021643E–44F` (camino no-Extensible del OID): **el handler YA
llamaba a SwitchChannel(ctx, canal, 0)** — el OID "aceptaba" pero la
rutina de sintonía se auto-bloqueaba con su propio gate bit17. Por eso
P1 (NOP del gate en SwitchChannel) es el fix principal y P2 solo amplía
el OID al modo ExtSTA.

### Hashes

- original: sha256 `ADE38351A626DC0A6BCDE1D09B214C94…` (2.244.952 B)
- parcheado+firmado: sha256 `1EC0E62E580AADD54CA28508198B8A52…` pre-firma
  (2.228.736 B tras truncar overlay; la firma Authenticode se añade después)

### Estado y siguientes pasos

- [x] Parche binario construido y verificado por desensamblado
- [x] Cert de testsigning creado y usado para firmar
- [x] Scripts deploy/restore escritos
- [x] **Despliegue ejecutado** (2026-09-20, `run_deploy.cmd` vía UAC
  aceptado): backup creado (`netr28ux.sys.orig`), cert en
  TrustedPublisher+Root, testsigning ON, y reemplazo del .sys AGENDADO en
  `PendingFileRenameOperations` (el driver estaba RUNNING; se sustituye
  en el arranque antes de cargarlo) — verificar tras reiniciar
- [ ] Verificación post-reinicio: `activate_monitor` → `set_monitor_channel`
  → si la radio se mueve, HITO; probar también `lab_monitor_cycle`
- [ ] Si el canal funciona, medir TX (check_injection_capability) — puede
  seguir bloqueada por Npcap #85 (capa independiente del driver)

## 6. Diario

- 2026-09-20: fase 1 completa. Desensamblado completo generado. Handler
  de canal localizado y leído.
- 2026-09-20 (2): **fase 2 resuelta** — `14006a7f4` es SwitchChannel
  (lect/esc RF + BBP + comando USB 0x5D4). El bloqueo es el gate del
  bit17 de opState, duplicado en OID-handler y en la propia rutina.
- 2026-09-20 (4): despliegue completado con `run_deploy.cmd` (una capa de
  quoting; los intentos con Start-Process anidado fallaban por quoting).
  Log en `deploy_log.txt`, EXITCODE 0. Backup + cert + testsigning OK,
  .sys agendado para sustitución en el próximo arranque.
- 2026-09-20 (3): parche aplicado (P1a+fix+P1b+P2, 17 bytes), verificado
  con objdump. Limpiado security directory viejo, overlay truncado,
  checksum recalculado. Cert `UIFIPILL Lab Test` creado con makecert y
  binario firmado con signtool (SHA-256). Scripts deploy/restore listos.
  Pendiente: despliegue con admin + reinicio + medición.
