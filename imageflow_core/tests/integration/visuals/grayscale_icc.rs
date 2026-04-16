//! Grayscale ICC profile integration tests.
//!
//! Tests the CMS pipeline for grayscale images with various ICC profiles:
//! - sGray (sRGB tone curve on grayscale, gamma ~2.2)
//! - Gray gamma 1.8 (Mac legacy)
//! - Linear gray (gamma 1.0)
//!
//! Each test creates a 256x1 grayscale gradient PNG with an embedded ICC
//! profile, decodes through imageflow's CMS pipeline, and verifies:
//! - Output is BGRA with R == G == B (neutral grayscale preserved)
//! - Alpha is 255 everywhere
//! - Gradient is monotonically non-decreasing
//! - CMS transform is applied correctly (linear gray should differ from sRGB)
//!
//! ICC profiles are generated at runtime using moxcms::ColorProfile::new_gray_with_gamma().

use crate::common::*;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use imageflow_core::Context;
use imageflow_types::{Execute001, Framewise, Node};
use std::io::Write;

// ============================================================================
// ICC profile generation
// ============================================================================

/// Generate a grayscale ICC profile with the given gamma using moxcms.
fn build_gray_icc(gamma: f32) -> Vec<u8> {
    let profile = moxcms::ColorProfile::new_gray_with_gamma(gamma);
    profile.encode().expect("moxcms encode failed for gray profile")
}

/// The bundled Adobe sGray ICC profile (2424 bytes, gamma 2.2).
/// This is a real-world profile from Adobe that appears in many gray JPEGs.
fn adobe_sgray_icc() -> &'static [u8] {
    include_bytes!("../../../src/codecs/gray.icc")
}

// ============================================================================
// PNG fixture generation
// ============================================================================

/// Build a 256x1 grayscale PNG gradient with an embedded ICC profile.
///
/// Pixel[x] = x (0..255), giving every possible 8-bit gray value.
/// The ICC profile is embedded as an iCCP chunk.
fn build_gray_gradient_png_with_icc(icc_bytes: &[u8]) -> Vec<u8> {
    build_gray_gradient_png_impl(256, 1, Some(icc_bytes))
}

/// Build a grayscale gradient PNG, optionally with ICC profile.
fn build_gray_gradient_png_impl(w: u32, h: u32, icc: Option<&[u8]>) -> Vec<u8> {
    // Build raw grayscale pixels: gradient across width, repeated for each row
    let mut pixels = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            pixels[(y * w + x) as usize] =
                if w > 1 { ((x as u64 * 255) / (w as u64 - 1)) as u8 } else { 128 };
        }
    }

    // Build PNG manually to control chunk order and embed iCCP
    let mut buf = Vec::new();

    // PNG signature
    buf.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(0); // color type 0 = grayscale
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut buf, b"IHDR", &ihdr);

    // iCCP (before IDAT, after IHDR)
    if let Some(icc_bytes) = icc {
        let mut data = Vec::new();
        // Profile name: "gray" + null separator
        data.extend_from_slice(b"gray\0");
        // Compression method: 0 = deflate
        data.push(0);
        // Compressed ICC profile
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(icc_bytes).unwrap();
        data.extend_from_slice(&encoder.finish().unwrap());
        write_png_chunk(&mut buf, b"iCCP", &data);
    }

    // IDAT: filtered scanlines (filter byte 0 = None for each row)
    let mut raw_data = Vec::new();
    for y in 0..h as usize {
        raw_data.push(0); // filter type None
        raw_data.extend_from_slice(&pixels[y * w as usize..(y + 1) * w as usize]);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw_data).unwrap();
    let compressed = encoder.finish().unwrap();
    write_png_chunk(&mut buf, b"IDAT", &compressed);

    // IEND
    write_png_chunk(&mut buf, b"IEND", &[]);

    buf
}

/// Compute CRC32 for PNG chunks (ISO 3309 / ITU-T V.42).
fn png_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFFFFFF
}

/// Write a single PNG chunk (length + type + data + CRC).
fn write_png_chunk(buf: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
    buf.extend_from_slice(chunk_type);
    buf.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    let crc = png_crc32(&crc_data);
    buf.extend_from_slice(&crc.to_be_bytes());
}

// ============================================================================
// Decode helpers
// ============================================================================

/// Decode a PNG through imageflow, capturing the raw bitmap.
/// Returns (width, height, bgra_pixels) from the internal bitmap directly.
fn decode_to_bitmap_bgra(png_bytes: &[u8]) -> (usize, usize, Vec<u8>) {
    test_init();
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png_bytes.to_vec()).unwrap();

    let capture_id = 0;
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::CaptureBitmapKey { capture_id },
        ]),
    })
    .unwrap();

    let bitmap_key = ctx.get_captured_bitmap_key(capture_id).unwrap();
    let bitmaps = ctx.borrow_bitmaps().unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let mut window = bm.get_window_u8().unwrap();

    let w = window.w() as usize;
    let h = window.h() as usize;

    let mut pixels = Vec::with_capacity(w * h * 4);
    for scanline in window.scanlines() {
        pixels.extend_from_slice(&scanline.row()[..w * 4]);
    }
    (w, h, pixels)
}

