//! Validate libpng gAMA handling by comparing two imageflow decoders.
//!
//! For each gAMA PNG in the test set, decode with both:
//! 1. LibPng (C wrapper) — the decoder affected by the gAMA-only fix
//! 2. ImagePng (pure Rust) — already handled gAMA-only correctly
//!
//! If both decoders produce the same sRGB output (within rounding tolerance),
//! the libpng fix is correct.
//!
//! Note: ImageMagick is NOT a valid reference for gAMA-only PNGs because it
//! ignores the gAMA chunk when cHRM is absent (treats as passthrough).
//!
//! Run with: cargo test -p imageflow_core --test test_gama_validation -- --nocapture

use imageflow_core::{Context, NamedDecoders};
use imageflow_types as s;
use std::path::Path;

const TEST_LIST: &str = "/tmp/test_gama_pngs.txt";
const OUT_DIR: &str = "/mnt/v/output/gama-validation";

/// Decode a PNG with a specific decoder and return raw sRGB PNG bytes.
fn decode_with_decoder(png_bytes: &[u8], decoder: NamedDecoders) -> Option<Vec<u8>> {
    let mut ctx = Context::create().ok()?;
    ctx.enabled_codecs.prefer_decoder(decoder);
    ctx.add_input_vector(0, png_bytes.to_vec()).ok()?;
    ctx.add_output_buffer(1).ok()?;

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

    ctx.execute_1(execute).ok()?;
    ctx.take_output_buffer(1).ok()
}

/// Extract raw RGBA pixels from a PNG via ImageMagick (for pixel comparison).
fn png_to_rgba(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let tmp = format!("/tmp/gama_val_{}.png", std::process::id());
    std::fs::write(&tmp, png_bytes).ok()?;
    let output = std::process::Command::new("convert")
        .args([&tmp, "-depth", "8", "RGBA:-"])
        .output()
        .ok()?;
    let _ = std::fs::remove_file(&tmp);
    if output.status.success() {
        Some(output.stdout)
    } else {
        None
    }
}

/// Compare two RGBA buffers: (max_channel_delta, mean_delta, pixel_count).
fn compare_rgba(a: &[u8], b: &[u8]) -> (u8, f64, usize) {
    if a.len() != b.len() {
        return (255, 255.0, 0);
    }
    let mut max_d: u8 = 0;
    let mut sum_d: u64 = 0;
    for (&va, &vb) in a.iter().zip(b.iter()) {
        let d = va.abs_diff(vb);
        if d > max_d {
            max_d = d;
        }
        sum_d += d as u64;
    }
    let mean = if a.is_empty() { 0.0 } else { sum_d as f64 / a.len() as f64 };
    (max_d, mean, a.len() / 4)
}

#[test]
fn validate_libpng_vs_image_png_for_gama_files() {
    if !Path::new(TEST_LIST).exists() {
        eprintln!("Skipping: {TEST_LIST} not found (run classify script first)");
        return;
    }

    let _ = std::fs::create_dir_all(OUT_DIR);

    let paths: Vec<String> = std::fs::read_to_string(TEST_LIST)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    eprintln!("\n=== libpng vs image_png decoder comparison: {} files ===\n", paths.len());

    let mut results: Vec<(String, u8, f64, usize, &str)> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    for (i, path) in paths.iter().enumerate() {
        let basename = Path::new(path).file_name().unwrap().to_string_lossy().to_string();

        let png_bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                errors.push((basename, format!("read error: {e}")));
                continue;
            }
        };

        // Decode with libpng (C wrapper)
        let libpng_png = match decode_with_decoder(&png_bytes, NamedDecoders::LibPngRsDecoder) {
            Some(p) => p,
            None => {
                errors.push((basename, "libpng decode failed".into()));
                continue;
            }
        };

        // Decode with image_png (pure Rust)
        let image_png_png = match decode_with_decoder(&png_bytes, NamedDecoders::ImageRsPngDecoder)
        {
            Some(p) => p,
            None => {
                errors.push((basename, "image_png decode failed".into()));
                continue;
            }
        };

        // Extract raw pixels from both outputs
        let libpng_px = match png_to_rgba(&libpng_png) {
            Some(p) => p,
            None => {
                errors.push((basename, "libpng output pixel extraction failed".into()));
                continue;
            }
        };

        let image_png_px = match png_to_rgba(&image_png_png) {
            Some(p) => p,
            None => {
                errors.push((basename, "image_png output pixel extraction failed".into()));
                continue;
            }
        };

        let (max_d, mean_d, n_px) = compare_rgba(&libpng_px, &image_png_px);

        let status = if libpng_px.len() != image_png_px.len() {
            "SIZE_MISMATCH"
        } else if max_d <= 2 {
            "OK"
        } else if max_d <= 5 {
            "WARN"
        } else {
            "FAIL"
        };

        eprintln!(
            "[{:>3}/{}] {:<4} max={:<3} mean={:.3} px={:<8} {}",
            i + 1,
            paths.len(),
            status,
            max_d,
            mean_d,
            n_px,
            basename
        );

        results.push((basename, max_d, mean_d, n_px, status));
    }

    // Summary
    eprintln!("\n=== Summary ===");
    let tested = results.len();
    let max_across_all = results.iter().map(|r| r.1).max().unwrap_or(0);
    let fails = results.iter().filter(|r| r.4 == "FAIL").count();
    let warns = results.iter().filter(|r| r.4 == "WARN").count();
    let size_mismatches = results.iter().filter(|r| r.4 == "SIZE_MISMATCH").count();
    let oks = tested - fails - warns - size_mismatches;

    eprintln!("  Tested: {tested}");
    eprintln!("  Errors: {}", errors.len());
    eprintln!("  Max delta across all files: {max_across_all}");
    eprintln!("  OK (<=2): {oks}");
    eprintln!("  WARN (3-5): {warns}");
    eprintln!("  FAIL (>5): {fails}");
    eprintln!("  SIZE_MISMATCH: {size_mismatches}");

    for (name, err) in &errors {
        eprintln!("  ERROR: {name}: {err}");
    }

    // Write TSV results
    let tsv_path = format!("{OUT_DIR}/gama_decoder_comparison.tsv");
    let mut tsv = String::from("file\tmax_delta\tmean_delta\tpixels\tstatus\n");
    for (name, max_d, mean_d, n_px, status) in &results {
        tsv.push_str(&format!("{name}\t{max_d}\t{mean_d:.4}\t{n_px}\t{status}\n"));
    }
    std::fs::write(&tsv_path, &tsv).unwrap();
    eprintln!("\nResults: {tsv_path}");

    // Both decoders should produce identical output.
    // - gAMA-only with neutral gamma (≈0.45455): treated as sRGB (no transform)
    // - gAMA+cHRM with sRGB values: treated as sRGB (no transform)
    // - gAMA-only with non-neutral gamma: both apply same GammaPrimaries transform
    // Allow <=2 for potential CMS rounding in non-neutral gamma cases.
    assert!(
        max_across_all <= 2,
        "Max delta {max_across_all} exceeds tolerance 2 — see output above"
    );
    assert_eq!(
        size_mismatches, 0,
        "{size_mismatches} files had pixel count mismatch between decoders"
    );
}
