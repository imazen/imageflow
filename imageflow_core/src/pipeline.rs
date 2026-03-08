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

// Intentional: .max().min() suppresses NaN; .clamp() propagates it.
#![allow(clippy::manual_clamp)]

use crate::error::FlowError;
use crate::io::IoStore;
use imageflow_types::*;
use zc::Orientation;
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

    Ok(imageflow_types::ImageInfo {
        format: format!("{:?}", info.format).to_lowercase(),
        w: info.width,
        h: info.height,
        has_alpha: info.has_alpha,
        orientation: if info.orientation.is_identity() {
            None
        } else {
            Some(info.orientation.exif_value() as u8)
        },
        color_profile: None, // TODO: extract ICC/CICP info
        has_ultrahdr: info.has_gain_map,
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
    let exif_orientation = source_info.orientation;
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
            Step::Rotate90 => {
                let (p, w, h) = rotate_90(&pixels, width, height);
                pixels = p;
                width = w;
                height = h;
            }
            Step::Rotate180 => {
                rotate_180(&mut pixels, width, height);
            }
            Step::Rotate270 => {
                let (p, w, h) = rotate_270(&pixels, width, height);
                pixels = p;
                width = w;
                height = h;
            }
            Step::Transpose => {
                let (p, w, h) = transpose_pixels(&pixels, width, height);
                pixels = p;
                width = w;
                height = h;
            }
            Step::Orient(orient) => {
                let exif = match orient {
                    OrientStep::Auto => exif_orientation,
                    OrientStep::Exif(n) => Orientation::from_exif(*n as u16),
                };
                apply_orientation(&mut pixels, &mut width, &mut height, exif);
            }
            Step::Region(region) => {
                let (new_pixels, new_w, new_h) = execute_region(&pixels, width, height, region)?;
                pixels = new_pixels;
                width = new_w;
                height = new_h;
            }
            Step::ExpandCanvas(expand) => {
                let (p, w, h) = execute_expand_canvas(&pixels, width, height, expand);
                pixels = p;
                width = w;
                height = h;
            }
            Step::FillRect(fill) => {
                execute_fill_rect(&mut pixels, width, height, fill);
            }
            Step::RoundCorners(rc) => {
                execute_round_corners(&mut pixels, width, height, rc);
            }
            Step::ColorFilter(filter) => {
                execute_color_filter(&mut pixels, width, height, filter);
            }
            Step::ColorAdjust(adj) => {
                execute_color_adjust(&mut pixels, width, height, adj);
            }
            Step::ColorMatrix(mat) => {
                execute_color_matrix(&mut pixels, width, height, mat);
            }
            Step::Sharpen(sharp) => {
                execute_sharpen(&mut pixels, width, height, sharp);
            }
            Step::Blur(blur_step) => {
                execute_blur(&mut pixels, width, height, blur_step);
            }
            Step::DrawImage(draw) => {
                let (new_pixels, new_w, new_h) =
                    execute_draw_image(&pixels, width, height, draw, io)?;
                pixels = new_pixels;
                width = new_w;
                height = new_h;
            }
            Step::Watermark(wm) => {
                let (new_pixels, new_w, new_h) = execute_watermark(&pixels, width, height, wm, io)?;
                pixels = new_pixels;
                width = new_w;
                height = new_h;
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
    let target_w = constrain.w.unwrap_or(in_w);
    let target_h = constrain.h.unwrap_or(in_h);

    // Map constraint mode
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

    // Build constraint with gravity and background
    let mut zl_constraint = zenlayout::Constraint::new(zl_mode, target_w, target_h);
    if let Some(gravity) = &constrain.gravity {
        zl_constraint = zl_constraint.gravity(map_gravity(gravity));
    }
    if let Some(bg) = &constrain.background {
        let c = color_to_rgba(bg);
        zl_constraint = zl_constraint.canvas_color(zenlayout::CanvasColor::Srgb {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        });
    }

    let pipeline = zenlayout::Pipeline::new(in_w, in_h).constrain(zl_constraint);
    let (ideal, _request) = pipeline.plan().map_err(|e| FlowError::Layout(format!("{e:?}")))?;

    // Use canvas dimensions — includes padding for FitPad/WithinPad
    let out_w = ideal.layout.canvas.width;
    let out_h = ideal.layout.canvas.height;

    if !ideal.layout.needs_resize() && !ideal.layout.needs_crop() && !ideal.layout.needs_padding() {
        let copy: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
        return Ok((copy.erase(), in_w, in_h));
    }

    let filter = constrain
        .hints
        .as_ref()
        .and_then(|h| h.filter)
        .map(map_filter)
        .unwrap_or(zenresize::Filter::Robidoux);

    // Convert to RGBA8 contiguous bytes for zenresize::execute
    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src_bytes = rgba.copy_to_contiguous_bytes();
    let desc = zenpixels::PixelDescriptor::RGBA8_SRGB;

    // zenresize::execute handles: source_crop → resize → canvas placement → padding
    let result_bytes = zenresize::execute(&src_bytes, &ideal, desc, filter);

    // Wrap result back into PixelBuffer
    let out_buf = zenpixels::PixelBuffer::from_vec(result_bytes, out_w, out_h, desc)
        .map_err(|e| FlowError::Layout(format!("buffer creation failed: {e}")))?;
    Ok((out_buf.erase(), out_w, out_h))
}

fn map_gravity(g: &Gravity) -> zenlayout::Gravity {
    match g {
        Gravity::Center => zenlayout::Gravity::Center,
        Gravity::TopLeft => zenlayout::Gravity::Percentage(0.0, 0.0),
        Gravity::Top => zenlayout::Gravity::Percentage(0.5, 0.0),
        Gravity::TopRight => zenlayout::Gravity::Percentage(1.0, 0.0),
        Gravity::Left => zenlayout::Gravity::Percentage(0.0, 0.5),
        Gravity::Right => zenlayout::Gravity::Percentage(1.0, 0.5),
        Gravity::BottomLeft => zenlayout::Gravity::Percentage(0.0, 1.0),
        Gravity::Bottom => zenlayout::Gravity::Percentage(0.5, 1.0),
        Gravity::BottomRight => zenlayout::Gravity::Percentage(1.0, 1.0),
    }
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
    let cropped = rgba.crop_copy(x1, y1, out_w, out_h);
    Ok((cropped.erase(), out_w, out_h))
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

// ─── Rotation ──────────────────────────────────────────────────────────

fn rotate_90(pixels: &PixelBuffer, in_w: u32, in_h: u32) -> (PixelBuffer, u32, u32) {
    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src = rgba.as_imgref();
    let stride = src.stride();
    let buf = src.buf();
    let out_w = in_h;
    let out_h = in_w;
    let mut out = vec![rgb::RGBA { r: 0, g: 0, b: 0, a: 0 }; (out_w * out_h) as usize];
    for y in 0..in_h as usize {
        for x in 0..in_w as usize {
            // Rotate 90° CW: (x, y) → (in_h - 1 - y, x)
            let dst_x = in_h as usize - 1 - y;
            let dst_y = x;
            out[dst_y * out_w as usize + dst_x] = buf[y * stride + x];
        }
    }
    let output = imgref::ImgVec::new(out, out_w as usize, out_h as usize);
    (PixelBuffer::from_imgvec(output).erase(), out_w, out_h)
}

fn rotate_180(pixels: &mut PixelBuffer, width: u32, height: u32) {
    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let w = width as usize;
    let h = height as usize;
    let buf = img.buf_mut();
    for y in 0..h {
        for x in 0..w {
            let from = y * stride + x;
            let to = (h - 1 - y) * stride + (w - 1 - x);
            if from < to {
                buf.swap(from, to);
            }
        }
    }
    *pixels = rgba.erase();
}

fn rotate_270(pixels: &PixelBuffer, in_w: u32, in_h: u32) -> (PixelBuffer, u32, u32) {
    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src = rgba.as_imgref();
    let stride = src.stride();
    let buf = src.buf();
    let out_w = in_h;
    let out_h = in_w;
    let mut out = vec![rgb::RGBA { r: 0, g: 0, b: 0, a: 0 }; (out_w * out_h) as usize];
    for y in 0..in_h as usize {
        for x in 0..in_w as usize {
            // Rotate 270° CW: (x, y) → (y, in_w - 1 - x)
            let dst_x = y;
            let dst_y = in_w as usize - 1 - x;
            out[dst_y * out_w as usize + dst_x] = buf[y * stride + x];
        }
    }
    let output = imgref::ImgVec::new(out, out_w as usize, out_h as usize);
    (PixelBuffer::from_imgvec(output).erase(), out_w, out_h)
}

fn transpose_pixels(pixels: &PixelBuffer, in_w: u32, in_h: u32) -> (PixelBuffer, u32, u32) {
    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src = rgba.as_imgref();
    let stride = src.stride();
    let buf = src.buf();
    let out_w = in_h;
    let out_h = in_w;
    let mut out = vec![rgb::RGBA { r: 0, g: 0, b: 0, a: 0 }; (out_w * out_h) as usize];
    for y in 0..in_h as usize {
        for x in 0..in_w as usize {
            out[x * out_w as usize + y] = buf[y * stride + x];
        }
    }
    let output = imgref::ImgVec::new(out, out_w as usize, out_h as usize);
    (PixelBuffer::from_imgvec(output).erase(), out_w, out_h)
}

// ─── EXIF Orientation ──────────────────────────────────────────────────

fn apply_orientation(
    pixels: &mut PixelBuffer,
    width: &mut u32,
    height: &mut u32,
    exif: Orientation,
) {
    match exif {
        Orientation::Normal => {}
        Orientation::FlipHorizontal => flip_horizontal(pixels, *width, *height),
        Orientation::Rotate180 => rotate_180(pixels, *width, *height),
        Orientation::FlipVertical => flip_vertical(pixels, *width, *height),
        Orientation::Transpose => {
            let (p, w, h) = transpose_pixels(pixels, *width, *height);
            *pixels = p;
            *width = w;
            *height = h;
        }
        Orientation::Rotate90 => {
            let (p, w, h) = rotate_90(pixels, *width, *height);
            *pixels = p;
            *width = w;
            *height = h;
        }
        Orientation::Transverse => {
            let (mut p, w, h) = transpose_pixels(pixels, *width, *height);
            rotate_180(&mut p, w, h);
            *pixels = p;
            *width = w;
            *height = h;
        }
        Orientation::Rotate270 => {
            let (p, w, h) = rotate_270(pixels, *width, *height);
            *pixels = p;
            *width = w;
            *height = h;
        }
        _ => {} // non_exhaustive
    }
}

// ─── Region (float-coordinate crop) ────────────────────────────────────

fn execute_region(
    pixels: &PixelBuffer,
    in_w: u32,
    in_h: u32,
    region: &RegionStep,
) -> Result<(PixelBuffer, u32, u32), FlowError> {
    let to_px = |v: f64, dim: u32| -> u32 {
        if v <= 1.0 {
            (v * dim as f64) as u32
        } else {
            v as u32
        }
    };
    let crop = CropStep {
        x1: to_px(region.x1, in_w).min(in_w),
        y1: to_px(region.y1, in_h).min(in_h),
        x2: to_px(region.x2, in_w).min(in_w),
        y2: to_px(region.y2, in_h).min(in_h),
    };
    execute_crop(pixels, in_w, in_h, &crop)
}

// ─── Canvas Operations ─────────────────────────────────────────────────

fn execute_expand_canvas(
    pixels: &PixelBuffer,
    in_w: u32,
    in_h: u32,
    expand: &ExpandCanvasStep,
) -> (PixelBuffer, u32, u32) {
    let out_w = in_w + expand.left + expand.right;
    let out_h = in_h + expand.top + expand.bottom;
    let bg = color_to_rgba(&expand.color);

    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src = rgba.as_imgref();
    let stride = src.stride();
    let src_buf = src.buf();

    let mut out = vec![bg; (out_w * out_h) as usize];
    for y in 0..in_h as usize {
        let src_start = y * stride;
        let dst_start = (y + expand.top as usize) * out_w as usize + expand.left as usize;
        out[dst_start..dst_start + in_w as usize]
            .copy_from_slice(&src_buf[src_start..src_start + in_w as usize]);
    }

    let output = imgref::ImgVec::new(out, out_w as usize, out_h as usize);
    (PixelBuffer::from_imgvec(output).erase(), out_w, out_h)
}

fn execute_fill_rect(pixels: &mut PixelBuffer, width: u32, height: u32, fill: &FillRectStep) {
    let color = color_to_rgba(&fill.color);
    let x1 = fill.x1.min(width) as usize;
    let y1 = fill.y1.min(height) as usize;
    let x2 = fill.x2.min(width) as usize;
    let y2 = fill.y2.min(height) as usize;

    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let buf = img.buf_mut();
    for y in y1..y2 {
        for x in x1..x2 {
            buf[y * stride + x] = color;
        }
    }
    *pixels = rgba.erase();
}

fn execute_round_corners(pixels: &mut PixelBuffer, width: u32, height: u32, rc: &RoundCornersStep) {
    let w = width as f32;
    let h = height as f32;
    let min_dim = w.min(h);

    let radius = match &rc.mode {
        RoundCornersMode::Pixels(px) => *px,
        RoundCornersMode::Percent(pct) => min_dim * pct / 100.0,
        RoundCornersMode::Circle => min_dim / 2.0,
        RoundCornersMode::Custom(radii) => radii[0],
    };
    let radii = match &rc.mode {
        RoundCornersMode::Custom(r) => *r,
        _ => [radius, radius, radius, radius],
    };
    let bg =
        rc.background.as_ref().map(color_to_rgba).unwrap_or(rgb::RGBA { r: 0, g: 0, b: 0, a: 0 });

    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let buf = img.buf_mut();
    let w_u = width as usize;
    let h_u = height as usize;

    for y in 0..h_u {
        for x in 0..w_u {
            // radii: [TL, TR, BR, BL]
            let (cr, cx, cy) = if (x as f32) < radii[0] && (y as f32) < radii[0] {
                (radii[0], radii[0], radii[0])
            } else if x as f32 >= w - radii[1] && (y as f32) < radii[1] {
                (radii[1], w - radii[1], radii[1])
            } else if x as f32 >= w - radii[2] && y as f32 >= h - radii[2] {
                (radii[2], w - radii[2], h - radii[2])
            } else if (x as f32) < radii[3] && y as f32 >= h - radii[3] {
                (radii[3], radii[3], h - radii[3])
            } else {
                continue;
            };
            if cr <= 0.0 {
                continue;
            }
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > cr {
                buf[y * stride + x] = bg;
            } else if dist > cr - 1.0 {
                let alpha = (cr - dist).max(0.0).min(1.0);
                let p = &mut buf[y * stride + x];
                p.r = (p.r as f32 * alpha + bg.r as f32 * (1.0 - alpha)) as u8;
                p.g = (p.g as f32 * alpha + bg.g as f32 * (1.0 - alpha)) as u8;
                p.b = (p.b as f32 * alpha + bg.b as f32 * (1.0 - alpha)) as u8;
                p.a = (p.a as f32 * alpha + bg.a as f32 * (1.0 - alpha)) as u8;
            }
        }
    }
    *pixels = rgba.erase();
}

// ─── Color Filters ─────────────────────────────────────────────────────

fn execute_color_filter(
    pixels: &mut PixelBuffer,
    width: u32,
    height: u32,
    filter: &ColorFilterStep,
) {
    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let w = width as usize;
    let h = height as usize;
    let buf = img.buf_mut();

    match filter {
        ColorFilterStep::GrayscaleBt709 => {
            for y in 0..h {
                for x in 0..w {
                    let p = &mut buf[y * stride + x];
                    let lum =
                        (0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32) as u8;
                    p.r = lum;
                    p.g = lum;
                    p.b = lum;
                }
            }
        }
        ColorFilterStep::GrayscaleNtsc => {
            for y in 0..h {
                for x in 0..w {
                    let p = &mut buf[y * stride + x];
                    let lum = (0.299 * p.r as f32 + 0.587 * p.g as f32 + 0.114 * p.b as f32) as u8;
                    p.r = lum;
                    p.g = lum;
                    p.b = lum;
                }
            }
        }
        ColorFilterStep::GrayscaleFlat => {
            for y in 0..h {
                for x in 0..w {
                    let p = &mut buf[y * stride + x];
                    let lum = ((p.r as u16 + p.g as u16 + p.b as u16) / 3) as u8;
                    p.r = lum;
                    p.g = lum;
                    p.b = lum;
                }
            }
        }
        ColorFilterStep::Sepia => {
            for y in 0..h {
                for x in 0..w {
                    let p = &mut buf[y * stride + x];
                    let r = p.r as f32;
                    let g = p.g as f32;
                    let b = p.b as f32;
                    p.r = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0) as u8;
                    p.g = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0) as u8;
                    p.b = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0) as u8;
                }
            }
        }
        ColorFilterStep::Invert => {
            for y in 0..h {
                for x in 0..w {
                    let p = &mut buf[y * stride + x];
                    p.r = 255 - p.r;
                    p.g = 255 - p.g;
                    p.b = 255 - p.b;
                }
            }
        }
        ColorFilterStep::Alpha(a) => {
            let alpha_mul = (*a * 255.0) as u16;
            for y in 0..h {
                for x in 0..w {
                    let p = &mut buf[y * stride + x];
                    p.a = ((p.a as u16 * alpha_mul) / 255) as u8;
                }
            }
        }
    }
    *pixels = rgba.erase();
}

