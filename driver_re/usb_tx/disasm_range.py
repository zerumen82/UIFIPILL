#!/usr/bin/env python3
"""Disassemble a file range of netr28ux.sys via objdump (cygwin) in binary mode.

Usage: python disasm_range.py <start_file_off_hex> <end_file_off_hex>
VMA = 0x140000000 + (file_off - 0x400 + 0x1000)  so rip-relative targets
resolve to real VAs; we post-annotate known string VAs.
"""
import subprocess, sys, tempfile, os

PATH = "../netr28ux.sys"
BASE = 0x140000000

start = int(sys.argv[1], 16)
end = int(sys.argv[2], 16)

data = open(PATH, "rb").read()

# collect string starts for annotation
strings = {}
cur = bytearray(); cur_off = 0
for i, b in enumerate(data):
    if 0x20 <= b < 0x7f:
        if not cur:
            cur_off = i
        cur.append(b)
    else:
        if len(cur) >= 8:
            strings.setdefault(bytes(cur), []).append(cur_off)
        cur = bytearray()

rva2str = {}
for s, offs in strings.items():
    for o in offs:
        rva = o - 0x400 + 0x1000
        rva2str[BASE + rva] = s.decode()

vma = BASE + (start - 0x400 + 0x1000)
with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as tf:
    tf.write(data[start:end])
    tmpname = tf.name
try:
    p = subprocess.run(
        ["objdump", "-D", "-b", "binary", "-m", "i386:x86-64",
         f"--adjust-vma={vma:#x}", "--start-address", f"{vma:#x}",
         "--stop-address", f"{vma + (end - start):#x}", tmpname],
        capture_output=True)
finally:
    os.unlink(tmpname)

out = p.stdout.decode("utf-8", errors="replace")
for line in out.splitlines():
    print(line)