// ============================================================================
// Verification helpers
// ============================================================================

/// Verify that output pixels are neutral grayscale (R == G == B) within tolerance.
fn assert_neutral_gray(bgra: &[u8], label: &str, max_spread: u8) {
    let mut worst_spread = 0u8;
    let mut non_gray_count = 0usize;
    let mut worst_pixel_idx = 0usize;
    for (i, px) in bgra.chunks_exact(4).enumerate() {
        let b = px[0];
        let g = px[1];
        let r = px[2];
        let spread = r.abs_diff(g).max(r.abs_diff(b)).max(g.abs_diff(b));
        if spread > max_spread {
            non_gray_count += 1;
        }
        if spread > worst_spread {
            worst_spread = spread;
            worst_pixel_idx = i;
        }
    }
    assert!(
        worst_spread <= max_spread,
        "{label}: not neutral grayscale. max channel spread={worst_spread} at pixel {worst_pixel_idx}, \
         {non_gray_count}/{} pixels exceed tolerance {max_spread}. \
         Pixel BGRA=[{},{},{},{}]",
        bgra.len() / 4,
        bgra[worst_pixel_idx * 4],
        bgra[worst_pixel_idx * 4 + 1],
        bgra[worst_pixel_idx * 4 + 2],
        bgra[worst_pixel_idx * 4 + 3],
    );
}

/// Verify all alpha values are 255.
fn assert_alpha_opaque(bgra: &[u8], label: &str) {
    for (i, px) in bgra.chunks_exact(4).enumerate() {
        assert_eq!(px[3], 255, "{label}: pixel {i} alpha={}, expected 255", px[3]);
    }
}

/// Verify the gray channel is monotonically non-decreasing across the width.
/// For a 256-wide gradient, pixel[x] should have gray >= pixel[x-1].
fn assert_monotonic_gray(bgra: &[u8], w: usize, label: &str) {
    // Check row 0 only (all rows should be identical for a gradient image)
    let mut prev_gray = 0u8;
    for x in 0..w {
        let b = bgra[x * 4]; // gray value (B channel in BGRA; R=G=B for grayscale)
        if x > 0 {
            assert!(
                b >= prev_gray,
                "{label}: monotonicity violation at x={x}: gray={b} < prev={prev_gray}"
            );
        }
        prev_gray = b;
    }
}

/// Compute max channel delta between two BGRA buffers (ignoring alpha).
fn max_rgb_delta(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    let mut max_d = 0u8;
    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        for ch in 0..3 {
            let d = pa[ch].abs_diff(pb[ch]);
            if d > max_d {
                max_d = d;
            }
        }
    }
    max_d
}

// ============================================================================
// Tests: moxcms-generated gray gamma profiles
// ============================================================================

/// Gray gamma 2.2 (sGray equivalent): should produce a near-identity transform
/// since sGray ≈ sRGB tone curve on grayscale.
#[test]
fn test_gray_icc_gamma22_identity() {
    test_init();
    let icc = build_gray_icc(2.2);
    let png = build_gray_gradient_png_with_icc(&icc);
    let (w, _h, bgra) = decode_to_bitmap_bgra(&png);

    assert_neutral_gray(&bgra, "gray_gamma22", 1);
    assert_alpha_opaque(&bgra, "gray_gamma22");
    assert_monotonic_gray(&bgra, w, "gray_gamma22");

    // sGray (gamma 2.2) → sRGB should be nearly identity.
    // The pure gamma 2.2 curve differs slightly from sRGB's piece-wise curve
    // (linear toe segment), so small deltas are expected in the dark values.
    let no_icc_png = build_gray_gradient_png_impl(256, 1, None);
    let (_w2, _h2, no_icc_bgra) = decode_to_bitmap_bgra(&no_icc_png);
    let delta = max_rgb_delta(&bgra, &no_icc_bgra);
    // Pure gamma 2.2 vs sRGB TRC: the piece-wise sRGB transfer function has a
    // linear segment below ~0.0031308 and transitions to a power curve above it.
    // A pure 2.2 power law diverges most in the very dark values (pixels 1-10)
    // where sRGB's linear toe segment compresses the curve. Max delta ~9 at pixel 1.
    assert!(
        delta <= 12,
        "gray_gamma22: delta vs no-ICC output should be moderate (got {delta}), \
         gamma 2.2 ≈ sRGB (diverges in dark tones due to linear toe)"
    );
    eprintln!("gray_gamma22: max delta vs no-ICC = {delta} (expected ≤12)");
}

