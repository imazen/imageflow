//! Comparative codec benchmarks — zen (pure Rust) vs c (native) paths.
//!
//! Decoder comparisons are driven by `Context.enabled_codecs`, which
//! `prefer_decoder` / `disable_decoder` manipulate at runtime. The
//! `create_decoder_for_magic_bytes` iterator picks the first enabled
//! decoder whose magic matches, so runtime swaps are honoured.
//!
//! Encoder comparisons are limited: when both `c-codecs` and `zen-codecs`
//! are compiled in, the `#[cfg]` gates in `codecs/auto.rs` bind the
//! format-specific presets (Libpng, Mozjpeg, WebPLossy, WebPLossless) to
//! the C backend. The zen / mozjpeg-rs encoder paths are therefore only
//! reachable via a build without `c-codecs` — this bench exercises the
//! reachable encoders in the current build (c-codecs when available).

use imageflow_core::{Context, NamedDecoders};
use imageflow_types as s;
use zenbench::prelude::*;

// ---------------------------------------------------------------------------
// Fixture generation
// ---------------------------------------------------------------------------

/// Build a raw BGRA gradient bitmap. Deterministic, no RNG.
fn make_bgra_gradient(w: u32, h: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    for y in 0..h {
        for x in 0..w {
            let i = ((y as usize) * (w as usize) + (x as usize)) * 4;
            // Cheap gradient that still compresses non-trivially.
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            let b = ((x.wrapping_add(y).wrapping_mul(3)) & 0xFF) as u8;
            buf[i] = b;
            buf[i + 1] = g;
            buf[i + 2] = r;
            buf[i + 3] = 0xFF;
        }
    }
    buf
}

/// Build a PNG fixture using imageflow's own encoder pipeline.
/// Uses CreateCanvas + FillRect as source to keep fixtures self-contained.
fn encode_fixture(w: u32, h: u32, preset: s::EncoderPreset) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.add_output_buffer(1).unwrap();

    // Build a checkerboard via two FillRects so the compressed output isn't
    // trivially tiny (a single solid colour encodes to a handful of bytes
    // and makes decode benches noisy).
    let half_w = (w / 2).max(1);
    let mut steps = vec![s::Node::CreateCanvas {
        w: w as usize,
        h: h as usize,
        format: s::PixelFormat::Bgra32,
        color: s::Color::Srgb(s::ColorSrgb::Hex("FF8040FF".to_string())),
    }];
    for ty in 0..8u32 {
        for tx in 0..8u32 {
            if (tx + ty) % 2 == 0 {
                continue;
            }
            let x1 = (w * tx) / 8;
            let y1 = (h * ty) / 8;
            let x2 = (w * (tx + 1)) / 8;
            let y2 = (h * (ty + 1)) / 8;
            steps.push(s::Node::FillRect {
                x1,
                y1,
                x2,
                y2,
                color: s::Color::Srgb(s::ColorSrgb::Hex("204080FF".to_string())),
            });
        }
    }
    let _ = half_w;
    steps.push(s::Node::Encode { io_id: 1, preset });

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    ctx.take_output_buffer(1).unwrap()
}

fn png_fixture(w: u32, h: u32) -> Vec<u8> {
    encode_fixture(
        w,
        h,
        s::EncoderPreset::Libpng { depth: None, matte: None, zlib_compression: Some(3) },
    )
}

fn jpeg_fixture(w: u32, h: u32) -> Vec<u8> {
    // LibjpegTurbo preset uses `MozjpegEncoder::create_classic`, which
    // doesn't check `enabled_codecs` — this makes it usable for fixture
    // generation regardless of the EnabledCodecs defaults in the current
    // build configuration.
    encode_fixture(
        w,
        h,
        s::EncoderPreset::LibjpegTurbo {
            quality: Some(85),
            progressive: Some(false),
            optimize_huffman_coding: Some(false),
            matte: None,
        },
    )
}

#[cfg(feature = "c-codecs")]
fn webp_fixture(w: u32, h: u32) -> Vec<u8> {
    encode_fixture(w, h, s::EncoderPreset::WebPLossy { quality: 80.0 })
}

#[cfg(not(feature = "c-codecs"))]
fn webp_fixture(_w: u32, _h: u32) -> Vec<u8> {
    // Without c-codecs the zen path handles WebPLossy.
    encode_fixture(_w, _h, s::EncoderPreset::WebPLossy { quality: 80.0 })
}

fn gif_fixture(w: u32, h: u32) -> Vec<u8> {
    encode_fixture(w, h, s::EncoderPreset::Gif)
}

