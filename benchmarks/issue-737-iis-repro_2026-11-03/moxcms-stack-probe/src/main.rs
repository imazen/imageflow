//! Measures how much thread stack moxcms needs to build the transforms imageflow asks it for
//! (imageflow#737), how much `BarycentricWeightScale::High` changes 8-bit output, and whether a
//! patched moxcms produces the same output as upstream.
//!
//! ```text
//! moxcms-stack-probe info   <profile>
//! moxcms-stack-probe build  <profile> <stack_kib> <high|low> <depth> [run]
//! moxcms-stack-probe diff   <profile>
//! moxcms-stack-probe digest <profile> <depth>
//! moxcms-stack-probe bench  <profile> <depth>
//! ```
//!
//! `<profile>` is an ICC file, a JPEG with an embedded profile, `builtin:display-p3` or
//! `builtin:gray-2.2`. `<depth>` is 8, 10, 12, 16, f32 or f64.
//!
//! `build` parses the profile on the main thread, then builds the transform to sRGB (RGBA, or
//! CMYK, or gray in; RGBA out) on a thread with `stack_kib` KiB of stack and prints `ok`; with
//! `run` it also transforms 4096 pixels on that thread. A stack overflow aborts the whole
//! process, so run.sh runs one size per process. `diff` builds 8-bit High and Low transforms and
//! compares their output on every RGB input, or on every 5th value of each CMYK channel.
//! `digest` prints an FNV-1a hash of the High and the Low transform's output over a fixed set
//! of inputs. `bench` prints the median time to build the transform and the median time per pixel
//! to run it over 65,536 pseudo-random pixels; bench.py compares versions across processes.

#![forbid(unsafe_code)]

#[cfg(not(any(
    feature = "v0_8_1",
    feature = "v0_9_1",
    feature = "v0_9_2",
    feature = "master",
    feature = "pin",
    feature = "patched"
)))]
compile_error!("enable one of the features v0_8_1, v0_9_1, v0_9_2, master, pin, patched");

#[cfg(feature = "v0_8_1")]
use moxcms_0_8_1 as moxcms;
#[cfg(feature = "v0_9_1")]
use moxcms_0_9_1 as moxcms;
#[cfg(feature = "v0_9_2")]
use moxcms_0_9_2 as moxcms;
#[cfg(feature = "master")]
use moxcms_master as moxcms;
#[cfg(feature = "patched")]
use moxcms_patched as moxcms;
#[cfg(feature = "pin")]
use moxcms_pin as moxcms;

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use moxcms::{
    BarycentricWeightScale, ColorProfile, DataColorSpace, InterpolationMethod, Layout,
    RenderingIntent, TransformExecutor, TransformOptions,
};

const VERSION: &str = if cfg!(feature = "v0_8_1") {
    "0.8.1"
} else if cfg!(feature = "v0_9_1") {
    "0.9.1"
} else if cfg!(feature = "master") {
    "master@097dccf3a9"
} else {
    "patched"
};

