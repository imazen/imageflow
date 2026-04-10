//! Fuzz target: decode arbitrary bytes through the v2 graph engine.
//!
//! Feeds raw bytes as image input through C-based codecs (mozjpeg, libpng,
//! giflib, libwebp). Tests format detection and all decoders for panics,
//! OOB reads, and unbounded allocations.
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
    if data.len() < 8 {
        return;
    }

    let Ok(mut ctx) = Context::create_can_panic() else { return; };
    ctx.configure_security(limits());
    if ctx.add_copied_input_buffer(0, data).is_err() { return; }

    let execute = s::Execute001 {
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
        ]),
        graph_recording: None,
        security: Some(limits()),
        job_options: None,
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ = ctx.execute_1(execute);
    }));
});
