//! Pipeline execution — the core of imageflow 4.
//!
//! Orchestrates zen crates to execute a sequence of image processing steps.
//! The pipeline:
//! 1. Plans geometry via zenlayout
//! 2. Negotiates pixel formats via zenpixels-convert
//! 3. Decodes via zencodecs
//! 4. Processes (resize, filter, compose) via zenresize + zen filters
//! 5. Encodes via zencodecs
//! 6. Handles JPEG lossless fast paths via zenjpeg
//! 7. Probes quality via zenjpeg::detect for match_source encoding

use crate::error::FlowError;
use crate::io::IoStore;
use imageflow_types::*;
use zenpixels::PixelBuffer;
use zenpixels_convert::PixelBufferConvertExt;

/// Execute a pipeline of steps against an I/O store.
pub fn execute(
    io: &mut IoStore,
    steps: &[Step],
    _security: &Option<SecurityLimits>,
) -> Result<BuildResult, FlowError> {
    // Walk the pipeline to find decode/encode pairs and processing steps
    let plan = analyze_pipeline(steps)?;

    let mut outputs = Vec::new();

    for segment in &plan.segments {
        let result = execute_segment(io, segment, steps)?;
        outputs.push(result);
    }

    Ok(BuildResult { outputs })
}

/// Probe an image for metadata without decoding pixels.
pub fn probe_image(data: &[u8]) -> Result<ImageInfo, FlowError> {
    let info =
        zencodecs::from_bytes(data).map_err(|e| FlowError::Codec(format!("probe failed: {e}")))?;

    Ok(ImageInfo {
        format: format!("{:?}", info.format).to_lowercase(),
        w: info.width,
        h: info.height,
        has_alpha: info.has_alpha,
        orientation: None,   // TODO: extract from EXIF
        color_profile: None, // TODO: extract ICC/CICP info
        has_ultrahdr: false, // TODO: detect UltraHDR
    })
}

// ─── Pipeline Analysis ─────────────────────────────────────────────────

/// A segment is a decode → [processing] → encode chain.
struct PipelineSegment {
    decode_step_idx: usize,
    encode_step_idx: usize,
    processing_range: std::ops::Range<usize>,
}

struct PipelinePlan {
    segments: Vec<PipelineSegment>,
}

/// Analyze pipeline steps to identify decode→encode segments.
fn analyze_pipeline(steps: &[Step]) -> Result<PipelinePlan, FlowError> {
    let mut segments = Vec::new();
    let mut current_decode: Option<usize> = None;

    for (i, step) in steps.iter().enumerate() {
        match step {
            Step::Decode(_) => {
                current_decode = Some(i);
            }
            Step::Encode(_) => {
                let decode_idx = current_decode.ok_or_else(|| {
                    FlowError::InvalidPipeline("encode without preceding decode".into())
                })?;
                segments.push(PipelineSegment {
                    decode_step_idx: decode_idx,
                    encode_step_idx: i,
                    processing_range: (decode_idx + 1)..i,
                });
            }
            _ => {}
        }
    }

    if segments.is_empty() {
        return Err(FlowError::InvalidPipeline(
            "pipeline must contain at least one decode→encode pair".into(),
        ));
    }

    Ok(PipelinePlan { segments })
}

// ─── Segment Execution ─────────────────────────────────────────────────

