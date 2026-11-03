"""Make CMYK JPEG test inputs from an RGB JPEG.

usage: python3 -I make_cmyk.py <rgb.jpg> <cmyk.icc> <outdir>
writes cmyk_swop.jpg (CMYK + embedded profile) and cmyk_noicc.jpg (CMYK, no profile)
"""
import site, sys
# python3 -I drops the user site-packages, where Pillow is often installed.
sys.path.append(site.getusersitepackages())
from PIL import Image

src, icc_path, outdir = sys.argv[1], sys.argv[2], sys.argv[3]
icc = open(icc_path, 'rb').read()
im = Image.open(src).convert('RGB').convert('CMYK')
im.save(f"{outdir}/cmyk_swop.jpg", quality=90, icc_profile=icc)
im.save(f"{outdir}/cmyk_noicc.jpg", quality=90)
for name in ("cmyk_swop.jpg", "cmyk_noicc.jpg"):
    with Image.open(f"{outdir}/{name}") as chk:
        print(name, chk.mode, chk.size, "icc bytes:", len(chk.info.get("icc_profile") or b""))