/// Gray gamma 1.8 (Mac legacy): visible transform, darker mid-tones.
#[test]
fn test_gray_icc_gamma18() {
    test_init();
    let icc = build_gray_icc(1.8);
    let png = build_gray_gradient_png_with_icc(&icc);
    let (w, _h, bgra) = decode_to_bitmap_bgra(&png);

    assert_neutral_gray(&bgra, "gray_gamma18", 1);
    assert_alpha_opaque(&bgra, "gray_gamma18");
    assert_monotonic_gray(&bgra, w, "gray_gamma18");

    // Gamma 1.8 → sRGB should produce a visible transform
    let no_icc_png = build_gray_gradient_png_impl(256, 1, None);
    let (_w2, _h2, no_icc_bgra) = decode_to_bitmap_bgra(&no_icc_png);
    let delta = max_rgb_delta(&bgra, &no_icc_bgra);
    assert!(
        delta >= 3,
        "gray_gamma18: expected visible CMS transform (delta={delta}), \
         gamma 1.8 should differ from sRGB"
    );
    eprintln!("gray_gamma18: max delta vs no-ICC = {delta} (expected ≥3)");
}

/// Linear gray (gamma 1.0): significant transform, much brighter mid-tones.
#[test]
fn test_gray_icc_linear() {
    test_init();
    let icc = build_gray_icc(1.0);
    let png = build_gray_gradient_png_with_icc(&icc);
    let (w, _h, bgra) = decode_to_bitmap_bgra(&png);

    assert_neutral_gray(&bgra, "gray_linear", 1);
    assert_alpha_opaque(&bgra, "gray_linear");
    assert_monotonic_gray(&bgra, w, "gray_linear");

    // Linear → sRGB should produce large deltas in mid-tones
    // (linear 128/255 ≈ 0.5 → sRGB ≈ 188)
    let no_icc_png = build_gray_gradient_png_impl(256, 1, None);
    let (_w2, _h2, no_icc_bgra) = decode_to_bitmap_bgra(&no_icc_png);
    let delta = max_rgb_delta(&bgra, &no_icc_bgra);
    assert!(
        delta >= 30,
        "gray_linear: expected large CMS transform delta (got {delta}), \
         linear gray → sRGB should produce ~60 level shift at mid-gray"
    );
    eprintln!("gray_linear: max delta vs no-ICC = {delta} (expected ≥30)");

    // Verify mid-gray is brighter (linear 128 → sRGB ~188)
    // Pixel index 128 in the 256-wide gradient
    let mid_b = bgra[128 * 4]; // B channel (=G=R for grayscale)
    assert!(
        mid_b > 160,
        "gray_linear: mid-gray pixel should be bright after linear→sRGB (got {mid_b}, expected >160)"
    );
}

// ============================================================================
// Tests: Adobe sGray ICC profile (real-world profile)
// ============================================================================

/// Test with the bundled Adobe sGray ICC profile (same as imageflow's gray.icc).
/// This is the profile most commonly found in grayscale JPEGs from Photoshop.
#[test]
fn test_gray_icc_adobe_sgray() {
    test_init();
    let icc = adobe_sgray_icc();
    let png = build_gray_gradient_png_with_icc(icc);
    let (w, _h, bgra) = decode_to_bitmap_bgra(&png);

    assert_neutral_gray(&bgra, "adobe_sgray", 1);
    assert_alpha_opaque(&bgra, "adobe_sgray");
    assert_monotonic_gray(&bgra, w, "adobe_sgray");

    // Adobe sGray uses the same gamma 2.2 curve as sRGB, so the transform
    // should be near-identity (within ~5 levels of no-ICC decode)
    let no_icc_png = build_gray_gradient_png_impl(256, 1, None);
    let (_w2, _h2, no_icc_bgra) = decode_to_bitmap_bgra(&no_icc_png);
    let delta = max_rgb_delta(&bgra, &no_icc_bgra);
    eprintln!("adobe_sgray: max delta vs no-ICC = {delta}");
    // The Adobe sGray profile may have slightly different TRC than pure 2.2
    assert!(delta <= 10, "adobe_sgray: delta vs no-ICC too large (got {delta}), sGray ≈ sRGB");
}

// ============================================================================
// Tests: JPEG grayscale with ICC (via S3 test corpus)
// ============================================================================

