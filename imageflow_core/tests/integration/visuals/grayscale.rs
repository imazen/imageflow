use crate::common::*;
use imageflow_core::Context;
use imageflow_types::{EncoderPreset, Execute001, Framewise, Node};

/// Build a minimal 8-bit grayscale PNG (no ICC, no gamma) in memory.
/// Gradient from 0 (black) to 255 (white) across `w` columns, `h` rows identical.
fn build_gray_png(w: u32, h: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            pixels[(y * w + x) as usize] = ((x * 255) / (w - 1).max(1)) as u8;
        }
    }
    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, w, h);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
    }
    buf
}

/// Build a grayscale JPEG from a gray PNG (decode + re-encode).
fn build_gray_jpeg(w: u32, h: u32, quality: u8) -> Vec<u8> {
    let png = build_gray_png(w, h);
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode {
                io_id: 1,
                preset: EncoderPreset::Mozjpeg {
                    quality: Some(quality),
                    progressive: None,
                    matte: None,
                },
            },
        ]),
    })
    .unwrap();
    ctx.take_output_buffer(1).unwrap()
}

/// Decode input, optionally resize, encode to given preset, return output bytes.
fn transcode(input: Vec<u8>, command: &str) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![Node::CommandString {
            kind: imageflow_types::CommandStringKind::ImageResizer4,
            value: command.to_owned(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }]),
    })
    .unwrap();
    ctx.take_output_buffer(1).unwrap()
}

/// Decode to bitmap, compare that R=G=B for all pixels (grayscale preserved).
fn assert_grayscale_bitmap(input: Vec<u8>, label: &str) {
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, input).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
        ]),
    })
    .unwrap();
    let png = ctx.take_output_buffer(1).unwrap();
    let img = lodepng::decode32(&png).unwrap();
    let mut non_gray = 0usize;
    let mut max_spread = 0u8;
    for px in &img.buffer {
        let spread = px.r.abs_diff(px.g).max(px.r.abs_diff(px.b)).max(px.g.abs_diff(px.b));
        if spread > 1 {
            non_gray += 1;
        }
        max_spread = max_spread.max(spread);
    }
    assert!(
        max_spread <= 1,
        "{label}: expected grayscale output (R≈G≈B), got max channel spread={max_spread} \
         ({non_gray}/{} non-gray pixels)",
        img.buffer.len()
    );
}

// ============================================================================
// Grayscale PNG tests
// ============================================================================

#[test]
fn test_gray_png_decode_preserves_grayscale() {
    test_init();
    let png = build_gray_png(64, 64);
    assert_grayscale_bitmap(png, "gray_png_64x64");
}

#[test]
fn test_gray_png_resize_preserves_grayscale() {
    test_init();
    let png = build_gray_png(256, 256);
    let out = transcode(png, "w=64&h=64&format=png");
    assert!(out.starts_with(b"\x89PNG"), "output should be PNG");
    assert_grayscale_bitmap(out, "gray_png_resize_64x64");
}

#[test]
fn test_gray_png_to_jpeg_roundtrip() {
    test_init();
    let png = build_gray_png(100, 100);
    let jpg = transcode(png, "format=jpg&quality=95");
    assert!(jpg.starts_with(b"\xFF\xD8\xFF"), "output should be JPEG");
    // JPEG is lossy — allow small channel spread from chroma subsampling
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, jpg).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(Execute001 {
        job_options: None,
        graph_recording: default_graph_recording(false),
        security: None,
        framewise: Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode { io_id: 1, preset: EncoderPreset::Lodepng { maximum_deflate: None } },
        ]),
    })
    .unwrap();
    let out_png = ctx.take_output_buffer(1).unwrap();
    let img = lodepng::decode32(&out_png).unwrap();
    let mut max_spread = 0u8;
    for px in &img.buffer {
        let spread = px.r.abs_diff(px.g).max(px.r.abs_diff(px.b));
        max_spread = max_spread.max(spread);
    }
    assert!(
        max_spread <= 3,
        "gray_png→jpeg roundtrip: max channel spread={max_spread} (expected ≤3 for high-quality JPEG)"
    );
}

#[test]
fn test_gray_png_to_webp_roundtrip() {
    test_init();
    let png = build_gray_png(100, 100);
    let webp = transcode(png.clone(), "format=webp&quality=95");
    assert!(webp.starts_with(b"RIFF"), "output should be WebP");
    // Decode WebP back and check grayscale preserved (lossy allows some spread)
    let out = transcode(webp, "format=png");
    let img = lodepng::decode32(&out).unwrap();
    let mut max_spread = 0u8;
    for px in &img.buffer {
        let spread = px.r.abs_diff(px.g).max(px.r.abs_diff(px.b));
        max_spread = max_spread.max(spread);
    }
    assert!(
        max_spread <= 5,
        "gray_png→webp roundtrip: max channel spread={max_spread} (expected ≤5 for lossy WebP)"
    );
}

