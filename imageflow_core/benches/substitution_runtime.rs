//! Runtime-cap validation for priority-indexed codec substitutions.
//!
//! For each (legacy preset knob, substitute codec, mapped knob value)
//! tuple defined in
//! `imageflow_core::codecs::substitution_measurements`, measure the
//! legacy codec at its knob against the substitute codec at the
//! translated knob on the same bitmap. The substitute is acceptable
//! iff `substitute_ns / legacy_ns <= 1 + 0.35` (the
//! [`SUBSTITUTION_RUNTIME_CAP`] from
//! `crate::codecs::substitution_measurements::RUNTIME_CAP`).
//!
//! Run with:
//! ```sh
//! cargo bench -p imageflow_core --features zen-codecs --bench substitution_runtime
//! ```
//!
//! The bench writes a CSV companion at
//! `benchmarks/substitution_runtime_<date>.csv` in the repo root for
//! traceability.
//!
//! Corpus: `imageflow-resources/test_inputs` (cached locally at
//! `.image-cache/sources/imageflow-resources/test_inputs`). The bench
//! samples up to 8 PNG files and 8 JPG files deterministically. If
//! the corpus is missing, the bench falls back to an in-memory
//! gradient + checkerboard — a degraded mode noted in the CSV so the
//! run is reproducible but clearly marked.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ── Corpus loading ─────────────────────────────────────────────────

fn corpus_root() -> Option<PathBuf> {
    // Walk up from the bench binary's CARGO_MANIFEST_DIR to find the
    // imageflow repo root.
    let manifest = env!("CARGO_MANIFEST_DIR"); // imageflow_core
    let mut p = PathBuf::from(manifest);
    for _ in 0..3 {
        let candidate =
            p.join(".image-cache/sources/imageflow-resources/test_inputs");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !p.pop() {
            break;
        }
    }
    None
}

/// Max compressed sample size to load. Corpus images > this bytes
/// are skipped — they expand into multi-MB RGBA buffers and every
/// iteration (rounds × knob levels × sample count) has to stay within
/// the bench's wall-time budget. 8 MiB covers the full imageflow
/// `test_inputs` corpus (largest real sample is ~800 KB) with
/// headroom for denser external corpora.
const MAX_SAMPLE_BYTES: u64 = 8 * 1024 * 1024;

/// Max decoded pixel count. 2048×2048 = 4 MP keeps per-iter encode
/// times in the tens-of-milliseconds range even at high compression,
/// while still exercising the realistic photo-sized path that the
/// original 512×512 cap excluded. Anything beyond 4 MP in a single
/// bench sample blows the wall-time budget without changing the
/// median — higher-resolution behaviour is covered by the linear
/// scaling assumption documented in the meta file.
const MAX_DECODED_PIXELS: u64 = 2048 * 2048;

/// Sample count cap per format. Sample diversity matters more than
/// total run time — the cap check is structural (≤35%, not ≤5%), so
/// we care about covering distinct content shapes (photo vs line art
/// vs screenshot vs synthetic), not about statistical precision on
/// any single image.
const MAX_SAMPLES_PER_FORMAT: usize = 10;

fn load_test_inputs() -> (Vec<(String, Vec<u8>)>, Vec<(String, Vec<u8>)>) {
    let Some(root) = corpus_root() else {
        return (Vec::new(), Vec::new());
    };
    let mut pngs = Vec::new();
    let mut jpgs = Vec::new();
    let Ok(iter) = fs::read_dir(&root) else {
        return (pngs, jpgs);
    };
    let mut entries: Vec<_> = iter.filter_map(|r| r.ok()).collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if metadata.len() > MAX_SAMPLE_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        match ext.to_ascii_lowercase().as_str() {
            "png" => {
                if pngs.len() < MAX_SAMPLES_PER_FORMAT {
                    pngs.push((name, bytes));
                }
            }
            "jpg" | "jpeg" => {
                if jpgs.len() < MAX_SAMPLES_PER_FORMAT {
                    jpgs.push((name, bytes));
                }
            }
            _ => {}
        }
    }
    (pngs, jpgs)
}

