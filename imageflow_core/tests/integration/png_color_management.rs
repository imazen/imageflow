//! PNG color management code path tests.
//!
//! Validates chunk combination handling, precedence rules, degenerate input
//! rejection, and decoder parity between LibPng and ImagePng backends.
//!
//! Uses tiny (4x4) synthetic PNGs built programmatically — no fixtures, no S3,
//! no checksums. Each test decodes through both backends and compares results.

use imageflow_core::{Context, NamedDecoders};
use imageflow_types as s;

// ---------------------------------------------------------------------------
// PNG builder
// ---------------------------------------------------------------------------

enum PngColorChunk {
    Gama(f64),
    Chrm {
        white: (f64, f64),
        red: (f64, f64),
        green: (f64, f64),
        blue: (f64, f64),
    },
    Srgb(u8),
    /// cICP chunk: (color_primaries, transfer_function, matrix_coefficients, full_range_flag)
    Cicp(u8, u8, u8, u8),
    /// iCCP chunk: profile name + raw (uncompressed) ICC profile bytes.
    /// The builder will zlib-compress the profile data per PNG spec.
    Iccp(Vec<u8>),
}

/// sRGB primaries for use with `PngColorChunk::Chrm`.
fn srgb_chrm() -> PngColorChunk {
    PngColorChunk::Chrm {
        white: (0.3127, 0.3290),
        red: (0.64, 0.33),
        green: (0.30, 0.60),
        blue: (0.15, 0.06),
    }
}

/// Build a minimal valid RGBA PNG with optional color-management chunks
/// inserted between IHDR and IDAT (per PNG spec ordering requirements).
fn build_test_png(w: u32, h: u32, pixels: &[u8], chunks: &[PngColorChunk]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    assert_eq!(pixels.len(), (w * h * 4) as usize);

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

    fn write_chunk(buf: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
        buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
        buf.extend_from_slice(chunk_type);
        buf.extend_from_slice(data);
        let mut crc_data = Vec::with_capacity(4 + data.len());
        crc_data.extend_from_slice(chunk_type);
        crc_data.extend_from_slice(data);
        let crc = png_crc32(&crc_data);
        buf.extend_from_slice(&crc.to_be_bytes());
    }

    let mut buf = Vec::new();
    // PNG signature
    buf.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR: width, height, bit_depth=8, color_type=6 (RGBA)
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_chunk(&mut buf, b"IHDR", &ihdr);

    // Color chunks — must come between IHDR and IDAT
    for chunk in chunks {
        match chunk {
            PngColorChunk::Gama(gamma) => {
                let val = (*gamma * 100_000.0) as u32;
                write_chunk(&mut buf, b"gAMA", &val.to_be_bytes());
            }
            PngColorChunk::Chrm { white, red, green, blue } => {
                let mut data = Vec::with_capacity(32);
                for &v in &[white.0, white.1, red.0, red.1, green.0, green.1, blue.0, blue.1] {
                    data.extend_from_slice(&((v * 100_000.0) as u32).to_be_bytes());
                }
                write_chunk(&mut buf, b"cHRM", &data);
            }
            PngColorChunk::Srgb(intent) => {
                write_chunk(&mut buf, b"sRGB", &[*intent]);
            }
            PngColorChunk::Cicp(cp, tc, mc, fr) => {
                write_chunk(&mut buf, b"cICP", &[*cp, *tc, *mc, *fr]);
            }
            PngColorChunk::Iccp(icc_bytes) => {
                let mut data = Vec::new();
                // Profile name: "test" + null separator
                data.extend_from_slice(b"test\0");
                // Compression method: 0 = deflate
                data.push(0);
                // Compressed ICC profile
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(icc_bytes).unwrap();
                data.extend_from_slice(&encoder.finish().unwrap());
                write_chunk(&mut buf, b"iCCP", &data);
            }
        }
    }

    // IDAT: filtered scanlines (filter byte 0 = None for each row)
    let mut raw_scanlines = Vec::new();
    for y in 0..h as usize {
        raw_scanlines.push(0u8); // filter byte: None
        raw_scanlines.extend_from_slice(&pixels[y * w as usize * 4..(y + 1) * w as usize * 4]);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw_scanlines).unwrap();
    let compressed = encoder.finish().unwrap();
    write_chunk(&mut buf, b"IDAT", &compressed);

    // IEND
    write_chunk(&mut buf, b"IEND", &[]);

    buf
}

// ---------------------------------------------------------------------------
// Decode + compare helpers
// ---------------------------------------------------------------------------

/// Decode a PNG through imageflow with a specific decoder and optional decoder
/// commands, re-encode as PNG32, and return raw RGBA pixels extracted via lodepng.
fn decode_to_rgba_with_commands(
    png_bytes: &[u8],
    decoder: NamedDecoders,
    commands: Option<Vec<s::DecoderCommand>>,
) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.enabled_codecs.prefer_decoder(decoder);
    ctx.add_input_vector(0, png_bytes.to_vec()).unwrap();
    ctx.add_output_buffer(1).unwrap();

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Libpng {
                    depth: Some(s::PngBitDepth::Png32),
                    matte: None,
                    zlib_compression: None,
                },
            },
        ]),
    };

    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.take_output_buffer(1).unwrap();

    // Extract raw RGBA via lodepng
    let result = lodepng::decode32(&output_bytes).unwrap();
    result.buffer.iter().flat_map(|px| [px.r, px.g, px.b, px.a]).collect()
}

