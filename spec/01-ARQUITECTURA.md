# spec/01 — ARQUITECTURA (procesos, contratos, módulos)

> Invariante: cada `invoke` del frontend tiene handler Tauri registrado en
> `lib.rs` (~95 comandos) y viceversa; args en **camelCase** (Tauri v2 los
> convierte), respuestas en snake_case.

## Procesos

| Proceso | Rol |
|---|---|
| `uifipill.exe` (Tauri v2, WebView2) | UI + orquestación de ataques/escaneos |
| Binarios externos (`tools/`, `<instalado>\_up_\tools\`) | reaver/wash, hashcat, aircrack-ng, hcxdumptool (Kali) |
| `rt3070_*` (`driver_re/usb_tx/`, rusb 0.9 sobre **WinUSB**) | TX/RX/scan/deauth crudos 802.11 |
| netr28ux.sys **parcheado** (testsigning) / Npcap LWF | modo monitor + canal + captura pasiva |
| Kali-WSL2 / Kali live USB | recetas RF que Windows no puede TX-eN (reaver real, hcxdumptool…) |

## Contratos clave

- **Eventos**: backend → frontend solo `attack-started` / `attack-progress`
  (chunks 4-8 KB, techo de eventos, coalescencia) / `attack-completed` /
  `attack-error`. Nunca un evento por línea de stdout. Frontend pinta por
  frame y **cero DOM con el tab oculto**.
- **Lock de ataque**: `claimAttack(id)` = busy-check + fijar
  `currentAttackId` atómicos; barra `#attack-status-bar` con ■ Detener
  siempre activo; fail-safe al entrar en Escanear.
- **Canales**: `selectedChannel()` hereda el canal del AP escaneado en TODOS
  los flujos (regla 8).
- **Éxito real** (regla 2): `.cracked` escrito por hashcat · `WPS PIN:` en
  stdout · `TXDONE` del chip · frames RX parseados. `success=false` si 0.
- **Interfaz Npcap**: `\Device\NPF_WIFI_{GUID}` (dlt127 radiotap) para
  wash/reaver/captura; `NPF_` (dlt1) solo Ethernet. Flag `-F` (FCS) en
  wash/reaver. `tools/libpcap.dll` = copia del `wpcap.dll` de Npcap.

## Estados de la radio (exclusivos, transiciones documentadas)

```
managed (netsh) ──activate_monitor──▶ monitor (OID 8 bytes, ULONG@4)
monitor ──restore_managed──▶ managed
managed/monitor ──switch_winusb (UAC)──▶ WinUSB crudo (no es WiFi para netsh)
WinUSB ──restore_netr28ux.ps1──▶ netr28ux (managed)
```

## Reglas duras USB/RT3070 (medido; detalle AGENTS.md regla 3)

- **NUNCA** kick `USB_DEVICE_MODE FIRMWARE(8)` bajo WinUSB (mata el chip 5/5;
  solo power-cycle físico 15 s lo recupera).
- Chip degradado (STALL, 0x00/0xff, timeout) → power-cycle antes del test.
- Vendor requests: `SINGLE_WRITE=2, SINGLE_READ=3, MULTI_WRITE=6, MULTI_READ=7`;
  modo en **wValue** de `USB_DEVICE_MODE`.
- Canal: path **rf3xxx** (N→RFCSR2, K→RFCSR3[3:0], R→RFCSR6[1:0]); no rf53xx.
- MCU: `MCU_CURRENT=0x36`; BBP preferido por MCU (`MCU_BBP_SIGNAL=0x80`) con
  fallback directo `BBP_CSR_CFG` (`bbp_probe_transport`); nunca kick con MCU vivo.