// ─── Color Adjust ──────────────────────────────────────────────────────

fn execute_color_adjust(pixels: &mut PixelBuffer, width: u32, height: u32, adj: &ColorAdjustStep) {
    let brightness = adj.brightness.unwrap_or(0.0);
    let contrast = adj.contrast.unwrap_or(0.0);
    let saturation = adj.saturation.unwrap_or(0.0);
    if brightness == 0.0 && contrast == 0.0 && saturation == 0.0 {
        return;
    }

    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let w = width as usize;
    let h = height as usize;
    let buf = img.buf_mut();

    for y in 0..h {
        for x in 0..w {
            let p = &mut buf[y * stride + x];
            let mut r = p.r as f32 / 255.0;
            let mut g = p.g as f32 / 255.0;
            let mut b = p.b as f32 / 255.0;

            // Brightness
            r += brightness;
            g += brightness;
            b += brightness;

            // Contrast: scale around 0.5
            r = (r - 0.5) * (1.0 + contrast) + 0.5;
            g = (g - 0.5) * (1.0 + contrast) + 0.5;
            b = (b - 0.5) * (1.0 + contrast) + 0.5;

            // Saturation: interpolate with BT.709 luminance
            if saturation != 0.0 {
                let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let s = 1.0 + saturation;
                r = lum + (r - lum) * s;
                g = lum + (g - lum) * s;
                b = lum + (b - lum) * s;
            }

            p.r = (r * 255.0).max(0.0).min(255.0) as u8;
            p.g = (g * 255.0).max(0.0).min(255.0) as u8;
            p.b = (b * 255.0).max(0.0).min(255.0) as u8;
        }
    }
    *pixels = rgba.erase();
}

