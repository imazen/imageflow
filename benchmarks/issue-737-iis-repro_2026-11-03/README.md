# Issue #737: CMYK ICC profile on an RGB JPEG, IIS app-pool crash

Reproduction harness and results for
[imazen/imageflow#737](https://github.com/imazen/imageflow/issues/737), run 2026-11-03.
Raw numbers are in [results.txt](results.txt).

## Report

A site on .NET Framework 4.8.1 under IIS resizes JPEGs with Imageflow.Net 0.15.1 and
Imageflow.NativeRuntime 2.3.1-rc01
([reporter's code](https://github.com/imazen/imageflow/issues/737#issuecomment-6597976600)).
Two images fail with `ColorProfileError: ICC profile has CMYK color space but image data is RGB`.
The code catches that exception, but the application pool stops after about ten failures.

## Findings

1. **The two attached images fail to decode, and always have.** They hold YCbCr pixels but embed
   the CMYK profile "U.S. Web Coated (SWOP) v2". 2.3.1-rc01 rejects them at
   `moxcms_transform.rs:188`; 2.2.0-rc01, 2.1.2-rc03 and 2.1.1-rc11 (lcms2) reject them with
   `Wrong input color space on transform`. The exception is caught normally. Workaround:
   `ignore_icc_errors=true`, whose output is byte-identical to the profile-stripped image.
2. **The app-pool crash is a native stack overflow in 2.3.1-rc01, on other images.** The
   reporter's event log shows `w3wp.exe` dying with exception `0xc00000fd` in `imageflow.dll`
   at offset `0x4cec17`. With the PDB from the GitHub release, that offset is `__chkstk`, the
   stack probe of a function with a large frame. The DLL's unwind data lists four such
   functions, all moxcms 0.8.1 `make_transformer_{3x3,4x3}_*`, at 786,728 bytes each:
   moxcms builds a 65,536-entry barycentric weight table on the stack when
   `BarycentricWeightScale::High` is set, and imageflow sets it since 4d2f7e6a, first released
   in 2.3.1-rc01. IIS worker threads have 512 KB of stack on x64 and 256 KB on x86. A stack
   overflow cannot be caught in .NET, so the worker dies, and IIS Rapid-Fail Protection
   disables the pool after 5 crashes in 5 minutes.
3. **Which images:** any CMYK JPEG (with or without an embedded profile; imageflow falls back to
   the same SWOP profile) and any RGB image whose ICC profile has LUT tags and no matrix/TRC.
   The transform is cached per process after it is built, so a worker dies on the first such
   image it sees. The two attached images never reach that code, which is why the first round
   of tests below did not crash.
4. **Reproduced with the exact binaries.** CMYK and LUT-profile JPEGs kill a .NET 4.8.1
   process on 512 KB (x64) and 256 KB (x86) threads and run on 1 MB threads. In IIS Express
   x64 the first CMYK request on a request thread kills the worker, and Windows logs the same
   module, timestamp, exception code and fault offset as the reporter's event. The 2.2.0-rc01
   DLL under the same Imageflow.Net handles the CMYK JPEG on 256 KB and 512 KB threads.
5. **Fix (main):** `moxcms_transform.rs` builds every moxcms LUT transform on a dedicated
   4 MB thread. Building the CMYK transform needs more than 768 KB and at most 1 MB on Linux.
   `small_stack.rs` runs whole jobs on 256 KB threads and failed before the fix.
6. **Workaround for 2.3.1-rc01:** run the Imageflow.Net call on a thread with a larger stack
   (the handler's `bigstack=4096` option does this), or convert CMYK and LUT-profile images
   before they reach imageflow.

The first round of tests, with only the two attached images, found no crash:

| host | bitness | load | failing decodes | result |
|---|---|---|---|---|
| console, default stack | x64 | 100 sequential | 100 | no crash |
| console, default / 256 KB stack | x86 | 50 + 50 sequential | 100 | no crash |
| console, 256 KB / 512 KB stack | x64 | 50 + 50 sequential | 100 | no crash |
| console, 16 threads, 256 KB stacks | x64, x86 | 1,600 per bitness, half failing | 800 per bitness | no crash |
| IIS Express 10, integrated pipeline | x64 | 934 requests, up to 16 concurrent | 732 | 934/934 HTTP 200 |
| IIS Express 10, integrated pipeline | x86 | 833 requests, up to 16 concurrent | 632 | 833/833 HTTP 200 |
| `abi_loop.py` on `libimageflow.so` (Linux) | x64 | 1,000 sequential | 1,000 | RSS flat at 16.7 MB |

## Inputs

The two images attached to the issue are not committed; download them from the issue:

| file | source | notes |
|---|---|---|
| `cmyk_icc_on_rgb_1.jpg` | `user-attachments/assets/e79947da-ce3c-46a7-8d96-6ad086bdc70e` | 500×500 baseline JPEG, 3 components (YCbCr 4:4:4), Photoshop CS6, embeds "U.S. Web Coated (SWOP) v2" (557 KB, Lab PCS, LUT-based) |
| `cmyk_icc_on_rgb_2.jpg` | `user-attachments/assets/01b983e8-23f0-4fcd-bd5b-f94ca8b8f540` | same camera, software and profile |

Made from the first one: `valid_noicc.jpg` (`exiftool -icc_profile=`), `valid_widegamut.jpg`
(`exiftool "-icc_profile<=WideGamutRGB.icc"`), and `cmyk_swop.jpg` / `cmyk_noicc.jpg`
(`make_cmyk.py`: PIL CMYK conversion, with and without the SWOP profile). `cmyk_logo.jpg`,
`MarsRGB_v4_sYCC_8bit.jpg` and `MarsRGB_tagged.jpg` are the integration-test inputs of those names.

## Running the harness

Windows with the .NET SDK and .NET Framework 4.8.1 (IIS Express for the web host):

```
cd console
dotnet build -c Release -p:Platform=x64 -o bin-x64     (or x86)
bin-x64\Repro737.exe "<img1>;<img2>" <iterations> [stackKb] [querystring] [threads]
```

```
cd iis-express
dotnet build -c Release -o <site>\bin
copy web.config <site>\
set IMAGEFLOW_REPRO_IMAGE_DIR=<folder with the images>      (default: <site>\images)
"C:\Program Files\IIS Express\iisexpress.exe" /path:<site> /port:3737 /clr:v4.0
curl "http://localhost:3737/resize?img=cmyk_swop.jpg&gc=1"                 (request thread)
curl "http://localhost:3737/resize?img=cmyk_swop.jpg&bigstack=4096"        (4 MB thread)
```

Use `C:\Program Files (x86)\IIS Express\iisexpress.exe` for a 32-bit worker. The handler writes
caught exceptions to `web-log.txt` in the image folder. Restart IIS Express between crash tests:
a transform built once is cached for the life of the process.

Analysis scripts (Python 3, standard library except `make_cmyk.py`, which needs Pillow):

```
python3 -I pe_frames.py imageflow.dll [min_bytes]     # x64 PE functions by stack allocation
python3 -I icc_tags.py <profile.icc>...               # color space, PCS, LUT vs matrix/TRC tags
python3 -I make_cmyk.py <rgb.jpg> <cmyk.icc> <outdir> # CMYK JPEGs with and without a profile
python3 -I abi_loop.py <path>/libimageflow.so <image> 1000 [querystring]
```

Symbolize a fault offset with the PDB from the matching GitHub release zip:
`llvm-symbolizer --obj=imageflow.dll 0x<ImageBase + offset>`.
