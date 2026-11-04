"""A 17^4 CMYK grid, each value a 16x16 cell, as a profile-less CMYK JPEG, so imageflow uses its
bundled SWOP profile.

    python3 mkgrid.py <out.jpg>
"""
import sys
from PIL import Image
n=17; cells=n*n; cell=16; side=cells*cell
vals=[round(i*255/(n-1)) for i in range(n)]
px=bytearray(side*side*4)
for c_i,c in enumerate(vals):
  for m_i,m in enumerate(vals):
    for y_i,y in enumerate(vals):
      for k_i,k in enumerate(vals):
        cx=c_i*n+m_i; cy=y_i*n+k_i
        for yy in range(cy*cell,(cy+1)*cell):
            row=bytes((c,m,y,k))*cell
            o=(yy*side+cx*cell)*4
            px[o:o+len(row)]=row
im=Image.frombytes('CMYK',(side,side),bytes(px))
im.save(sys.argv[1],quality=100,subsampling=0)
print(side,side)
