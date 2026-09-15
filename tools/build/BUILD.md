# Build Status — UIFIPILL Native Windows Tools

## Compilado desde fuente (MSYS2/MinGW64, `build_all.ps1` → `build_reaver.sh`)
| Herramienta | Estado | Tamano | Notas |
|---|---|---|---|
| **reaver** | ✅ COMPILADO 2026-09-13 | 1.23 MB | WPS PIN bruteforce + Pixie Dust (`-K`). `read_iface_mac()` real (iphlpapi), canal fijado desde Rust. |
| **wash** | ✅ = copia de reaver | 1.23 MB | `ln -sf` no genera wash.exe en MSYS: el script duplica reaver.exe → wash.exe. |
| **DLLs runtime** | ✅ en `tools/` | — | libpcap.dll + libwinpthread-1.dll + libcrypto/libssl-3-x64.dll (de `/mingw64/bin`). Sin ellas: 0xC0000135. |

## Descargados (no compite compilar: sin port Windows)
| Herramienta | Estado | Origen |
|---|---|---|
| **hashcat 7.1.2** | ✅ en `tools/` (exe + modules/rules/masks/OpenCL) | https://github.com/hashcat/hashcat/releases (7z oficial) |
| **rockyou.txt** | ✅ en `tools/` (140 MB) | naive-hashcat release (github brannondorsey) |
| hcxdumptool/hcxpcapngtool | ❌ sin build Win (nl80211/Linux) | Captura en Kali; conversión nativa Rust aquí |
| bully / pixiewps / mdk3 | ❌ sin build Win confirmado | reaver cubre WPS; mdk3 → Kali |

## No portables — reemplazo Rust nativo
| Herramienta | Razon |
|---|---|
| **hcxdumptool** | Depende de `<linux/filter.h>`, `<linux/genetlink.h>`, `<linux/nl80211.h>`, `<sys/epoll.h>`, `<sys/timerfd.h>` — exclusivos de Linux. |
| **hcxtools** | Dependencias profundas POSIX/Linux (SQLite3, nl80211). |
| **bully** | ⚠️ OPCIONAL (Opción C) — no se compila: `reaver.exe` es la ruta por defecto; `bully.exe` solo como externo (`BULLY_PATH`/`tools/bully.exe`/`PATH`) o vía WSL. Requiere `<linux/if_ether.h>`, `<linux/wireless.h>`, `<linux/if_tun.h>`, `scandir()`, signals/timers POSIX. |
| **mdk3** | Depende de `<sys/socket.h>`, `<netinet/in.h>`, `<dlfcn.h>` (socket raw con POSIX). Tiene `osdep/cygwin.c` que requiere Cygwin, no MinGW64 puro. |

## Opción C — Integración WPS (reaver por defecto + bully opcional)
- Backend dispatcher: `wps_pin_bruteforce[_bg]` usa `bully -b BSSID IFACE` solo si `has_bully()`; si no, fallback `reaver -i IFACE -b BSSID -vv -L`.
- Ruta nativa: `wps_bruteforce_reaver[_bg]`, `wps_pbc_attack[_bg]` (`-S`), `wps_pixiedust[_bg]` (`-K 1`), `wash_scan[_bg]`; todos aceptan `channel` → `-c` + intento `set_monitor_channel` Npcap (reaver Win lleva `change_channel` en stub).
- Detección: `reaver.exe/wash.exe` requeridos; `bully.exe/pixiewps.exe` opcionales (`required:false`, no bloquean `all_ready`).
- UI: tarjeta Wash + selector `auto/reaver/bully` + campos canal; PBC/Pixie/Brute en `_bg` + `cancel_attack`. Lab-only: Npcap + RTL8812AU/AR9271 (Intel sin monitor).

## Build script
`build_all.ps1` (admin no requerido) llama a `build_reaver.sh` en MSYS2/MINGW64.
El .sh existe porque el entrecomillado PowerShell→`bash -c` dejaba `CFLAGS_USER`
vacío y el build fallaba (stubs + win32_compat.h no llegaban al compilador).

