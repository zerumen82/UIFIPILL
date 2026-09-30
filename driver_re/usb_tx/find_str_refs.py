#!/usr/bin/env python3
"""Locate BBP-related strings in netr28ux.sys .text and find code references.

Mapping from memory.md §1 (bitácora 2026-09-25): file offset -> rva = file_off - 0x400 + 0x1000
Image base for a PE32+ driver: 0x140000000 (typical for x64).
We scan the whole file for disp32 little-endian values that match the
VA of each string, then report the file offset of the referencing instruction.
"""
import struct, sys

PATH = "../netr28ux.sys"
FILE2RVA = lambda off: off - 0x400 + 0x1000
RVA2FILE = lambda rva: rva + 0x400 - 0x1000

data = open(PATH, "rb").read()
print(f"file size: {len(data)}")

# strings we hunt (from previous session notes)
targets = [
    b"AsicBbpTuning",
    b"PostBBPInitialization",
    b"BbpInit7601",
    b"NICRestoreBBPValue",
    b"AsicWriteBBPR66",
    b"WriteBBPR66",
    b"RTUSBBulkOutPktCmd",
    b"RTUSBBulkOutMLMEPacket",
    b"RTUSBBulkReceive",
]

# find all occurrences (string + possible \0, aligned or not)
locs = {}
for t in targets:
    start = 0
    while True:
        i = data.find(t, start)
        if i < 0:
            break
        locs.setdefault(t, []).append(i)
        start = i + 1

for t, offs in locs.items():
    for off in offs:
        print(f"STR {t.decode():28s} file=0x{off:06x} rva=0x{FILE2RVA(off):06x}")

# Now scan .text for RIP-RELATIVE disp32 refs to each string.
# .text: file 0x400..0x1bbc00, rva 0x1000..0x1bbc00 (mapping file->rva = off-0x400+0x1000).
TEXT_START, TEXT_END = 0x400, 0x1BBC00
BASE = 0x140000000  # typical PE32+ driver image base

# reference model: instruction ends (disp32 last byte + 1) at file offset E,
# next insn VA = BASE + FILE2RVA(E). rip-rel target = next + disp32.
# So disp32 = str_va - (BASE + FILE2RVA(foff_of_disp + 4)).
# We scan every disp32-sized window and check whether it points at a target string.
# To be efficient: compute target set first, then scan all 4-byte windows in .text
# and test membership (targets ~ tens of entries, windows ~ 1.8M -> fine).

target_vas = {}
for t, offs in locs.items():
    for off in offs:
        target_vas.setdefault(BASE + FILE2RVA(off), []).append(t.decode())

# map from disp value to (str names, target va)
from collections import defaultdict
by_disp = defaultdict(list)
# NOTE: only refd via rip-rel; the ref may point at string start OR anywhere inside
# (e.g. lea rax, [rip+disp] pointing at 'Post' vs the tail). Cover every byte
# offset inside each string too.
for t, offs in locs.items():
    for off in offs:
        for k in range(len(t)):
            va = BASE + FILE2RVA(off + k)
            by_disp[va - BASE].append((t.decode(), off, k))  # store rva

text = data[TEXT_START:TEXT_END]
found = 0
for i in range(len(text) - 4):
    disp = struct.unpack_from("<i", text, i)[0]  # signed
    disp_end_file = TEXT_START + i + 4
    next_rva = FILE2RVA(disp_end_file)
    target_rva = next_rva + disp
    if target_rva in by_disp:
        names = by_disp[target_rva]
        foff = TEXT_START + i
        print(f"REF disp@file=0x{foff:06x} rva=0x{FILE2RVA(foff):06x} -> "
              f"{names[0][0]} (str file=0x{names[0][1]:06x}, +{names[0][2]})")
        found += 1
print(f"\ntotal rip-relative refs found: {found}")
