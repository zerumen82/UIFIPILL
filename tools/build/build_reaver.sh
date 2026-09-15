#!/bin/bash
# build_reaver.sh — compila reaver+wash para Windows (MSYS2 MINGW64).
# Se llama desde build_all.ps1 SIN argumentos para evitar el entrecomillado
# PowerShell->bash (que dejaba CFLAGS_USER vacío y rompía la compilación).
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"   # tools/build
SRC="$ROOT/reaver/src"
STUBS="$ROOT/stubs"
COMPAT="$ROOT/win32_compat.h"
cd "$SRC"
make clean >/dev/null 2>&1 || true
make -j"$(nproc 2>/dev/null || echo 4)" CC=gcc \
  CFLAGS_USER="-Wall -O3 -DCONFIG_IPV6 -DCONFIG_NATIVE_WINDOWS -Wno-unused-but-set-variable -I$STUBS -include $COMPAT" \
  LDFLAGS="-lm -lpcap -lpthread -lws2_32" \
  LIBNL_CFLAGS="" LIBNL_LDFLAGS=""