## Clones upstream y parches portables
Los clones anidados (`tools/build/{reaver,bully,hcxdumptool,hcxtools,mdk3}/`) están
**ignorados por git**: el port Windows vive como parche aplicable en este repo, no
dentro del clon. Hash upstream usado en cada clon:

| Clon | Upstream | HEAD usado | Parche portable |
|---|---|---|---|
| reaver | https://github.com/t6x/reaver-wps-fork-t6x | `f9b5e15` fix libnl3 support | `tools/build/patches/reaver-win-port.patch` (12 ficheros) |
| bully | https://github.com/aanarchyy/bully | `3ab3bc8` | — (Opción C, no compilado) |
| hcxdumptool | https://github.com/ZerBea/hcxdumptool | `0997eef` | — (no portable) |
| hcxtools | https://github.com/ZerBea/hcxtools | `7738995` | — (no portable) |
| mdk3 | https://github.com/aircrack-ng/mdk3 | `3be47e2` | — (no portable) |

Reconstruir reaver.exe/wash.exe desde cero:

```bash
git clone https://github.com/t6x/reaver-wps-fork-t6x tools/build/reaver
git -C tools/build/reaver checkout f9b5e15
git -C tools/build/reaver apply "$PWD/tools/build/patches/reaver-win-port.patch"
# MSYS2 + mingw-w64-x86_64-{gcc,pkg-config,libpcap,openssl} instalados:
pwsh -File tools/build/build_all.ps1     # deja tools/reaver.exe y tools/wash.exe
```

El parche incluye todo el port Win de reaver (Makefile MinGW, `defs.h`, `iface.c`
real vía GetAdaptersAddresses, `WPS_UUID`, `sigalrm`/`sigint` Win32, `wps_ufd`
con `scandir`, `lwe/wireless.h`) — se regenera con
`git -C tools/build/reaver add -N src/lwe/wireless.h && git -C tools/build/reaver diff`.

## Parches aplicados a reaver
- `Makefile`: deteccion automatica MinGW, `os_win32.o`/`eloop_win.o` en vez de `os_unix.o`/`eloop.o`, quita `-lrt`
- `defs.h`: `#undef NO_ERROR`, `#undef X509_CERT` antes de enums conflictivos con Windows
- `libwps/libwps.h`: renombrado `UUID` -> `WPS_UUID` (conflicto con `typedef GUID UUID` de Windows)
- `libwps/libwps.c`: actualizado referencias `UUID` -> `WPS_UUID`
- `utils/common.h`: agregado bloque `_WIN32` con `_byteswap_ushort`/`_byteswap_ulong`/`_byteswap_uint64`
- `sigint.c`: usa `signal()` en Windows en vez de `sigaction()`
- `sigalrm.c`: reescrito con `CreateTimerQueueTimer` API de Windows
- `iface.c`: Windows implementado — `read_iface_mac()` real vía GetAdaptersAddresses
  (iphlpapi, match por GUID/nombre; requiere `-liphlpapi` en el link MinGW).
  `next_channel()` es no-op deliberado y `change_channel()` solo registra estado + avisa:
  Npcap no permite a reaver mover la radio, el canal se fija con `-c` + `set_monitor_channel`
  desde el backend Rust antes de lanzar.
- `wpsmon.c`: protegido codigo POSIX timer/signal con `#ifndef _WIN32`
- `session.c`: `dprintf()` -> `fprintf(stderr, ...)`
- `wps/wps_ufd.c`: agregado `scandir()`/`alphasort()` para MinGW64 + `mkdir` compat wrapper

## Stubs POSIX creados en `tools/build/stubs/`
arpa/inet.h, byteswap.h, endian.h, linux/if.h, linux/if_ether.h, linux/if_tun.h, linux/nl80211.h, linux/wireless.h, net/ethernet.h, net/if.h, net/if_arp.h, net/if_dl.h, netdb.h, netinet/if_ether.h, netinet/in.h, sys/ioctl.h, sys/socket.h, sys/uio.h, sys/un.h, sys/wait.h, unistd.h