/// Grayscale JPEG with gamma 2.2 ICC from the Flickr corpus.
/// This is a real-world grayscale JPEG that triggered gray ICC handling.
#[test]
fn test_gray_jpeg_icc_corpus_gamma22() {
    test_init();
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/wide-gamut/gray-gamma-22/flickr_2f4bbf638f18ebea.jpg";
    let jpg_bytes = get_url_bytes_with_retry(url).unwrap();

    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, jpg_bytes).unwrap();
    let capture_id = 0;
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::CaptureBitmapKey { capture_id },
        ]),
    })
    .unwrap();

    let bitmap_key = ctx.get_captured_bitmap_key(capture_id).unwrap();
    let bitmaps = ctx.borrow_bitmaps().unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let mut window = bm.get_window_u8().unwrap();

    let w = window.w() as usize;
    let h = window.h() as usize;

    // Verify it decoded to a reasonable size
    assert!(w > 0 && h > 0, "decoded to {w}x{h}");

    // Verify output is BGRA with neutral grayscale (real photos may have slight chroma)
    let mut pixels = Vec::with_capacity(w * h * 4);
    for scanline in window.scanlines() {
        pixels.extend_from_slice(&scanline.row()[..w * 4]);
    }
    // Real grayscale photos should have R≈G≈B within tight tolerance
    assert_neutral_gray(&pixels, "corpus_gamma22", 1);
    assert_alpha_opaque(&pixels, "corpus_gamma22");
    eprintln!("corpus_gamma22: {w}x{h} decoded OK, neutral grayscale verified");
}

// ============================================================================
// Tests: Grayscale without ICC (control group)
// ============================================================================

/// Grayscale PNG without any ICC profile should produce neutral BGRA.
/// This is the control case — no CMS transform should occur.
#[test]
fn test_gray_no_icc_baseline() {
    test_init();
    let png = build_gray_gradient_png_impl(256, 1, None);
    let (w, _h, bgra) = decode_to_bitmap_bgra(&png);

    assert_neutral_gray(&bgra, "no_icc_baseline", 0); // exact neutral
    assert_alpha_opaque(&bgra, "no_icc_baseline");
    assert_monotonic_gray(&bgra, w, "no_icc_baseline");

    // Verify exact identity: pixel[x] should have gray value = x
    for x in 0..256usize {
        let b = bgra[x * 4];
        assert_eq!(b, x as u8, "no_icc_baseline: pixel[{x}] gray={b}, expected {x}");
    }
}

// ============================================================================
// Tests: Larger images (verify stride handling)
// ============================================================================

/// 256x16 grayscale with linear ICC — tests multi-row stride handling.
#[test]
fn test_gray_icc_linear_multirow() {
    test_init();
    let icc = build_gray_icc(1.0);
    let png = build_gray_gradient_png_impl(256, 16, Some(&icc));
    let (w, h, bgra) = decode_to_bitmap_bgra(&png);

    assert_eq!(w, 256);
    assert_eq!(h, 16);
    assert_neutral_gray(&bgra, "linear_multirow", 1);
    assert_alpha_opaque(&bgra, "linear_multirow");

    // All rows should be identical (same gradient repeated)
    let row0 = &bgra[..w * 4];
    for y in 1..h {
        let row = &bgra[y * w * 4..(y + 1) * w * 4];
        assert_eq!(row, row0, "row {y} differs from row 0");
    }
}

// ============================================================================
// Tests: Cross-profile comparison
// ============================================================================

/// Verify that different gray ICC profiles produce different output.
/// This catches regressions where the CMS ignores the profile entirely.
#[test]
fn test_gray_icc_profiles_differ() {
    test_init();

    let gamma_22_icc = build_gray_icc(2.2);
    let gamma_18_icc = build_gray_icc(1.8);
    let linear_icc = build_gray_icc(1.0);

    let png_22 = build_gray_gradient_png_with_icc(&gamma_22_icc);
    let png_18 = build_gray_gradient_png_with_icc(&gamma_18_icc);
    let png_linear = build_gray_gradient_png_with_icc(&linear_icc);

    let (_w, _h, bgra_22) = decode_to_bitmap_bgra(&png_22);
    let (_w, _h, bgra_18) = decode_to_bitmap_bgra(&png_18);
    let (_w, _h, bgra_linear) = decode_to_bitmap_bgra(&png_linear);

    // gamma 2.2 vs gamma 1.8: should differ
    let delta_22_vs_18 = max_rgb_delta(&bgra_22, &bgra_18);
    assert!(delta_22_vs_18 >= 2, "gamma 2.2 vs 1.8 should differ (delta={delta_22_vs_18})");

    // gamma 2.2 vs linear: should differ significantly
    let delta_22_vs_linear = max_rgb_delta(&bgra_22, &bgra_linear);
    assert!(
        delta_22_vs_linear >= 20,
        "gamma 2.2 vs linear should differ significantly (delta={delta_22_vs_linear})"
    );

    // gamma 1.8 vs linear: should differ
    let delta_18_vs_linear = max_rgb_delta(&bgra_18, &bgra_linear);
    assert!(
        delta_18_vs_linear >= 15,
        "gamma 1.8 vs linear should differ (delta={delta_18_vs_linear})"
    );

    eprintln!(
        "Profile deltas: 2.2/1.8={delta_22_vs_18}, 2.2/linear={delta_22_vs_linear}, 1.8/linear={delta_18_vs_linear}"
    );
}

