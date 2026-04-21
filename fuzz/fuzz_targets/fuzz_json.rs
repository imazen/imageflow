//! Fuzz target: JSON API command parsing and execution.
//!
//! Feeds arbitrary JSON to the v1/execute endpoint. Tests JSON
//! deserialization of Execute001, graph translation, and execution
//! with a pre-loaded 4x4 PNG as input. Catches panics in both the
//! JSON parsing layer and the graph engine.
#![no_main]

use libfuzzer_sys::fuzz_target;
use imageflow_core::Context;
use imageflow_types as s;

/// Minimal valid 4x4 RGBA PNG (78 bytes, no metadata).
const SEED_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
    0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04,
    0x08, 0x06, 0x00, 0x00, 0x00, 0xa9, 0xf1, 0x9e, 0x7e, 0x00, 0x00, 0x00,
    0x15, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0x6c, 0x70, 0x50, 0xf8,
    0xcf, 0x80, 0x04, 0x98, 0x18, 0xd0, 0x00, 0x61, 0x01, 0x00, 0x7b, 0x34,
    0x01, 0xe7, 0x7d, 0x1b, 0xc8, 0xf3, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn limits() -> s::ExecutionSecurity {
    let mut sec = s::ExecutionSecurity::sane_defaults();
    let limit = Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 });
    sec.max_decode_size = limit;
    sec.max_frame_size = limit;
    sec.max_encode_size = limit;
    sec
}

fuzz_target!(|data: &[u8]| {
    // Skip trivially small inputs that can't be valid JSON.
    if data.len() < 2 {
        return;
    }

    // Phase 1: try to deserialize as Execute001. This tests the serde
    // deserialization of the full type tree (Framewise, Node, EncoderPreset,
    // Constraint, Color, etc.) without executing anything.
    let parsed: s::Execute001 = match serde_json::from_slice(data) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Phase 2: if it parsed, try to execute it with a real Context.
    // Pre-load io_id 0 with a valid PNG so decode nodes have something
    // to work with. Add output buffer at io_id 1.
    let Ok(mut ctx) = Context::create_can_panic() else { return; };
    let _ = ctx.configure_security(limits());
    let _ = ctx.add_copied_input_buffer(0, SEED_PNG);
    let _ = ctx.add_output_buffer(1);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ = ctx.execute_1(parsed);
    }));
});
