#!/usr/bin/env python3
"""Scan .text of netr28ux.sys for immediate constants of RT2800 CSR registers.

Finds 4-byte LE imm32 matches and clusters them, printing surrounding context
so we can identify the register-access helper routines.
"""
import struct
from collections import defaultdict

PATH = "../netr28ux.sys"
TEXT_START, TEXT_END = 0x400, 0x1BBC00
BASE = 0x140000000
F2R = lambda off: off - 0x400 + 0x1000

REGS = {
    0x11C: "BBP_CSR_CFG",
    0x138: "RF_CSR_CFG",
    0x7010: "H2M_MAILBOX_CSR",
    0x7014: "H2M_MAILBOX_CID",
    0x701C: "H2M_MAILBOX_STATUS",
    0x7028: "H2M_BBP_AGENT",
    0x0404: "HOST_CMD_CSR",
    0x02A0: "USB_DMA_CFG",
    0x1004: "MAC_SYS_CTRL",
    0x1200: "MAC_STATUS_CFG",
    0x1208: "AUTOWAKEUP_CFG",
    0x0208: "PBF_CSR_CTRL?",  # misc
    0x0400: "MCU_CMD?",
}

data = open(PATH, "rb").read()
text = data[TEXT_START:TEXT_END]

hits = defaultdict(list)
for reg, name in REGS.items():
    needle = struct.pack("<I", reg)
    start = 0
    while True:
        i = text.find(needle, start)
        if i < 0:
            break
        hits[reg].append(TEXT_START + i)
        start = i + 1

for reg, offs in sorted(hits.items()):
    name = REGS[reg]
    print(f"\n=== {name} (0x{reg:x}): {len(offs)} hits ===")
    for o in offs[:40]:
        ctx = data[o - 6:o + 4]
        print(f"  file=0x{o:06x} rva=0x{F2R(o):06x} ctx={ctx.hex(' ')}")
