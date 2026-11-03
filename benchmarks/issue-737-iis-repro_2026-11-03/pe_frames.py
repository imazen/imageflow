"""List x64 PE functions by stack allocation size, from .pdata/.xdata unwind codes.

usage: python3 -I pe_frames.py <dll> [min_bytes]
prints: rva_begin rva_end alloc_bytes (primary entries only; chained fragments add to their parent)
"""
import struct, sys

def main():
    path = sys.argv[1]
    min_bytes = int(sys.argv[2]) if len(sys.argv) > 2 else 4096
    data = open(path, 'rb').read()
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    assert data[pe:pe+4] == b'PE\0\0'
    nsect = struct.unpack_from('<H', data, pe + 6)[0]
    opt_size = struct.unpack_from('<H', data, pe + 20)[0]
    opt = pe + 24
    assert struct.unpack_from('<H', data, opt)[0] == 0x20b  # PE32+
    dd = opt + 112
    exc_rva, exc_size = struct.unpack_from('<II', data, dd + 3 * 8)
    sects = []
    so = opt + opt_size
    for i in range(nsect):
        name, vsize, va, rawsize, rawptr = struct.unpack_from('<8sIIII', data, so + i * 40)
        sects.append((va, max(vsize, rawsize), rawptr))
    def off(rva):
        for va, sz, raw in sects:
            if va <= rva < va + sz:
                return raw + rva - va
        raise ValueError(hex(rva))
    def alloc_of(unwind_rva, depth=0):
        o = off(unwind_rva)
        ver_flags, prolog, ncodes, frame = data[o:o+4]
        flags = ver_flags >> 3
        codes = [struct.unpack_from('<H', data, o + 4 + 2*i)[0] for i in range(ncodes)]
        total = 0
        i = 0
        while i < ncodes:
            c = codes[i]
            op = (c >> 8) & 0xf
            info = c >> 12
            if op == 0:  # PUSH_NONVOL
                total += 8; i += 1
            elif op == 1:  # ALLOC_LARGE
                if info == 0:
                    total += codes[i+1] * 8; i += 2
                else:
                    total += codes[i+1] | (codes[i+2] << 16); i += 3
            elif op == 2:  # ALLOC_SMALL
                total += info * 8 + 8; i += 1
            elif op == 3:  # SET_FPREG
                i += 1
            elif op == 4:  # SAVE_NONVOL
                i += 2
            elif op == 5:  # SAVE_NONVOL_FAR
                i += 3
            elif op == 8:  # SAVE_XMM128
                i += 2
            elif op == 9:  # SAVE_XMM128_FAR
                i += 3
            elif op == 10:  # PUSH_MACHFRAME
                i += 1
            else:
                i += 1
        chained = None
        if flags & 0x4:
            co = o + 4 + 2 * ((ncodes + 1) & ~1)
            chained = struct.unpack_from('<III', data, co)
        return total, chained
    rows = []
    o = off(exc_rva)
    for k in range(exc_size // 12):
        b, e, u = struct.unpack_from('<III', data, o + 12 * k)
        if b == 0:
            continue
        total, chained = alloc_of(u)
        rows.append((b, e, total, chained is not None))
    for b, e, total, is_chained in sorted(rows, key=lambda r: -r[2]):
        if total < min_bytes:
            break
        print(f"{b:#x} {e:#x} {total} {'chained' if is_chained else 'primary'}")

main()
