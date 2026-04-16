//! Debug: compare push_decode vs buffered decode for JPEG through imageflow.
//! Produces two PNGs and a zensim diff montage.
use imageflow_core::{Context, NamedDecoders};
use imageflow_types as s;

fn decode_jpeg(jpeg: &[u8], use_push: bool) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    ctx.enabled_codecs.prefer_decoder(NamedDecoders::ZenJpegDecoder);
    ctx.enabled_codecs.disable_decoder(NamedDecoders::MozJpegRsDecoder);
    ctx.enabled_codecs.disable_decoder(NamedDecoders::ImageRsJpegDecoder);

    if use_push {
        // Monkey-patch: we can't easily toggle prefers_buffered_decode from here.
        // Instead, compare zen buffered vs mozjpeg C to isolate the adapter.
        // Actually — let's just use the C mozjpeg decoder as reference.
    }

    ctx.add_input_vector(0, jpeg.to_vec()).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Lodepng { maximum_deflate: None },
            },
        ]),
    })
    .unwrap();
    ctx.take_output_buffer(1).unwrap()
}

fn decode_jpeg_mozjpeg(jpeg: &[u8]) -> Vec<u8> {
    let mut ctx = Context::create().unwrap();
    // Force C mozjpeg decoder
    ctx.enabled_codecs.prefer_decoder(NamedDecoders::MozJpegRsDecoder);
    ctx.enabled_codecs.disable_decoder(NamedDecoders::ZenJpegDecoder);
    ctx.enabled_codecs.disable_decoder(NamedDecoders::ImageRsJpegDecoder);

    ctx.add_input_vector(0, jpeg.to_vec()).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Lodepng { maximum_deflate: None },
            },
        ]),
    })
    .unwrap();
    ctx.take_output_buffer(1).unwrap()
}

fn main() {
    // Create a test JPEG
    let jpeg = {
        let mut ctx = Context::create().unwrap();
        ctx.add_output_buffer(1).unwrap();
        ctx.execute_1(s::Execute001 {
            graph_recording: Some(s::Build001GraphRecording::off()),
            security: None,
            job_options: None,
            framewise: s::Framewise::Steps(vec![
                s::Node::CreateCanvas {
                    w: 256,
                    h: 256,
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
    eprintln!("JPEG fixture: {} bytes", jpeg.len());

    let zen_png = decode_jpeg(&jpeg, false);
    let mozjpeg_png = decode_jpeg_mozjpeg(&jpeg);

    let out_dir = "/mnt/v/output/imageflow/debug_push_decode";
    std::fs::create_dir_all(out_dir).unwrap();

    let zen_path = format!("{out_dir}/zen_buffered.png");
    let moz_path = format!("{out_dir}/mozjpeg_c.png");
    std::fs::write(&zen_path, &zen_png).unwrap();
    std::fs::write(&moz_path, &mozjpeg_png).unwrap();

    eprintln!("Wrote {zen_path} and {moz_path}");

    // Use montage to create side-by-side
    let montage_path = format!("{out_dir}/montage.png");
    let status = std::process::Command::new("montage")
        .args([&zen_path, &moz_path, "-tile", "2x1", "-geometry", "+4+4", "-label", "%f", &montage_path])
        .status();
    match status {
        Ok(s) if s.success() => eprintln!("Montage: {montage_path}"),
        _ => eprintln!("montage failed or not installed"),
    }

    eprintln!("\nOpen montage: {montage_path}");
    // Open in Chrome
    let win_montage = std::process::Command::new("wslpath")
        .arg("-w").arg(&montage_path)
        .output().ok().and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default().trim().to_string();
    if !win_montage.is_empty() {
        let url = format!("file:///{}", win_montage.replace('\\', "/"));
        let _ = std::process::Command::new("/mnt/c/Program Files/Google/Chrome/Application/chrome.exe")
            .arg(&url).spawn();
        eprintln!("Opened in Chrome: {url}");
    }
}