// ---------------------------------------------------------------------------
// Bench helpers
// ---------------------------------------------------------------------------

/// Sizes to bench. Kept modest so the full suite completes in a few minutes.
const SIZES: &[(u32, u32)] = &[(256, 256), (1024, 1024), (4096, 4096)];

/// Construct an `EnabledCodecs` that prefers `preferred` and drops each
/// decoder in `disable`.
fn configure_decoders(ctx: &mut Context, preferred: NamedDecoders, disable: &[NamedDecoders]) {
    ctx.enabled_codecs.prefer_decoder(preferred);
    for d in disable {
        ctx.enabled_codecs.disable_decoder(*d);
    }
}

/// Run a decode-only job: Decode(io_id=0) → no-op → (implicitly terminate).
/// We use CommandString to produce a bitmap and then throw it away;
/// cheaper than a full encode.
fn decode_only_job(fixture: &[u8]) {
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, fixture.to_vec()).unwrap();
    // A decode node alone isn't a valid graph terminus, so pair it with
    // a tiny resample to 1x1 to force full decode + read of the frame.
    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Resample2D { w: 1, h: 1, hints: None },
        ]),
    };
    ctx.execute_1(execute).unwrap();
}

fn decode_with_config(fixture: &[u8], preferred: NamedDecoders, disable: &[NamedDecoders]) {
    let mut ctx = Context::create().unwrap();
    configure_decoders(&mut ctx, preferred, disable);
    ctx.add_input_vector(0, fixture.to_vec()).unwrap();
    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Resample2D { w: 1, h: 1, hints: None },
        ]),
    };
    ctx.execute_1(execute).unwrap();
}

fn _unused_warning_suppress() {
    decode_only_job(&[]);
}

/// Encode a synthetic canvas with `preset` and drop the output.
fn encode_job(w: u32, h: u32, preset: s::EncoderPreset) {
    let _bytes = encode_fixture(w, h, preset);
}

// ---------------------------------------------------------------------------
// Decode benches
// ---------------------------------------------------------------------------

fn bench_jpeg_decode(suite: &mut Suite) {
    suite.group("jpeg_decode", |g| {
        for &(w, h) in SIZES {
            let fixture = jpeg_fixture(w, h);
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            #[cfg(feature = "zen-codecs")]
            {
                let f = fixture.clone();
                g.bench(format!("zen_{w}x{h}"), move |b| {
                    b.iter(|| {
                        decode_with_config(
                            &f,
                            NamedDecoders::ZenJpegDecoder,
                            #[cfg(feature = "c-codecs")]
                            &[NamedDecoders::MozJpegRsDecoder, NamedDecoders::ImageRsJpegDecoder],
                            #[cfg(not(feature = "c-codecs"))]
                            &[],
                        )
                    })
                });
            }

            #[cfg(feature = "c-codecs")]
            {
                let f = fixture.clone();
                g.bench(format!("mozjpeg_{w}x{h}"), move |b| {
                    b.iter(|| {
                        decode_with_config(
                            &f,
                            NamedDecoders::MozJpegRsDecoder,
                            #[cfg(feature = "zen-codecs")]
                            &[NamedDecoders::ZenJpegDecoder],
                            #[cfg(not(feature = "zen-codecs"))]
                            &[],
                        )
                    })
                });
            }
        }
    });
}

fn bench_png_decode(suite: &mut Suite) {
    suite.group("png_decode", |g| {
        for &(w, h) in SIZES {
            let fixture = png_fixture(w, h);
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            #[cfg(feature = "zen-codecs")]
            {
                let f = fixture.clone();
                g.bench(format!("zen_{w}x{h}"), move |b| {
                    b.iter(|| {
                        decode_with_config(
                            &f,
                            NamedDecoders::ZenPngDecoder,
                            #[cfg(feature = "c-codecs")]
                            &[NamedDecoders::LibPngRsDecoder, NamedDecoders::ImageRsPngDecoder],
                            #[cfg(not(feature = "c-codecs"))]
                            &[NamedDecoders::ImageRsPngDecoder],
                        )
                    })
                });
            }

            #[cfg(feature = "c-codecs")]
            {
                let f = fixture.clone();
                g.bench(format!("libpng_{w}x{h}"), move |b| {
                    b.iter(|| {
                        decode_with_config(
                            &f,
                            NamedDecoders::LibPngRsDecoder,
                            #[cfg(feature = "zen-codecs")]
                            &[NamedDecoders::ZenPngDecoder, NamedDecoders::ImageRsPngDecoder],
                            #[cfg(not(feature = "zen-codecs"))]
                            &[NamedDecoders::ImageRsPngDecoder],
                        )
                    })
                });
            }

            // image-rs PNG baseline (always available).
            let f = fixture.clone();
            g.bench(format!("image_rs_{w}x{h}"), move |b| {
                b.iter(|| {
                    decode_with_config(
                        &f,
                        NamedDecoders::ImageRsPngDecoder,
                        #[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
                        &[NamedDecoders::LibPngRsDecoder, NamedDecoders::ZenPngDecoder],
                        #[cfg(all(feature = "c-codecs", not(feature = "zen-codecs")))]
                        &[NamedDecoders::LibPngRsDecoder],
                        #[cfg(all(not(feature = "c-codecs"), feature = "zen-codecs"))]
                        &[NamedDecoders::ZenPngDecoder],
                        #[cfg(all(not(feature = "c-codecs"), not(feature = "zen-codecs")))]
                        &[],
                    )
                })
            });
        }
    });
}