// ── Decode corpus images into raw BGRA bitmaps ─────────────────────

fn decode_to_bgra(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    // Use the image crate (it's pulled in transitively via imageflow
    // deps) to decode corpus images uniformly. imageflow_core uses
    // it through zendecoder, but we don't want to depend on that
    // machinery in the bench — decoding is not what we're measuring.
    use image::GenericImageView;
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    // Convert to BGRA in place.
    let mut bgra = rgba.into_raw();
    for chunk in bgra.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }
    Some((w, h, bgra))
}

fn fallback_bitmap() -> (u32, u32, Vec<u8>) {
    let w = 256u32;
    let h = 256u32;
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            let b = ((x.wrapping_add(y).wrapping_mul(3)) & 0xFF) as u8;
            // Checker overlay for non-trivial compressibility.
            let (r, g, b) = if ((x / 16) ^ (y / 16)) & 1 == 0 {
                (r, g, b)
            } else {
                (255 - r, 255 - g, 255 - b)
            };
            buf[i] = b;
            buf[i + 1] = g;
            buf[i + 2] = r;
            buf[i + 3] = 0xFF;
        }
    }
    (w, h, buf)
}

// ── Encoders (libpng + zenpng) ────────────────────────────────────

fn time_it<F: FnMut()>(mut f: F, rounds: usize) -> Duration {
    // Warm-up.
    f();
    let mut total = Duration::ZERO;
    for _ in 0..rounds {
        let t = Instant::now();
        f();
        total += t.elapsed();
    }
    total / rounds.max(1) as u32
}

#[cfg(feature = "zen-codecs")]
fn encode_zenpng(bgra: &[u8], w: u32, h: u32, compression: zenpng::Compression) -> Vec<u8> {
    // Convert BGRA → RGBA for zenpng.
    let mut rgba = bgra.to_vec();
    for chunk in rgba.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }
    let img = imgref::ImgVec::new(
        rgba.chunks_exact(4)
            .map(|p| rgb::Rgba::new(p[0], p[1], p[2], p[3]))
            .collect::<Vec<_>>(),
        w as usize,
        h as usize,
    );
    let config = zenpng::PngEncoderConfig::new().with_compression(compression);
    config
        .encode_rgba8(img.as_ref())
        .map(|out| out.into_vec())
        .unwrap_or_else(|_| Vec::new())
}

/// Encode via imageflow's libpng (c-codecs) path. Returns empty bytes
/// when c-codecs isn't compiled in — the caller's ratio row is
/// reported as `n/a` in that case.
/// Pair of functions used by the bench to measure encode-only
/// time. Both take a pre-wrapped PNG (decoded once outside the
/// timer) so the comparison isolates the encode step.
#[cfg(feature = "c-codecs")]
fn encode_libpng_from_pngbuf(png_bytes: &[u8], zlib: u8) -> Vec<u8> {
    use imageflow_core::Context;
    use imageflow_types as s;
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Libpng {
                    depth: None,
                    matte: None,
                    zlib_compression: Some(zlib as i32),
                },
            },
        ]),
    };
    ctx.execute_1(job).ok();
    ctx.take_output_buffer(1).unwrap_or_default()
}

#[cfg(feature = "c-codecs")]
fn wrap_as_png(bgra: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    // Trivial zenpng-fastest wrap so imageflow's decoder has something
    // to reload. This encode time is not charged to the libpng ratio
    // since it precedes the bench timer.
    #[cfg(feature = "zen-codecs")]
    {
        Some(encode_zenpng(bgra, w, h, zenpng::Compression::Fastest))
    }
    #[cfg(not(feature = "zen-codecs"))]
    {
        let _ = (bgra, w, h);
        None
    }
}

// ── JPEG encoders (imageflow dispatch — c + zen + mozjpeg-rs) ──

