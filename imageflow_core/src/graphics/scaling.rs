use crate::graphics::bitmaps::BitmapCompositing;
use crate::graphics::color::{self, ColorContext, WorkingFloatspace};
use crate::graphics::prelude::*;
use zenresize::{
    AlphaMode, Filter as ZenFilter, PixelDescriptor, ResizeConfig, StreamingResize,
    SolidBackground,
};

#[derive(Copy, Clone)]
pub struct ScaleAndRenderParams {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub sharpen_percent_goal: f32,
    pub interpolation_filter: crate::graphics::weights::Filter,
    pub scale_in_colorspace: WorkingFloatspace,
}

pub fn scale_and_render(
    input: BitmapWindowMut<u8>,
    mut canvas_without_crop: BitmapWindowMut<u8>,
    info: &ScaleAndRenderParams,
) -> Result<(), FlowError> {
    if info.h + info.y > canvas_without_crop.h() || info.w + info.x > canvas_without_crop.w() {
        return Err(nerror!(
            ErrorKind::InvalidArgument,
            "Destination rectangle for scale2d is out of bounds"
        ));
    }
    let mut cropped_canvas;
    if info.x == 0
        && info.y == 0
        && info.w == canvas_without_crop.w()
        && info.h == canvas_without_crop.h()
    {
        cropped_canvas = canvas_without_crop;
    } else {
        cropped_canvas = canvas_without_crop
            .window(info.x, info.y, info.x + info.w, info.y + info.h)
            .ok_or_else(|| nerror!(ErrorKind::InvalidArgument, "Crop window out of bounds"))?
    };

    if input.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::MethodNotImplemented));
    }
    if cropped_canvas.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::MethodNotImplemented));
    }

    let compositing = cropped_canvas.info().compose().clone();
    let alpha_meaningful = input.info().alpha_meaningful();
    let linear = info.scale_in_colorspace == WorkingFloatspace::LinearRGB;
    let zen_filter = map_filter(info.interpolation_filter);

    match compositing {
        BitmapCompositing::ReplaceSelf => {
            resize_to_canvas(&input, &mut cropped_canvas, info, zen_filter, linear, alpha_meaningful)?;
        }
        BitmapCompositing::BlendWithMatte(ref color) => {
            let bgra = color.to_bgra8().unwrap_or(rgb::alt::BGRA8 { b: 0, g: 0, r: 0, a: 0 });
            resize_with_matte(
                &input,
                &mut cropped_canvas,
                info,
                zen_filter,
                linear,
                alpha_meaningful,
                bgra,
            )?;
        }
        BitmapCompositing::BlendWithSelf => {
            resize_and_composite(
                &input,
                &mut cropped_canvas,
                info,
                zen_filter,
                linear,
                alpha_meaningful,
            )?;
        }
    }
    Ok(())
}

/// Direct u8→u8 resize, writing output rows straight to the canvas.
fn resize_to_canvas(
    input: &BitmapWindowMut<u8>,
    canvas: &mut BitmapWindowMut<u8>,
    info: &ScaleAndRenderParams,
    filter: ZenFilter,
    linear: bool,
    alpha_meaningful: bool,
) -> Result<(), FlowError> {
    let desc = pixel_desc(alpha_meaningful);
    let mut builder = ResizeConfig::builder(input.w(), input.h(), info.w, info.h)
        .filter(filter)
        .format(desc);
    if info.sharpen_percent_goal > 0.0 {
        builder = builder.resize_sharpen(info.sharpen_percent_goal);
    }
    if linear {
        builder = builder.linear();
    } else {
        builder = builder.srgb();
    }
    let config = builder.build();
    let mut stream = StreamingResize::new(&config);

    drain_resize_u8(&input, &mut stream, canvas, alpha_meaningful)
}

/// Resize with a solid matte color background (u8→u8).
fn resize_with_matte(
    input: &BitmapWindowMut<u8>,
    canvas: &mut BitmapWindowMut<u8>,
    info: &ScaleAndRenderParams,
    filter: ZenFilter,
    linear: bool,
    alpha_meaningful: bool,
    matte: rgb::alt::BGRA8,
) -> Result<(), FlowError> {
    let desc = pixel_desc(alpha_meaningful);
    let mut builder = ResizeConfig::builder(input.w(), input.h(), info.w, info.h)
        .filter(filter)
        .format(desc);
    if info.sharpen_percent_goal > 0.0 {
        builder = builder.resize_sharpen(info.sharpen_percent_goal);
    }
    if linear {
        builder = builder.linear();
    } else {
        builder = builder.srgb();
    }
    let config = builder.build();

    // SolidBackground takes (r, g, b, a) positional args. Since our data is BGRA,
    // pass B as the first arg so pixel[0] = linear(B), matching the BGRA channel order.
    let bg = SolidBackground::from_srgb_u8(matte.b, matte.g, matte.r, matte.a, desc);
    let mut stream = StreamingResize::with_background(&config, bg)
        .map_err(|e| nerror!(ErrorKind::InvalidState, "Composite error: {}", e))?;

    drain_resize_u8(&input, &mut stream, canvas, alpha_meaningful)
}

