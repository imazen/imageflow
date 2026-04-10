//! Fuzz target: RIAPI URL query string parsing.
//!
//! Parses attacker-controlled query strings like "w=800&h=600&mode=crop"
//! through the full IR4 pipeline: query → Instructions → layout → Nodes.
//! Pure Rust, no C codecs — very fast.
#![no_main]

use libfuzzer_sys::fuzz_target;
use imageflow_riapi::ir4::{Ir4Command, Ir4Translate};

fuzz_target!(|data: &[u8]| {
    let Ok(query) = std::str::from_utf8(data) else { return; };

    let query = query.to_string();

    // Phase 1: parse query string into Instructions.
    let cmd = Ir4Command::QueryString(query.clone());
    if cmd.parse().is_err() { return; }

    // Phase 2: translate through the full pipeline (decode/encode
    // node insertion, layout computation, CommandString generation).
    let translate = Ir4Translate {
        i: Ir4Command::QueryString(query),
        decode_id: Some(0),
        encode_id: Some(1),
        watermarks: None,
    };
    let _ = translate.translate();
});
