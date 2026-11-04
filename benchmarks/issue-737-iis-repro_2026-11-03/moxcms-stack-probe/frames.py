#!/usr/bin/env python3
"""Lists functions whose prologue reserves at least MIN_BYTES of stack.

    python3 -I frames.py <objdump> <rlib|object|binary> [min_bytes]

Reads `<objdump> -d -r -C -w --no-show-raw-insn` output and prints
`bytes<TAB>function`, largest first. Handles the prologues rustc emits on
x86_64 and i686 (ELF and COFF: `sub $n,%rsp`, inline stack probes through
%r11/%eax, `__chkstk` / `_chkstk` / `__rust_probestack` calls) and on aarch64
(`sub sp, sp, #n[, lsl #12]`, inline probes through x9). Pushes and
pre-indexed stores are not counted, so results are a few bytes low.
"""

import re
import subprocess
import sys

FUNC = re.compile(r"^[0-9a-f]+ <(.+)>:$")
FUNCLET = re.compile(r"^\?(?:dtor|catch|fin|filt)\$")
# x86: `sub $0x2a8,%rsp`, `sub $0xc0000,%r11` (inline probe target).
X86_SUB_SP = re.compile(r"\bsub[lq]?\s+\$(0x[0-9a-f]+|[0-9]+),%(?:rsp|esp)$")
X86_SUB_PROBE = re.compile(r"\bsub[lq]?\s+\$(0x[0-9a-f]+|[0-9]+),%(r11|eax)$")
# The inline probe copies the stack pointer into the register it then lowers.
X86_PROBE_BASE = {"r11": "mov %rsp,%r11", "eax": "mov %esp,%eax"}
X86_MOV_EAX = re.compile(r"\bmov[lq]?\s+\$(0x[0-9a-f]+|[0-9]+),%(?:eax|rax)$")
X86_PROBE_CALL = re.compile(r"\bcall.*(?:_?_chkstk|__rust_probestack|_?_alloca_probe|__probestack)")
# aarch64: `sub sp, sp, #0x2a0`, `sub sp, sp, #0x1, lsl #12`, `sub x9, sp, #0xc0, lsl #12`.
A64_SUB = re.compile(
    r"\bsub\s+(sp|x9|x10|x16|x17), (sp|x9|x10|x16|x17), #(0x[0-9a-f]+|[0-9]+)(?:, lsl #(\d+))?$"
)
SCAN = 64  # prologue instructions examined per function
PAGE = 0x1000


def num(s):
    return int(s, 16) if s.startswith("0x") else int(s)


def frame_bytes(insns):
    total = 0
    probing = False
    pending_mov = None
    prev = ""
    for ins in insns[:SCAN]:
        last, prev = prev, ins
        m = X86_SUB_PROBE.search(ins)
        if m and last == X86_PROBE_BASE[m.group(2)]:
            total += num(m.group(1))
            probing = True
            continue
        m = X86_SUB_SP.search(ins)
        if m:
            v = num(m.group(1))
            if not (probing and v == PAGE):
                total += v
            continue
        m = X86_MOV_EAX.search(ins)
        if m:
            pending_mov = num(m.group(1))
            continue
        if pending_mov is not None and X86_PROBE_CALL.search(ins):
            total += pending_mov
            pending_mov = None
            probing = True
            continue
        m = A64_SUB.search(ins)
        if m:
            dst, src, v, shift = m.groups()
            v = num(v) << (int(shift) if shift else 0)
            if dst == "sp" and src == "sp":
                if not (probing and v == PAGE):
                    total += v
            elif src == "sp":
                total += v
                probing = True
            elif dst == src and probing:
                total += v
            continue
        if re.search(r"\bret[lq]?\b", ins):
            break
    return total


def main():
    objdump, path = sys.argv[1], sys.argv[2]
    min_bytes = int(sys.argv[3]) if len(sys.argv) > 3 else 16384
    proc = subprocess.run(
        [objdump, "-d", "-r", "-C", "-w", "--no-show-raw-insn", path],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        errors="replace",
    )
    results = {}
    name, insns = None, []

    def flush():
        if name is not None:
            b = frame_bytes(insns)
            if b >= min_bytes:
                results[name] = max(results.get(name, 0), b)

    for line in proc.stdout.splitlines():
        m = FUNC.match(line)
        if m:
            flush()
            name, insns = m.group(1), []
            # MSVC unwind funclets are labels inside their parent's section; disassembly
            # from them starts mid-instruction.
            if FUNCLET.match(name):
                name = None
        elif name is not None and "\t" in line:
            insns.append(" ".join(line.split("\t", 1)[1].split()))
    flush()
    for fn, b in sorted(results.items(), key=lambda kv: (-kv[1], kv[0])):
        print(f"{b}\t{fn}")


if __name__ == "__main__":
    main()