/// Decode a PNG through imageflow with a specific decoder (no extra commands).
fn decode_to_rgba(png_bytes: &[u8], decoder: NamedDecoders) -> Vec<u8> {
    decode_to_rgba_with_commands(png_bytes, decoder, None)
}

/// Maximum per-channel absolute difference between two RGBA buffers.
fn max_channel_delta(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len(), "buffer length mismatch: {} vs {}", a.len(), b.len());
    a.iter().zip(b.iter()).map(|(&va, &vb)| va.abs_diff(vb)).max().unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Test gradient (4×4 RGBA)
// ---------------------------------------------------------------------------

/// 4×4 test gradient spanning the full tonal range.
/// Dark values are most sensitive to gamma transforms.
#[rustfmt::skip]
fn test_gradient() -> Vec<u8> {
    vec![
        // Row 0: gray ramp
          0,  0,  0,255,   64, 64, 64,255,  128,128,128,255,  192,192,192,255,
        // Row 1: saturated primaries + yellow
        255,  0,  0,255,    0,255,  0,255,    0,  0,255,255,  255,255,  0,255,
        // Row 2: fine gray ramp (dark-sensitive)
         32, 32, 32,255,   96, 96, 96,255,  160,160,160,255,  224,224,224,255,
        // Row 3: mixed colors + white
        255,128,  0,255,  128,  0,255,255,    0,255,128,255,  255,255,255,255,
    ]
}

// ---------------------------------------------------------------------------
// Helper: decode through both decoders, return (libpng_pixels, image_png_pixels)
// ---------------------------------------------------------------------------

fn decode_both(png: &[u8]) -> (Vec<u8>, Vec<u8>) {
    decode_both_with_commands(png, None)
}

fn decode_both_with_commands(
    png: &[u8],
    commands: Option<Vec<s::DecoderCommand>>,
) -> (Vec<u8>, Vec<u8>) {
    let libpng =
        decode_to_rgba_with_commands(png, NamedDecoders::LibPngRsDecoder, commands.clone());
    let image_png = decode_to_rgba_with_commands(png, NamedDecoders::ImageRsPngDecoder, commands);
    (libpng, image_png)
}

/// Assert decoder parity and no-op behavior (output ≈ input).
fn assert_noop(test_name: &str, input: &[u8], png: &[u8]) {
    let (libpng, image_png) = decode_both(png);
    let parity = max_channel_delta(&libpng, &image_png);
    assert!(parity <= 2, "{test_name}: decoder parity failed — max delta {parity} (expected ≤ 2)");
    let libpng_delta = max_channel_delta(input, &libpng);
    assert!(
        libpng_delta <= 1,
        "{test_name}: libpng not a no-op — max delta {libpng_delta} (expected ≤ 1)"
    );
    let image_png_delta = max_channel_delta(input, &image_png);
    assert!(
        image_png_delta <= 1,
        "{test_name}: image_png not a no-op — max delta {image_png_delta} (expected ≤ 1)"
    );
    eprintln!(
        "{test_name}: OK (parity={parity}, libpng_delta={libpng_delta}, image_png_delta={image_png_delta})"
    );
}

/// Apply the sRGB EOTF (linear → sRGB encoding) to a single channel value.
/// Input is linear light [0,1], output is sRGB-encoded [0,1].
fn linear_to_srgb(v: f64) -> f64 {
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Compute expected sRGB output for a PNG with the given encoding gamma and
/// sRGB primaries (i.e., only the TRC differs from sRGB).
///
/// The PNG gAMA chunk stores the *encoding* gamma. The decoding exponent is
/// 1/gamma. For gAMA=1.0 (linear), decoding is identity (the pixel values
/// ARE linear light). For gAMA=0.55556 (Mac), decoding exponent is 1.8.
///
/// With sRGB primaries on both source and destination, the only operation is:
///   1. Decode source TRC: srgb_value^(1/encoding_gamma) → linear
///      (for gAMA=1.0, the file already contains linear values, so this is identity)
///   2. Encode to sRGB TRC: linear_to_srgb(linear) → sRGB
///
/// Wait — the pixel values in the file represent the *encoded* signal. The
/// encoding gamma tells us how they were encoded. To get linear light:
///   linear = file_value ^ (1 / encoding_gamma)
/// Then to get sRGB:
///   output = linear_to_srgb(linear)
fn expected_gamma_to_srgb(input: &[u8], encoding_gamma: f64) -> Vec<u8> {
    let decoding_exponent = 1.0 / encoding_gamma;
    input
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            if i % 4 == 3 {
                // Alpha channel is never transformed
                v
            } else {
                let normalized = v as f64 / 255.0;
                let linear = normalized.powf(decoding_exponent);
                let srgb = linear_to_srgb(linear);
                (srgb * 255.0 + 0.5).floor().min(255.0).max(0.0) as u8
            }
        })
        .collect()
}