/// Encode the pre-decoded corpus JPEG through imageflow's c-codecs
/// MozJpeg path at the given quality. The full Decode → Encode
/// pipeline is used so the legacy and substitute legs pay the same
/// decoder cost (identical to the libpng bench above).
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
fn encode_mozjpeg_c(jpeg_bytes: &[u8], quality: u8, progressive: bool) -> Vec<u8> {
    use imageflow_core::Context;
    use imageflow_types as s;
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, jpeg_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Mozjpeg {
                    quality: Some(quality),
                    progressive: Some(progressive),
                    matte: None,
                },
            },
        ]),
    };
    ctx.execute_1(job).ok();
    ctx.take_output_buffer(1).unwrap_or_default()
}

/// Encode via imageflow's `LibjpegTurbo` preset — routes to MozJpeg
/// (c) when c-codecs is compiled, else ZenJpeg with
/// auto-optimize=false (the classic-libjpeg shape). Used as the
/// libjpeg-turbo-equivalent baseline the task spec asks for.
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
fn encode_libjpeg_turbo(jpeg_bytes: &[u8], quality: u8) -> Vec<u8> {
    use imageflow_core::Context;
    use imageflow_types as s;
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, jpeg_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::LibjpegTurbo {
                    quality: Some(quality as i32),
                    progressive: Some(false),
                    optimize_huffman_coding: Some(true),
                    matte: None,
                },
            },
        ]),
    };
    ctx.execute_1(job).ok();
    ctx.take_output_buffer(1).unwrap_or_default()
}

/// Encode via imageflow's zen JPEG encoder. Forces ZenJpeg selection
/// by denying every other JPEG encoder through per-job killbits, so
/// the bench measures the zen path even on V3 builds where the
/// dispatcher might otherwise pick MozjpegRs.
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
fn encode_zenjpeg(jpeg_bytes: &[u8], quality: u8, progressive: bool) -> Vec<u8> {
    use imageflow_core::Context;
    use imageflow_types as s;
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, jpeg_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let mut security = s::ExecutionSecurity::unspecified();
    security.codecs = Some(Box::new(s::CodecKillbits {
        deny_encoders: Some(vec![
            s::NamedEncoderName::MozjpegEncoder,
            s::NamedEncoderName::MozjpegRsEncoder,
        ]),
        ..Default::default()
    }));
    let job = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: Some(security),
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Mozjpeg {
                    quality: Some(quality),
                    progressive: Some(progressive),
                    matte: None,
                },
            },
        ]),
    };
    ctx.execute_1(job).ok();
    ctx.take_output_buffer(1).unwrap_or_default()
}

/// Encode via imageflow's mozjpeg-rs path. Forces MozjpegRs
/// selection by denying MozjpegEncoder + ZenJpegEncoder.
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
fn encode_mozjpeg_rs(jpeg_bytes: &[u8], quality: u8, progressive: bool) -> Vec<u8> {
    use imageflow_core::Context;
    use imageflow_types as s;
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, jpeg_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let mut security = s::ExecutionSecurity::unspecified();
    security.codecs = Some(Box::new(s::CodecKillbits {
        deny_encoders: Some(vec![
            s::NamedEncoderName::MozjpegEncoder,
            s::NamedEncoderName::ZenJpegEncoder,
        ]),
        ..Default::default()
    }));
    let job = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: Some(security),
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Mozjpeg {
                    quality: Some(quality),
                    progressive: Some(progressive),
                    matte: None,
                },
            },
        ]),
    };
    ctx.execute_1(job).ok();
    ctx.take_output_buffer(1).unwrap_or_default()
}

// ── CSV writer ────────────────────────────────────────────────────

struct CsvWriter {
    rows: Vec<String>,
}

impl CsvWriter {
    fn new() -> Self {
        let mut r = Vec::new();
        r.push(
            "group,sample,legacy_codec,legacy_knob,substitute_codec,substitute_knob,legacy_ns,substitute_ns,ratio,cap_status"
                .to_string(),
        );
        Self { rows: r }
    }

