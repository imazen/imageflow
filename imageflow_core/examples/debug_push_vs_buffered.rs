//! Direct comparison: zenjpeg push_decode (via zencodec sink) vs buffered decode.
//! Writes both outputs as raw BGRA, then compares byte-by-byte.
#![allow(unused)]
use std::borrow::Cow;

fn main() {
    // 1. Create a small JPEG via imageflow
    let jpeg = {
        use imageflow_core::Context;
        use imageflow_types as s;
        let mut ctx = Context::create().unwrap();
        ctx.add_output_buffer(1).unwrap();
        ctx.execute_1(s::Execute001 {
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: None,
            job_options: None,
            framewise: s::Framewise::Steps(vec![
                s::Node::CreateCanvas {
                    w: 64,
                    h: 64,
                    format: s::PixelFormat::Bgra32,
                    color: s::Color::Srgb(s::ColorSrgb::Hex("FF8040FF".into())),
                },
                s::Node::Encode {
                    io_id: 1,
                    preset: s::EncoderPreset::LibjpegTurbo {
                        quality: Some(85),
                        progressive: Some(false),
                        optimize_huffman_coding: None,
                        matte: None,
                    },
                },
            ]),
        })
        .unwrap();
        ctx.take_output_buffer(1).unwrap()
    };
    eprintln!("JPEG: {} bytes", jpeg.len());

    let preferred = [zenpixels::PixelDescriptor::BGRA8_SRGB];

    // 2. Buffered decode (into_decoder().decode()) via dyn dispatch
    let config: Box<dyn zc::decode::DynDecoderConfig> = Box::new(
        zenjpeg::JpegDecoderConfig::new().cmyk_handling(zenjpeg::CmykHandling::Passthrough),
    );
    let job = config.dyn_job();
    let dec = job.into_decoder(Cow::Borrowed(&jpeg), &preferred).unwrap();
    let output = dec.decode().unwrap();
    let ps = output.pixels();
    let buffered_w = ps.width();
    let buffered_h = ps.rows();
    let buffered_desc = ps.descriptor();
    let buffered_bytes: Vec<u8> = (0..buffered_h).flat_map(|y| ps.row(y).to_vec()).collect();
    eprintln!(
        "Buffered: {}x{} {:?} {} bytes",
        buffered_w,
        buffered_h,
        buffered_desc,
        buffered_bytes.len()
    );

    // 3. Push decode into a strided buffer (simulating imageflow bitmap)
    let stride = ((buffered_w as usize * 4 + 31) / 32) * 32; // SIMD align to 32 bytes
    let mut push_buf = vec![0xCCu8; stride * buffered_h as usize]; // fill with 0xCC sentinel
    {
        use zc::decode::{DecodeRowSink, SinkError};
        struct TestSink<'a> {
            data: &'a mut [u8],
            stride: usize,
            w: u32,
            h: u32,
        }
        impl DecodeRowSink for TestSink<'_> {
            fn begin(
                &mut self,
                w: u32,
                h: u32,
                _desc: zenpixels::PixelDescriptor,
            ) -> Result<(), SinkError> {
                self.w = w;
                self.h = h;
                Ok(())
            }
            fn provide_next_buffer(
                &mut self,
                y: u32,
                height: u32,
                width: u32,
                descriptor: zenpixels::PixelDescriptor,
            ) -> Result<zenpixels::PixelSliceMut<'_>, SinkError> {
                let row_start = y as usize * self.stride;
                let row_bytes = width as usize * 4;
                let needed =
                    if height > 0 { (height as usize - 1) * self.stride + row_bytes } else { 0 };
                let slice = &mut self.data[row_start..row_start + needed];
                zenpixels::PixelSliceMut::new(slice, width, height, self.stride, descriptor)
                    .map_err(|e| -> SinkError { format!("{e}").into() })
            }
        }
        let config2: Box<dyn zc::decode::DynDecoderConfig> = Box::new(
            zenjpeg::JpegDecoderConfig::new().cmyk_handling(zenjpeg::CmykHandling::Passthrough),
        );
        let job2 = config2.dyn_job();
        let mut sink = TestSink { data: &mut push_buf, stride, w: 0, h: 0 };
        match job2.push_decode(Cow::Borrowed(&jpeg), &mut sink, &preferred) {
            Ok(info) => {
                eprintln!("Push decode OK: {}x{} {:?}", info.width, info.height, info.native_format)
            }
            Err(e) => {
                eprintln!("Push decode error: {e}");
                return;
            }
        }
    }

    // 4. Compare row-by-row
    let row_bytes = buffered_w as usize * 4;
    let mut max_delta = 0u8;
    let mut diff_pixels = 0usize;
    let mut first_diff_y = None;
    for y in 0..buffered_h as usize {
        let buf_row = &buffered_bytes[y * row_bytes..(y + 1) * row_bytes];
        let push_row = &push_buf[y * stride..y * stride + row_bytes];
        for x in 0..row_bytes {
            let d = buf_row[x].abs_diff(push_row[x]);
            if d > 0 {
                diff_pixels += 1;
                max_delta = max_delta.max(d);
                if first_diff_y.is_none() {
                    first_diff_y = Some((y, x));
                    eprintln!(
                        "First diff at y={y} x={x}: buffered={:#04x} push={:#04x} delta={d}",
                        buf_row[x], push_row[x]
                    );
                }
            }
        }
    }
    let total = buffered_w as usize * buffered_h as usize * 4;
    eprintln!("\nComparison: {diff_pixels}/{total} bytes differ, max_delta={max_delta}");

    // Check for sentinel bytes (0xCC) in the push buffer's pixel region
    let mut sentinel_count = 0;
    for y in 0..buffered_h as usize {
        for x in 0..row_bytes {
            if push_buf[y * stride + x] == 0xCC {
                sentinel_count += 1;
            }
        }
    }
    if sentinel_count > 0 {
        eprintln!(
            "WARNING: {sentinel_count} sentinel bytes (0xCC) in push output — unwritten pixels!"
        );
    }

    // Print sentinel-containing rows
    eprintln!("\nRows with sentinel bytes:");
    for y in 0..buffered_h as usize {
        let row = &push_buf[y * stride..y * stride + row_bytes];
        let sentinel = row.iter().filter(|&&b| b == 0xCC).count();
        if sentinel > 0 {
            eprintln!(
                "  y={y}: {sentinel}/{row_bytes} sentinel bytes, first 32: {:02x?}",
                &row[..32.min(row_bytes)]
            );
        }
    }
}