/// Assert decoder parity and that pixel values match the expected gamma→sRGB
/// transform within tolerance.
fn assert_transform(test_name: &str, input: &[u8], png: &[u8], encoding_gamma: f64) {
    assert_transform_with_commands(test_name, input, png, encoding_gamma, None);
}

fn assert_transform_with_commands(
    test_name: &str,
    input: &[u8],
    png: &[u8],
    encoding_gamma: f64,
    commands: Option<Vec<s::DecoderCommand>>,
) {
    let expected = expected_gamma_to_srgb(input, encoding_gamma);
    let (libpng, image_png) = decode_both_with_commands(png, commands);

    let parity = max_channel_delta(&libpng, &image_png);
    assert!(parity <= 2, "{test_name}: decoder parity failed — max delta {parity} (expected ≤ 2)");

    // Verify against reference math (tolerance of 2 for CMS rounding)
    let libpng_vs_ref = max_channel_delta(&expected, &libpng);
    assert!(
        libpng_vs_ref <= 2,
        "{test_name}: libpng output doesn't match reference — max delta {libpng_vs_ref} (expected ≤ 2)"
    );
    let image_png_vs_ref = max_channel_delta(&expected, &image_png);
    assert!(
        image_png_vs_ref <= 2,
        "{test_name}: image_png output doesn't match reference — max delta {image_png_vs_ref} (expected ≤ 2)"
    );

    // Also verify that the transform actually changed something
    let input_vs_ref = max_channel_delta(input, &expected);
    assert!(
        input_vs_ref >= 10,
        "{test_name}: reference transform too weak — max delta {input_vs_ref} (expected ≥ 10)"
    );

    eprintln!(
        "{test_name}: OK (parity={parity}, vs_ref: libpng={libpng_vs_ref} image_png={image_png_vs_ref}, transform_strength={input_vs_ref})"
    );
}

// ===========================================================================
// Group 1: No-op cases (output ≈ input, delta ≤ 1)
// ===========================================================================

#[test]
fn test_png_no_color_chunks() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[]);
    assert_noop("no_color_chunks", &input, &png);
}

#[test]
fn test_png_srgb_chunk_only() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Srgb(0)]);
    assert_noop("srgb_chunk_only", &input, &png);
}

#[test]
fn test_png_gama_neutral_only() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.45455)]);
    assert_noop("gama_neutral_only", &input, &png);
}

#[test]
fn test_png_gama_neutral_with_srgb_chrm() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.45455), srgb_chrm()]);
    assert_noop("gama_neutral_with_srgb_chrm", &input, &png);
}

#[test]
fn test_png_chrm_only_no_gama() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[srgb_chrm()]);
    assert_noop("chrm_only_no_gama", &input, &png);
}

// ===========================================================================
// Group 2: Transform cases (output ≠ input, midtone delta ≥ 10)
// ===========================================================================

#[test]
fn test_png_gama_linear_only() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(1.0)]);
    // gAMA-only requires HonorGamaOnly to trigger a transform (off by default)
    assert_transform_with_commands(
        "gama_linear_only",
        &input,
        &png,
        1.0,
        Some(vec![s::DecoderCommand::HonorGamaOnly(true)]),
    );
    // Without HonorGamaOnly, gAMA-only is ignored (legacy behavior)
    let (libpng, image_png) = decode_both(&png);
    let libpng_delta = max_channel_delta(&input, &libpng);
    let image_png_delta = max_channel_delta(&input, &image_png);
    assert!(
        libpng_delta <= 1,
        "gama_linear_only: legacy libpng should be no-op (delta={libpng_delta})"
    );
    assert!(
        image_png_delta <= 1,
        "gama_linear_only: legacy image_png should be no-op (delta={image_png_delta})"
    );
}

#[test]
fn test_png_gama_mac_only() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.55556)]);
    // gAMA-only requires HonorGamaOnly to trigger a transform (off by default)
    assert_transform_with_commands(
        "gama_mac_only",
        &input,
        &png,
        0.55556,
        Some(vec![s::DecoderCommand::HonorGamaOnly(true)]),
    );
}

#[test]
fn test_png_gama_linear_with_srgb_chrm() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(1.0), srgb_chrm()]);
    assert_transform("gama_linear_with_srgb_chrm", &input, &png, 1.0);
}

// ===========================================================================
// Group 3: Precedence cases (sRGB overrides gAMA → no-op)
// ===========================================================================

#[test]
fn test_png_srgb_overrides_linear_gama() {
    let input = test_gradient();
    // sRGB chunk should override the gAMA(1.0) — result should be no-op
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Srgb(0), PngColorChunk::Gama(1.0)]);
    assert_noop("srgb_overrides_linear_gama", &input, &png);
}

#[test]
fn test_png_srgb_overrides_gama_chrm() {
    let input = test_gradient();
    // sRGB should override both gAMA and cHRM
    let png = build_test_png(
        4,
        4,
        &input,
        &[PngColorChunk::Srgb(0), PngColorChunk::Gama(1.0), srgb_chrm()],
    );
    assert_noop("srgb_overrides_gama_chrm", &input, &png);
}