/// imageflow's `lut_transform_opts()` (imageflow_core/src/codecs/moxcms_transform.rs), with the
/// Perceptual intent `create_cmyk_transform` uses for CMYK sources.
fn imageflow_opts(cmyk: bool, scale: BarycentricWeightScale) -> TransformOptions {
    TransformOptions {
        allow_use_cicp_transfer: false,
        barycentric_weight_scale: scale,
        interpolation_method: InterpolationMethod::Tetrahedral,
        rendering_intent: if cmyk {
            RenderingIntent::Perceptual
        } else {
            RenderingIntent::RelativeColorimetric
        },
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
enum Depth {
    U8,
    U10,
    U12,
    U16,
    F32,
    F64,
}

fn parse_depth(s: &str) -> Depth {
    match s {
        "8" => Depth::U8,
        "10" => Depth::U10,
        "12" => Depth::U12,
        "16" => Depth::U16,
        "f32" => Depth::F32,
        "f64" => Depth::F64,
        _ => panic!("depth must be 8, 10, 12, 16, f32 or f64"),
    }
}

/// Source layout: gray profiles take one channel; RGB and CMYK take four.
fn src_layout(profile: &ColorProfile) -> (Layout, usize) {
    if profile.color_space == DataColorSpace::Gray {
        (Layout::Gray, 1)
    } else {
        (Layout::Rgba, 4)
    }
}

/// Deterministic input samples (xorshift32).
fn samples(n: usize) -> impl Iterator<Item = u32> {
    let mut x = 0x9e37_79b9u32;
    (0..n).map(move |_| {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    })
}

fn run<T: Copy + Default>(
    t: &(dyn TransformExecutor<T> + Send + Sync),
    src: &[T],
    in_ch: usize,
) -> Result<Vec<T>, String> {
    let mut out = vec![T::default(); src.len() / in_ch * 4];
    t.transform(src, &mut out).map_err(|e| format!("{e:?}"))?;
    Ok(out)
}

/// Builds the transform to sRGB and, if `src` is given, runs it. Returns the output.
fn build(
    profile: &ColorProfile,
    depth: Depth,
    scale: BarycentricWeightScale,
    src: Option<&[u32]>,
) -> Result<Output, String> {
    let cmyk = profile.color_space == DataColorSpace::Cmyk;
    let dst = ColorProfile::new_srgb();
    let opts = imageflow_opts(cmyk, scale);
    let (layout, ch) = src_layout(profile);
    let e = |e: moxcms::CmsError| format!("{e:?}");
    let ints = |bits: u32| -> Vec<u16> {
        src.unwrap_or(&[]).iter().map(|&v| (v & ((1 << bits) - 1)) as u16).collect()
    };
    let floats = || -> Vec<f32> {
        src.unwrap_or(&[]).iter().map(|&v| (v >> 8) as f32 / 16_777_215.0).collect()
    };
    Ok(match depth {
        Depth::U8 => {
            let t = profile.create_transform_8bit(layout, &dst, Layout::Rgba, opts).map_err(e)?;
            let input: Vec<u8> = src.unwrap_or(&[]).iter().map(|&v| v as u8).collect();
            Output::U8(if src.is_some() { run(t.as_ref(), &input, ch)? } else { Vec::new() })
        }
        Depth::U10 | Depth::U12 | Depth::U16 => {
            let (t, bits) = match depth {
                Depth::U10 => {
                    (profile.create_transform_10bit(layout, &dst, Layout::Rgba, opts), 10)
                }
                Depth::U12 => {
                    (profile.create_transform_12bit(layout, &dst, Layout::Rgba, opts), 12)
                }
                _ => (profile.create_transform_16bit(layout, &dst, Layout::Rgba, opts), 16),
            };
            let t = t.map_err(e)?;
            Output::U16(if src.is_some() { run(t.as_ref(), &ints(bits), ch)? } else { Vec::new() })
        }
        Depth::F32 => {
            let t = profile.create_transform_f32(layout, &dst, Layout::Rgba, opts).map_err(e)?;
            Output::F32(if src.is_some() { run(t.as_ref(), &floats(), ch)? } else { Vec::new() })
        }
        Depth::F64 => {
            let t = profile.create_transform_f64(layout, &dst, Layout::Rgba, opts).map_err(e)?;
            let input: Vec<f64> = floats().iter().map(|&v| v as f64).collect();
            Output::F64(if src.is_some() { run(t.as_ref(), &input, ch)? } else { Vec::new() })
        }
    })
}

enum Output {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl Output {
    /// FNV-1a over the little-endian bytes of every output sample.
    fn fnv1a(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        match self {
            Output::U8(v) => eat(v),
            Output::U16(v) => v.iter().for_each(|x| eat(&x.to_le_bytes())),
            Output::F32(v) => v.iter().for_each(|x| eat(&x.to_bits().to_le_bytes())),
            Output::F64(v) => v.iter().for_each(|x| eat(&x.to_bits().to_le_bytes())),
        }
        h
    }

    fn len(&self) -> usize {
        match self {
            Output::U8(v) => v.len(),
            Output::U16(v) => v.len(),
            Output::F32(v) => v.len(),
            Output::F64(v) => v.len(),
        }
    }
}

fn load_profile(path: &str) -> ColorProfile {
    match path {
        "builtin:display-p3" => return ColorProfile::new_display_p3(),
        "builtin:gray-2.2" => return ColorProfile::new_gray_with_gamma(2.2),
        _ => {}
    }
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let icc = if bytes.starts_with(&[0xFF, 0xD8]) {
        jpeg_icc(&bytes).unwrap_or_else(|| panic!("{path}: no ICC profile"))
    } else {
        bytes
    };
    ColorProfile::new_from_slice(&icc).unwrap_or_else(|e| panic!("{path}: {e:?}"))
}

/// Concatenates a JPEG's APP2 `ICC_PROFILE` chunks in sequence order.
fn jpeg_icc(jpeg: &[u8]) -> Option<Vec<u8>> {
    let mut chunks: Vec<(u8, &[u8])> = Vec::new();
    let mut i = 2;
    while i + 4 <= jpeg.len() && jpeg[i] == 0xFF && jpeg[i + 1] != 0xDA {
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        let segment = jpeg.get(i + 4..i + 2 + len)?;
        if jpeg[i + 1] == 0xE2 && segment.len() > 14 && segment.starts_with(b"ICC_PROFILE\0") {
            chunks.push((segment[12], &segment[14..]));
        }
        i += 2 + len;
    }
    chunks.sort_by_key(|&(seq, _)| seq);
    (!chunks.is_empty()).then(|| chunks.iter().flat_map(|&(_, data)| data).copied().collect())
}

fn parse_scale(s: &str) -> BarycentricWeightScale {
    match s {
        "high" => BarycentricWeightScale::High,
        "low" => BarycentricWeightScale::Low,
        _ => panic!("scale must be high or low"),
    }
}

fn info(profile: &ColorProfile) {
    let a2b = profile.lut_a_to_b_perceptual.is_some() || profile.lut_a_to_b_colorimetric.is_some();
    println!(
        "moxcms {VERSION}: color space {:?}, A2B LUT {}, matrix/TRC {}",
        profile.color_space,
        if a2b { "yes" } else { "no" },
        if profile.red_trc.is_some() { "yes" } else { "no" }
    );
}

fn build_on_small_stack(
    profile: ColorProfile,
    stack_kib: usize,
    scale: &str,
    depth: &str,
    run: bool,
) {
    let (scale, depth) = (parse_scale(scale), parse_depth(depth));
    let (_, ch) = src_layout(&profile);
    let src: Option<Vec<u32>> = run.then(|| samples(4096 * ch).collect());
    let handle = std::thread::Builder::new()
        .name(format!("build-{stack_kib}k"))
        .stack_size(stack_kib * 1024)
        .spawn(move || build(&profile, depth, scale, src.as_deref()).map(|_| ()))
        .expect("spawn build thread");
    match handle.join().expect("build thread panicked") {
        Ok(()) => println!("ok"),
        Err(e) => {
            println!("error {e}");
            std::process::exit(2);
        }
    }
}

fn digest(profile: &ColorProfile, depth_s: &str) {
    let depth = parse_depth(depth_s);
    let (_, ch) = src_layout(profile);
    let cmyk = profile.color_space == DataColorSpace::Cmyk;
    // 8-bit: every RGB value, every 5th value of each CMYK channel, every gray value.
    // Other depths: 2^20 pseudo-random pixels.
    let src: Vec<u32> = match depth {
        Depth::U8 if cmyk => {
            let values: Vec<u32> = (0..=255).step_by(5).collect();
            let mut v = Vec::new();
            for &c in &values {
                for &m in &values {
                    for &y in &values {
                        for &k in &values {
                            v.extend_from_slice(&[c, m, y, k]);
                        }
                    }
                }
            }
            v
        }
        Depth::U8 if ch == 1 => (0..=255).collect(),
        Depth::U8 => {
            (0..1u32 << 24).flat_map(|rgb| [rgb >> 16, (rgb >> 8) & 255, rgb & 255, 255]).collect()
        }
        _ => samples((1 << 20) * ch).collect(),
    };
    for (name, scale) in
        [("high", BarycentricWeightScale::High), ("low", BarycentricWeightScale::Low)]
    {
        match build(profile, depth, scale, Some(&src)) {
            Ok(out) => {
                assert_eq!(out.len(), src.len() / ch * 4);
                println!(
                    "moxcms {VERSION}: depth {depth_s} {name}: {} input pixels, fnv1a {:016x}",
                    src.len() / ch,
                    out.fnv1a()
                );
            }
            Err(e) => println!("moxcms {VERSION}: depth {depth_s} {name}: error {e}"),
        }
    }
}

fn median(mut ns: Vec<u128>) -> u128 {
    ns.sort_unstable();
    ns[ns.len() / 2]
}

/// Prints the median of 41 builds and of 41 runs over `src`.
fn time_transform<T: Copy + Default>(
    make: impl Fn() -> Arc<dyn TransformExecutor<T> + Send + Sync>,
    src: &[T],
    ch: usize,
) {
    let pixels = src.len() / ch;
    let mut build_ns = Vec::new();
    for _ in 0..41 {
        let start = Instant::now();
        black_box(make());
        build_ns.push(start.elapsed().as_nanos());
    }
    let t = make();
    let mut out = vec![T::default(); pixels * 4];
    let mut run_ns = Vec::new();
    for _ in 0..41 {
        let start = Instant::now();
        t.transform(black_box(src), black_box(&mut out)).expect("run transform");
        run_ns.push(start.elapsed().as_nanos());
    }
    println!(
        "build_ns {} run_ns_per_px {:.3}",
        median(build_ns),
        median(run_ns) as f64 / pixels as f64
    );
}

fn bench(profile: &ColorProfile, depth_s: &str) {
    let cmyk = profile.color_space == DataColorSpace::Cmyk;
    let dst = ColorProfile::new_srgb();
    let opts = imageflow_opts(cmyk, BarycentricWeightScale::High);
    let (layout, ch) = src_layout(profile);
    let rgba = Layout::Rgba;
    let src: Vec<u32> = samples(65_536 * ch).collect();
    let ints =
        |bits: u32| -> Vec<u16> { src.iter().map(|&v| (v & ((1 << bits) - 1)) as u16).collect() };
    let floats: Vec<f32> = src.iter().map(|&v| (v >> 8) as f32 / 16_777_215.0).collect();
    match parse_depth(depth_s) {
        Depth::U8 => {
            let input: Vec<u8> = src.iter().map(|&v| v as u8).collect();
            let make = || profile.create_transform_8bit(layout, &dst, rgba, opts).expect("build");
            time_transform(make, &input, ch);
        }
        Depth::U10 => {
            let make = || profile.create_transform_10bit(layout, &dst, rgba, opts).expect("build");
            time_transform(make, &ints(10), ch);
        }
        Depth::U12 => {
            let make = || profile.create_transform_12bit(layout, &dst, rgba, opts).expect("build");
            time_transform(make, &ints(12), ch);
        }
        Depth::U16 => {
            let make = || profile.create_transform_16bit(layout, &dst, rgba, opts).expect("build");
            time_transform(make, &ints(16), ch);
        }
        Depth::F32 => {
            let make = || profile.create_transform_f32(layout, &dst, rgba, opts).expect("build");
            time_transform(make, &floats, ch);
        }
        Depth::F64 => {
            let input: Vec<f64> = floats.iter().map(|&v| v as f64).collect();
            let make = || profile.create_transform_f64(layout, &dst, rgba, opts).expect("build");
            time_transform(make, &input, ch);
        }
    }
}

fn diff(profile: &ColorProfile) {
    let cmyk = profile.color_space == DataColorSpace::Cmyk;
    let dst = ColorProfile::new_srgb();
    let make = |scale| {
        profile
            .create_transform_8bit(Layout::Rgba, &dst, Layout::Rgba, imageflow_opts(cmyk, scale))
            .expect("build 8-bit transform")
    };
    let (high, low) = (make(BarycentricWeightScale::High), make(BarycentricWeightScale::Low));

    let mut src = Vec::new();
    if cmyk {
        let values: Vec<u8> = (0..=255).step_by(5).collect();
        for &c in &values {
            for &m in &values {
                for &y in &values {
                    for &k in &values {
                        src.extend_from_slice(&[c, m, y, k]);
                    }
                }
            }
        }
    } else {
        for r in 0..=255u8 {
            for g in 0..=255u8 {
                for b in 0..=255u8 {
                    src.extend_from_slice(&[r, g, b, 255]);
                }
            }
        }
    }
    let mut out_high = vec![0u8; src.len()];
    let mut out_low = vec![0u8; src.len()];
    high.transform(&src, &mut out_high).expect("High transform");
    low.transform(&src, &mut out_low).expect("Low transform");

    let (mut pixels, mut differing_pixels, mut max_delta) = (0u64, 0u64, 0u8);
    let mut channels_by_delta = [0u64; 256];
    for (h, l) in out_high.as_chunks::<4>().0.iter().zip(out_low.as_chunks::<4>().0) {
        pixels += 1;
        let mut differs = false;
        for c in 0..3 {
            let d = h[c].abs_diff(l[c]);
            channels_by_delta[d as usize] += 1;
            max_delta = max_delta.max(d);
            differs |= d > 0;
        }
        differing_pixels += differs as u64;
    }
    let histogram: Vec<String> = channels_by_delta
        .iter()
        .enumerate()
        .filter(|&(d, &n)| d > 0 && n > 0)
        .map(|(d, n)| format!("delta {d}: {n}"))
        .collect();
    println!(
        "moxcms {VERSION}: 8-bit High vs Low over {pixels} {} inputs: {differing_pixels} pixels \
         differ, max channel delta {max_delta}{}{}",
        if cmyk { "CMYK" } else { "RGB" },
        if histogram.is_empty() { "" } else { "; channels with " },
        histogram.join(", ")
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match (args.get(1).map(String::as_str), args.len()) {
        (Some("info"), 3) => info(&load_profile(&args[2])),
        (Some("build"), 6 | 7) => build_on_small_stack(
            load_profile(&args[2]),
            args[3].parse().expect("stack_kib"),
            &args[4],
            &args[5],
            match args.get(6).map(String::as_str) {
                None => false,
                Some("run") => true,
                Some(other) => panic!("expected `run`, got {other}"),
            },
        ),
        (Some("diff"), 3) => diff(&load_profile(&args[2])),
        (Some("digest"), 4) => digest(&load_profile(&args[2]), &args[3]),
        (Some("bench"), 4) => bench(&load_profile(&args[2]), &args[3]),
        _ => {
            eprintln!(
                "usage: moxcms-stack-probe info|diff <profile>\n       \
                 moxcms-stack-probe build <profile> <stack_kib> <high|low> <depth> [run]\n       \
                 moxcms-stack-probe digest|bench <profile> <depth>"
            );
            std::process::exit(64);
        }
    }
}