// ─── Color Matrix ──────────────────────────────────────────────────────

fn execute_color_matrix(pixels: &mut PixelBuffer, width: u32, height: u32, mat: &ColorMatrixStep) {
    let m = &mat.matrix;
    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let w = width as usize;
    let h = height as usize;
    let buf = img.buf_mut();

    for y in 0..h {
        for x in 0..w {
            let p = &mut buf[y * stride + x];
            let r = p.r as f32 / 255.0;
            let g = p.g as f32 / 255.0;
            let b = p.b as f32 / 255.0;
            let a = p.a as f32 / 255.0;
            // [R',G',B',A',1] = M × [R,G,B,A,1] (row-major 5×5)
            let nr = m[0] * r + m[1] * g + m[2] * b + m[3] * a + m[4];
            let ng = m[5] * r + m[6] * g + m[7] * b + m[8] * a + m[9];
            let nb = m[10] * r + m[11] * g + m[12] * b + m[13] * a + m[14];
            let na = m[15] * r + m[16] * g + m[17] * b + m[18] * a + m[19];
            p.r = (nr * 255.0).max(0.0).min(255.0) as u8;
            p.g = (ng * 255.0).max(0.0).min(255.0) as u8;
            p.b = (nb * 255.0).max(0.0).min(255.0) as u8;
            p.a = (na * 255.0).max(0.0).min(255.0) as u8;
        }
    }
    *pixels = rgba.erase();
}

