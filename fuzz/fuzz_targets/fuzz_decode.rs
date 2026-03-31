//! Fuzz target: decode arbitrary bytes through the zen pipeline.
//!
//! Feeds raw bytes as image input through a decode-only pipeline. Tests
//! format detection and all decoders (JPEG, PNG, WebP, GIF) for panics,
//! OOB reads, and unbounded allocations.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

use imageflow_types::{ExecutionSecurity, Framewise, FrameSizeLimit, JobOptions, Node};

/// Tight security limits for fuzzing: 4096x4096 max, ~64MB memory.
fn fuzz_security() -> ExecutionSecurity {
    ExecutionSecurity {
        max_decode_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_frame_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_encode_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
    }
}

fuzz_target!(|data: &[u8]| {
    // Skip trivially small inputs that can't be valid images.
    if data.len() < 8 {
        return;
    }

    let steps = Framewise::Steps(vec![Node::Decode { io_id: 0, commands: None }]);

    let mut io_buffers = HashMap::new();
    io_buffers.insert(0, data.to_vec());

    let security = fuzz_security();
    let job_options = JobOptions::default();

    // We don't care about the result — only that it doesn't panic or
    // trigger undefined behavior.
    let _ = zenpipe::imageflow_compat::execute::execute_framewise(
        &steps,
        &io_buffers,
        &security,
        &job_options,
    );
});