/// Resize to premultiplied linear f32, then composite over existing canvas.
fn resize_and_composite(
    input: &BitmapWindowMut<u8>,
    canvas: &mut BitmapWindowMut<u8>,
    info: &ScaleAndRenderParams,
    filter: ZenFilter,
    linear: bool,
    alpha_meaningful: bool,
) -> Result<(), FlowError> {
    let in_desc = pixel_desc(alpha_meaningful);
    // Output premultiplied linear f32 so we can composite in linear space
    let out_desc = if alpha_meaningful {
        PixelDescriptor::RGBAF32_LINEAR.with_alpha(Some(AlphaMode::Premultiplied))
    } else {
        // No meaningful alpha → treat as 4-channel without alpha semantics
        PixelDescriptor::RGBAF32_LINEAR.with_alpha(None)
    };

    let mut builder = ResizeConfig::builder(input.w(), input.h(), info.w, info.h)
        .filter(filter)
        .input(in_desc)
        .output(out_desc);
    if info.sharpen_percent_goal > 0.0 {
        builder = builder.resize_sharpen(info.sharpen_percent_goal);
    }
    if linear {
        builder = builder.linear();
    } else {
        builder = builder.srgb();
    }
    let config = builder.build();
    let mut stream = StreamingResize::new(&config);

    let cc = ColorContext::new(info.scale_in_colorspace, 0f32);

    let out_row_len = info.w as usize * 4;
    let mut out_y = 0usize;

    for y in 0..input.h() as usize {
        stream
            .push_row(input.row(y).unwrap())
            .map_err(|e| nerror!(ErrorKind::InvalidState, "push_row failed: {}", e))?;
        while let Some(f32_row) = stream.next_output_row_f32() {
            let canvas_row = canvas.row_mut(out_y).unwrap();
            composite_premul_f32_over_srgb_u8(&cc, f32_row, canvas_row, alpha_meaningful);
            out_y += 1;
        }
    }
    let remaining = stream.finish();
    for _ in 0..remaining {
        let f32_row = stream
            .next_output_row_f32()
            .ok_or_else(|| nerror!(ErrorKind::InvalidState, "finish promised rows but got none"))?;
        let canvas_row = canvas.row_mut(out_y).unwrap();
        composite_premul_f32_over_srgb_u8(&cc, f32_row, canvas_row, alpha_meaningful);
        out_y += 1;
    }
    Ok(())
}

/// Push all input rows and drain u8 output rows to the canvas.
fn drain_resize_u8<B: zenresize::Background>(
    input: &BitmapWindowMut<u8>,
    stream: &mut StreamingResize<B>,
    canvas: &mut BitmapWindowMut<u8>,
    alpha_meaningful: bool,
) -> Result<(), FlowError> {
    let out_row_len = canvas.w() as usize * 4;
    let mut out_y = 0usize;

    for y in 0..input.h() as usize {
        stream
            .push_row(input.row(y).unwrap())
            .map_err(|e| nerror!(ErrorKind::InvalidState, "push_row failed: {}", e))?;
        while let Some(row) = stream.next_output_row() {
            let dest = canvas.row_mut(out_y).unwrap();
            dest[..out_row_len].copy_from_slice(&row[..out_row_len]);
            if !alpha_meaningful {
                // Ensure alpha=255 when alpha isn't meaningful
                for pixel in dest[..out_row_len].chunks_exact_mut(4) {
                    pixel[3] = 255;
                }
            }
            out_y += 1;
        }
    }
    let remaining = stream.finish();
    for _ in 0..remaining {
        let row = stream
            .next_output_row()
            .ok_or_else(|| nerror!(ErrorKind::InvalidState, "finish promised rows but got none"))?;
        let dest = canvas.row_mut(out_y).unwrap();
        dest[..out_row_len].copy_from_slice(&row[..out_row_len]);
        if !alpha_meaningful {
            for pixel in dest[..out_row_len].chunks_exact_mut(4) {
                pixel[3] = 255;
            }
        }
        out_y += 1;
    }
    Ok(())
}

