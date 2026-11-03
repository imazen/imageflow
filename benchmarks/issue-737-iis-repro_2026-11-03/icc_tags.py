"""Print color space, PCS, version and tag signatures of ICC profiles.

usage: python3 -I icc_tags.py <profile.icc>...
"""
import struct, sys

for path in sys.argv[1:]:
    d = open(path, 'rb').read()
    cs, pcs = d[16:20].decode('latin1'), d[20:24].decode('latin1')
    ver = d[8]
    n = struct.unpack_from('>I', d, 128)[0]
    tags = [d[132 + 12 * i:136 + 12 * i].decode('latin1') for i in range(n)]
    lut = [t for t in tags if t.startswith('A2B') or t.startswith('B2A')]
    trc = [t for t in tags if t in ('rTRC', 'gTRC', 'bTRC', 'rXYZ', 'gXYZ', 'bXYZ')]
    print(f"{path.split('/')[-1]:28} cs={cs!r} pcs={pcs!r} v{ver} lut={lut} matrix_trc={len(trc)}/6")
