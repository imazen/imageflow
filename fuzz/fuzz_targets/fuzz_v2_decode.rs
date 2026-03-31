//! Fuzz target for the v2 backend — C-based codecs + graph engine.
//!
//! Tests mozjpeg/libpng/giflib/libwebp decode through the v2 graph engine.
//! Uses Execute001.security to set limits directly.
#![no_main]

use libfuzzer_sys::fuzz_target;
use imageflow_core::Context;
use imageflow_types as s;

fn limits() -> s::ExecutionSecurity {
    s::ExecutionSecurity {
        max_decode_size: Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_frame_size: Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_encode_size: Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }

    let Ok(mut ctx) = Context::create_can_panic() else { return; };
    ctx.configure_security(limits());
    if ctx.add_copied_input_buffer(0, data).is_err() { return; }
    if ctx.add_output_buffer(1).is_err() { return; }

    let execute = s::Execute001 {
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode { io_id: 1, preset: s::EncoderPreset::Libpng {
                depth: None, matte: None, zlib_compression: None,
            }},
        ]),
        graph_recording: None,
        security: Some(limits()),
        job_options: None,
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ = ctx.execute_1(execute);
    }));
});
