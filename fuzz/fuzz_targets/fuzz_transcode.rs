//! Fuzz target: decode arbitrary bytes + re-encode in a random format.
//!
//! First 2 bytes select output format and quality, rest is image data.
//! Tests the full decode → pixel conversion → encode path through C codecs.
#![no_main]

use libfuzzer_sys::fuzz_target;
use imageflow_core::Context;
use imageflow_types as s;

fn limits() -> s::ExecutionSecurity {
    let mut sec = s::ExecutionSecurity::sane_defaults();
    let limit = Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 });
    sec.max_decode_size = limit;
    sec.max_frame_size = limit;
    sec.max_encode_size = limit;
    sec
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 10 {
        return;
    }

    let format_byte = data[0];
    let quality_byte = data[1];
    let image_data = &data[2..];

    let preset = match format_byte % 4 {
        0 => s::EncoderPreset::Mozjpeg {
            quality: Some(quality_byte.min(100)),
            progressive: Some(false),
            matte: None,
        },
        1 => s::EncoderPreset::Libpng {
            depth: None,
            matte: None,
            zlib_compression: None,
        },
        2 => s::EncoderPreset::WebPLossy {
            quality: (quality_byte as f32).min(100.0),
        },
        _ => s::EncoderPreset::Gif,
    };

    let Ok(mut ctx) = Context::create_can_panic() else { return; };
    ctx.configure_security(limits());
    if ctx.add_copied_input_buffer(0, image_data).is_err() { return; }
    if ctx.add_output_buffer(1).is_err() { return; }

    let execute = s::Execute001 {
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode { io_id: 1, preset },
        ]),
        graph_recording: None,
        security: None,
        job_options: None,
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ = ctx.execute_1(execute);
    }));
});
