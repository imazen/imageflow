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
   functions, all moxcms 0.8.1 `make_transformer_{3x3,4x3}_*`, at 786,728 bytes each. Their
   8-bit instances reserve the 786,432-byte, 65,536-entry barycentric weight table that
   moxcms's `create_binned` builds on the stack for the `BarycentricWeightScale::High` arm.
   That code exists only with moxcms's `options` feature, which imageflow enabled in 4d2f7e6a
   (first released in 2.3.1-rc01), and the space is reserved whichever scale is selected at
   run time (finding 7). IIS worker threads have 512 KB of stack on x64 and 256 KB on x86. A stack
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
7. **The latest moxcms needs the same stack** (`moxcms-stack-probe/`, x86_64 Linux, release
   build with fat LTO, imageflow's moxcms features and options). Building the 8-bit CMYK or
   LUT RGB transform overflows a 768 KiB thread and succeeds on 800 KiB in 0.8.1, in 0.9.1
   (the newest release) and in master 097dccf3a9 (unreleased), with `High` or `Low`. Master
   selects `Low` for every 8-bit transform, but its 8-bit builders still reserve 786,432 bytes.
   With `create_binned` allocating the table on the heap (`create_binned-heap.patch`, against
   0.9.1), those frames disappear and the same builds succeed on 48 KiB. 8-bit `High` and `Low`
   output is byte-identical on all three versions (every RGB input; every 5th CMYK value).
8. **The same pattern runs through the rest of moxcms, and one branch removes all of it.**
   `Box::new([value; N])` builds the array on the stack before moving it to the heap, and the
   optimizer removes that temporary only sometimes. On master, 3 sites keep it in optimized
   builds: `create_binned` (`f32` and `i16`) and the scratch array in
   `make_gamma_parametric_table`, which reserves its space even when another branch of
   `build_gamma_table` runs. Which functions show it depends on the type, target and inlining: the
   `f32` table stays on the stack on aarch64 and under LTO but not in a plain x86_64 library build.
   Debug builds keep all of them, including 15 more `Box::new` table sites. Branch
   `heap-lookup-tables` of [lilith/moxcms](https://github.com/lilith/moxcms/tree/heap-lookup-tables)
   (`6279765571`) allocates every table of 4096 or more entries directly on the heap.
   See [The moxcms branch](#the-moxcms-branch).

## The moxcms branch

Functions reserving 16 KiB or more of stack (`frames.sh`, `frames.py`; library builds unless noted):

| build | master `097dccf3a9` | branch `6279765571` |
|---|---|---|
| x86_64 Linux release, imageflow's features | 11, largest 786,440 bytes | 0 |
| x86_64 Linux release, all stable features | 11, largest 786,440 | 0 |
| x86_64 Linux release, fat LTO (probe binary) | 26, largest 786,648 | 0 |
| x86_64 Windows MSVC release | 11, largest 786,600 | 0 |
| i686 Windows MSVC release | 11, largest 786,456 | 0 |
| i686 Linux release | 11, largest 786,476 | 0 |
| aarch64 Linux release | 35, largest 786,512 | 0 |
| x86_64 Linux debug | 122, largest 786,808 | 18, largest 19,488 |

The 18 debug frames left are SIMD interpolators, `Hypercube::tetra_vec3` and
`new_from_slice_with_options`, none of which hold a table. The 256-entry tables are unchanged.

Smallest thread stack, in KiB and 16 KiB steps, that builds a transform to sRGB and runs it on
4096 pixels (`run.sh`, x86_64 Linux, fat LTO in release; "16" means 16 or less):

| transform | master release | master debug | branch release | branch debug |
|---|---|---|---|---|
| 8-bit CMYK or LUT RGB | 800 | 384-576 | 16 | 64 |
| f32 or f64 CMYK or LUT RGB | 800 | 1072 | 16 | 64 |
| 16-bit CMYK or LUT RGB | 288 | 1072 | 16 | 64 |
| 12-bit CMYK or LUT RGB | 96 | 480-576 | 16 | 64 |
| 10-bit CMYK or LUT RGB | 64 | 464-576 | 16 | 64 |
| 16-bit or f64, matrix RGB or gray | 272 | 560-944 | 16 | 48 |
| f32, matrix RGB or gray | 144 | 624 | 16 | 48 |
| 12-bit, matrix RGB or gray | 80 | 560 | 16 | 48 |
| 8- or 10-bit, matrix RGB or gray | 48 | 384-560 | 16 | 48 |

On 0.8.1 and 0.9.1, 16-bit CMYK and LUT RGB need 16 and 288 KiB
(`moxcms-stack-probe/results/summary_releases_2026-11-03.txt`).

- **Output:** release-build output hashes match master for all 60 combinations of 5 profiles, 6
  depths, and `High`/`Low`. Inputs: every 8-bit RGB value, every 5th CMYK value, every gray value;
  2^20 pseudo-random pixels at other depths.
- **Build time:** not slower. Median wall time of 150 alternating process runs, branch over
  master: 0.96 (CMYK 8-bit), 0.99 (CMYK 16-bit), 0.91 (LUT RGB 8-bit), 0.98 (P3 16-bit), 0.98
  (P3 f32).
- **Windows** (`moxcms-stack-probe/results/windows_2026-11-03.tsv`, fat LTO, x64 and x86): master
  overflows at 768 KiB and passes at 832 KiB for 8-bit and f32 CMYK and LUT RGB, and overflows at 256
  KiB and passes at 512 KiB for 16-bit. The x64 branch build passes all 72 runs, from 64 KiB, the smallest size tried. The x86
  branch build was measured only for 8-bit CMYK, which passes at 64 to 512 KiB; the other 68 launches
  were denied (see the warning under [Running the harness](#running-the-harness)).
- **Tests:** the new test `transforms_build_on_a_256_kib_thread_stack` overflows on master in debug
  builds and in release builds with `options`, and passes on the branch. moxcms's CI lists, run
  locally: x86_64 16 test and 17 clippy feature sets, aarch64 (Neoverse-N1) 13 and 9, and 28 build
  and MSRV 1.89 commands. Not run: the powerpc build and the qemu-isolated test jobs.

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

Stack needed by each moxcms version (Linux; downloads two test JPEGs into the output folder):

```
nice -n19 moxcms-stack-probe/run.sh <out-dir>                     # master vs the heap branch
VERSIONS="v0_8_1 v0_9_1" nice -n19 moxcms-stack-probe/run.sh <out-dir>
nice -n19 moxcms-stack-probe/frames.sh <moxcms-checkout> <out.tsv> [min_bytes]
                                       # functions reserving that much stack, per target
```

In `moxcms-stack-probe/results/`, `sweep_2026-11-03.tsv`, `summary_2026-11-03.txt` and
`sweep_heap_patch_2026-11-03.tsv` came from the earlier `run.sh` in `1565d7f8`. To test `create_binned-heap.patch` alone, apply it to a copy
of the moxcms 0.9.1 source and point the probe's `moxcms_0_9_1` dependency at it with `path = ...`.

**Windows probes run on the Windows desktop, so ask its owner first.** They were cross-linked from WSL
with `LIB="<msvc lib>;<sdk um>;<sdk ucrt>" cargo build --release --locked --features <version>
--target <x86_64|i686>-pc-windows-msvc --config 'target.<target>.linker="rust-lld"'`. Never put
`patch`, `setup`, `install` or `update` in the name of a 32-bit exe: Windows treats it as an installer
and raises a UAC prompt on every launch, which blocks the desktop.