// ─── Sharpen (unsharp mask) ────────────────────────────────────────────

fn execute_sharpen(pixels: &mut PixelBuffer, width: u32, height: u32, sharp: &SharpenStep) {
    let amount = sharp.amount / 100.0;
    let w = width as usize;
    let h = height as usize;

    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src = rgba.as_imgref();
    let stride = src.stride();
    let src_buf = src.buf();

    let mut packed: Vec<rgb::RGBA<u8>> = Vec::with_capacity(w * h);
    for y in 0..h {
        packed.extend_from_slice(&src_buf[y * stride..y * stride + w]);
    }

    let blurred = box_blur_rgba(&packed, w, h, 2);

    let mut out = packed.clone();
    for i in 0..out.len() {
        let o = packed[i];
        let b = blurred[i];
        out[i] = rgb::RGBA {
            r: ((o.r as f32 + amount * (o.r as f32 - b.r as f32)).max(0.0).min(255.0)) as u8,
            g: ((o.g as f32 + amount * (o.g as f32 - b.g as f32)).max(0.0).min(255.0)) as u8,
            b: ((o.b as f32 + amount * (o.b as f32 - b.b as f32)).max(0.0).min(255.0)) as u8,
            a: o.a,
        };
    }

    let output = imgref::ImgVec::new(out, w, h);
    *pixels = PixelBuffer::from_imgvec(output).erase();
}

