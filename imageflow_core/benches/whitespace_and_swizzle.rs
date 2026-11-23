use imageflow_core::graphics::bitmaps::{Bitmap, BitmapCompositing, ColorSpace};
use imageflow_core::graphics::whitespace::detect_content;
use imageflow_core::{Context, NamedDecoders};
use imageflow_types::{
    Build001GraphRecording, Color, ColorSrgb, Execute001, Framewise, Node, PixelFormat, PixelLayout,
};
use zenbench::prelude::*;

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &d in data {
        a = (a + d as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn make_textured_rgb_png(w: u32, h: u32) -> Vec<u8> {
    let row_bytes = (w as usize) * 3;
    let mut raw = Vec::with_capacity((row_bytes + 1) * (h as usize));
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            let fx = (x as f32) * 0.17;
            let fy = (y as f32) * 0.23;
            let r = (128.0 + 90.0 * fx.sin() * (fy * 0.8).cos() + ((x ^ y) & 31) as f32)
                .clamp(0.0, 255.0) as u8;
            let g = (120.0 + 80.0 * (fx * 1.3 + 0.5).cos() + ((x.wrapping_mul(5) ^ y) & 31) as f32)
                .clamp(0.0, 255.0) as u8;
            let b = (110.0 + 95.0 * (fx + fy).sin() + ((y.wrapping_mul(7)) & 31) as f32)
                .clamp(0.0, 255.0) as u8;
            raw.extend_from_slice(&[r, g, b]);
        }
    }
    assert!(raw.len() <= u16::MAX as usize);
    let mut z = vec![0x78, 0x01, 0x01];
    let len = raw.len() as u16;
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&(!len).to_le_bytes());
    z.extend_from_slice(&raw);
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    png_chunk(&mut png, b"IHDR", &ihdr);
    png_chunk(&mut png, b"IDAT", &z);
    png_chunk(&mut png, b"IEND", &[]);
    png
}

fn make_bordered_bitmap(w: u32, h: u32) -> Bitmap {
    let mut bitmap = Bitmap::create_u8(
        w,
        h,
        PixelLayout::BGRA,
        true,
        true,
        ColorSpace::StandardRGB,
        BitmapCompositing::ReplaceSelf,
    )
    .unwrap();
    {
        let mut win = bitmap.get_window_u8().unwrap();
        win.fill_rect(0, 0, w, h, &Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_string()))).unwrap();
        // Multiple contrasting tiles inside border so whitespace strips find edges cleanly.
        win.fill_rect(
            w / 4,
            h / 4,
            w / 2,
            h / 2,
            &Color::Srgb(ColorSrgb::Hex("1A4B8CFF".to_string())),
        )
        .unwrap();
        win.fill_rect(
            w / 2,
            h / 2,
            (w * 3) / 4,
            (h * 3) / 4,
            &Color::Srgb(ColorSrgb::Hex("C83E2BFF".to_string())),
        )
        .unwrap();
    }
    bitmap
}

fn bench_whitespace_and_swizzle(suite: &mut Suite) {
    let (w, h) = (1280u32, 960u32);
    let mut bordered = make_bordered_bitmap(w, h);
    let initial_crop = {
        let win = bordered.get_window_u8().unwrap();
        detect_content(&win, 20).unwrap()
    };
    assert!(initial_crop.x1 > 0 && initial_crop.x2 < w);

    suite.group("whitespace_detect_1280x960", |g| {
        g.throughput(Throughput::Elements((w as u64) * (h as u64)));
        g.throughput_unit("px");
        if cfg!(debug_assertions) || std::env::args().any(|arg| arg == "--test") {
            let c = g.config();
            c.min_rounds(1).max_rounds(1);
            c.warmup_time(std::time::Duration::ZERO);
            c.min_iterations = 1;
            c.max_iterations = 1;
        }
        g.bench("detect_content", move |b| {
            b.iter(|| {
                let win = bordered.get_window_u8().unwrap();
                zenbench::black_box(detect_content(&win, 20).unwrap())
            })
        });
    });

    #[cfg(feature = "zen-codecs")]
    {
        let png_rgb24: &'static [u8] =
            Box::leak(make_textured_rgb_png(128, 128).into_boxed_slice());
        suite.group("zen_decoder_rgb24_128x128", |g| {
            g.throughput(Throughput::Elements(128 * 128));
            g.throughput_unit("px");
            if cfg!(debug_assertions) || std::env::args().any(|arg| arg == "--test") {
                let c = g.config();
                c.min_rounds(1).max_rounds(1);
                c.warmup_time(std::time::Duration::ZERO);
                c.min_iterations = 1;
                c.max_iterations = 1;
            }
            g.bench("decode_zenpng_rgb24", move |b| {
                b.iter(|| {
                    let mut ctx = Context::create().unwrap();
                    ctx.enabled_codecs.prefer_decoder(NamedDecoders::ZenPngDecoder);
                    #[cfg(feature = "c-codecs")]
                    ctx.enabled_codecs.disable_decoder(NamedDecoders::LibPngRsDecoder);
                    ctx.enabled_codecs.disable_decoder(NamedDecoders::ImageRsPngDecoder);
                    ctx.add_input_vector(0, png_rgb24.to_vec()).unwrap();
                    let exec = Execute001 {
                        graph_recording: Some(Build001GraphRecording::off()),
                        security: None,
                        job_options: None,
                        framewise: Framewise::Steps(vec![Node::Decode {
                            io_id: 0,
                            commands: None,
                        }]),
                    };
                    zenbench::black_box(ctx.execute_1(exec).unwrap());
                })
            });
        });
    }

    // Exercise Bgr24 CreateCanvas -> CopyRectToCanvas -> BGRA32 swizzle if supported
    suite.group("copy_rect_bgr24_to_bgra_640x480", |g| {
        g.throughput(Throughput::Elements(640 * 480));
        g.throughput_unit("px");
        if cfg!(debug_assertions) || std::env::args().any(|arg| arg == "--test") {
            let c = g.config();
            c.min_rounds(1).max_rounds(1);
            c.warmup_time(std::time::Duration::ZERO);
            c.min_iterations = 1;
            c.max_iterations = 1;
        }
        let png_rgb24: &'static [u8] =
            Box::leak(make_textured_rgb_png(128, 128).into_boxed_slice());
        g.bench("trim_step_pipeline", move |b| {
            b.iter(|| {
                let mut ctx = Context::create().unwrap();
                ctx.add_input_vector(0, png_rgb24.to_vec()).unwrap();
                let exec = Execute001 {
                    graph_recording: Some(Build001GraphRecording::off()),
                    security: None,
                    job_options: None,
                    framewise: Framewise::Steps(vec![
                        Node::CreateCanvas {
                            format: PixelFormat::Bgra32,
                            w: 640,
                            h: 480,
                            color: Color::Srgb(ColorSrgb::Hex("FFFFFFFF".to_string())),
                        },
                        Node::FillRect {
                            x1: 100,
                            y1: 100,
                            x2: 500,
                            y2: 380,
                            color: Color::Srgb(ColorSrgb::Hex("2266AAFF".to_string())),
                        },
                        Node::CropWhitespace { threshold: 20, percent_padding: 0.0 },
                    ]),
                };
                zenbench::black_box(ctx.execute_1(exec).unwrap());
            })
        });
    });
}

zenbench::main!(bench_whitespace_and_swizzle);