fn execute_segment(
    io: &mut IoStore,
    segment: &PipelineSegment,
    steps: &[Step],
) -> Result<EncodeResult, FlowError> {
    let decode_step = match &steps[segment.decode_step_idx] {
        Step::Decode(d) => d,
        _ => unreachable!(),
    };
    let encode_step = match &steps[segment.encode_step_idx] {
        Step::Encode(e) => e,
        _ => unreachable!(),
    };

    let source_data = io.get_input(decode_step.io_id)?;

    // Check for JPEG lossless fast path
    if encode_step.prefer_lossless_jpeg {
        if let Some(result) =
            try_jpeg_lossless(source_data, &steps[segment.processing_range.clone()], encode_step)?
        {
            io.write_output(encode_step.io_id, result.data)?;
            return Ok(result.encode_result);
        }
    }

    // Full decode → process → encode path
    let decoded = zencodecs::DecodeRequest::new(source_data)
        .decode()
        .map_err(|e| FlowError::Codec(format!("decode failed: {e}")))?;

    let source_info = decoded.info().clone();
    let mut pixels = decoded.into_buffer();
    let mut width = source_info.width;
    let mut height = source_info.height;

    // Process each step in the segment
    for step_idx in segment.processing_range.clone() {
        let step = &steps[step_idx];
        match step {
            Step::Constrain(constrain) => {
                let (new_pixels, new_w, new_h) =
                    execute_constrain(&pixels, width, height, constrain)?;
                pixels = new_pixels;
                width = new_w;
                height = new_h;
            }
            Step::Crop(crop) => {
                let (new_pixels, new_w, new_h) = execute_crop(&pixels, width, height, crop)?;
                pixels = new_pixels;
                width = new_w;
                height = new_h;
            }
            Step::FlipH => {
                flip_horizontal(&mut pixels, width, height);
            }
            Step::FlipV => {
                flip_vertical(&mut pixels, width, height);
            }
            Step::Rotate90 | Step::Rotate180 | Step::Rotate270 | Step::Transpose => {
                // TODO: implement rotation/transpose via pixel manipulation
            }
            Step::Orient(orient) => {
                // TODO: apply EXIF orientation
                let _ = orient;
            }
            Step::ColorFilter(_)
            | Step::ColorAdjust(_)
            | Step::ColorMatrix(_)
            | Step::Sharpen(_)
            | Step::Blur(_) => {
                // TODO: implement filters via zenfilters / manual Oklab ops
            }
            Step::ExpandCanvas(_) | Step::FillRect(_) | Step::RoundCorners(_) | Step::Region(_) => {
                // TODO: canvas operations
            }
            Step::DrawImage(_) | Step::Watermark(_) => {
                // TODO: composition operations
            }
            Step::CommandString(cmd) => {
                let (new_pixels, new_w, new_h) =
                    execute_command_string(&pixels, width, height, cmd)?;
                pixels = new_pixels;
                width = new_w;
                height = new_h;
            }
            Step::Decode(_) | Step::Encode(_) => {
                // These are handled at the segment level, not here
            }
        }
    }

    // Encode
    let (encoded_data, format_name, mime_type) =
        execute_encode(&pixels, width, height, encode_step, source_data)?;

    let byte_count = encoded_data.len() as u64;
    io.write_output(encode_step.io_id, encoded_data)?;

    Ok(EncodeResult {
        io_id: encode_step.io_id,
        format: format_name,
        mime_type,
        w: width,
        h: height,
        bytes: byte_count,
    })
}

// ─── Constrain (resize via zenlayout + zenresize) ──────────────────────

fn execute_constrain(
    pixels: &PixelBuffer,
    in_w: u32,
    in_h: u32,
    constrain: &ConstrainStep,
) -> Result<(PixelBuffer, u32, u32), FlowError> {
    // Use zenlayout to compute target dimensions
    let target_w = constrain.w.unwrap_or(in_w);
    let target_h = constrain.h.unwrap_or(in_h);

    let pipeline = zenlayout::Pipeline::new(in_w, in_h);

    // Map our constraint mode to zenlayout
    let zl_mode = match constrain.mode {
        ConstraintMode::Fit => zenlayout::ConstraintMode::Fit,
        ConstraintMode::Within => zenlayout::ConstraintMode::Within,
        ConstraintMode::FitCrop => zenlayout::ConstraintMode::FitCrop,
        ConstraintMode::WithinCrop => zenlayout::ConstraintMode::WithinCrop,
        ConstraintMode::FitPad => zenlayout::ConstraintMode::FitPad,
        ConstraintMode::WithinPad => zenlayout::ConstraintMode::WithinPad,
        ConstraintMode::PadWithin => zenlayout::ConstraintMode::PadWithin,
        ConstraintMode::Distort => zenlayout::ConstraintMode::Distort,
        ConstraintMode::AspectCrop => zenlayout::ConstraintMode::AspectCrop,
    };

    let pipeline = pipeline.constrain(zenlayout::Constraint::new(zl_mode, target_w, target_h));

    let (ideal, _request) = pipeline.plan().map_err(|e| FlowError::Layout(format!("{e:?}")))?;

    let out_w = ideal.layout.resize_to.width;
    let out_h = ideal.layout.resize_to.height;

    if out_w == in_w && out_h == in_h {
        // No resize needed — clone the buffer
        // No change needed — return a copy via rgba8 roundtrip
        let copy: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
        return Ok((copy.erase(), in_w, in_h));
    }

    // Determine resize filter
    let filter = constrain
        .hints
        .as_ref()
        .and_then(|h| h.filter)
        .map(map_filter)
        .unwrap_or(zenresize::Filter::Robidoux);

    // Convert to RGBA8 for resize
    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let img_ref = rgba.as_imgref();

    let config = zenresize::ResizeConfig::builder(in_w, in_h, out_w, out_h)
        .filter(filter)
        .format(zenresize::PixelDescriptor::RGBA8_SRGB)
        .build();

    let output = zenresize::resize_4ch(
        img_ref,
        out_w,
        out_h,
        zenresize::PixelDescriptor::RGBA8_SRGB,
        &config,
    );

    // Convert ImgVec<RGBA<u8>> back to PixelBuffer
    let out_buf = PixelBuffer::from_imgvec(output);
    Ok((out_buf.erase(), out_w, out_h))
}