// ===========================================================================
// Group 4: Degenerate input cases (safe fallback to sRGB → no-op)
// ===========================================================================

#[test]
fn test_png_gama_zero_fallback() {
    let input = test_gradient();
    // gAMA(0) is degenerate (would cause division by zero) — should be rejected
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.0)]);
    assert_noop("gama_zero_fallback", &input, &png);
}

#[test]
fn test_png_chrm_y_zero_fallback() {
    let input = test_gradient();
    // cHRM with blue_y=0 is degenerate — should be rejected, fallback to sRGB
    let png = build_test_png(
        4,
        4,
        &input,
        &[
            PngColorChunk::Gama(0.5),
            PngColorChunk::Chrm {
                white: (0.3127, 0.3290),
                red: (0.64, 0.33),
                green: (0.30, 0.60),
                blue: (0.15, 0.0), // degenerate: y=0
            },
        ],
    );
    assert_noop("chrm_y_zero_fallback", &input, &png);
}

// ===========================================================================
// Group 5: cICP precedence (PNG 3rd Edition)
//
// cICP is the highest-priority color chunk. These tests verify that:
// 1. cICP is actually parsed and used by the image_png decoder
// 2. cICP overrides gAMA (PNG v3 hierarchy: cICP > iCCP > sRGB > gAMA+cHRM)
// 3. A non-sRGB cICP causes a visible transform (not a false positive)
//
// Note: libpng (C decoder) does NOT support cICP, so these tests only use
// the image_png (Rust) decoder. This is a known limitation.
// ===========================================================================

/// cICP(BT.709 + sRGB transfer) is effectively sRGB — should be no-op.
/// This verifies cICP parsing works (if it were ignored, gAMA(1.0) would
/// cause a visible transform, failing the no-op assertion).
#[test]
fn test_png_cicp_srgb_overrides_linear_gama() {
    let input = test_gradient();
    // cICP says sRGB (cp=1, tc=13), but gAMA says linear (1.0).
    // If cICP is correctly prioritized, result is no-op.
    // If cICP is ignored, gAMA(1.0) would cause a visible transform.
    let png =
        build_test_png(4, 4, &input, &[PngColorChunk::Cicp(1, 13, 0, 1), PngColorChunk::Gama(1.0)]);
    let image_png = decode_to_rgba(&png, NamedDecoders::ImageRsPngDecoder);
    let delta = max_channel_delta(&input, &image_png);
    assert!(
        delta <= 1,
        "cicp_srgb_overrides_linear_gama: image_png delta {delta} (expected ≤ 1). \
         cICP(sRGB) should override gAMA(1.0) — if delta is high, cICP is not being parsed."
    );
    eprintln!("cicp_srgb_overrides_linear_gama: OK (image_png delta={delta})");
}

/// cICP(BT.709 + sRGB transfer) overrides gAMA+cHRM — verify full PNG v3 hierarchy.
#[test]
fn test_png_cicp_overrides_gama_chrm() {
    let input = test_gradient();
    let png = build_test_png(
        4,
        4,
        &input,
        &[PngColorChunk::Cicp(1, 13, 0, 1), PngColorChunk::Gama(1.0), srgb_chrm()],
    );
    let image_png = decode_to_rgba(&png, NamedDecoders::ImageRsPngDecoder);
    let delta = max_channel_delta(&input, &image_png);
    assert!(
        delta <= 1,
        "cicp_overrides_gama_chrm: image_png delta {delta} (expected ≤ 1). \
         cICP(sRGB) should override gAMA(1.0)+cHRM."
    );
    eprintln!("cicp_overrides_gama_chrm: OK (image_png delta={delta})");
}

/// cICP(BT.709 + sRGB transfer) overrides sRGB chunk — cICP > sRGB in PNG v3.
#[test]
fn test_png_cicp_overrides_srgb_chunk() {
    let input = test_gradient();
    // Both cICP and sRGB say sRGB, so this is a no-op regardless. But it
    // verifies the precedence path doesn't error when both are present.
    let png =
        build_test_png(4, 4, &input, &[PngColorChunk::Cicp(1, 13, 0, 1), PngColorChunk::Srgb(0)]);
    let image_png = decode_to_rgba(&png, NamedDecoders::ImageRsPngDecoder);
    let delta = max_channel_delta(&input, &image_png);
    assert!(delta <= 1, "cicp_overrides_srgb_chunk: image_png delta {delta} (expected ≤ 1).");
    eprintln!("cicp_overrides_srgb_chunk: OK (image_png delta={delta})");
}