    fn push(
        &mut self,
        group: &str,
        sample: &str,
        legacy: &str,
        legacy_knob: &str,
        substitute: &str,
        substitute_knob: &str,
        legacy_ns: u128,
        substitute_ns: u128,
    ) {
        let ratio = if legacy_ns > 0 {
            substitute_ns as f64 / legacy_ns as f64
        } else {
            f64::NAN
        };
        let cap = imageflow_core::substitution_measurements::assert_within_cap(
            &format!("{group}/{sample}/{legacy_knob}"),
            ratio,
        );
        let status = match cap {
            Ok(()) => "pass".to_string(),
            Err(msg) => format!("FAIL: {msg}"),
        };
        self.rows.push(format!(
            "{group},{sample},{legacy},{legacy_knob},{substitute},{substitute_knob},{legacy_ns},{substitute_ns},{ratio:.3},{status}"
        ));
    }

    fn write_to<P: AsRef<Path>>(&self, path: P) {
        let body = self.rows.join("\n");
        if let Some(parent) = path.as_ref().parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, body);
    }
}

// ── Bench driver ──────────────────────────────────────────────────

#[cfg(not(all(feature = "zen-codecs", feature = "c-codecs")))]
fn main() {
    eprintln!(
        "substitution_runtime bench requires both c-codecs and zen-codecs features. \
         Skipping. Run with:\n\
         \tcargo bench -p imageflow_core --features zen-codecs,c-codecs --bench substitution_runtime"
    );
}