#[test]
fn test_gray_png_to_gif_roundtrip() {
    test_init();
    let png = build_gray_png(64, 64);
    let gif = transcode(png, "format=gif");
    assert!(gif.starts_with(b"GIF89a") || gif.starts_with(b"GIF87a"), "output should be GIF");
    assert_grayscale_bitmap(gif, "gray_png→gif roundtrip");
}

#[test]
fn test_gray_png_to_webp_lossless_roundtrip() {
    test_init();
    let png = build_gray_png(64, 64);
    let webp = transcode(png, "format=webp&webp.lossless=true");
    assert!(webp.starts_with(b"RIFF"), "output should be WebP");
    assert_grayscale_bitmap(webp, "gray_png→webp_lossless roundtrip");
}

// ============================================================================
// Grayscale JPEG tests
// ============================================================================

#[test]
fn test_gray_jpeg_decode_preserves_grayscale() {
    test_init();
    let jpg = build_gray_jpeg(100, 100, 95);
    assert_grayscale_bitmap(jpg, "gray_jpeg_decode");
}

#[test]
fn test_gray_jpeg_resize_preserves_grayscale() {
    test_init();
    let jpg = build_gray_jpeg(200, 200, 95);
    let out = transcode(jpg, "w=50&h=50&format=png");
    assert_grayscale_bitmap(out, "gray_jpeg_resize_50x50");
}

#[test]
fn test_gray_jpeg_to_png_roundtrip() {
    test_init();
    let jpg = build_gray_jpeg(100, 100, 100);
    let png = transcode(jpg, "format=png");
    assert!(png.starts_with(b"\x89PNG"), "output should be PNG");
    assert_grayscale_bitmap(png, "gray_jpeg→png roundtrip");
}

#[test]
fn test_gray_jpeg_to_jpeg_roundtrip() {
    test_init();
    let jpg = build_gray_jpeg(100, 100, 95);
    let jpg2 = transcode(jpg, "format=jpg&quality=95");
    assert!(jpg2.starts_with(b"\xFF\xD8\xFF"), "output should be JPEG");
    assert_grayscale_bitmap(jpg2, "gray_jpeg→jpeg roundtrip");
}

// ============================================================================
// Grayscale JPEG with ICC profile
// ============================================================================

#[test]
fn test_gray_jpeg_with_icc_decode() {
    test_init();
    // Use the corpus gray-gamma-22 JPEG (has embedded gray ICC profile)
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/wide-gamut/gray-gamma-22/flickr_2f4bbf638f18ebea.jpg";
    let out = transcode_url(url, "format=png");
    assert!(out.starts_with(b"\x89PNG"), "output should be PNG");
    assert_grayscale_bitmap(out, "gray_jpeg_icc_decode");
}

#[test]
fn test_gray_jpeg_with_icc_resize() {
    test_init();
    let url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/wide-gamut/gray-gamma-22/flickr_2f4bbf638f18ebea.jpg";
    let out = transcode_url(url, "w=200&format=png");
    assert!(out.starts_with(b"\x89PNG"), "output should be PNG");
    assert_grayscale_bitmap(out, "gray_jpeg_icc_resize");
}

// ============================================================================
// Cross-format grayscale matrix
// ============================================================================

#[test]
fn test_gray_source_to_all_formats() {
    test_init();
    let png = build_gray_png(64, 64);

    let formats: &[(&str, &str, &[u8])] = &[
        ("png", "format=png", b"\x89PNG" as &[u8]),
        ("jpg_q95", "format=jpg&quality=95", b"\xFF\xD8\xFF"),
        ("gif", "format=gif", b"GIF"),
        ("webp_lossy", "format=webp&quality=90", b"RIFF"),
        ("webp_lossless", "format=webp&webp.lossless=true", b"RIFF"),
    ];

    for (name, cmd, magic) in formats {
        let out = transcode(png.clone(), cmd);
        assert!(
            out.starts_with(magic),
            "gray→{name}: output should start with {magic:?}, got {:?}",
            &out[..4.min(out.len())]
        );
        // Verify output is still grayscale (allowing lossy codec spread)
        let decoded = transcode(out, "format=png");
        let img = lodepng::decode32(&decoded).unwrap();
        let mut max_spread = 0u8;
        for px in &img.buffer {
            let spread = px.r.abs_diff(px.g).max(px.r.abs_diff(px.b));
            max_spread = max_spread.max(spread);
        }
        let tolerance = if name.contains("lossy") || name.contains("jpg") { 5 } else { 1 };
        assert!(
            max_spread <= tolerance,
            "gray→{name}: channel spread={max_spread} exceeds tolerance={tolerance}"
        );
    }
}

// ============================================================================
// Helper: transcode from URL
// ============================================================================

fn transcode_url(url: &str, command: &str) -> Vec<u8> {
    let input = crate::common::get_url_bytes_with_retry(url).unwrap();
    transcode(input, command)
}