/// Non-sRGB cICP (BT.709 primaries, BT.709 transfer tc=1) should cause a
/// visible transform. This proves CICP is actually being used, not just detected.
/// BT.709 transfer (tc=1, gamma ~1/0.45 ≈ 2.222) differs from sRGB (tc=13)
/// in the linear-segment toe, producing measurable differences in dark values.
#[test]
fn test_png_cicp_bt709_transfer_causes_transform() {
    let input = test_gradient();
    // cp=1 (BT.709 primaries — same as sRGB)
    // tc=1 (BT.709 transfer — different from sRGB tc=13)
    // mc=0, full_range=1
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Cicp(1, 1, 0, 1)]);
    let image_png = decode_to_rgba(&png, NamedDecoders::ImageRsPngDecoder);
    let delta = max_channel_delta(&input, &image_png);
    // BT.709 transfer vs sRGB differ primarily in the linear toe region.
    // The transform strength is modest (the curves are similar), but
    // should produce a non-zero delta on our test gradient's dark values.
    assert!(
        delta >= 1,
        "cicp_bt709_transfer: image_png delta {delta} (expected ≥ 1). \
         Non-sRGB CICP should cause a visible transform — if delta is 0, CICP may not be used."
    );
    eprintln!("cicp_bt709_transfer: OK (image_png delta={delta}, transform verified)");
}

// ===========================================================================
// Group 6: iCCP precedence (iCCP > sRGB > gAMA+cHRM)
//
// These tests use a minimal ICC profile built from raw bytes.
// The profile has sRGB primaries but gamma 1.8 (ProPhoto-like TRC),
// producing a visible transform when applied. iCCP must override sRGB.
// ===========================================================================

