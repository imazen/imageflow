"""Compares PNGs from dump.rs, named {old,new}_{moxcms,lcms2}_<name>.png: old is moxcms 0.8.1, new is main.

    python3 pxcmp.py <dir> <name>...
"""
import sys
import numpy as np
from PIL import Image
Image.MAX_IMAGE_PIXELS=None
d=sys.argv[1]+'/'
names=sys.argv[2:]
def L(p): return np.asarray(Image.open(p).convert('RGBA')).astype(int)
def stat(a,b):
    D=np.abs(a-b)[...,:3]; m=D.max(axis=2)
    return f"changed {100*(m>0).mean():6.2f}%  max {D.max():3d} per-ch {D.reshape(-1,3).max(0).tolist()}  mean|d| {D.mean():.3f}  p99 {int(np.percentile(m,99))}  >2:{100*(m>2).mean():.2f}% >5:{100*(m>5).mean():.2f}%"
for n in names:
    old,new,lc=L(d+f'old_moxcms_{n}.png'),L(d+f'new_moxcms_{n}.png'),L(d+f'new_lcms2_{n}.png')
    lo=L(d+f'old_lcms2_{n}.png')
    print(n, old.shape[1],'x',old.shape[0],'lcms2 stable across versions:',bool((lc==lo).all()))
    print('  0.8.1 vs main  :',stat(old,new))
    print('  0.8.1 vs lcms2 :',stat(old,lc))
    print('  main  vs lcms2 :',stat(new,lc))
    if n=='cmyk_grid17':
        # Cell centres only: JPEG ringing does not contribute.
        c=16; o=8
        oc,nc,lc2=old[o::c,o::c],new[o::c,o::c],lc[o::c,o::c]
        print('  centres 0.8.1 vs main :',stat(oc,nc))
        print('  centres 0.8.1 vs lcms2:',stat(oc,lc2))
        print('  centres main  vs lcms2:',stat(nc,lc2))
