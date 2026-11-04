"""Compares two REGRESS_MANIFEST_PATH manifests by output hash.

    python3 manifest_cmp.py <before.tsv> <after.tsv>
"""
import csv,sys
def load(n):
    out={}
    for r in csv.reader(open(n),delimiter='\t'):
        if not r or r[0].startswith('#'): continue
        out[r[0]]=r
    return out
a=load(sys.argv[1]); b=load(sys.argv[2])
print('checks before',len(a),'after',len(b),'only-before',sorted(set(a)-set(b))[:10],'only-after',sorted(set(b)-set(a))[:10])
same=[k for k in a if k in b and a[k][4]==b[k][4]]
diff=[k for k in a if k in b and a[k][4]!=b[k][4]]
print('identical output hash:',len(same),' changed:',len(diff))
for k in sorted(diff):
    print(f"{k}\n   before {a[k][1]:9} {a[k][4]}\n   after  {b[k][1]:9} {b[k][4]}  zdsim={b[k][2]} tol={b[k][3]}\n   {b[k][10][:170]}")
print('status changes:')
for k in sorted(set(a)&set(b)):
    if a[k][1]!=b[k][1]: print(' ',k,a[k][1],'->',b[k][1])
