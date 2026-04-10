//! Fuzz target for v2 backend transcode — decode through C codecs, re-encode.
//!
//! Uses structured fuzzing to vary the output format (JPEG/PNG/WebP/GIF).
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use imageflow_core::Context;
use imageflow_types as s;

#[derive(Debug, Arbitrary)]
struct TranscodeInput {
    format: u8,
    quality: u8,
    data: Vec<u8>,
}

fn limits() -> s::ExecutionSecurity {
    s::ExecutionSecurity {
        max_decode_size: Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_frame_size: Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_encode_size: Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
    }
}

fuzz_target!(|input: TranscodeInput| {
    if input.data.len() < 8 {
        return;
    }

    let Ok(mut ctx) = Context::create_can_panic() else { return; };
    ctx.configure_security(limits());
    if ctx.add_copied_input_buffer(0, &input.data).is_err() { return; }
    if ctx.add_output_buffer(1).is_err() { return; }

    let format = match input.format % 4 {
        0 => s::OutputImageFormat::Jpeg,
        1 => s::OutputImageFormat::Png,
        2 => s::OutputImageFormat::Webp,
        _ => s::OutputImageFormat::Gif,
    };
    let preset = s::EncoderPreset::Format {
        format,
        quality_profile: Some(s::QualityProfile::Percent((input.quality as f32).clamp(1.0, 100.0))),
        quality_profile_dpr: None,
        matte: None,
        lossless: None,
        allow: None,
        encoder_hints: None,
    };

    let execute = s::Execute001 {
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode { io_id: 1, preset },
        ]),
        graph_recording: None,
        security: Some(limits()),
        job_options: None,
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ = ctx.execute_1(execute);
    }));
});
