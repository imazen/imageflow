# Issue #737: CMYK ICC profile on an RGB JPEG, reported IIS app-pool crash

Reproduction harness and results for
[imazen/imageflow#737](https://github.com/imazen/imageflow/issues/737), run 2026-11-03.
Raw numbers are in [results.txt](results.txt).

## Report

A site on .NET Framework 4.8.1 under IIS resizes JPEGs with Imageflow.Net
([reporter's code](https://github.com/imazen/imageflow/issues/737#issuecomment-6597976600)).
Two images fail with `ColorProfileError: ICC profile has CMYK color space but image data is RGB`.
The code catches the exception, but the reporter says the application pool stops after about
ten such failures, with an error in the Windows event log.

## Inputs

The two images attached to the issue (not committed; download them from the issue):

| file | source | notes |
|---|---|---|
| `cmyk_icc_on_rgb_1.jpg` | `user-attachments/assets/e79947da-ce3c-46a7-8d96-6ad086bdc70e` | 500×500 baseline JPEG, 3 components (YCbCr 4:4:4), Photoshop CS6, embeds the CMYK profile "U.S. Web Coated (SWOP) v2" (557 KB, Lab PCS, LUT-based) |
| `cmyk_icc_on_rgb_2.jpg` | `user-attachments/assets/01b983e8-23f0-4fcd-bd5b-f94ca8b8f540` | same camera, software and profile |

Control images made from the first one:
`exiftool -icc_profile= valid_noicc.jpg` (profile stripped) and
`exiftool "-icc_profile<=WideGamutRGB.icc" valid_widegamut.jpg` (a valid RGB profile, exercises
the successful CMS path).

The reporter's stack trace names commit `0635e386`, which is tag `v2.3.1-rc01`: the native
runtime shipped as `Imageflow.NativeRuntime.* 2.3.1-rc01`, which `Imageflow.Net 0.15.1` /
`Imageflow.AllPlatforms 0.15.1` depend on. All runs use those exact NuGet binaries.

## Findings

1. **The decode failure reproduces and is long-standing.** 2.3.1-rc01 rejects both images at
   `moxcms_transform.rs:188`. 2.2.0-rc01, 2.1.2-rc03 and 2.1.1-rc11 (lcms2) reject them too,
   with `Wrong input color space on transform`. The JPEG holds YCbCr data; the embedded profile
   describes CMYK. The mozjpeg decoder already ignores a GRAY profile on a color JPEG and decodes
   as sRGB, but sends a CMYK profile on YCbCr data to the RGB transform path, which refuses it.
2. **Workaround:** add `ignore_icc_errors=true` to the command string. The output is
   byte-identical to decoding the same image with its profile stripped, and valid profiles on
   other images still apply. `ignoreicc=true` gives the same output for these two images but
   drops every profile, including valid ones.
3. **The app-pool crash does not reproduce.** The reporter's verbatim `GetImageBytes`, with the
   same binaries and images, survived every configuration with flat memory and handle counts:

   | host | bitness | load | failing decodes | result |
   |---|---|---|---|---|
   | console, default stack | x64 | 100 sequential | 100 | no crash |
   | console, default / 256 KB stack | x86 | 50 + 50 sequential | 100 | no crash |
   | console, 256 KB / 512 KB stack | x64 | 50 + 50 sequential | 100 | no crash |
   | console, 16 threads, 256 KB stacks | x64, x86 | 1,600 per bitness, half failing | 800 per bitness | no crash |
   | IIS Express 10, integrated pipeline | x64 | 934 requests, up to 16 concurrent | 732 | same worker PID throughout, 934/934 HTTP 200 |
   | IIS Express 10, integrated pipeline | x86 | 833 requests, up to 16 concurrent | 632 | same worker PID throughout, 833/833 HTTP 200 |
   | `abi_loop.py` on `libimageflow.so` (Linux) | x64 | 1,000 sequential | 1,000 | RSS flat at 16.7 MB |

   `w3wp.exe` reserves 512 KB per thread on x64 and 256 KB on x86 (`SizeOfStackReserve` in
   its PE header); `iisexpress.exe` uses the same values, so the IIS Express runs had IIS-sized
   request-thread stacks. Full IIS adds Rapid-Fail Protection (IIS schema defaults: on, 5
   worker-process failures within 00:05:00 stop the pool). That matches "the application pool
   stops", but it only fires after something kills `w3wp.exe`, and nothing did here.

## Next step

The crash needs the reporter's Windows event log entries from around a pool stop. Ask for:
from the Application log, "Application Error" (event 1000, which names the faulting module and
exception code), ".NET Runtime" (1026, which has the managed stack) and any "ASP.NET 4.0.30319.0"
entries; from the System log, the "WAS" entries (5011 for a worker-process failure, 5002 for the
pool being disabled). Also ask for the app pool bitness and the installed Imageflow.Net and
Imageflow.NativeRuntime versions.

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
curl "http://localhost:3737/resize?img=cmyk_icc_on_rgb_1.jpg&gc=1"
```

Use `C:\Program Files (x86)\IIS Express\iisexpress.exe` for a 32-bit worker. The handler writes
caught exceptions to `web-log.txt` in the image folder.

Linux, against `libimageflow.so` from `Imageflow.NativeRuntime.linux-x64`:

```
python3 -I abi_loop.py <path>/libimageflow.so <image> 1000 [querystring]
```