/// Composite a premultiplied linear f32 row (BGRA order) over a sRGB u8 canvas row.
fn composite_premul_f32_over_srgb_u8(
    cc: &ColorContext,
    src: &[f32],
    canvas: &mut [u8],
    alpha_meaningful: bool,
) {
    let dest_alpha_coeff = if alpha_meaningful { 1.0f32 / 255.0f32 } else { 0.0f32 };
    let dest_alpha_offset = if alpha_meaningful { 0.0f32 } else { 1.0f32 };

    for (src_px, canvas_px) in src.chunks_exact(4).zip(canvas.chunks_exact_mut(4)) {
        let src_a = src_px[3];
        if src_a > 0.994f32 || !alpha_meaningful {
            canvas_px[0] = cc.floatspace_to_srgb(src_px[0]);
            canvas_px[1] = cc.floatspace_to_srgb(src_px[1]);
            canvas_px[2] = cc.floatspace_to_srgb(src_px[2]);
            canvas_px[3] = 255;
        } else {
            let dest_a = canvas_px[3];
            let dest_coeff =
                (1.0f32 - src_a) * (dest_alpha_coeff * dest_a as i32 as f32 + dest_alpha_offset);
            let final_alpha = src_a + dest_coeff;
            canvas_px[0] = cc.floatspace_to_srgb(
                (src_px[0] + dest_coeff * cc.srgb_to_floatspace(canvas_px[0])) / final_alpha,
            );
            canvas_px[1] = cc.floatspace_to_srgb(
                (src_px[1] + dest_coeff * cc.srgb_to_floatspace(canvas_px[1])) / final_alpha,
            );
            canvas_px[2] = cc.floatspace_to_srgb(
                (src_px[2] + dest_coeff * cc.srgb_to_floatspace(canvas_px[2])) / final_alpha,
            );
            canvas_px[3] = uchar_clamp_ff(final_alpha * 255_f32);
        }
    }
}

fn uchar_clamp_ff(clr: f32) -> u8 {
    color::uchar_clamp_ff(clr)
}

/// Build the PixelDescriptor for BGRA u8 sRGB with the right alpha mode.
fn pixel_desc(alpha_meaningful: bool) -> PixelDescriptor {
    if alpha_meaningful {
        // Straight alpha — zenresize will premultiply internally before filtering
        PixelDescriptor::BGRA8_SRGB
    } else {
        // 4 channels, no meaningful alpha — skip premultiply/unpremultiply
        PixelDescriptor::BGRA8_SRGB.with_alpha(None)
    }
}

/// Map imageflow's internal Filter enum to zenresize's Filter enum.
/// Both share the same filter set (zenresize was extracted from imageflow).
fn map_filter(f: crate::graphics::weights::Filter) -> ZenFilter {
    use crate::graphics::weights::Filter;
    match f {
        Filter::RobidouxFast => ZenFilter::RobidouxFast,
        Filter::Robidoux => ZenFilter::Robidoux,
        Filter::RobidouxSharp => ZenFilter::RobidouxSharp,
        Filter::Ginseng => ZenFilter::Ginseng,
        Filter::GinsengSharp => ZenFilter::GinsengSharp,
        Filter::Lanczos => ZenFilter::Lanczos,
        Filter::LanczosSharp => ZenFilter::LanczosSharp,
        Filter::Lanczos2 => ZenFilter::Lanczos2,
        Filter::Lanczos2Sharp => ZenFilter::Lanczos2Sharp,
        Filter::CubicFast => ZenFilter::CubicFast,
        Filter::Cubic => ZenFilter::Cubic,
        Filter::CubicSharp => ZenFilter::CubicSharp,
        Filter::CatmullRom => ZenFilter::CatmullRom,
        Filter::Mitchell => ZenFilter::Mitchell,
        Filter::CubicBSpline => ZenFilter::CubicBSpline,
        Filter::Hermite => ZenFilter::Hermite,
        Filter::Jinc => ZenFilter::Jinc,
        Filter::RawLanczos3 => ZenFilter::RawLanczos3,
        Filter::RawLanczos3Sharp => ZenFilter::RawLanczos3Sharp,
        Filter::RawLanczos2 => ZenFilter::RawLanczos2,
        Filter::RawLanczos2Sharp => ZenFilter::RawLanczos2Sharp,
        Filter::Triangle => ZenFilter::Triangle,
        Filter::Linear => ZenFilter::Linear,
        Filter::Box => ZenFilter::Box,
        Filter::CatmullRomFast => ZenFilter::CatmullRomFast,
        Filter::CatmullRomFastSharp => ZenFilter::CatmullRomFastSharp,
        Filter::Fastest => ZenFilter::Fastest,
        Filter::MitchellFast => ZenFilter::MitchellFast,
        Filter::NCubic => ZenFilter::NCubic,
        Filter::NCubicSharp => ZenFilter::NCubicSharp,
        Filter::LegacyIDCTFilter => ZenFilter::LegacyIDCTFilter,
    }
}

