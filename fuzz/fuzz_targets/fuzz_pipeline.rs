//! Fuzz target: decode + random processing + encode.
//!
//! Structured fuzzing with arbitrary pipeline steps: resize, crop,
//! flip, rotate, color filters, etc. Tests the graph engine and
//! processing nodes with untrusted input.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

use imageflow_types::{
    Color, ColorFilterSrgb, Constraint, ConstraintMode, EncoderPreset, ExecutionSecurity,
    Framewise, FrameSizeLimit, JobOptions, Node, QualityProfile,
};

/// A processing step the fuzzer can insert between decode and encode.
#[derive(Debug, Arbitrary)]
enum FuzzStep {
    FlipH,
    FlipV,
    Rotate90,
    Rotate180,
    Rotate270,
    Transpose,
    /// Resize within bounds. Dimensions are clamped to [1, 2048].
    Constrain { w: u16, h: u16 },
    /// Crop with arbitrary coordinates (will be clamped).
    Crop { x1: u16, y1: u16, x2: u16, y2: u16 },
    /// Resample to specific dimensions.
    Resample { w: u16, h: u16 },
    /// Expand canvas with padding.
    ExpandCanvas { left: u8, top: u8, right: u8, bottom: u8 },
    /// Color filter.
    GrayscaleNtsc,
    GrayscaleBt709,
    Sepia,
    Invert,
    /// Brightness adjustment (-1.0 to 1.0).
    Brightness { value: i8 },
    /// Contrast adjustment (-1.0 to 1.0).
    Contrast { value: i8 },
    /// Saturation adjustment (-1.0 to 1.0).
    Saturation { value: i8 },
}

/// Which output format to encode to.
#[derive(Debug, Arbitrary)]
enum FuzzOutputFormat {
    Jpeg,
    Png,
    WebP,
    Gif,
    Auto,
}

/// Structured fuzz input.
#[derive(Debug, Arbitrary)]
struct FuzzInput {
    /// Raw image bytes.
    image_data: Vec<u8>,
    /// Processing steps to apply (0-4 steps to keep runtime bounded).
    steps: Vec<FuzzStep>,
    /// Output format.
    output_format: FuzzOutputFormat,
}

fn fuzz_security() -> ExecutionSecurity {
    ExecutionSecurity {
        max_decode_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_frame_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
        max_encode_size: Some(FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 }),
    }
}

/// Convert a FuzzStep into an imageflow Node.
fn step_to_node(step: &FuzzStep) -> Node {
    match step {
        FuzzStep::FlipH => Node::FlipH,
        FuzzStep::FlipV => Node::FlipV,
        FuzzStep::Rotate90 => Node::Rotate90,
        FuzzStep::Rotate180 => Node::Rotate180,
        FuzzStep::Rotate270 => Node::Rotate270,
        FuzzStep::Transpose => Node::Transpose,
        FuzzStep::Constrain { w, h } => {
            let w = (*w).max(1).min(2048) as u32;
            let h = (*h).max(1).min(2048) as u32;
            Node::Constrain(Constraint {
                mode: ConstraintMode::Within,
                w: Some(w),
                h: Some(h),
                hints: None,
                gravity: None,
                canvas_color: None,
            })
        }
        FuzzStep::Crop { x1, y1, x2, y2 } => {
            // Ensure x2 > x1 and y2 > y1 (at least 1px).
            let x1 = *x1 as u32;
            let y1 = *y1 as u32;
            let x2 = (x1 + 1).max(*x2 as u32);
            let y2 = (y1 + 1).max(*y2 as u32);
            Node::Crop { x1, y1, x2, y2 }
        }
        FuzzStep::Resample { w, h } => {
            let w = (*w).max(1).min(2048) as u32;
            let h = (*h).max(1).min(2048) as u32;
            Node::Resample2D { w, h, hints: None }
        }
        FuzzStep::ExpandCanvas { left, top, right, bottom } => Node::ExpandCanvas {
            left: *left as u32,
            top: *top as u32,
            right: *right as u32,
            bottom: *bottom as u32,
            color: Color::Transparent,
        },
        FuzzStep::GrayscaleNtsc => Node::ColorFilterSrgb(ColorFilterSrgb::GrayscaleNtsc),
        FuzzStep::GrayscaleBt709 => Node::ColorFilterSrgb(ColorFilterSrgb::GrayscaleBt709),
        FuzzStep::Sepia => Node::ColorFilterSrgb(ColorFilterSrgb::Sepia),
        FuzzStep::Invert => Node::ColorFilterSrgb(ColorFilterSrgb::Invert),
        FuzzStep::Brightness { value } => {
            let v = (*value as f32) / 127.0; // normalize to roughly -1.0..1.0
            Node::ColorFilterSrgb(ColorFilterSrgb::Brightness(v))
        }
        FuzzStep::Contrast { value } => {
            let v = (*value as f32) / 127.0;
            Node::ColorFilterSrgb(ColorFilterSrgb::Contrast(v))
        }
        FuzzStep::Saturation { value } => {
            let v = (*value as f32) / 127.0;
            Node::ColorFilterSrgb(ColorFilterSrgb::Saturation(v))
        }
    }
}

fuzz_target!(|input: FuzzInput| {
    if input.image_data.len() < 8 {
        return;
    }

    // Limit pipeline depth to prevent excessive runtime.
    let max_steps = 4;
    let processing_steps: Vec<Node> =
        input.steps.iter().take(max_steps).map(step_to_node).collect();

    let preset = match input.output_format {
        FuzzOutputFormat::Jpeg => EncoderPreset::Mozjpeg {
            quality: Some(75),
            progressive: Some(false),
            matte: None,
        },
        FuzzOutputFormat::Png => EncoderPreset::Libpng {
            depth: None,
            matte: None,
            zlib_compression: None,
        },
        FuzzOutputFormat::WebP => EncoderPreset::WebPLossy { quality: 75.0 },
        FuzzOutputFormat::Gif => EncoderPreset::Gif,
        FuzzOutputFormat::Auto => EncoderPreset::Auto {
            quality_profile: QualityProfile::Medium,
            quality_profile_dpr: None,
            matte: None,
            lossless: None,
            allow: None,
        },
    };

    let mut nodes = Vec::with_capacity(processing_steps.len() + 2);
    nodes.push(Node::Decode { io_id: 0, commands: None });
    nodes.extend(processing_steps);
    nodes.push(Node::Encode { io_id: 1, preset });

    let steps = Framewise::Steps(nodes);

    let mut io_buffers = HashMap::new();
    io_buffers.insert(0, input.image_data);

    let security = fuzz_security();
    let job_options = JobOptions::default();

    let _ = zenpipe::imageflow_compat::execute::execute_framewise(
        &steps,
        &io_buffers,
        &security,
        &job_options,
    );
});
