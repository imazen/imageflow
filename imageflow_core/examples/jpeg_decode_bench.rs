//! Standalone JPEG decode benchmark comparing three paths:
//!   1. mozjpeg-sys direct FFI (no imageflow, no zencodec)
//!   2. zenjpeg native Decoder API (no zencodec layer)
//!   3. zenjpeg via zencodec `Decode::decode` (buffered zencodec)
//!   4. zenjpeg via zencodec `push_decode` + BitmapRowSink-like sink (mirrors imageflow)
//!
//! Usage:
//!   cargo run --release --features c-codecs,zen-codecs \
//!       --example jpeg_decode_bench
//!
//! Sizes 256^2, 1024^2, 4096^2, 30 rounds each, checkerboard fixture matching
//! the imageflow bench_codecs bench.
#![allow(unused)]

use std::borrow::Cow;
use std::hint::black_box;
use std::time::Instant;

use imageflow_core::Context;
use imageflow_types as s;

// Build a JPEG fixture identical to the one used by bench_codecs.rs.
fn jpeg_fixture(w: u32, h: u32) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.add_output_buffer(1).unwrap();
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
    steps.push(s::Node::Encode {
        io_id: 1,
        preset: s::EncoderPreset::LibjpegTurbo {
            quality: Some(85),
            progressive: Some(false),
            optimize_huffman_coding: Some(false),
            matte: None,
        },
    });
    ctx.execute_1(s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(steps),
    })
    .unwrap();
    ctx.take_output_buffer(1).unwrap()
}

/// 1. mozjpeg-sys direct FFI, output RGBA via the `mozjpeg` safe crate
/// (which imageflow already depends on for version 0.10). This is
/// libjpeg-turbo + mozjpeg extensions, no imageflow wrapping.
fn decode_mozjpeg_ffi(data: &[u8]) -> (u32, u32, Vec<u8>) {
    use mozjpeg::Decompress;
    let d = Decompress::new_mem(data).unwrap();
    let w = d.width();
    let h = d.height();
    let mut dec = d.rgba().unwrap();
    let bytes = dec.read_scanlines::<u8>().unwrap();
    assert!(dec.finish().is_ok());
    (w as u32, h as u32, bytes)
}

/// 2. zenjpeg native Decoder API, no zencodec involvement. Decodes to packed RGBA.
fn decode_zenjpeg_native(data: &[u8]) -> (u32, u32, Vec<u8>) {
    use enough::Unstoppable;
    use zenjpeg::decoder::{Decoder, PixelFormat};
    let result = Decoder::new()
        .output_format(PixelFormat::Rgba)
        .decode(data, Unstoppable)
        .expect("zenjpeg native decode failed");
    let w = result.width();
    let h = result.height();
    let bytes = result.into_pixels_u8().unwrap();
    (w, h, bytes)
}

/// 3. zenjpeg via zencodec `Decode::decode` — buffered path.
fn decode_zenjpeg_zc_buffered(data: &[u8]) -> (u32, u32, Vec<u8>) {
    use zc::decode::DynDecoderConfig;
    use zenpixels::PixelDescriptor;
    let config = zenjpeg::JpegDecoderConfig::new()
        .cmyk_handling(zenjpeg::CmykHandling::Passthrough);
    let cfg_box: Box<dyn DynDecoderConfig> = Box::new(config);
    let job = cfg_box.dyn_job();
    let preferred = [
        PixelDescriptor::BGRA8_SRGB,
        PixelDescriptor::RGBA8_SRGB,
        PixelDescriptor::RGB8_SRGB,
        PixelDescriptor::GRAY8_SRGB,
    ];
    let dec = job.into_decoder(Cow::Borrowed(data), &preferred).expect("zc into_decoder");
    let out = dec.decode().expect("zc decode");
    let ps = out.pixels();
    let w = ps.width();
    let h = ps.rows();
    // Copy rows out so we don't just measure allocation of the PixelSlice.
    let bpp = ps.descriptor().bytes_per_pixel();
    let mut bytes = vec![0u8; w as usize * h as usize * bpp];
    let row_bytes = w as usize * bpp;
    for y in 0..h {
        bytes[y as usize * row_bytes..(y as usize + 1) * row_bytes]
            .copy_from_slice(&ps.row(y)[..row_bytes]);
    }
    (w, h, bytes)
}

