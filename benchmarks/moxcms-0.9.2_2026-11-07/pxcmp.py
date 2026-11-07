"""Compares PNGs from run.sh, named {old,new}_{moxcms,lcms2}_<name>.png.

    python3 pxcmp.py <dir> <name>...

For each input: old moxcms vs new moxcms, and each against lcms2 (from the new build; the line
'lcms2 same in both builds' checks that lcms2 output did not change). Deltas are in 8-bit levels
over R, G and B; alpha is reported separately.
"""
import sys
import numpy as np
from PIL import Image
Image.MAX_IMAGE_PIXELS = None
d = sys.argv[1] + '/'
def load(p): return np.asarray(Image.open(p).convert('RGBA')).astype(int)
def stat(a, b):
    D = np.abs(a - b)[..., :3]; m = D.max(axis=2)
    return (f"changed {100*(m>0).mean():6.2f}%  max {D.max():3d} per-ch {D.reshape(-1,3).max(0).tolist()}"
            f"  mean|d| {D.mean():.3f}  >1:{100*(m>1).mean():5.2f}%  >2:{100*(m>2).mean():5.2f}%"
            f"  alpha-changed {int((a[...,3]!=b[...,3]).sum())}")
def signed(a, b):
    S = (b - a)[..., :3].reshape(-1, 3)
    return "mean signed new-old per-ch " + str([round(float(x), 3) for x in S.mean(0)])
for n in sys.argv[2:]:
    o, w = load(d+f'old_moxcms_{n}.png'), load(d+f'new_moxcms_{n}.png')
    lc, lo = load(d+f'new_lcms2_{n}.png'), load(d+f'old_lcms2_{n}.png')
    print(f"{n} {o.shape[1]}x{o.shape[0]}  lcms2 same in both builds: {bool((lc==lo).all())}")
    print('  old vs new    :', stat(o, w), ' ', signed(o, w))
    print('  old vs lcms2  :', stat(o, lc))
    print('  new vs lcms2  :', stat(w, lc))