fn map_filter(f: Filter) -> zenresize::Filter {
    match f {
        Filter::Robidoux => zenresize::Filter::Robidoux,
        Filter::RobidouxSharp => zenresize::Filter::RobidouxSharp,
        Filter::RobidouxFast => zenresize::Filter::RobidouxFast,
        Filter::Lanczos => zenresize::Filter::Lanczos,
        Filter::LanczosSharp => zenresize::Filter::LanczosSharp,
        Filter::Lanczos2 => zenresize::Filter::Lanczos2,
        Filter::Lanczos2Sharp => zenresize::Filter::Lanczos2Sharp,
        Filter::Ginseng => zenresize::Filter::Ginseng,
        Filter::GinsengSharp => zenresize::Filter::GinsengSharp,
        Filter::Mitchell => zenresize::Filter::Mitchell,
        Filter::CatmullRom => zenresize::Filter::CatmullRom,
        Filter::CubicBSpline => zenresize::Filter::CubicBSpline,
        Filter::Hermite => zenresize::Filter::Hermite,
        Filter::Triangle => zenresize::Filter::Triangle,
        Filter::Box => zenresize::Filter::Box,
        Filter::Fastest => zenresize::Filter::Fastest,
        Filter::Cubic => zenresize::Filter::Cubic,
        Filter::CubicSharp => zenresize::Filter::CubicSharp,
        Filter::CubicFast => zenresize::Filter::CubicFast,
    }
}

// ─── Crop ──────────────────────────────────────────────────────────────

fn execute_crop(
    pixels: &PixelBuffer,
    in_w: u32,
    in_h: u32,
    crop: &CropStep,
) -> Result<(PixelBuffer, u32, u32), FlowError> {
    let x1 = crop.x1.min(in_w);
    let y1 = crop.y1.min(in_h);
    let x2 = crop.x2.min(in_w).max(x1);
    let y2 = crop.y2.min(in_h).max(y1);
    let out_w = x2 - x1;
    let out_h = y2 - y1;

    if out_w == 0 || out_h == 0 {
        return Err(FlowError::InvalidPipeline("crop region is empty".into()));
    }

    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src = rgba.as_imgref();
    let stride = src.stride();
    let buf = src.buf();
    let mut out_pixels: Vec<rgb::RGBA<u8>> = Vec::with_capacity((out_w * out_h) as usize);

    for y in y1..y2 {
        let row_start = y as usize * stride + x1 as usize;
        let row_end = row_start + out_w as usize;
        out_pixels.extend_from_slice(&buf[row_start..row_end]);
    }

    let output = imgref::ImgVec::new(out_pixels, out_w as usize, out_h as usize);
    let out_buf = PixelBuffer::from_imgvec(output);
    Ok((out_buf.erase(), out_w, out_h))
}

// ─── Flip ──────────────────────────────────────────────────────────────

fn flip_horizontal(pixels: &mut PixelBuffer, width: u32, height: u32) {
    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let w = width as usize;
    let buf = img.buf_mut();
    for y in 0..height as usize {
        let start = y * stride;
        buf[start..start + w].reverse();
    }
    *pixels = rgba.erase();
}