/// Build a minimal valid ICC v2 display RGB profile with sRGB primaries
/// and a specified gamma (as a simple curv tag). Returns raw ICC bytes.
fn build_gamma_icc_profile(gamma: f64) -> Vec<u8> {
    fn write_s15f16(buf: &mut Vec<u8>, val: f64) {
        let fixed = (val * 65536.0).round() as i32;
        buf.extend_from_slice(&fixed.to_be_bytes());
    }

    fn write_xyz_tag(buf: &mut Vec<u8>, x: f64, y: f64, z: f64) {
        buf.extend_from_slice(b"XYZ ");
        buf.extend_from_slice(&[0u8; 4]); // reserved
        write_s15f16(buf, x);
        write_s15f16(buf, y);
        write_s15f16(buf, z);
    }

    // Tag layout: 9 tags (desc, cprt, wtpt, rXYZ, gXYZ, bXYZ, rTRC, gTRC, bTRC)
    let tag_count: u32 = 9;
    let tag_table_size = 4 + tag_count as usize * 12;
    let data_offset = 128 + tag_table_size;

    // Pre-build tag data blocks with their offsets
    struct TagDef {
        sig: [u8; 4],
        data: Vec<u8>,
    }
    let mut tags: Vec<TagDef> = Vec::new();

    // desc tag (profileDescriptionTag)
    let mut desc_data = Vec::new();
    desc_data.extend_from_slice(b"desc");
    desc_data.extend_from_slice(&[0u8; 4]); // reserved
    let desc_str = b"Gamma Test Profile\0";
    desc_data.extend_from_slice(&(desc_str.len() as u32).to_be_bytes());
    desc_data.extend_from_slice(desc_str);
    // Pad to 4-byte alignment
    while desc_data.len() % 4 != 0 {
        desc_data.push(0);
    }
    tags.push(TagDef { sig: *b"desc", data: desc_data });

    // cprt tag (copyrightTag)
    let mut cprt_data = Vec::new();
    cprt_data.extend_from_slice(b"text");
    cprt_data.extend_from_slice(&[0u8; 4]); // reserved
    cprt_data.extend_from_slice(b"PD\0");
    while cprt_data.len() % 4 != 0 {
        cprt_data.push(0);
    }
    tags.push(TagDef { sig: *b"cprt", data: cprt_data });

    // wtpt tag (mediaWhitePointTag) — D50 illuminant (PCS illuminant)
    let mut wtpt_data = Vec::new();
    write_xyz_tag(&mut wtpt_data, 0.9505, 1.0000, 1.0890);
    tags.push(TagDef { sig: *b"wtpt", data: wtpt_data });

    // sRGB colorants (D50 adapted via Bradford)
    // rXYZ
    let mut rxyz_data = Vec::new();
    write_xyz_tag(&mut rxyz_data, 0.4360747, 0.2225045, 0.0139322);
    tags.push(TagDef { sig: *b"rXYZ", data: rxyz_data });

    // gXYZ
    let mut gxyz_data = Vec::new();
    write_xyz_tag(&mut gxyz_data, 0.3850649, 0.7168786, 0.0971045);
    tags.push(TagDef { sig: *b"gXYZ", data: gxyz_data });

    // bXYZ
    let mut bxyz_data = Vec::new();
    write_xyz_tag(&mut bxyz_data, 0.1430804, 0.0606169, 0.7141733);
    tags.push(TagDef { sig: *b"bXYZ", data: bxyz_data });

    // Shared curv tag for rTRC, gTRC, bTRC
    let mut curv_data = Vec::new();
    curv_data.extend_from_slice(b"curv");
    curv_data.extend_from_slice(&[0u8; 4]); // reserved
    curv_data.extend_from_slice(&1u32.to_be_bytes()); // count = 1 (single gamma)
    let gamma_u8f8 = (gamma * 256.0).round() as u16;
    curv_data.extend_from_slice(&gamma_u8f8.to_be_bytes());
    while curv_data.len() % 4 != 0 {
        curv_data.push(0);
    }
    // rTRC, gTRC, bTRC share the same data block
    tags.push(TagDef { sig: *b"rTRC", data: curv_data });
    // gTRC and bTRC will point to the same offset (handled below)

    // Compute offsets
    let mut current_offset = data_offset;
    let mut tag_offsets: Vec<(usize, usize)> = Vec::new(); // (offset, size)
    for tag in &tags {
        tag_offsets.push((current_offset, tag.data.len()));
        current_offset += tag.data.len();
    }
    // gTRC and bTRC share rTRC's data
    let rtrc_entry = tag_offsets[6]; // rTRC is index 6

    let total_size = current_offset;

    // Build the profile
    let mut icc = Vec::with_capacity(total_size);

    // --- Header (128 bytes) ---
    icc.extend_from_slice(&(total_size as u32).to_be_bytes()); // 0: profile size
    icc.extend_from_slice(&[0u8; 4]); // 4: preferred CMM
    icc.extend_from_slice(&[2u8, 0x20, 0, 0]); // 8: version 2.2.0
    icc.extend_from_slice(b"mntr"); // 12: display device class
    icc.extend_from_slice(b"RGB "); // 16: color space
    icc.extend_from_slice(b"XYZ "); // 20: PCS
    icc.extend_from_slice(&[0u8; 12]); // 24: date/time
    icc.extend_from_slice(b"acsp"); // 36: file signature
    icc.extend_from_slice(&[0u8; 4]); // 40: primary platform
    icc.extend_from_slice(&[0u8; 4]); // 44: profile flags
    icc.extend_from_slice(&[0u8; 4]); // 48: device manufacturer
    icc.extend_from_slice(&[0u8; 4]); // 52: device model
    icc.extend_from_slice(&[0u8; 8]); // 56: device attributes
    icc.extend_from_slice(&[0u8; 4]); // 64: rendering intent
                                      // 68: PCS illuminant D50 (s15Fixed16: X=0.9642, Y=1.0, Z=0.8249)
    write_s15f16(&mut icc, 0.9642);
    write_s15f16(&mut icc, 1.0000);
    write_s15f16(&mut icc, 0.8249);
    icc.extend_from_slice(&[0u8; 4]); // 80: profile creator
    icc.extend_from_slice(&[0u8; 16]); // 84: profile ID
    icc.extend_from_slice(&[0u8; 28]); // 100: reserved
    assert_eq!(icc.len(), 128);

    // --- Tag table ---
    icc.extend_from_slice(&tag_count.to_be_bytes());
    // Tags 0-6 (desc, cprt, wtpt, rXYZ, gXYZ, bXYZ, rTRC)
    for (i, tag) in tags.iter().enumerate() {
        icc.extend_from_slice(&tag.sig);
        icc.extend_from_slice(&(tag_offsets[i].0 as u32).to_be_bytes());
        icc.extend_from_slice(&(tag_offsets[i].1 as u32).to_be_bytes());
    }
    // gTRC — points to same data as rTRC
    icc.extend_from_slice(b"gTRC");
    icc.extend_from_slice(&(rtrc_entry.0 as u32).to_be_bytes());
    icc.extend_from_slice(&(rtrc_entry.1 as u32).to_be_bytes());
    // bTRC — points to same data as rTRC
    icc.extend_from_slice(b"bTRC");
    icc.extend_from_slice(&(rtrc_entry.0 as u32).to_be_bytes());
    icc.extend_from_slice(&(rtrc_entry.1 as u32).to_be_bytes());

    assert_eq!(icc.len(), data_offset);

    // --- Tag data ---
    for tag in &tags {
        icc.extend_from_slice(&tag.data);
    }

    assert_eq!(icc.len(), total_size);
    icc
}

/// iCCP(gamma 1.8) + sRGB chunk: iCCP must take precedence (PNG 3rd Ed).
/// The gamma 1.8 ICC profile should cause a visible transform. If sRGB
/// wrongly overrode iCCP, the output would match input (no-op).
///
/// Only tests image_png decoder — libpng's iCCP handling goes through
/// from_decoder_color_info which reads the profile from C-layer DecoderColorInfo.
#[test]
fn test_png_iccp_overrides_srgb_chunk() {
    let input = test_gradient();
    let icc = build_gamma_icc_profile(1.8);

    // PNG with iCCP + sRGB — iCCP should win
    let png =
        build_test_png(4, 4, &input, &[PngColorChunk::Iccp(icc.clone()), PngColorChunk::Srgb(0)]);
    let image_png = decode_to_rgba(&png, NamedDecoders::ImageRsPngDecoder);
    let delta = max_channel_delta(&input, &image_png);
    // Gamma 1.8 → sRGB should produce a visible transform
    assert!(
        delta >= 5,
        "iccp_overrides_srgb: image_png delta {delta} (expected ≥ 5). \
         If delta is low, iCCP may not be taking precedence over sRGB chunk."
    );
    eprintln!("iccp_overrides_srgb: OK (delta={delta}, iCCP took precedence)");

    // Verify the iCCP profile alone (without sRGB chunk) produces the same result
    let png_iccp_only = build_test_png(4, 4, &input, &[PngColorChunk::Iccp(icc)]);
    let image_png_iccp_only = decode_to_rgba(&png_iccp_only, NamedDecoders::ImageRsPngDecoder);
    let parity = max_channel_delta(&image_png, &image_png_iccp_only);
    assert!(
        parity == 0,
        "iccp_overrides_srgb: output differs with/without sRGB chunk (delta={parity}). \
         iCCP should produce identical results regardless of sRGB chunk presence."
    );
    eprintln!("iccp_overrides_srgb: iCCP-only parity confirmed (delta={parity})");
}

