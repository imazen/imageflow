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

/// Decode a PNG through imageflow with a specific decoder, re-encode as PNG32,
/// and return raw RGBA pixels extracted via lodepng.
fn decode_to_rgba(png_bytes: &[u8], decoder: NamedDecoders) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.enabled_codecs.prefer_decoder(decoder);
    ctx.add_input_vector(0, png_bytes.to_vec()).unwrap();
    ctx.add_output_buffer(1).unwrap();

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
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
    result
        .buffer
        .iter()
        .flat_map(|px| [px.r, px.g, px.b, px.a])
        .collect()
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
    let libpng = decode_to_rgba(png, NamedDecoders::LibPngRsDecoder);
    let image_png = decode_to_rgba(png, NamedDecoders::ImageRsPngDecoder);
    (libpng, image_png)
}

/// Assert decoder parity and no-op behavior (output ≈ input).
fn assert_noop(test_name: &str, input: &[u8], png: &[u8]) {
    let (libpng, image_png) = decode_both(png);
    let parity = max_channel_delta(&libpng, &image_png);
    assert!(
        parity <= 2,
        "{test_name}: decoder parity failed — max delta {parity} (expected ≤ 2)"
    );
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
    let expected = expected_gamma_to_srgb(input, encoding_gamma);
    let (libpng, image_png) = decode_both(png);

    let parity = max_channel_delta(&libpng, &image_png);
    assert!(
        parity <= 2,
        "{test_name}: decoder parity failed — max delta {parity} (expected ≤ 2)"
    );

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
    assert_transform("gama_linear_only", &input, &png, 1.0);
}

#[test]
fn test_png_gama_mac_only() {
    let input = test_gradient();
    let png = build_test_png(4, 4, &input, &[PngColorChunk::Gama(0.55556)]);
    assert_transform("gama_mac_only", &input, &png, 0.55556);
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