// ─── Blur (box blur × 3 ≈ Gaussian) ───────────────────────────────────

fn execute_blur(pixels: &mut PixelBuffer, width: u32, height: u32, blur_step: &BlurStep) {
    let radius = (blur_step.sigma * 2.0).ceil() as usize;
    if radius == 0 {
        return;
    }
    let w = width as usize;
    let h = height as usize;

    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src = rgba.as_imgref();
    let stride = src.stride();
    let src_buf = src.buf();

    let mut packed: Vec<rgb::RGBA<u8>> = Vec::with_capacity(w * h);
    for y in 0..h {
        packed.extend_from_slice(&src_buf[y * stride..y * stride + w]);
    }

    let pass_radius = (radius / 3).max(1);
    for _ in 0..3 {
        packed = box_blur_rgba(&packed, w, h, pass_radius);
    }

    let output = imgref::ImgVec::new(packed, w, h);
    *pixels = PixelBuffer::from_imgvec(output).erase();
}

/// Separable box blur on tightly-packed RGBA8 buffer.
fn box_blur_rgba(input: &[rgb::RGBA<u8>], w: usize, h: usize, radius: usize) -> Vec<rgb::RGBA<u8>> {
    let diameter = 2 * radius + 1;
    let inv = 1.0 / diameter as f32;

    // Horizontal pass
    let mut temp = vec![rgb::RGBA { r: 0, g: 0, b: 0, a: 0 }; w * h];
    for y in 0..h {
        for x in 0..w {
            let (mut rs, mut gs, mut bs, mut a_s) = (0u32, 0u32, 0u32, 0u32);
            for di in 0..diameter {
                let sx = (x as i64 + di as i64 - radius as i64).max(0).min(w as i64 - 1) as usize;
                let p = input[y * w + sx];
                rs += p.r as u32;
                gs += p.g as u32;
                bs += p.b as u32;
                a_s += p.a as u32;
            }
            temp[y * w + x] = rgb::RGBA {
                r: (rs as f32 * inv) as u8,
                g: (gs as f32 * inv) as u8,
                b: (bs as f32 * inv) as u8,
                a: (a_s as f32 * inv) as u8,
            };
        }
    }

    // Vertical pass
    let mut output = vec![rgb::RGBA { r: 0, g: 0, b: 0, a: 0 }; w * h];
    for x in 0..w {
        for y in 0..h {
            let (mut rs, mut gs, mut bs, mut a_s) = (0u32, 0u32, 0u32, 0u32);
            for di in 0..diameter {
                let sy = (y as i64 + di as i64 - radius as i64).max(0).min(h as i64 - 1) as usize;
                let p = temp[sy * w + x];
                rs += p.r as u32;
                gs += p.g as u32;
                bs += p.b as u32;
                a_s += p.a as u32;
            }
            output[y * w + x] = rgb::RGBA {
                r: (rs as f32 * inv) as u8,
                g: (gs as f32 * inv) as u8,
                b: (bs as f32 * inv) as u8,
                a: (a_s as f32 * inv) as u8,
            };
        }
    }

    output
}

// ─── Helpers ───────────────────────────────────────────────────────────

fn color_to_rgba(color: &Color) -> rgb::RGBA<u8> {
    match color {
        Color::Srgb { r, g, b, a } => rgb::RGBA { r: *r, g: *g, b: *b, a: *a },
        Color::Hex(hex) => parse_hex_color(hex),
    }
}

fn parse_hex_color(hex: &str) -> rgb::RGBA<u8> {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0);
            rgb::RGBA { r: r * 17, g: g * 17, b: b * 17, a: 255 }
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            rgb::RGBA { r, g, b, a: 255 }
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            rgb::RGBA { r, g, b, a }
        }
        _ => rgb::RGBA { r: 0, g: 0, b: 0, a: 255 },
    }
}

// ─── Composition (DrawImage / Watermark) ───────────────────────────────

/// Decode an overlay image from the IO store and return RGBA8 pixels + dimensions.
fn decode_overlay(io: &IoStore, io_id: i32) -> Result<(PixelBuffer, u32, u32), FlowError> {
    let overlay_data = io.get_input(io_id)?;
    let decoded = zencodecs::DecodeRequest::new(overlay_data)
        .decode()
        .map_err(|e| FlowError::Codec(format!("overlay decode failed: {e}")))?;

    let info = decoded.info().clone();
    let overlay_pixels = decoded.into_buffer();
    Ok((overlay_pixels, info.width, info.height))
}