fn flip_vertical(pixels: &mut PixelBuffer, width: u32, height: u32) {
    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let w = width as usize;
    let h = height as usize;
    let buf = img.buf_mut();
    for y in 0..h / 2 {
        let top_start = y * stride;
        let bot_start = (h - 1 - y) * stride;
        for x in 0..w {
            buf.swap(top_start + x, bot_start + x);
        }
    }
    *pixels = rgba.erase();
}

// ─── Command String (RIAPI) ────────────────────────────────────────────

fn execute_command_string(
    pixels: &PixelBuffer,
    in_w: u32,
    in_h: u32,
    cmd: &CommandStringStep,
) -> Result<(PixelBuffer, u32, u32), FlowError> {
    // Parse RIAPI query string via zenlayout
    let result = zenlayout::riapi::parse(&cmd.value);

    let pipeline = result
        .instructions
        .to_pipeline(in_w, in_h, None)
        .map_err(|e| FlowError::Layout(format!("RIAPI parse error: {e:?}")))?;

    let (ideal, _request) =
        pipeline.plan().map_err(|e| FlowError::Layout(format!("RIAPI layout error: {e:?}")))?;

    let out_w = ideal.layout.resize_to.width;
    let out_h = ideal.layout.resize_to.height;

    if out_w == in_w && out_h == in_h {
        // No change needed — return a copy via rgba8 roundtrip
        let copy: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
        return Ok((copy.erase(), in_w, in_h));
    }

    // Resize with default filter
    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let config = zenresize::ResizeConfig::builder(in_w, in_h, out_w, out_h)
        .filter(zenresize::Filter::Robidoux)
        .format(zenresize::PixelDescriptor::RGBA8_SRGB)
        .build();

    let output = zenresize::resize_4ch(
        rgba.as_imgref(),
        out_w,
        out_h,
        zenresize::PixelDescriptor::RGBA8_SRGB,
        &config,
    );

    let out_buf = PixelBuffer::from_imgvec(output);
    Ok((out_buf.erase(), out_w, out_h))
}

// ─── Encode ────────────────────────────────────────────────────────────

fn execute_encode(
    pixels: &PixelBuffer,
    _width: u32,
    _height: u32,
    encode: &EncodeStep,
    source_data: &[u8],
) -> Result<(Vec<u8>, String, String), FlowError> {
    let source_format = zencodecs::from_bytes(source_data).ok().map(|i| i.format);

    let format = match &encode.format {
        Some(OutputFormat::Jpeg) => zencodecs::ImageFormat::Jpeg,
        Some(OutputFormat::Png) => zencodecs::ImageFormat::Png,
        Some(OutputFormat::WebP) => zencodecs::ImageFormat::WebP,
        Some(OutputFormat::Gif) => zencodecs::ImageFormat::Gif,
        Some(OutputFormat::Avif) => zencodecs::ImageFormat::Avif,
        Some(OutputFormat::Jxl) => zencodecs::ImageFormat::Jxl,
        Some(OutputFormat::Keep) | None => source_format.unwrap_or(zencodecs::ImageFormat::Jpeg),
        Some(OutputFormat::Auto { .. }) => {
            // Auto-select based on alpha
            if pixels.has_alpha() {
                zencodecs::ImageFormat::WebP
            } else {
                zencodecs::ImageFormat::Jpeg
            }
        }
    };

    let quality = match &encode.quality {
        Some(QualityTarget::Quality(q)) => *q,
        Some(QualityTarget::MatchSource { tolerance, shrink_guarantee }) => {
            estimate_source_quality(source_data, *tolerance, *shrink_guarantee)
        }
        Some(QualityTarget::Lossless) => 100.0,
        Some(QualityTarget::Butteraugli(_)) => 85.0, // TODO: proper BA targeting
        Some(QualityTarget::Ssimulacra2(_)) => 85.0, // TODO: proper SSIM2 targeting
        None => 85.0,
    };

    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let img_ref = rgba.as_imgref();

    let encode_output = zencodecs::EncodeRequest::new(format)
        .with_quality(quality)
        .encode_rgba8(img_ref)
        .map_err(|e| FlowError::Codec(format!("encode failed: {e}")))?;

    let format_name = format!("{:?}", encode_output.format()).to_lowercase();
    let mime = encode_output.mime_type().to_string();
    let data = encode_output.into_vec();

    Ok((data, format_name, mime))
}

