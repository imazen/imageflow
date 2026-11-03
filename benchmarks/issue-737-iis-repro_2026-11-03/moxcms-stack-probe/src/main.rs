//! Measures how much thread stack moxcms needs to build the transforms imageflow asks it for
//! (imageflow#737), and how much `BarycentricWeightScale::High` changes 8-bit output.
//!
//! ```text
//! moxcms-stack-probe info  <profile.icc | image.jpg>
//! moxcms-stack-probe build <profile.icc | image.jpg> <stack_kib> <high|low> <8|16>
//! moxcms-stack-probe diff  <profile.icc | image.jpg>
//! ```
//!
//! `build` parses the profile on the main thread, then builds the transform to sRGB on a thread
//! with `stack_kib` KiB of stack and prints `ok`. A stack overflow aborts the whole process, so
//! run.sh runs one size per process. `diff` builds 8-bit High and Low transforms and compares
//! their output on every RGB input, or on every 5th value of each CMYK channel.

#[cfg(not(any(feature = "v0_8_1", feature = "v0_9_1", feature = "master")))]
compile_error!("enable one of the features v0_8_1, v0_9_1, master");

#[cfg(feature = "v0_8_1")]
use moxcms_0_8_1 as moxcms;
#[cfg(feature = "v0_9_1")]
use moxcms_0_9_1 as moxcms;
#[cfg(feature = "master")]
use moxcms_master as moxcms;

use moxcms::{
    BarycentricWeightScale, ColorProfile, DataColorSpace, InterpolationMethod, Layout,
    RenderingIntent, TransformOptions,
};

const VERSION: &str = if cfg!(feature = "v0_8_1") {
    "0.8.1"
} else if cfg!(feature = "v0_9_1") {
    "0.9.1"
} else {
    "master@097dccf3a9"
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

/// The `create_transform_8bit` call imageflow makes for CMYK and LUT-based RGB profiles
/// (RGBA or CMYK in, RGBA out), or its 16-bit counterpart.
fn build(profile: &ColorProfile, bits: u32, scale: BarycentricWeightScale) -> Result<(), String> {
    let cmyk = profile.color_space == DataColorSpace::Cmyk;
    let dst = ColorProfile::new_srgb();
    let opts = imageflow_opts(cmyk, scale);
    let result = match bits {
        8 => profile.create_transform_8bit(Layout::Rgba, &dst, Layout::Rgba, opts).map(|_| ()),
        16 => profile.create_transform_16bit(Layout::Rgba, &dst, Layout::Rgba, opts).map(|_| ()),
        _ => panic!("bits must be 8 or 16"),
    };
    result.map_err(|e| format!("{e:?}"))
}

fn load_profile(path: &str) -> ColorProfile {
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

fn build_on_small_stack(profile: ColorProfile, stack_kib: usize, scale: &str, bits: u32) {
    let scale = parse_scale(scale);
    let handle = std::thread::Builder::new()
        .name(format!("build-{stack_kib}k"))
        .stack_size(stack_kib * 1024)
        .spawn(move || build(&profile, bits, scale))
        .expect("spawn build thread");
    match handle.join().expect("build thread panicked") {
        Ok(()) => println!("ok"),
        Err(e) => {
            println!("error {e}");
            std::process::exit(2);
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
        (Some("build"), 6) => build_on_small_stack(
            load_profile(&args[2]),
            args[3].parse().expect("stack_kib"),
            &args[4],
            args[5].parse().expect("bits"),
        ),
        (Some("diff"), 3) => diff(&load_profile(&args[2])),
        _ => {
            eprintln!(
                "usage: moxcms-stack-probe info|diff <profile.icc|image.jpg>\n       \
                 moxcms-stack-probe build <profile.icc|image.jpg> <stack_kib> <high|low> <8|16>"
            );
            std::process::exit(64);
        }
    }
}