/// 4. zenjpeg via zencodec `push_decode` with a direct-BGRA sink — mirrors the
/// imageflow BitmapRowSink fast path (4bpp direct writes).
fn decode_zenjpeg_zc_push(data: &[u8]) -> (u32, u32, Vec<u8>) {
    use zc::decode::{DecodeRowSink, DynDecoderConfig, SinkError};
    use zenpixels::{ChannelLayout, ChannelType, PixelDescriptor, PixelSliceMut};

    struct Sink<'a> {
        data: &'a mut [u8],
        stride: usize,
        width: u32,
        height: u32,
    }
    impl DecodeRowSink for Sink<'_> {
        fn begin(&mut self, w: u32, h: u32, desc: PixelDescriptor) -> Result<(), SinkError> {
            if desc.channel_type() != ChannelType::U8 || desc.bytes_per_pixel() != 4 {
                return Err(format!("need 4bpp U8, got {:?}", desc).into());
            }
            self.width = w;
            self.height = h;
            Ok(())
        }
        fn provide_next_buffer(
            &mut self,
            y: u32,
            height: u32,
            width: u32,
            descriptor: PixelDescriptor,
        ) -> Result<PixelSliceMut<'_>, SinkError> {
            let row_start = y as usize * self.stride;
            let row_bytes = width as usize * 4;
            let needed = if height > 0 {
                (height as usize - 1) * self.stride + row_bytes
            } else {
                0
            };
            let slice = &mut self.data[row_start..row_start + needed];
            PixelSliceMut::new(slice, width, height, self.stride, descriptor)
                .map_err(|e| -> SinkError { format!("{e}").into() })
        }
    }

    let config = zenjpeg::JpegDecoderConfig::new()
        .cmyk_handling(zenjpeg::CmykHandling::Passthrough);
    let cfg_box: Box<dyn DynDecoderConfig> = Box::new(config);
    let job = cfg_box.dyn_job();
    let preferred = [
        PixelDescriptor::BGRA8_SRGB,
        PixelDescriptor::RGBA8_SRGB,
        PixelDescriptor::RGB8_SRGB,
        PixelDescriptor::GRAY8_SRGB,
    ];

    // Probe size so we can allocate the destination before calling push_decode.
    let probe_job = cfg_box.dyn_job();
    let info = probe_job.probe(data).expect("probe");
    let w = info.width;
    let h = info.height;
    let stride = w as usize * 4;
    let mut dst = vec![0u8; stride * h as usize];
    let mut sink = Sink { data: &mut dst, stride, width: 0, height: 0 };
    job.push_decode(Cow::Borrowed(data), &mut sink, &preferred).expect("push_decode");
    (w, h, dst)
}

fn time<F: FnMut()>(mut f: F, rounds: usize) -> f64 {
    // Warm-up
    f();
    let start = Instant::now();
    for _ in 0..rounds {
        f();
    }
    let elapsed = start.elapsed().as_secs_f64();
    elapsed / rounds as f64
}

fn bench_size(w: u32, h: u32, rounds: usize) {
    let fixture = jpeg_fixture(w, h);
    println!(
        "\n=== {w}x{h}   fixture={} bytes   rounds={rounds} ===",
        fixture.len()
    );
    let pixels = (w as u64) * (h as u64);

    let t_moz = time(|| { let r = decode_mozjpeg_ffi(&fixture); black_box(r); }, rounds);
    let t_zn  = time(|| { let r = decode_zenjpeg_native(&fixture); black_box(r); }, rounds);
    let t_zcb = time(|| { let r = decode_zenjpeg_zc_buffered(&fixture); black_box(r); }, rounds);
    let t_zcp = time(|| { let r = decode_zenjpeg_zc_push(&fixture); black_box(r); }, rounds);

    let ops = |s: f64| (pixels as f64) / s / 1e9;
    println!("  mozjpeg-sys (rgba)           : {:8.2} ms/op  {:6.2} Gpix/s", t_moz*1e3, ops(t_moz));
    println!("  zenjpeg native (rgba)        : {:8.2} ms/op  {:6.2} Gpix/s  ({:.2}x mozjpeg)",
             t_zn*1e3, ops(t_zn), t_moz/t_zn);
    println!("  zenjpeg zc Decode::decode    : {:8.2} ms/op  {:6.2} Gpix/s  ({:.2}x mozjpeg)",
             t_zcb*1e3, ops(t_zcb), t_moz/t_zcb);
    println!("  zenjpeg zc push_decode(sink) : {:8.2} ms/op  {:6.2} Gpix/s  ({:.2}x mozjpeg)",
             t_zcp*1e3, ops(t_zcp), t_moz/t_zcp);
}

fn main() {
    // Dump fixtures for cross-testing with zenjpeg's own profile_decode_only
    for &(w, h) in &[(256, 256), (1024, 1024), (4096, 4096)] {
        let f = jpeg_fixture(w, h);
        let p = format!("/tmp/imageflow_fix_{w}x{h}.jpg");
        std::fs::write(&p, &f).unwrap();
        eprintln!("wrote {p} ({} bytes)", f.len());
    }

    println!("JPEG decode benchmark — mozjpeg-sys vs zenjpeg (native / zc buffered / zc push)");
    bench_size(256, 256, 200);
    bench_size(1024, 1024, 60);
    bench_size(4096, 4096, 10);
}
