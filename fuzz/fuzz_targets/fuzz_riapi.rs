//! Fuzz target: RIAPI URL query string parsing.
//!
//! Parses attacker-controlled query strings like "w=800&h=600&mode=crop"
//! through the full IR4 pipeline: query → Instructions → layout → Nodes.
//! Pure Rust, no C codecs — extremely fast (~1M exec/s).
#![no_main]

use libfuzzer_sys::fuzz_target;
use imageflow_riapi::ir4::{Ir4Command, Ir4Translate};

fuzz_target!(|data: &[u8]| {
    let Ok(query) = std::str::from_utf8(data) else { return; };

    // Phase 1: parse query string into Instructions.
    // Ir4Command::QueryString internally builds a URL and calls parse_url.
    // Guard against the expect() on invalid URL construction.
    let cmd = Ir4Command::QueryString(query.to_string());
    let parse_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cmd.parse()
    }));
    let Ok(Ok(_)) = parse_result else { return; };

    // Phase 2: translate through the full pipeline (adds decode/encode nodes,
    // processes layout, generates CommandString nodes).
    let cmd2 = Ir4Command::QueryString(query.to_string());
    let translate = Ir4Translate {
        i: cmd2,
        decode_id: Some(0),
        encode_id: Some(1),
        watermarks: None,
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = translate.translate();
    }));
});