// ===========================================================================
// Group 7: sRGB > gAMA+cHRM with non-sRGB values (strong override test)
//
// Uses non-sRGB chromaticities that would cause a visible transform if
// gAMA+cHRM were honored. The sRGB chunk should override, making it a no-op.
// ===========================================================================

/// Display P3 chromaticities (wider green than sRGB).
fn p3_chrm() -> PngColorChunk {
    PngColorChunk::Chrm {
        white: (0.3127, 0.3290),
        red: (0.680, 0.320),
        green: (0.265, 0.690),
        blue: (0.150, 0.060),
    }
}

/// sRGB chunk should override non-sRGB gAMA+cHRM. With P3 primaries and
/// linear gamma, gAMA+cHRM alone would cause a significant transform.
/// The sRGB chunk must make this a no-op.
#[test]
fn test_png_srgb_overrides_nonsrgb_gama_chrm() {
    let input = test_gradient();

    // First verify gAMA(1.0) + P3 cHRM WITHOUT sRGB causes a visible transform
    let png_no_srgb = build_test_png(4, 4, &input, &[PngColorChunk::Gama(1.0), p3_chrm()]);
    let (libpng_no_srgb, image_png_no_srgb) = decode_both(&png_no_srgb);
    let transform_delta = max_channel_delta(&input, &image_png_no_srgb);
    assert!(
        transform_delta >= 10,
        "srgb_overrides_nonsrgb_gama_chrm: gAMA+P3 cHRM without sRGB should transform (delta={transform_delta})"
    );
    let parity_no_srgb = max_channel_delta(&libpng_no_srgb, &image_png_no_srgb);
    eprintln!(
        "srgb_overrides_nonsrgb_gama_chrm: without sRGB, transform_delta={transform_delta}, parity={parity_no_srgb}"
    );

    // Now add sRGB chunk — should override gAMA+cHRM → no-op
    let png_with_srgb = build_test_png(
        4,
        4,
        &input,
        &[PngColorChunk::Srgb(0), PngColorChunk::Gama(1.0), p3_chrm()],
    );
    assert_noop("srgb_overrides_nonsrgb_gama_chrm", &input, &png_with_srgb);
}

// ===========================================================================
// Group 8: Near-sRGB rounding (edge cases for neutral gamma / sRGB primaries)
// ===========================================================================

/// gAMA value at the boundary of neutral-gamma detection should be treated
/// as sRGB. Gamma 0.454 * 2.2 = 0.9988, which is within ±0.05 of 1.0.
#[test]
fn test_png_near_srgb_gamma_rounds_to_srgb() {
    let input = test_gradient();
    // 0.454 is within the neutral threshold (0.4318 to 0.4773)
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.454)]);
    assert_noop("near_srgb_gamma_rounds (0.454)", &input, &png);
}

/// gAMA at the other edge of the neutral range with sRGB cHRM.
#[test]
fn test_png_near_srgb_gamma_high_edge() {
    let input = test_gradient();
    // 0.477 * 2.2 = 1.0494, within ±0.05 of 1.0
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.477), srgb_chrm()]);
    assert_noop("near_srgb_gamma_high_edge (0.477)", &input, &png);
}

/// gAMA just outside the neutral range should trigger a transform when HonorGamaOnly is set.
/// Without HonorGamaOnly, gAMA-only is ignored (legacy behavior).
#[test]
fn test_png_outside_neutral_gamma_transforms() {
    let input = test_gradient();
    // 0.42 * 2.2 = 0.924, outside ±0.05 of 1.0 (below 0.95 threshold)
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.42)]);

    // With HonorGamaOnly: should trigger a transform
    let honor_cmd = Some(vec![s::DecoderCommand::HonorGamaOnly(true)]);
    let (libpng, image_png) = decode_both_with_commands(&png, honor_cmd);
    let parity = max_channel_delta(&libpng, &image_png);
    assert!(parity <= 2, "outside_neutral_gamma: parity {parity} (expected ≤ 2)");
    let delta = max_channel_delta(&input, &image_png);
    assert!(
        delta >= 5,
        "outside_neutral_gamma: delta {delta} (expected ≥ 5). \
         Gamma 0.42 is outside neutral range and should trigger a transform."
    );

    // Without HonorGamaOnly: should be a no-op (legacy behavior)
    let (_, image_png_default) = decode_both(&png);
    let default_delta = max_channel_delta(&input, &image_png_default);
    assert!(
        default_delta <= 1,
        "outside_neutral_gamma: legacy should be no-op (delta={default_delta})"
    );
    eprintln!(
        "outside_neutral_gamma: OK (parity={parity}, delta={delta}, legacy_delta={default_delta})"
    );
}

