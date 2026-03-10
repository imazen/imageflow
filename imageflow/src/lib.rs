//! High-level Rust API for imageflow image processing.
//!
//! Re-exports the core types and provides a convenient builder API.

pub use imageflow_core::{Context, FlowError};
pub use imageflow_types as types;

/// Process an image from bytes to bytes with a pipeline.
///
/// # Example
/// ```no_run
/// use imageflow_api::process;
/// use imageflow_types::*;
///
/// let input = std::fs::read("input.jpg").unwrap();
/// let output = process(&input, &[
///     Step::Decode(DecodeStep { io_id: 0, color: None, hints: None, ultrahdr: None }),
///     Step::Constrain(ConstrainStep {
///         mode: ConstraintMode::Within,
///         w: Some(800),
///         h: Some(600),
///         gravity: None,
///         background: None,
///         hints: None,
///     }),
///     Step::Encode(EncodeStep {
///         io_id: 1,
///         format: None,
///         quality: None,
///         color: None,
///         ultrahdr: None,
///         prefer_lossless_jpeg: false,
///         hints: None,
///         matte: None,
///     }),
/// ]).unwrap();
/// std::fs::write("output.jpg", output).unwrap();
/// ```
pub fn process(input: &[u8], steps: &[imageflow_types::Step]) -> Result<Vec<u8>, FlowError> {
    let ctx = Context::new();
    ctx.add_input_buffer(0, input)?;
    ctx.add_output_buffer(1)?;

    let request = imageflow_types::ExecuteRequest { pipeline: steps.to_vec(), security: None };

    let json = serde_json::to_vec(&request)
        .map_err(|e| FlowError::Internal(format!("serialize request: {e}")))?;

    let response = ctx.send_json("v2/execute", &json);
    if response.status_code != 200 {
        let msg = String::from_utf8_lossy(&response.response_json);
        return Err(FlowError::Internal(format!("pipeline failed: {msg}")));
    }

    ctx.get_output_buffer(1)
}