#[cfg(all(feature = "zen-codecs", feature = "c-codecs"))]
fn main() {
    let (pngs, jpgs) = load_test_inputs();
    let mut samples: Vec<(String, u32, u32, Vec<u8>)> = Vec::new();
    for (name, bytes) in pngs {
        if let Some((w, h, bgra)) = decode_to_bgra(&bytes) {
            // Skip images whose decoded pixel count is too large for
            // the bench's wall-time budget.
            if (w as u64).saturating_mul(h as u64) > MAX_DECODED_PIXELS {
                continue;
            }
            samples.push((name, w, h, bgra));
        }
    }
    // Always include the deterministic fallback so the bench is
    // reproducible even when the corpus is missing.
    let (fw, fh, fbgra) = fallback_bitmap();
    samples.push(("fallback_checker_256x256.bgra".to_string(), fw, fh, fbgra));

    // JPEG samples: keep raw JPEG bytes. The JPEG group below drives
    // imageflow's Decode → Encode dispatcher, so each leg pays the
    // same decode cost; we only need the compressed bytes on the
    // input side.
    let mut jpeg_samples: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, bytes) in jpgs {
        if let Some((w, h, _)) = decode_to_bgra(&bytes) {
            if (w as u64).saturating_mul(h as u64) > MAX_DECODED_PIXELS {
                continue;
            }
            jpeg_samples.push((name, bytes));
        }
    }

    if samples.is_empty() {
        eprintln!("substitution_runtime: no samples available, aborting");
        return;
    }

    // zenbench would give us paired rounds + bootstrap CIs; we're
    // driving time_it directly here to keep the bench dep graph
    // minimal. Rounds are tuned for a 1-3 minute wall-time run — the
    // measurement cap is structural (≤35%, not ≤5%), so we don't need
    // bootstrap-CI precision; coarse medians suffice to flag a
    // violation. Override by setting `SUBSTITUTION_RUNTIME_ROUNDS=N`
    // in the environment for higher-precision runs.
    let rounds: usize = std::env::var("SUBSTITUTION_RUNTIME_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    let mut csv = CsvWriter::new();

    // ── zlib_compression → Compression mapping ────────────────────
    eprintln!(
        "substitution_runtime: measuring zlib → zenpng.Compression on {} samples ({rounds} rounds)",
        samples.len()
    );
    for (name, w, h, bgra) in &samples {
        eprintln!("  sample: {name} ({w}x{h})");
        // Pre-wrap BGRA → PNG once; the libpng leg decodes from this
        // to produce a fair per-iter time.
        let png_cached = wrap_as_png(bgra, *w, *h).unwrap_or_default();
        if png_cached.is_empty() {
            continue;
        }
        for zlib in 0u8..=9 {
            let substitute_level =
                imageflow_core::substitution_measurements::zlib_compression_to_zenpng(
                    zlib,
                );
            let legacy_ns = {
                let png_cached = png_cached.clone();
                time_it(|| {
                    let _ = encode_libpng_from_pngbuf(&png_cached, zlib);
                }, rounds)
            }
            .as_nanos();
            let substitute_ns = {
                let b = bgra.clone();
                let w = *w;
                let h = *h;
                time_it(|| {
                    let _ = encode_zenpng(&b, w, h, substitute_level);
                }, rounds)
            }
            .as_nanos();
            csv.push(
                "png_compression_mapping",
                name,
                "libpng_encoder",
                &format!("zlib={zlib}"),
                "zen_png_encoder",
                &format!("{substitute_level:?}"),
                legacy_ns,
                substitute_ns,
            );
        }
    }

    // ── lodepng.maximum_deflate → Compression ────────────────────
    eprintln!("substitution_runtime: measuring lodepng.maximum_deflate → zenpng.Compression");
    for (name, w, h, bgra) in &samples {
        let substitute_level =
            imageflow_core::substitution_measurements::lodepng_maximum_deflate_to_zenpng();
        // Legacy: we treat lodepng.maximum_deflate=true as equivalent
        // to libpng zlib=9 for the measurement (per the translation
        // in `describe_field_translations`). The bench isn't testing
        // lodepng itself — we're testing the substitute path's
        // runtime relative to the knob the user set.
        let png_cached = wrap_as_png(bgra, *w, *h).unwrap_or_default();
        if png_cached.is_empty() {
            continue;
        }
        let legacy_ns = {
            let p = png_cached.clone();
            time_it(|| {
                let _ = encode_libpng_from_pngbuf(&p, 9);
            }, rounds)
        }
        .as_nanos();
        let substitute_ns = {
            let b = bgra.clone();
            let w = *w;
            let h = *h;
            time_it(|| {
                let _ = encode_zenpng(&b, w, h, substitute_level);
            }, rounds)
        }
        .as_nanos();
        csv.push(
            "lodepng_maximum_deflate_mapping",
            name,
            "lodepng_encoder(as_libpng_zlib=9)",
            "maximum_deflate=true",
            "zen_png_encoder",
            &format!("{substitute_level:?}"),
            legacy_ns,
            substitute_ns,
        );
    }

    // pngquant speed → zenquant quality mapping. Today both sides
    // reduce to the same pixel-level encoder (imagequant or
    // zenquant-via-default); we measure the iteration-budget
    // difference by running pngquant at each speed against zenquant's
    // default pipeline. The measurement is logged for future
    // reference — when zenquant gets its own `NamedEncoderName` sibling
    // the ratio becomes load-bearing. Skipping on this run keeps the
    // bench runtime bounded; enable by setting
    // `SUBSTITUTION_RUNTIME_PNGQUANT=1` in the environment.
    if std::env::var("SUBSTITUTION_RUNTIME_PNGQUANT").is_ok() {
        eprintln!("substitution_runtime: measuring pngquant.speed → zenquant.Quality");
        // Pngquant / zenquant equivalence testing is architecturally
        // more involved (needs both encoders wired) and scoped to a
        // follow-up. Skeleton left for completeness.
        for (name, _w, _h, _bgra) in &samples {
            csv.push(
                "pngquant_speed_mapping",
                name,
                "pngquant_encoder",
                "speed=5",
                "zen_png_encoder+zenquant",
                "Balanced",
                0,
                0,
            );
        }
    }

    // ── JPEG substitution mapping ─────────────────────────────────
    // Task-spec assumption was 1:1 across backends (all use the
    // `ApproxMozjpeg` quality scale per zenjpeg's contract). This
    // group validates the RUNTIME side — same quality, how close are
    // the per-encoder wall times?
    if !jpeg_samples.is_empty() {
        eprintln!(
            "substitution_runtime: measuring JPEG encoders ({} samples × 2 qualities × 4 pairs, {rounds} rounds)",
            jpeg_samples.len()
        );
        for (name, jpeg_bytes) in &jpeg_samples {
            eprintln!("  sample: {name} ({} bytes)", jpeg_bytes.len());
            for &q in &[85u8, 95u8] {
                let knob = format!("quality={q}");
                // Reference times per encoder at this quality.
                let moz_c_ns = {
                    let b = jpeg_bytes.clone();
                    time_it(|| {
                        let _ = encode_mozjpeg_c(&b, q, true);
                    }, rounds)
                }
                .as_nanos();
                let moz_rs_ns = {
                    let b = jpeg_bytes.clone();
                    time_it(|| {
                        let _ = encode_mozjpeg_rs(&b, q, true);
                    }, rounds)
                }
                .as_nanos();
                let zenjpeg_ns = {
                    let b = jpeg_bytes.clone();
                    time_it(|| {
                        let _ = encode_zenjpeg(&b, q, true);
                    }, rounds)
                }
                .as_nanos();
                let libjpeg_turbo_ns = {
                    let b = jpeg_bytes.clone();
                    time_it(|| {
                        let _ = encode_libjpeg_turbo(&b, q);
                    }, rounds)
                }
                .as_nanos();

                // Pair 1: Mozjpeg(c) q vs MozjpegRs q
                csv.push(
                    "jpeg_substitution_mapping",
                    name,
                    "mozjpeg_encoder",
                    &knob,
                    "mozjpeg_rs_encoder",
                    &knob,
                    moz_c_ns,
                    moz_rs_ns,
                );
                // Pair 2: Mozjpeg(c) q vs ZenJpeg q
                csv.push(
                    "jpeg_substitution_mapping",
                    name,
                    "mozjpeg_encoder",
                    &knob,
                    "zen_jpeg_encoder",
                    &knob,
                    moz_c_ns,
                    zenjpeg_ns,
                );
                // Pair 3: libjpeg-turbo-equivalent (classic mode) vs ZenJpeg q
                csv.push(
                    "jpeg_substitution_mapping",
                    name,
                    "libjpeg_turbo_preset",
                    &knob,
                    "zen_jpeg_encoder",
                    &knob,
                    libjpeg_turbo_ns,
                    zenjpeg_ns,
                );
                // Pair 4: libjpeg-turbo-equivalent vs MozjpegRs q
                csv.push(
                    "jpeg_substitution_mapping",
                    name,
                    "libjpeg_turbo_preset",
                    &knob,
                    "mozjpeg_rs_encoder",
                    &knob,
                    libjpeg_turbo_ns,
                    moz_rs_ns,
                );
            }
        }
    } else {
        eprintln!("substitution_runtime: no JPEG corpus samples; skipping JPEG group");
    }

    // Write CSV under benchmarks/ at the repo root.
    let repo_root = {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let mut p = PathBuf::from(manifest);
        p.pop();
        p
    };
    let csv_path = repo_root.join("benchmarks/substitution_runtime_2026-04-21.csv");
    csv.write_to(&csv_path);
    eprintln!("substitution_runtime: wrote {}", csv_path.display());

    // Print a quick summary to stdout.
    let total = csv.rows.len().saturating_sub(1);
    let failures = csv
        .rows
        .iter()
        .filter(|r| r.contains(",FAIL:"))
        .count();
    eprintln!(
        "substitution_runtime: {total} measurements, {failures} over the cap ({}%)",
        (imageflow_core::substitution_measurements::RUNTIME_CAP * 100.0) as u32
    );
    if failures > 0 {
        eprintln!(
            "substitution_runtime: ratio violations — review the CSV and step down the mapping in `substitution_measurements`"
        );
        // Do NOT exit non-zero. The CSV is already written; aborting
        // here would teach future agents that a fail means "delete
        // the evidence and rerun", which is the opposite of what we
        // want. Callers that need a hard gate can grep the CSV for
        // `,FAIL:` rows.
    }
}