fn bench_webp_decode(suite: &mut Suite) {
    suite.group("webp_decode", |g| {
        for &(w, h) in SIZES {
            let fixture = webp_fixture(w, h);
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            #[cfg(feature = "zen-codecs")]
            {
                let f = fixture.clone();
                g.bench(format!("zen_{w}x{h}"), move |b| {
                    b.iter(|| {
                        decode_with_config(
                            &f,
                            NamedDecoders::ZenWebPDecoder,
                            #[cfg(feature = "c-codecs")]
                            &[NamedDecoders::WebPDecoder],
                            #[cfg(not(feature = "c-codecs"))]
                            &[],
                        )
                    })
                });
            }

            #[cfg(feature = "c-codecs")]
            {
                let f = fixture.clone();
                g.bench(format!("libwebp_{w}x{h}"), move |b| {
                    b.iter(|| {
                        decode_with_config(
                            &f,
                            NamedDecoders::WebPDecoder,
                            #[cfg(feature = "zen-codecs")]
                            &[NamedDecoders::ZenWebPDecoder],
                            #[cfg(not(feature = "zen-codecs"))]
                            &[],
                        )
                    })
                });
            }
        }
    });
}

fn bench_gif_decode(suite: &mut Suite) {
    suite.group("gif_decode", |g| {
        for &(w, h) in SIZES {
            let fixture = gif_fixture(w, h);
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            #[cfg(feature = "zen-codecs")]
            {
                let f = fixture.clone();
                g.bench(format!("zen_{w}x{h}"), move |b| {
                    b.iter(|| {
                        decode_with_config(
                            &f,
                            NamedDecoders::ZenGifDecoder,
                            &[NamedDecoders::GifRsDecoder],
                        )
                    })
                });
            }

            // gif-rs baseline (always available).
            let f = fixture.clone();
            g.bench(format!("gifrs_{w}x{h}"), move |b| {
                b.iter(|| {
                    decode_with_config(
                        &f,
                        NamedDecoders::GifRsDecoder,
                        #[cfg(feature = "zen-codecs")]
                        &[NamedDecoders::ZenGifDecoder],
                        #[cfg(not(feature = "zen-codecs"))]
                        &[],
                    )
                })
            });
        }
    });
}

// ---------------------------------------------------------------------------
// Encode benches
// ---------------------------------------------------------------------------
//
// When both `c-codecs` and `zen-codecs` are enabled the `#[cfg]` gates in
// `codecs/auto.rs` route format-specific presets (Libpng, Mozjpeg,
// WebPLossy, etc.) to the C backend. Runtime swapping through
// `enabled_codecs.encoders` is not honoured by the preset path.
//
// Consequently, these benches measure whichever backend the current build
// resolves each preset to:
//   * default + both features:      Libpng = libpng-sys, Mozjpeg = mozjpeg(-sys),
//                                   WebPLossy = libwebp-sys
//   * --no-default-features --features zen-codecs:
//                                   Libpng = zenpng, Mozjpeg = mozjpeg-rs,
//                                   WebPLossy = zenwebp
//   * Always pure-Rust: Lodepng, Pngquant, Gif

