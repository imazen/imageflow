import re,os,collections,sys
X=os.path.expanduser('~/tmp/resize-why')
def load(p):
    d={}
    for l in open(p):
        if l.startswith('#') or not l.strip(): continue
        x=l.rstrip('\n').split('\t'); d[x[0]]=x
    return d
def norm(s):
    m=re.search(r'([0-9a-f]{10,})',s)
    return m.group(1)[:10] if m else s
pre=load(os.path.expanduser('~/tmp/triage/hist_pre.tsv'))
ctrl=load(X+'/manifests/control.tsv')
H=lambda d,k:norm(d[k][4]) if k in d else None
com=[k for k in pre if k in ctrl]
diff=[k for k in com if H(pre,k)!=H(ctrl,k)]
print('checks in both',len(com),'; differ from pre-zenresize on zenresize',len(diff))
for n in ['control','control2','f32','all','only_DECODE','only_ENCODE','only_UNPREMUL','only_WEIGHTS','only_KERNELS','only_F32CACHE','without_DECODE','without_ENCODE','without_UNPREMUL','without_WEIGHTS','without_KERNELS','without_F32CACHE','all_no_force_f32']:
    m=load(f'{X}/manifests/{n}.tsv')
    eq=sum(H(pre,k)==H(m,k) for k in com)
    fixed=sum(H(pre,k)==H(m,k) for k in diff)
    st=collections.Counter(x[1] for x in m.values())
    print(f"{n:20s} match pre {eq:3d}/{len(com)}  of the {len(diff)} changed, restored {fixed:3d}  status {dict(st)}")
m=load(X+'/manifests/all.tsv')
left=[k for k in com if H(pre,k)!=H(m,k)]
print('\nstill differing with every stage swapped:',len(left))
for k in left: print('  ',k,m[k][1],m[k][2])
