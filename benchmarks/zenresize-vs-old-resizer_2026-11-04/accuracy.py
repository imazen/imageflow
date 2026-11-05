# Compares each resize path with an exact f64 reference: same weights formula, f64 throughout, one rounding at the end.
import numpy as np,os,subprocess,math,sys
from PIL import Image
X=os.path.expanduser('~/tmp/resize-why'); D=X+'/dump/target/release/dump'; T=X+'/acc'; os.makedirs(T,exist_ok=True)
S=os.path.expanduser('~/work/imageflow/.image-cache/sources/imageflow-resources/test_inputs/')
B,C=0.3782157550939987,0.3108921224530007
p1,p2,p3=1-B/3,-3+2*B+C,2-1.5*B-C
q1,q2,q3,q4=(4/3)*B+4*C,-8*C-2*B,B+5*C,(-1/6)*B-C
def f(x):
    x=np.abs(x)
    return np.where(x<1,p1+x*x*(p2+x*p3),np.where(x<2,q1+x*(q2+x*(q3+x*q4)),0.0))
def wm(i,o):
    s=o/i; d=min(1,s); M=np.zeros((o,i))
    for u in range(o):
        c=(u+.5)/s-.5; l=max(0,math.ceil(c-2/d-1e-4)); r=min(i-1,math.floor(c+2/d+1e-4))
        ix=np.arange(l,r+1); w=f(d*(ix-c)); w[np.abs(w)<=2e-8]=0; M[u,l:r+1]=w/w.sum()
    return M
def s2l(v):
    v=v/255.0; return np.where(v<=0.04045,v/12.92,((v+0.055)/1.055)**2.4)
def l2s(v):
    v=np.clip(v,0,1); return np.where(v<=0.0031308,12.92*v,1.055*v**(1/2.4)-0.055)*255.0
def ref(a,ow,oh,alpha):
    h,w=a.shape[:2]; Wh=wm(w,ow); Wv=wm(h,oh)
    lin=s2l(a[...,:3].astype(np.float64)); al=a[...,3].astype(np.float64)/255.0 if alpha else np.ones((h,w))
    out=np.zeros((oh,ow,3))
    for c in range(3): out[...,c]=Wv@(lin[...,c]*al)@Wh.T
    oa=Wv@al@Wh.T
    if alpha:
        safe=oa>0
        for c in range(3): out[...,c]=np.where(safe,out[...,c]/np.where(safe,oa,1),out[...,c])
    return np.floor(l2s(out)+0.5).astype(int),np.floor(np.clip(oa,0,1)*255+0.5).astype(int)
PATHS=[('zenresize default',{}),
       ('zenresize F32 path (f16 cache)',{'ZENRESIZE_FORCE_F32':'1'}),
       ('zenresize F32 path, f32 cache',{'ZENRESIZE_FORCE_F32':'1','ZR_OLD_F32CACHE':'1'}),
       ('F32 path, old 16K encoder',{'ZENRESIZE_FORCE_F32':'1','ZR_OLD_ENCODE':'1'}),
       ('F32 path, exact encoder',{'ZENRESIZE_FORCE_F32':'1','ZR_EXACT_ENCODE':'1'}),
       ('F32, f32 cache, exact encoder',{'ZENRESIZE_FORCE_F32':'1','ZR_OLD_F32CACHE':'1','ZR_EXACT_ENCODE':'1'}),
       ('old stages (bit-exact with old)',{'ZENRESIZE_FORCE_F32':'1','ZR_OLD_DECODE':'1','ZR_OLD_ENCODE':'1','ZR_OLD_UNPREMUL':'1','ZR_OLD_WEIGHTS':'1','ZR_OLD_KERNELS':'1','ZR_OLD_F32CACHE':'1'})]
def run(name,a,ow,oh,mode):
    h,w=a.shape[:2]; raw=f'{T}/in.raw'; a[...,[2,1,0,3]].astype(np.uint8).tofile(raw)
    alpha=mode=='alpha'
    R,RA=ref(a,ow,oh,alpha)
    vis=(RA>0) if alpha else np.ones_like(RA,bool)
    print(f'\n{name}  {w}x{h} -> {ow}x{oh} {mode}; reference level<32 on {100*np.mean(R[vis]<32):.0f}% of channels')
    print(f"  {'path':34s}{'format':11s}{'% ch off':>9s}{'max':>5s}{'bias':>9s}{'dark %off':>10s}{'dark max':>9s}")
    for label,env in PATHS:
        o=f'{T}/o.raw'; e=dict(os.environ); e.update(env)
        p=subprocess.run([D,raw,str(w),str(h),str(ow),str(oh),mode,o],env=e,capture_output=True,text=True)
        assert p.returncode==0,p.stderr
        b=np.fromfile(o,dtype=np.uint8).reshape(oh,ow,4)[...,[2,1,0]].astype(int)
        d=(b-R)[vis]; dark=R[vis]<32
        print(f"  {label:34s}{p.stderr.strip():11s}{100*np.mean(d!=0):9.3f}{np.abs(d).max():5d}{d.mean():9.4f}{(100*np.mean(d[dark]!=0) if dark.any() else 0):10.3f}{(np.abs(d[dark]).max() if dark.any() else 0):9d}")
def load(f):
    return np.array(Image.open(S+f).convert('RGBA'))
wh=load('waterhouse.jpg'); wh[...,3]=255
h,w=wh.shape[:2]; print('waterhouse',w,h)
for k in (2,2.56,3):
    run(f'photo /{k}',wh,round(w/k),round(h/k),'opaque')
dk=wh.copy(); dk[...,:3]=np.floor(s2l(wh[...,:3].astype(np.float64))*0.03/ (0.03) *0+ l2s(s2l(wh[...,:3].astype(np.float64))*0.02)+0.5).astype(np.uint8)
print('darkened photo: linear x0.02, max level',dk[...,:3].max())
for k in (2,2.56):
    run(f'dark photo /{k}',dk,round(w/k),round(h/k),'opaque')
rng=np.random.default_rng(737)
g=np.zeros((1024,1024,4),np.uint8); g[...,3]=255
base=np.linspace(0,24,1024)[None,:,None]+rng.normal(0,1.5,(1024,1024,3)); g[...,:3]=np.clip(np.round(base),0,255)
for k in (2,2.56):
    run(f'dark ramp 0-24 with noise /{k}',g,round(1024/k),round(1024/k),'opaque')
b=rng.integers(0,2,(1024,1024,1))*255; n=np.zeros((1024,1024,4),np.uint8); n[...,:3]=b; n[...,3]=255
for k in (2,2.56):
    run(f'black/white binary noise /{k}',n,round(1024/k),round(1024/k),'opaque')
e=np.zeros((1024,1024,4),np.uint8); e[...,3]=255; yy,xx=np.mgrid[0:1024,0:1024]; e[...,:3]=(((xx//2+yy//2)%2)*255)[...,None]
for k in (2,2.56):
    run(f'2px checker /{k}',e,round(1024/k),round(1024/k),'opaque')
e2=np.zeros((1024,1024,4),np.uint8); e2[...,3]=255; e2[...,:3]=np.where((xx%16==0)[...,None],255,3)
for k in (2,2.56):
    run(f'thin white lines on near-black /{k}',e2,round(1024/k),round(1024/k),'opaque')
sh=load('shirt_transparent.png'); h2,w2=sh.shape[:2]
run('transparent png /2.56',sh,round(w2/2.56),round(h2/2.56),'alpha')