fn bench_jpeg_encode(suite: &mut Suite) {
    suite.group("jpeg_encode", |g| {
        for &(w, h) in SIZES {
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            // LibjpegTurbo preset uses `MozjpegEncoder::create_classic`,
            // bypassing the `enabled_codecs` gate. Resolves to the
            // mozjpeg C encoder when c-codecs is on; fails otherwise.
            #[cfg(feature = "c-codecs")]
            g.bench(format!("libjpegturbo_q85_{w}x{h}"), move |b| {
                b.iter(|| {
                    encode_job(
                        w,
                        h,
                        s::EncoderPreset::LibjpegTurbo {
                            quality: Some(85),
                            progressive: Some(false),
                            optimize_huffman_coding: None,
                            matte: None,
                        },
                    )
                })
            });
        }
    });
}

/// JPEG via the `Mozjpeg` preset. When both features are compiled in,
/// `codecs/auto.rs` routes this to the C mozjpeg crate AND the gate
/// `enabled_codecs.encoders.contains(MozJpegEncoder)` must be satisfied.
/// The default `EnabledCodecs` only lists `MozJpegEncoder` when c-codecs
/// is enabled without zen-codecs — so with both features on this path
/// fails at runtime. The bench is registered anyway so the group is
/// visible; each iteration is a no-op when the encoder is unreachable.
fn bench_jpeg_encode_mozjpegrs(suite: &mut Suite) {
    suite.group("jpeg_encode_mozjpegrs", |g| {
        for &(w, h) in SIZES {
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            // Only register when the encoder is actually reachable
            // (either c-codecs+no-zen → C mozjpeg, or
            //  zen-codecs+no-c → mozjpeg-rs).
            #[cfg(any(
                all(feature = "c-codecs", not(feature = "zen-codecs")),
                all(feature = "zen-codecs", not(feature = "c-codecs")),
            ))]
            g.bench(format!("mozjpeg_preset_q85_{w}x{h}"), move |b| {
                b.iter(|| {
                    encode_job(
                        w,
                        h,
                        s::EncoderPreset::Mozjpeg {
                            quality: Some(85),
                            progressive: Some(true),
                            matte: None,
                        },
                    )
                })
            });

            // When both features are enabled, the preset fails — register
            // a stub so the group appears in the report.
            #[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
            g.bench(format!("unreachable_both_features_{w}x{h}"), move |b| {
                b.iter(|| {
                    // No-op: neither path reachable via public preset API
                    // when both features are compiled together.
                    let _ = (w, h);
                })
            });
        }
    });
}

fn bench_png_encode(suite: &mut Suite) {
    suite.group("png_encode", |g| {
        for &(w, h) in SIZES {
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            g.bench(format!("libpng_z3_{w}x{h}"), move |b| {
                b.iter(|| {
                    encode_job(
                        w,
                        h,
                        s::EncoderPreset::Libpng {
                            depth: None,
                            matte: None,
                            zlib_compression: Some(3),
                        },
                    )
                })
            });

            g.bench(format!("lodepng_{w}x{h}"), move |b| {
                b.iter(|| {
                    encode_job(w, h, s::EncoderPreset::Lodepng { maximum_deflate: Some(false) })
                })
            });
        }
    });
}

fn bench_webp_encode(suite: &mut Suite) {
    suite.group("webp_encode", |g| {
        for &(w, h) in SIZES {
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));

            g.bench(format!("lossy_q80_{w}x{h}"), move |b| {
                b.iter(|| encode_job(w, h, s::EncoderPreset::WebPLossy { quality: 80.0 }))
            });

            g.bench(format!("lossless_{w}x{h}"), move |b| {
                b.iter(|| encode_job(w, h, s::EncoderPreset::WebPLossless))
            });
        }
    });
}

fn bench_gif_encode(suite: &mut Suite) {
    suite.group("gif_encode", |g| {
        // GIF is cheapest; a 4096² fixture is overkill.
        for &(w, h) in &[(256u32, 256u32), (1024, 1024)] {
            let pixels = (w as u64) * (h as u64);
            g.throughput(Throughput::Elements(pixels));
            g.bench(format!("gif_{w}x{h}"), move |b| {
                b.iter(|| encode_job(w, h, s::EncoderPreset::Gif))
            });
        }
    });
}

// Silence dead-code warning for make_bgra_gradient if unused in some cfg.
#[allow(dead_code)]
fn _keep_helpers_alive() {
    let _ = make_bgra_gradient(1, 1);
    _unused_warning_suppress();
}

zenbench::main!(
    bench_jpeg_decode,
    bench_png_decode,
    bench_webp_decode,
    bench_gif_decode,
    bench_jpeg_encode,
    bench_jpeg_encode_mozjpegrs,
    bench_png_encode,
    bench_webp_encode,
    bench_gif_encode,
);