// ===========================================================================
// Group 9: HonorGamaChrm(false) decoder command
//
// Verifies that HonorGamaChrm(false) makes the decoder ignore gAMA+cHRM chunks
// while still honoring iCCP and sRGB chunks.
// ===========================================================================

/// Decode with HonorGamaChrm(false) and a specific decoder backend.
fn decode_with_honor_gama_chrm_false(png_bytes: &[u8], decoder: NamedDecoders) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.enabled_codecs.prefer_decoder(decoder);
    ctx.add_input_vector(0, png_bytes.to_vec()).unwrap();
    ctx.add_output_buffer(1).unwrap();

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode {
                io_id: 0,
                commands: Some(vec![s::DecoderCommand::HonorGamaChrm(false)]),
            },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Libpng {
                    depth: Some(s::PngBitDepth::Png32),
                    matte: None,
                    zlib_compression: None,
                },
            },
        ]),
    };

    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.take_output_buffer(1).unwrap();
    let result = lodepng::decode32(&output_bytes).unwrap();
    result.buffer.iter().flat_map(|px| [px.r, px.g, px.b, px.a]).collect()
}

/// HonorGamaChrm(false) should make gAMA(1.0) (linear) a no-op even when HonorGamaOnly is set.
/// With HonorGamaOnly, gAMA(1.0) causes a visible transform; HonorGamaChrm(false) cancels it.
#[test]
fn test_png_honor_gama_chrm_false_ignores_linear_gama() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(1.0)]);

    // With HonorGamaOnly: gAMA(1.0) causes a visible transform
    let honor_cmd = Some(vec![s::DecoderCommand::HonorGamaOnly(true)]);
    let normal = decode_to_rgba_with_commands(&png, NamedDecoders::ImageRsPngDecoder, honor_cmd);
    let normal_delta = max_channel_delta(&input, &normal);
    assert!(
        normal_delta >= 10,
        "baseline: gAMA(1.0) with HonorGamaOnly should transform (delta={normal_delta})"
    );

    // With HonorGamaOnly + HonorGamaChrm(false): gAMA is ignored → no-op (test both decoders)
    let both_cmds =
        Some(vec![s::DecoderCommand::HonorGamaOnly(true), s::DecoderCommand::HonorGamaChrm(false)]);
    let discarded_ipng =
        decode_to_rgba_with_commands(&png, NamedDecoders::ImageRsPngDecoder, both_cmds.clone());
    let delta_ipng = max_channel_delta(&input, &discarded_ipng);
    assert!(
        delta_ipng <= 1,
        "honor_gama_chrm_false (image_png): delta {delta_ipng} (expected ≤ 1)"
    );
    let discarded_lpng =
        decode_to_rgba_with_commands(&png, NamedDecoders::LibPngRsDecoder, both_cmds);
    let delta_lpng = max_channel_delta(&input, &discarded_lpng);
    assert!(delta_lpng <= 1, "honor_gama_chrm_false (libpng): delta {delta_lpng} (expected ≤ 1)");
    eprintln!("honor_gama_chrm_false_ignores_linear_gama: OK");
}

/// HonorGamaChrm(false) should NOT affect iCCP — iCCP is still honored.
#[test]
fn test_png_honor_gama_chrm_false_preserves_iccp() {
    let input = test_gradient();
    let icc = build_gamma_icc_profile(1.8);
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Iccp(icc)]);

    // With HonorGamaChrm(false): iCCP should still be applied
    let discarded = decode_with_honor_gama_chrm_false(&png, NamedDecoders::ImageRsPngDecoder);
    let delta = max_channel_delta(&input, &discarded);
    assert!(
        delta >= 5,
        "honor_gama_chrm_false_preserves_iccp: delta {delta} (expected ≥ 5). \
         iCCP should still be honored even with HonorGamaChrm(false)."
    );
    eprintln!("honor_gama_chrm_false_preserves_iccp: OK (delta={delta})");
}

/// Chromaticities slightly off sRGB but within tolerance should round to sRGB.
#[test]
fn test_png_near_srgb_primaries_round_to_srgb() {
    let input = test_gradient();
    // sRGB primaries with small perturbations (within ±0.01 tolerance)
    let png = build_test_png(
        4,
        4,
        &input,
        &[
            PngColorChunk::Gama(0.45455),
            PngColorChunk::Chrm {
                white: (0.3130, 0.3295), // sRGB: 0.3127, 0.3290
                red: (0.645, 0.335),     // sRGB: 0.64, 0.33
                green: (0.305, 0.605),   // sRGB: 0.30, 0.60
                blue: (0.155, 0.065),    // sRGB: 0.15, 0.06
            },
        ],
    );
    assert_noop("near_srgb_primaries_round", &input, &png);
}