/// Estimate source quality and compute re-encode quality.
fn estimate_source_quality(
    source_data: &[u8],
    tolerance: Option<f32>,
    _shrink_guarantee: bool,
) -> f32 {
    // Try zenjpeg quality probe
    match zenjpeg::detect::probe(source_data) {
        Ok(probe) => {
            let tol = tolerance.unwrap_or(0.3);
            match probe.reencode_settings(tol) {
                Ok(settings) => settings.quality.to_internal(),
                Err(_) => probe.quality.value.min(95.0),
            }
        }
        Err(_) => 85.0, // Fallback for non-JPEG or probe failure
    }
}

// ─── JPEG Lossless Fast Path ───────────────────────────────────────────

struct LosslessResult {
    data: Vec<u8>,
    encode_result: EncodeResult,
}

/// Try to execute the pipeline via JPEG lossless transforms.
/// Returns None if the pipeline has operations that can't be done losslessly.
fn try_jpeg_lossless(
    source_data: &[u8],
    processing_steps: &[Step],
    encode: &EncodeStep,
) -> Result<Option<LosslessResult>, FlowError> {
    // Only works for JPEG → JPEG
    let source_info = zencodecs::from_bytes(source_data).ok();
    if source_info.as_ref().map(|i| i.format) != Some(zencodecs::ImageFormat::Jpeg) {
        return Ok(None);
    }

    let target_is_jpeg =
        matches!(encode.format, Some(OutputFormat::Jpeg) | Some(OutputFormat::Keep) | None);
    if !target_is_jpeg {
        return Ok(None);
    }

    // Check if all processing steps can be done losslessly
    let transform = match classify_lossless_steps(processing_steps) {
        Some(t) => t,
        None => return Ok(None),
    };

    // Execute lossless transform
    let config = zenjpeg::lossless::TransformConfig {
        transform,
        edge_handling: zenjpeg::lossless::EdgeHandling::TrimPartialBlocks,
    };

    let result = zenjpeg::lossless::transform(source_data, &config, enough::Unstoppable)
        .map_err(|e| FlowError::Codec(format!("lossless transform failed: {e}")))?;

    // Get dimensions from the result
    let result_info = zencodecs::from_bytes(&result)
        .map_err(|e| FlowError::Codec(format!("probe after lossless failed: {e}")))?;

    Ok(Some(LosslessResult {
        encode_result: EncodeResult {
            io_id: encode.io_id,
            format: "jpeg".into(),
            mime_type: "image/jpeg".into(),
            w: result_info.width,
            h: result_info.height,
            bytes: result.len() as u64,
        },
        data: result,
    }))
}

/// Classify processing steps as a single lossless JPEG transform, or None
/// if any step requires pixel decoding.
fn classify_lossless_steps(steps: &[Step]) -> Option<zenjpeg::lossless::LosslessTransform> {
    use zenjpeg::lossless::LosslessTransform;

    if steps.is_empty() {
        return Some(LosslessTransform::None);
    }

    // Only simple orientation steps can be lossless
    let mut combined = LosslessTransform::None;
    for step in steps {
        let step_transform = match step {
            Step::FlipH => LosslessTransform::FlipHorizontal,
            Step::FlipV => LosslessTransform::FlipVertical,
            Step::Rotate90 => LosslessTransform::Rotate90,
            Step::Rotate180 => LosslessTransform::Rotate180,
            Step::Rotate270 => LosslessTransform::Rotate270,
            Step::Transpose => LosslessTransform::Transpose,
            Step::Orient(OrientStep::Auto) => {
                // Auto-orient is handled by the lossless path via EXIF
                LosslessTransform::None // Will be resolved from EXIF
            }
            _ => return None, // Non-lossless operation
        };
        combined = compose_transforms(combined, step_transform);
    }
    Some(combined)
}

fn compose_transforms(
    a: zenjpeg::lossless::LosslessTransform,
    b: zenjpeg::lossless::LosslessTransform,
) -> zenjpeg::lossless::LosslessTransform {
    use zenjpeg::lossless::LosslessTransform::*;
    if matches!(b, None) {
        return a;
    }
    if matches!(a, None) {
        return b;
    }
    // For complex compositions, fall back to the last transform.
    // A proper implementation would use D4 group composition.
    b
}