// ============================================================================
// Real-world ICC profile fixtures (JPEG, from zenpixels-icc corpus)
// ============================================================================

/// Directory containing CC0-licensed 256x1 grayscale JPEG gradients.
/// Copyrighted ICC profile fixtures are on S3 and downloaded on demand.
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/grayscale_icc/");

/// S3 base URL for copyrighted grayscale ICC test fixtures.
const S3_FIXTURE_URL: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/grayscale_icc";

/// Load a JPEG fixture by name (without .jpg extension).
/// Tries local file first, falls back to S3 download with caching.
fn load_fixture(name: &str) -> Vec<u8> {
    let local_path = format!("{}{}.jpg", FIXTURE_DIR, name);
    if let Ok(data) = std::fs::read(&local_path) {
        return data;
    }
    // Not in git (copyrighted ICC profile) — fetch from S3
    let url = format!("{}/{}.jpg", S3_FIXTURE_URL, name);
    crate::common::get_url_bytes_with_retry(&url)
        .unwrap_or_else(|e| panic!("Fixture {name} not local and S3 fetch failed: {e}"))
}

/// Decode a JPEG fixture and return BGRA pixels.
fn decode_fixture(name: &str) -> (usize, usize, Vec<u8>) {
    let bytes = load_fixture(name);
    decode_to_bitmap_bgra(&bytes)
}

/// Full validation suite for a grayscale ICC JPEG fixture.
///
/// Checks: dimensions, neutral gray, opaque alpha, monotonic gradient,
/// and delta vs no-ICC baseline.
struct GrayIccFixtureTest {
    name: &'static str,
    /// Maximum allowed channel spread (R vs G vs B) per pixel.
    max_channel_spread: u8,
    /// Expected delta range vs no-ICC baseline [min, max].
    /// If None, no delta check is performed.
    delta_vs_baseline: Option<(u8, u8)>,
    /// Expected mid-gray (pixel 128) brightness range [min, max].
    /// Only checked if Some.
    mid_gray_range: Option<(u8, u8)>,
    /// If true, expect decode to fail (unsupported profile). The test verifies
    /// a graceful error message rather than pixel correctness.
    expect_unsupported: bool,
}

impl GrayIccFixtureTest {
    fn run(&self) {
        test_init();

        if self.expect_unsupported {
            // Verify this profile produces a graceful error, not a panic or corruption
            let bytes = load_fixture(self.name);
            let mut ctx = Context::create().unwrap();
            ctx.add_input_vector(0, bytes).unwrap();
            let result = ctx.execute_1(Execute001 {
                job_options: None,
                graph_recording: default_graph_recording(false),
                security: None,
                framewise: Framewise::Steps(vec![
                    Node::Decode { io_id: 0, commands: None },
                    Node::CaptureBitmapKey { capture_id: 0 },
                ]),
            });
            assert!(
                result.is_err(),
                "{}: expected decode to fail for unsupported ICC profile, but it succeeded",
                self.name
            );
            let err_msg = format!("{:?}", result.unwrap_err());
            eprintln!(
                "{}: correctly rejected unsupported profile: {}",
                self.name,
                &err_msg[..err_msg.len().min(120)]
            );
            return;
        }

        let (w, _h, bgra) = decode_fixture(self.name);

        // JPEG may round 256x1 up to 256x8 (MCU alignment), so just check width
        assert_eq!(w, 256, "{}: expected 256px wide, got {w}", self.name);

        // Use first row only (all rows should be identical for gradient)
        let row0 = &bgra[..w * 4];

        assert_neutral_gray(row0, self.name, self.max_channel_spread);
        assert_alpha_opaque(row0, self.name);
        assert_monotonic_gray(row0, w, self.name);

        if let Some((min_delta, max_delta)) = self.delta_vs_baseline {
            let baseline = load_fixture("no_icc_baseline");
            let (_bw, _bh, baseline_bgra) = decode_to_bitmap_bgra(&baseline);
            let baseline_row0 = &baseline_bgra[..256 * 4];
            let delta = max_rgb_delta(row0, baseline_row0);
            assert!(
                delta >= min_delta,
                "{}: delta vs baseline too small: {delta} < {min_delta}",
                self.name
            );
            assert!(
                delta <= max_delta,
                "{}: delta vs baseline too large: {delta} > {max_delta}",
                self.name
            );
            eprintln!(
                "{}: max delta vs no-ICC baseline = {delta} (expected {min_delta}..{max_delta})",
                self.name
            );
        }

        if let Some((min_val, max_val)) = self.mid_gray_range {
            let mid_b = row0[128 * 4]; // B channel at pixel 128
            assert!(
                mid_b >= min_val && mid_b <= max_val,
                "{}: mid-gray pixel 128 = {mid_b}, expected {min_val}..{max_val}",
                self.name
            );
            eprintln!("{}: mid-gray pixel 128 = {mid_b}", self.name);
        }
    }
}