/// Apply opacity to overlay pixels (multiply alpha channel).
fn apply_opacity(pixels: &mut PixelBuffer, width: u32, height: u32, opacity: f32) {
    if (opacity - 1.0).abs() < f32::EPSILON {
        return;
    }
    let mut rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let mut img = rgba.as_imgref_mut();
    let stride = img.stride();
    let w = width as usize;
    let h = height as usize;
    let buf = img.buf_mut();
    let opacity_u16 = (opacity * 255.0).max(0.0).min(255.0) as u16;
    for y in 0..h {
        for x in 0..w {
            let p = &mut buf[y * stride + x];
            p.a = ((p.a as u16 * opacity_u16) / 255) as u8;
        }
    }
    *pixels = rgba.erase();
}

/// Extract a sub-rectangle of RGBA8 canvas bytes, convert to premultiplied linear f32.
/// Out-of-bounds pixels are transparent black.
fn extract_region_premul_f32(
    canvas_bytes: &[u8],
    canvas_w: u32,
    canvas_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> Vec<f32> {
    let mut region_bytes = vec![0u8; (w * h * 4) as usize];
    for row in 0..h {
        let cy = y + row as i32;
        if cy < 0 || cy >= canvas_h as i32 {
            continue;
        }
        let cx_start = x.max(0);
        let cx_end = (x + w as i32).min(canvas_w as i32);
        if cx_start >= cx_end {
            continue;
        }
        let copy_w = (cx_end - cx_start) as usize;
        let src_off = (cy as usize * canvas_w as usize + cx_start as usize) * 4;
        let dst_x = (cx_start - x) as usize;
        let dst_off = (row as usize * w as usize + dst_x) * 4;
        region_bytes[dst_off..dst_off + copy_w * 4]
            .copy_from_slice(&canvas_bytes[src_off..src_off + copy_w * 4]);
    }

    let f32_count = (w * h * 4) as usize;
    let mut linear = vec![0.0f32; f32_count];
    linear_srgb::default::srgb_u8_to_linear_rgba_slice(&region_bytes, &mut linear);

    // Premultiply: RGB *= alpha
    for chunk in linear.chunks_exact_mut(4) {
        let a = chunk[3];
        chunk[0] *= a;
        chunk[1] *= a;
        chunk[2] *= a;
    }
    linear
}

/// Blit an RGBA8 region back into a larger RGBA8 canvas byte buffer.
/// Clips to canvas bounds.
fn blit_rgba8_region(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    region: &[u8],
    region_w: u32,
    region_h: u32,
    x: i32,
    y: i32,
) {
    for row in 0..region_h {
        let cy = y + row as i32;
        if cy < 0 || cy >= canvas_h as i32 {
            continue;
        }
        let cx_start = x.max(0);
        let cx_end = (x + region_w as i32).min(canvas_w as i32);
        if cx_start >= cx_end {
            continue;
        }
        let copy_w = (cx_end - cx_start) as usize;
        let src_x = (cx_start - x) as usize;
        let src_off = (row as usize * region_w as usize + src_x) * 4;
        let dst_off = (cy as usize * canvas_w as usize + cx_start as usize) * 4;
        canvas[dst_off..dst_off + copy_w * 4]
            .copy_from_slice(&region[src_off..src_off + copy_w * 4]);
    }
}

fn execute_draw_image(
    canvas: &PixelBuffer,
    canvas_w: u32,
    canvas_h: u32,
    draw: &DrawImageStep,
    io: &IoStore,
) -> Result<(PixelBuffer, u32, u32), FlowError> {
    let (overlay_pixels, overlay_w, overlay_h) = decode_overlay(io, draw.io_id)?;

    let target_w = draw.w.max(1);
    let target_h = draw.h.max(1);
    let target_size = zenlayout::Size::new(target_w, target_h);

    // Resize overlay to target dims, canvas = resize_to (no padding → no canvas fill)
    let plan = zenlayout::LayoutPlan::identity(zenlayout::Size::new(overlay_w, overlay_h))
        .with_resize_to(target_size)
        .with_canvas(target_size);

    // Get canvas as contiguous RGBA8 bytes
    let rgba: PixelBuffer<rgb::RGBA<u8>> = canvas.to_rgba8();
    let mut canvas_bytes = rgba.copy_to_contiguous_bytes();

    // Extract overlay target region from canvas as premul f32 background
    let bg_f32 = extract_region_premul_f32(
        &canvas_bytes,
        canvas_w,
        canvas_h,
        draw.x,
        draw.y,
        target_w,
        target_h,
    );
    let background = zenresize::SliceBackground::new(&bg_f32, target_w as usize * 4);

    let overlay_rgba: PixelBuffer<rgb::RGBA<u8>> = overlay_pixels.to_rgba8();
    let overlay_bytes = overlay_rgba.copy_to_contiguous_bytes();
    let desc = zenpixels::PixelDescriptor::RGBA8_SRGB;

    // Resize + composite overlay onto background region
    let result_bytes = zenresize::execute_layout_with_background(
        &overlay_bytes,
        overlay_w,
        overlay_h,
        &plan,
        desc,
        zenresize::Filter::Lanczos,
        background,
    )
    .map_err(|e| FlowError::Layout(format!("draw_image composite failed: {e}")))?;

    // Blit composited result back into canvas
    blit_rgba8_region(
        &mut canvas_bytes,
        canvas_w,
        canvas_h,
        &result_bytes,
        target_w,
        target_h,
        draw.x,
        draw.y,
    );

    let out_buf = zenpixels::PixelBuffer::from_vec(canvas_bytes, canvas_w, canvas_h, desc)
        .map_err(|e| FlowError::Layout(format!("buffer creation failed: {e}")))?;
    Ok((out_buf.erase(), canvas_w, canvas_h))
}

fn execute_watermark(
    canvas: &PixelBuffer,
    canvas_w: u32,
    canvas_h: u32,
    wm: &WatermarkStep,
    io: &IoStore,
) -> Result<(PixelBuffer, u32, u32), FlowError> {
    // Check minimum canvas size thresholds — skip silently if too small
    if wm.min_canvas_width.is_some_and(|min_w| canvas_w < min_w)
        || wm.min_canvas_height.is_some_and(|min_h| canvas_h < min_h)
    {
        let copy: PixelBuffer<rgb::RGBA<u8>> = canvas.to_rgba8();
        return Ok((copy.erase(), canvas_w, canvas_h));
    }

    let (mut overlay_pixels, overlay_w, overlay_h) = decode_overlay(io, wm.io_id)?;

    // Apply opacity
    apply_opacity(&mut overlay_pixels, overlay_w, overlay_h, wm.opacity);

    // Calculate fit box in pixels (defaults to full canvas)
    let fit = wm.fit_box.as_ref();
    let box_left = fit.map(|f| (f.left * canvas_w as f32) as i32).unwrap_or(0);
    let box_top = fit.map(|f| (f.top * canvas_h as f32) as i32).unwrap_or(0);
    let box_right = fit.map(|f| (f.right * canvas_w as f32) as u32).unwrap_or(canvas_w);
    let box_bottom = fit.map(|f| (f.bottom * canvas_h as f32) as u32).unwrap_or(canvas_h);
    let box_w = (box_right as i32 - box_left).max(1) as u32;
    let box_h = (box_bottom as i32 - box_top).max(1) as u32;

    // Delegate aspect-ratio-preserving fit to zenlayout (never upscale)
    let layout = zenlayout::Constraint::new(zenlayout::ConstraintMode::Within, box_w, box_h)
        .compute(overlay_w, overlay_h)
        .map_err(|e| FlowError::Layout(format!("{e:?}")))?;

    let target_w = layout.resize_to.width;
    let target_h = layout.resize_to.height;
    let target_size = zenlayout::Size::new(target_w, target_h);

    // Position overlay within fit box using gravity
    let (gx_frac, gy_frac) = gravity_to_fractions(&wm.gravity);
    let place_x = box_left + ((box_w - target_w) as f32 * gx_frac) as i32;
    let place_y = box_top + ((box_h - target_h) as f32 * gy_frac) as i32;

    // Resize overlay to target dims, canvas = resize_to (no padding → no canvas fill)
    let plan = zenlayout::LayoutPlan::identity(zenlayout::Size::new(overlay_w, overlay_h))
        .with_resize_to(target_size)
        .with_canvas(target_size);

    // Get canvas as contiguous RGBA8 bytes
    let rgba: PixelBuffer<rgb::RGBA<u8>> = canvas.to_rgba8();
    let mut canvas_bytes = rgba.copy_to_contiguous_bytes();

    // Extract overlay target region from canvas as premul f32 background
    let bg_f32 = extract_region_premul_f32(
        &canvas_bytes,
        canvas_w,
        canvas_h,
        place_x,
        place_y,
        target_w,
        target_h,
    );
    let background = zenresize::SliceBackground::new(&bg_f32, target_w as usize * 4);

    let filter = wm
        .hints
        .as_ref()
        .and_then(|h| h.filter)
        .map(map_filter)
        .unwrap_or(zenresize::Filter::Lanczos);

    let overlay_rgba: PixelBuffer<rgb::RGBA<u8>> = overlay_pixels.to_rgba8();
    let overlay_bytes = overlay_rgba.copy_to_contiguous_bytes();
    let desc = zenpixels::PixelDescriptor::RGBA8_SRGB;

    // Resize + composite overlay onto background region
    let result_bytes = zenresize::execute_layout_with_background(
        &overlay_bytes,
        overlay_w,
        overlay_h,
        &plan,
        desc,
        filter,
        background,
    )
    .map_err(|e| FlowError::Layout(format!("watermark composite failed: {e}")))?;

    // Blit composited result back into canvas
    blit_rgba8_region(
        &mut canvas_bytes,
        canvas_w,
        canvas_h,
        &result_bytes,
        target_w,
        target_h,
        place_x,
        place_y,
    );

    let out_buf = zenpixels::PixelBuffer::from_vec(canvas_bytes, canvas_w, canvas_h, desc)
        .map_err(|e| FlowError::Layout(format!("buffer creation failed: {e}")))?;
    Ok((out_buf.erase(), canvas_w, canvas_h))
}

fn gravity_to_fractions(g: &Gravity) -> (f32, f32) {
    match g {
        Gravity::TopLeft => (0.0, 0.0),
        Gravity::Top => (0.5, 0.0),
        Gravity::TopRight => (1.0, 0.0),
        Gravity::Left => (0.0, 0.5),
        Gravity::Center => (0.5, 0.5),
        Gravity::Right => (1.0, 0.5),
        Gravity::BottomLeft => (0.0, 1.0),
        Gravity::Bottom => (0.5, 1.0),
        Gravity::BottomRight => (1.0, 1.0),
    }
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

    let out_w = ideal.layout.canvas.width;
    let out_h = ideal.layout.canvas.height;

    if !ideal.layout.needs_resize() && !ideal.layout.needs_crop() && !ideal.layout.needs_padding() {
        let copy: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
        return Ok((copy.erase(), in_w, in_h));
    }

    let rgba: PixelBuffer<rgb::RGBA<u8>> = pixels.to_rgba8();
    let src_bytes = rgba.copy_to_contiguous_bytes();
    let desc = zenpixels::PixelDescriptor::RGBA8_SRGB;

    let result_bytes = zenresize::execute(&src_bytes, &ideal, desc, zenresize::Filter::Robidoux);

    let out_buf = zenpixels::PixelBuffer::from_vec(result_bytes, out_w, out_h, desc)
        .map_err(|e| FlowError::Layout(format!("buffer creation failed: {e}")))?;
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

    // Get EXIF orientation for Auto orient in lossless path
    let exif_orientation =
        source_info.as_ref().map(|i| i.orientation).unwrap_or(Orientation::Normal);

    // Check if all processing steps can be done losslessly
    let transform = match classify_lossless_steps(processing_steps, exif_orientation) {
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
fn classify_lossless_steps(
    steps: &[Step],
    exif_orientation: Orientation,
) -> Option<zenjpeg::lossless::LosslessTransform> {
    use zenjpeg::lossless::LosslessTransform;

    if steps.is_empty() {
        return Some(LosslessTransform::None);
    }

    let mut combined = LosslessTransform::None;
    for step in steps {
        let step_transform = match step {
            Step::FlipH => LosslessTransform::FlipHorizontal,
            Step::FlipV => LosslessTransform::FlipVertical,
            Step::Rotate90 => LosslessTransform::Rotate90,
            Step::Rotate180 => LosslessTransform::Rotate180,
            Step::Rotate270 => LosslessTransform::Rotate270,
            Step::Transpose => LosslessTransform::Transpose,
            Step::Orient(OrientStep::Auto) => orientation_to_lossless(exif_orientation),
            Step::Orient(OrientStep::Exif(n)) => {
                orientation_to_lossless(Orientation::from_exif(*n as u16))
            }
            _ => return None, // Non-lossless operation
        };
        combined = compose_transforms(combined, step_transform);
    }
    Some(combined)
}

fn orientation_to_lossless(o: Orientation) -> zenjpeg::lossless::LosslessTransform {
    use zenjpeg::lossless::LosslessTransform;
    match o {
        Orientation::Normal => LosslessTransform::None,
        Orientation::FlipHorizontal => LosslessTransform::FlipHorizontal,
        Orientation::Rotate180 => LosslessTransform::Rotate180,
        Orientation::FlipVertical => LosslessTransform::FlipVertical,
        Orientation::Transpose => LosslessTransform::Transpose,
        Orientation::Rotate90 => LosslessTransform::Rotate90,
        Orientation::Transverse => LosslessTransform::Transverse,
        Orientation::Rotate270 => LosslessTransform::Rotate270,
        _ => LosslessTransform::None, // non_exhaustive
    }
}

/// D4 dihedral group composition: result of applying `a` then `b`.
///
/// Each element is represented as (rotation_steps: 0-3, flip_h: bool),
/// meaning: rotate CW by rot*90°, then optionally flip horizontally.
fn compose_transforms(
    a: zenjpeg::lossless::LosslessTransform,
    b: zenjpeg::lossless::LosslessTransform,
) -> zenjpeg::lossless::LosslessTransform {
    let (ra, fa) = to_d4(a);
    let (rb, fb) = to_d4(b);
    let (rr, fr) =
        if !fb { ((ra + rb) % 4, fa) } else { (((rb as i8 - ra as i8).rem_euclid(4)) as u8, !fa) };
    from_d4(rr, fr)
}

fn to_d4(t: zenjpeg::lossless::LosslessTransform) -> (u8, bool) {
    use zenjpeg::lossless::LosslessTransform::*;
    match t {
        None => (0, false),
        Rotate90 => (1, false),
        Rotate180 => (2, false),
        Rotate270 => (3, false),
        FlipHorizontal => (0, true),
        Transpose => (1, true),
        FlipVertical => (2, true),
        Transverse => (3, true),
    }
}

fn from_d4(rot: u8, flip: bool) -> zenjpeg::lossless::LosslessTransform {
    use zenjpeg::lossless::LosslessTransform::*;
    match (rot % 4, flip) {
        (0, false) => None,
        (1, false) => Rotate90,
        (2, false) => Rotate180,
        (3, false) => Rotate270,
        (0, true) => FlipHorizontal,
        (1, true) => Transpose,
        (2, true) => FlipVertical,
        (3, true) => Transverse,
        _ => unreachable!(),
    }
}
