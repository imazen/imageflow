# moxcms 0.9.2 vs the git pin (2026-11-07)

imageflow pinned moxcms main at 841044439 on 2026-11-04 for its heap lookup tables (#737, moxcms
PR 211). moxcms 0.9.2 is the first release with that change. It also carries two later commits:
"fix f32 clipping" (float samples only; imageflow builds 8-bit transforms) and "fixed d50
adaptation". The second changes imageflow's output.

**Why output changes.** Built-in profiles such as `ColorProfile::new_srgb()`, the destination of
every transform imageflow builds, now adapt their colorants to the ICC PCS white (0.9642, 1, 0.8249)
instead of D50 computed from xy (0.3457, 0.3585), about (0.96429, 1, 0.82510). Embedded profiles
already use the PCS white, so on 0.9.2 their white maps to sRGB white. Built-in to built-in
transforms (Display P3, gray) are unchanged. Blue rises on average.

**Size.** Decoded without resizing, each of the 14 inputs moves by at most 1 level per channel,
except 2 levels in blue on one CMYK JPEG with an embedded profile. Alpha never changes. 51 of 140
visual checks change hash; their per-check deltas between the versions were not measured.

**Against lcms2.** 0.9.2 is closer to lcms2 (lower mean error, or the same mean error on fewer
pixels) on 7 of 14 inputs (Adobe RGB: 17.33% of
pixels differ from lcms2 on the pin, 16.32% on 0.9.2; Display P3 10.14% to 8.49%; the CMYK JPEG's
mean error 0.486 to 0.286 levels, though 77.27% of its pixels differ instead of 76.82%), equal on 4,
and further on 3: Rec.2020 barely (43.81% to 43.89%), `MarsRGB_tagged.jpg`, which matched lcms2
exactly and is now 1 level lower in red on every pixel, and the LUT-only `MarsRGB_v4_sYCC_8bit.jpg`,
from a mean error of 2.00 to 2.33 levels.

**Stack.** Same as the pin: 16 KiB or less to build and run every transform in release, 48-64 KiB
in debug; no function reserves 16 KiB or more in any optimized build of 0.9.2
(`../issue-737-iis-repro_2026-11-03/moxcms-stack-probe/results/*_2026-11-07.*`).

Data and commands: `results_2026-11-07.txt`. Tools: `fetch.sh`, `run.sh`, `dump.rs`, `pxcmp.py`,
`manifest_cmp.py`.