// --- Gamma 1.8 profiles (the deferred ones — most important to validate) ---

#[test]
fn test_fixture_gray_gamma_1_8() {
    // Gamma 1.8 → sRGB: 128/255 in gamma 1.8 = 0.502^1.8 ≈ 0.268 linear
    // → sRGB OETF ≈ 0.558 → pixel ≈ 142. Mid-gray gets brighter because
    // gamma 1.8 is less aggressive than sRGB's ~2.2 effective gamma.
    GrayIccFixtureTest {
        name: "gray_gamma_1_8",
        max_channel_spread: 1,
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_epson_gray_gamma_1_8() {
    GrayIccFixtureTest {
        name: "epson_gray_gamma_1_8",
        max_channel_spread: 1,
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_generic_gray_profile_macos() {
    GrayIccFixtureTest {
        name: "generic_gray_profile_macOS",
        max_channel_spread: 1,
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_generic_gray_profile_0faa32c0() {
    GrayIccFixtureTest {
        name: "generic_gray_profile_0faa32c0",
        max_channel_spread: 1,
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_generic_gray_profile_10233312() {
    GrayIccFixtureTest {
        name: "generic_gray_profile_10233312",
        max_channel_spread: 1,
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_generic_gray_profile_94722fe2() {
    GrayIccFixtureTest {
        name: "generic_gray_profile_94722fe2",
        max_channel_spread: 1,
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_sgray_artifex_gamma_1_8() {
    GrayIccFixtureTest {
        name: "sgray_artifex_gamma_1_8",
        max_channel_spread: 1,
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_windows_blackwhite() {
    GrayIccFixtureTest {
        name: "windows_blackwhite",
        max_channel_spread: 1,
        // para(g=1.8) — same gamma 1.8 transform
        delta_vs_baseline: Some((3, 30)),
        mid_gray_range: Some((140, 155)),
        expect_unsupported: false,
    }
    .run();
}

// --- Gamma 2.2 profiles (near-identity with sRGB) ---

#[test]
fn test_fixture_gray_gamma_2_2_simple() {
    GrayIccFixtureTest {
        name: "gray_gamma_2_2_simple",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 12)),
        mid_gray_range: Some((120, 140)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_gray_gamma_2_2_curv1024() {
    GrayIccFixtureTest {
        name: "gray_gamma_2_2_curv1024",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 12)),
        mid_gray_range: Some((120, 140)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_gray_gamma_2_2_curv1024_v2() {
    GrayIccFixtureTest {
        name: "gray_gamma_2_2_curv1024_v2",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 12)),
        mid_gray_range: Some((120, 140)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_gray_gamma_2_2_expanded() {
    GrayIccFixtureTest {
        name: "gray_gamma_2_2_expanded",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 12)),
        mid_gray_range: Some((120, 140)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_calibrated_gray() {
    GrayIccFixtureTest {
        name: "calibrated_gray",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 12)),
        mid_gray_range: Some((120, 140)),
        expect_unsupported: false,
    }
    .run();
}

// --- sRGB TRC on gray (should be near-identity) ---

#[test]
fn test_fixture_with_srgb_trc() {
    GrayIccFixtureTest {
        name: "with_srgb_trc",
        max_channel_spread: 1,
        // sRGB TRC on gray → sRGB output: should be identity or very close
        delta_vs_baseline: Some((0, 5)),
        mid_gray_range: Some((125, 132)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_sgrey_v4() {
    GrayIccFixtureTest {
        name: "sgrey_v4",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 5)),
        mid_gray_range: Some((125, 132)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_sgrey_v2_nano() {
    GrayIccFixtureTest {
        name: "sgrey_v2_nano",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 5)),
        mid_gray_range: Some((125, 132)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_sgrey_v2_micro() {
    GrayIccFixtureTest {
        name: "sgrey_v2_micro",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 5)),
        mid_gray_range: Some((125, 132)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_sgrey_v2_magic() {
    GrayIccFixtureTest {
        name: "sgrey_v2_magic",
        max_channel_spread: 1,
        delta_vs_baseline: Some((0, 5)),
        mid_gray_range: Some((125, 132)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_default_gray_artifex() {
    GrayIccFixtureTest {
        name: "default_gray_artifex",
        max_channel_spread: 1,
        // Artifex default_gray has a 1024-point sRGB-like curve
        delta_vs_baseline: Some((0, 5)),
        mid_gray_range: Some((125, 132)),
        expect_unsupported: false,
    }
    .run();
}

// --- Linear (gamma 1.0) profiles ---

#[test]
fn test_fixture_ps_gray_linear() {
    GrayIccFixtureTest {
        name: "ps_gray_linear",
        max_channel_spread: 1,
        // Linear → sRGB: large transform, mid-gray brightens significantly
        delta_vs_baseline: Some((30, 80)),
        mid_gray_range: Some((170, 200)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_gray_linear() {
    GrayIccFixtureTest {
        name: "gray_linear",
        max_channel_spread: 1,
        delta_vs_baseline: Some((30, 80)),
        mid_gray_range: Some((170, 200)),
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_gray_cie_l() {
    // Gray CIE*L uses Lab PCS instead of XYZ PCS. moxcms does not support
    // Lab-PCS grayscale profiles, so this profile is rejected during decode.
    // This is a known limitation — CIE L* is a perceptual lightness scale
    // that requires Lab↔XYZ conversion moxcms hasn't implemented for gray.
    GrayIccFixtureTest {
        name: "gray_cie_l",
        max_channel_spread: 0,
        delta_vs_baseline: None,
        mid_gray_range: None,
        expect_unsupported: true,
    }
    .run();
}

// --- Dot Gain profiles ---

#[test]
fn test_fixture_dot_gain_10() {
    GrayIccFixtureTest {
        name: "dot_gain_10",
        max_channel_spread: 1,
        // Dot gain compensates for ink spread in printing. Even 10% dot gain
        // produces significant lightening of darks — the curv lookup table
        // maps encoded values to lighter linear values to counteract gain.
        delta_vs_baseline: Some((1, 55)),
        mid_gray_range: None,
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_dot_gain_15() {
    GrayIccFixtureTest {
        name: "dot_gain_15",
        max_channel_spread: 1,
        delta_vs_baseline: Some((1, 60)),
        mid_gray_range: None,
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_dot_gain_20() {
    GrayIccFixtureTest {
        name: "dot_gain_20",
        max_channel_spread: 1,
        delta_vs_baseline: Some((1, 70)),
        mid_gray_range: None,
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_dot_gain_20_variant() {
    GrayIccFixtureTest {
        name: "dot_gain_20_variant",
        max_channel_spread: 1,
        delta_vs_baseline: Some((1, 70)),
        mid_gray_range: None,
        expect_unsupported: false,
    }
    .run();
}

#[test]
fn test_fixture_dot_gain_20_curv256() {
    GrayIccFixtureTest {
        name: "dot_gain_20_curv256",
        max_channel_spread: 1,
        delta_vs_baseline: Some((1, 70)),
        mid_gray_range: None,
        expect_unsupported: false,
    }
    .run();
}

// --- Newspaper profiles ---

#[test]
fn test_fixture_iso_newspaper26v4() {
    // ISOnewspaper26v4_gr uses Lab PCS and 'prtr' (printer/output) device class.
    // moxcms does not support Lab-PCS grayscale profiles. These are printing
    // industry profiles designed for newsprint dot gain compensation.
    GrayIccFixtureTest {
        name: "iso_newspaper26v4",
        max_channel_spread: 0,
        delta_vs_baseline: None,
        mid_gray_range: None,
        expect_unsupported: true,
    }
    .run();
}

#[test]
fn test_fixture_wan_ifra_newspaper26v5() {
    // WAN-IFRAnewspaper26v5_gr — same Lab PCS issue as ISOnewspaper26v4.
    GrayIccFixtureTest {
        name: "wan_ifra_newspaper26v5",
        max_channel_spread: 0,
        delta_vs_baseline: None,
        mid_gray_range: None,
        expect_unsupported: true,
    }
    .run();
}

// --- No-ICC baseline JPEG ---

#[test]
fn test_fixture_no_icc_baseline_jpeg() {
    test_init();
    let (w, _h, bgra) = decode_fixture("no_icc_baseline");
    assert_eq!(w, 256);
    let row0 = &bgra[..w * 4];
    // JPEG Q100 should be near-lossless for gradients
    assert_neutral_gray(row0, "no_icc_baseline_jpeg", 1);
    assert_alpha_opaque(row0, "no_icc_baseline_jpeg");
    assert_monotonic_gray(row0, w, "no_icc_baseline_jpeg");

    // Verify JPEG Q100 is close to identity (allow ±1 for JPEG rounding)
    for x in 0..256usize {
        let b = row0[x * 4];
        let expected = x as u8;
        assert!(
            b.abs_diff(expected) <= 1,
            "no_icc_baseline_jpeg: pixel[{x}] gray={b}, expected ~{expected} (JPEG rounding)"
        );
    }
}

// --- Cross-profile consistency: all gamma 1.8 fixtures should agree ---

#[test]
fn test_fixture_gamma_1_8_cross_consistency() {
    test_init();
    let gamma_1_8_fixtures = [
        "gray_gamma_1_8",
        "epson_gray_gamma_1_8",
        "generic_gray_profile_macOS",
        "generic_gray_profile_0faa32c0",
        "generic_gray_profile_10233312",
        "generic_gray_profile_94722fe2",
        "sgray_artifex_gamma_1_8",
        "windows_blackwhite",
    ];

    // Decode all gamma 1.8 fixtures
    let decoded: Vec<(String, Vec<u8>)> = gamma_1_8_fixtures
        .iter()
        .map(|name| {
            let (_w, _h, bgra) = decode_fixture(name);
            let row0 = bgra[..256 * 4].to_vec();
            (name.to_string(), row0)
        })
        .collect();

    // All gamma 1.8 profiles should produce similar output (within tolerance
    // for different TRC representations: simple gamma vs para vs curv table)
    let reference = &decoded[0].1;
    for (name, row) in &decoded[1..] {
        let delta = max_rgb_delta(reference, row);
        assert!(
            delta <= 3,
            "gamma 1.8 cross-check: {} vs {} delta={delta} (expected ≤3, \
             all gamma 1.8 profiles should produce nearly identical output)",
            decoded[0].0,
            name
        );
    }
    eprintln!(
        "gamma 1.8 cross-consistency: all {} profiles agree within tolerance",
        gamma_1_8_fixtures.len()
    );
}

// --- Cross-profile consistency: gamma 1.8 vs gamma 2.2 should differ ---

#[test]
fn test_fixture_gamma_1_8_vs_2_2_differ() {
    test_init();
    let (_w, _h, bgra_18) = decode_fixture("gray_gamma_1_8");
    let (_w, _h, bgra_22) = decode_fixture("gray_gamma_2_2_simple");

    let row_18 = &bgra_18[..256 * 4];
    let row_22 = &bgra_22[..256 * 4];
    let delta = max_rgb_delta(row_18, row_22);

    assert!(delta >= 3, "gamma 1.8 vs 2.2 should differ visibly (delta={delta})");
    eprintln!("gamma 1.8 vs 2.2: max delta = {delta}");
}

// --- Verify dot gain profiles differ from gamma profiles ---

#[test]
fn test_fixture_dot_gain_vs_gamma_differ() {
    test_init();
    let (_w, _h, bgra_dg) = decode_fixture("dot_gain_20");
    let (_w, _h, bgra_g22) = decode_fixture("gray_gamma_2_2_simple");

    let row_dg = &bgra_dg[..256 * 4];
    let row_g22 = &bgra_g22[..256 * 4];
    let delta = max_rgb_delta(row_dg, row_g22);

    assert!(delta >= 5, "dot gain 20% vs gamma 2.2 should differ (delta={delta})");
    eprintln!("dot_gain_20 vs gamma_2.2: max delta = {delta}");
}

// ============================================================================
// Tests: Grayscale JPEG encode→decode roundtrip with ICC
// ============================================================================

/// Encode a grayscale-with-ICC PNG to JPEG and back, verify grayscale preserved.
#[test]
fn test_gray_icc_jpeg_roundtrip() {
    test_init();
    let icc = build_gray_icc(2.2);
    let png = build_gray_gradient_png_impl(64, 64, Some(&icc));

    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![Node::CommandString {
            kind: imageflow_types::CommandStringKind::ImageResizer4,
            value: "format=jpg&quality=95".to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }]),
    })
    .unwrap();
    let jpg = ctx.take_output_buffer(1).unwrap();
    assert!(jpg.starts_with(b"\xFF\xD8\xFF"), "output should be JPEG");

    // Decode JPEG back and verify grayscale
    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, jpg).unwrap();
    let capture_id = 0;
    ctx2.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::CaptureBitmapKey { capture_id },
        ]),
    })
    .unwrap();

    let bitmap_key = ctx2.get_captured_bitmap_key(capture_id).unwrap();
    let bitmaps = ctx2.borrow_bitmaps().unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let mut window = bm.get_window_u8().unwrap();

    let w = window.w() as usize;
    let h = window.h() as usize;
    let mut pixels = Vec::with_capacity(w * h * 4);
    for scanline in window.scanlines() {
        pixels.extend_from_slice(&scanline.row()[..w * 4]);
    }

    // JPEG re-encode may introduce slight chroma due to YCbCr conversion
    assert_neutral_gray(&pixels, "gray_icc_jpeg_roundtrip", 3);
    assert_alpha_opaque(&pixels, "gray_icc_jpeg_roundtrip");
}
