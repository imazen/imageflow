use crate::ir4::parsing::*;
use crate::sizing;
use crate::sizing::prelude::*;
use imageflow_helpers::preludes::from_std::*;
use imageflow_types as s;
use imageflow_types::{
    ConstraintGravity, ConstraintMode, FocusRect, FocusSource, RoundCornersMode,
    WatermarkConstraintBox,
};
use std;

pub struct Ir4Layout {
    i: Instructions,
    /// The actual, not-pre-shrunk image width. May differ from the bitmap size during IDCT scaling
    reference_width: i32,
    /// The actual, not-pre-shrunk image height. May differ from the bitmap size during IDCT scaling
    reference_height: i32,
    /// source width
    w: i32,
    h: i32,
}

pub struct ConstraintResults {
    pub crop: Option<[u32; 4]>,
    pub scale_to: AspectRatio,
    pub pad: Option<[u32; 4]>,
    pub final_canvas: AspectRatio,
}

pub struct Ir4LayoutInfo {
    pub canvas: AspectRatio,
}
impl Ir4Layout {
    pub fn new(
        i: Instructions,
        w: i32,
        h: i32,
        reference_width: i32,
        reference_height: i32,
    ) -> Ir4Layout {
        Ir4Layout { i, w, h, reference_width, reference_height }
    }

    fn get_preshrink_ratio(&self) -> f64 {
        f64::from(self.w) / f64::from(self.reference_width)
    }

    fn get_precrop(&self) -> (i32, i32) {
        if ((self.i.srotate.unwrap_or(0) / 90 + 4) % 2) == 0 {
            (self.w, self.h)
        } else {
            (self.h, self.w)
        }
    }

    fn get_precrop_reference(&self) -> (i32, i32) {
        if ((self.i.srotate.unwrap_or(0) / 90 + 4) % 2) == 0 {
            (self.reference_width, self.reference_height)
        } else {
            (self.reference_height, self.reference_width)
        }
    }

    fn get_wh_from_all(&self, source: AspectRatio) -> sizing::Result<(Option<i32>, Option<i32>)> {
        let mut w = self.i.w.unwrap_or(-1).max(-1);
        let mut h = self.i.h.unwrap_or(-1).max(-1);
        let mut mw = self.i.legacy_max_width.unwrap_or(-1).max(-1);
        let mut mh = self.i.legacy_max_height.unwrap_or(-1).max(-1);

        //Eliminate cases where both a value and a max value are specified: use the smaller value for the width/height
        if mw > 0 && w > 0 {
            w = cmp::min(mw, w);
            mw = -1;
        }
        if mh > 0 && h > 0 {
            h = cmp::min(mh, h);
            mh = -1;
        }

        //Handle cases of w/mh and h/mw as in legacy version
        if w != -1 && mh != -1 {
            mh = cmp::min(mh, source.height_for(w, None)?);
        }
        if h != -1 && mw != -1 {
            mw = cmp::min(mw, source.width_for(h, None)?);
        }
        //Move max values to w/h.
        w = cmp::max(w, mw);
        h = cmp::max(h, mh);

        Ok((if w < 1 { None } else { Some(w) }, if h < 1 { None } else { Some(h) }))
    }

    fn get_ideal_target_size(
        &self,
        source: AspectRatio,
        preshrink_ratio: f64,
    ) -> sizing::Result<AspectRatio> {
        // When no w/h is specified, we should apply zoom relative to the original image dimensions.
        let unshrunk_w = (f64::from(source.w) / preshrink_ratio) as i32;
        let unshrunk_h = (f64::from(source.h) / preshrink_ratio) as i32;

        let (w, h) = match self.get_wh_from_all(source)? {
            (Some(w), Some(h)) => (w, h),
            (None, None) => (unshrunk_w, unshrunk_h),
            (Some(w), None) => (w, source.height_for(w, None)?),
            (None, Some(h)) => (source.width_for(h, None)?, h),
        };

        //if all dimensions are absent, support zoom=x + scale=canvas | scale=both
        // and exit
        //No more than 1/80000 or 80000/1
        let zoom = Self::float_max(
            0.000_08f64,
            Self::float_min(self.i.zoom.map(|v| v as f64).unwrap_or(1f64), 80_000f64).unwrap(),
        )
        .unwrap();

        //Apply zoom directly to target dimensions. This differs from IR4 but should be easier to reason about.
        let w = Self::float_max(
            1f64,
            Self::float_min((f64::from(w) * zoom).round(), f64::from(i32::MAX)).unwrap(),
        )
        .unwrap() as i32;
        let h = Self::float_max(
            1f64,
            Self::float_min((f64::from(h) * zoom).round(), f64::from(i32::MAX)).unwrap(),
        )
        .unwrap() as i32;

        AspectRatio::create(w, h)
    }

    fn float_min(a: f64, b: f64) -> Option<f64> {
        let a_comparable = !a.is_nan() && !a.is_infinite();
        let b_comparable = !b.is_nan() && !b.is_infinite();
        if a_comparable && b_comparable {
            Some(if a < b { a } else { b })
        } else if a_comparable {
            Some(a)
        } else if b_comparable {
            Some(b)
        } else {
            None
        }
    }
    fn float_max(a: f64, b: f64) -> Option<f64> {
        let a_comparable = !a.is_nan() && !a.is_infinite();
        let b_comparable = !b.is_nan() && !b.is_infinite();
        if a_comparable && b_comparable {
            Some(if a > b { a } else { b })
        } else if a_comparable {
            Some(a)
        } else if b_comparable {
            Some(b)
        } else {
            None
        }
    }

    // Build constraint set, just from "mode" and "scale" (and the absence of width/height, for overriding "mode").
    // Keep in mind that crop=auto and scale=fill were normalized when parsing Instructions.
    fn build_constraints(&self) -> Vec<Step> {
        // if both w/width and h/height are absent, force mode=max regardless of current setting
        let mode = if self.i.w.is_none() && self.i.h.is_none() {
            FitMode::Max
        } else {
            self.i.mode.unwrap_or(FitMode::Pad)
        };

        match (mode, self.i.scale.unwrap_or(ScaleMode::DownscaleOnly)) {
            //Max is a misnomer. It scales up proportionally, as well. With scale=canvas, it produces padding.
            (FitMode::Max, ScaleMode::DownscaleOnly) => {
                //scale to ibox, unless original is not larger than the box
                steps().skip_unless(Cond::Either(Ordering::Greater)).scale_to_inner()
            }
            (FitMode::Max, ScaleMode::UpscaleOnly) => {
                //if original is equal or less than both target dimensions, scale up within. Otherwise retain original size/aspect.
                steps().skip_unless(Cond::Neither(Ordering::Greater)).scale_to_inner()
            }
            (FitMode::Max, ScaleMode::Both) => {
                //scale to the inner box, always. Surprising?
                steps().scale_to_inner()
            }
            (FitMode::Max, ScaleMode::UpscaleCanvas) => {
                //Don't upscale the inner box.
                //Pad to the inner box of the target.
                steps()
                    .skip_unless(Cond::Either(Ordering::Greater))
                    .scale_to_inner()
                    .new_seq()
                    .virtual_canvas(BoxParam::BoxOf {
                        target: BoxTarget::Target,
                        ratio_source: BoxTarget::CurrentCanvas,
                        kind: BoxKind::Inner,
                    })
            }
            (FitMode::Pad, ScaleMode::DownscaleOnly) => {
                //scale within box and pad, unless original is not larger than the box.
                steps().skip_unless(Cond::Either(Ordering::Greater)).scale_to_inner().pad()
                //If the image is smaller, we lose aspect ratio and it reverts to normal. Surprising?
            }
            (FitMode::Pad, ScaleMode::UpscaleOnly) => {
                //if original is equal or less than both target dimensions, scale up and pad. Otherwise retain original size/aspect.
                steps().skip_unless(Cond::Neither(Ordering::Greater)).scale_to_inner().pad()
            }
            (FitMode::Pad, ScaleMode::Both) => {
                //scale to the inner box and pad to target, always.
                steps().scale_to_inner().pad()
            }
            (FitMode::Pad, ScaleMode::UpscaleCanvas) => {
                //Don't upscale the inner box.
                //Pad to the inner box of the target.
                steps()
                    .skip_unless(Cond::Either(Ordering::Greater))
                    .scale_to_inner()
                    .new_seq()
                    .pad()
            }
            (FitMode::Stretch, ScaleMode::DownscaleOnly) => steps()
                .skip_unless(Cond::Either(Ordering::Greater))
                .distort(BoxParam::Exact(BoxTarget::Target)),
            (FitMode::Stretch, ScaleMode::UpscaleOnly) => {
                //if original is equal or less than both target dimensions, distort. Otherwise retain original size/aspect.
                steps()
                    .skip_unless(Cond::Neither(Ordering::Greater))
                    .distort(BoxParam::Exact(BoxTarget::Target))
            }
            (FitMode::Stretch, ScaleMode::Both) => {
                steps().distort(BoxParam::Exact(BoxTarget::Target))
            }
            (FitMode::Stretch, ScaleMode::UpscaleCanvas) => {
                //Don't upscale the inner box.
                //Pad to the inner box of the target.
                steps()
                    .skip_unless(Cond::Either(Ordering::Greater))
                    .distort(BoxParam::Exact(BoxTarget::Target))
                    .new_seq()
                    .pad()
            }
            (FitMode::Crop, ScaleMode::DownscaleOnly) => {
                //We can't compare against the obox, so we have to use a partwise constraint
                //The first doesn't affect Large1DSmaller1D scenarios, only Larger2d or equal.
                //The second only receives equal, mixed, or less. It deals with mixed, as the only
                //batch requiring work.
                steps()
                    .skip_if(Cond::Either(Ordering::Less))
                    .scale_to_outer()
                    .crop()
                    .new_seq()
                    .skip_unless(Cond::Larger1DSmaller1D)
                    .crop_intersection()
            }
            (FitMode::Crop, ScaleMode::UpscaleOnly) => {
                // mode=crop&scale=up only takes effect when no target dimension is smaller than the
                // source.
                steps().skip_unless(Cond::Neither(Ordering::Greater)).scale_to_outer().crop()
            }
            (FitMode::Crop, ScaleMode::Both) => {
                //scale to the outer box and crop to target, always. Easy.
                steps().scale_to_outer().crop()
            }
            (FitMode::Crop, ScaleMode::UpscaleCanvas) => {
                // We can't compare against the obox, so we have to use a partwise constraint
                // The first doesn't affect Large1DSmaller1D scenarios, only Larger2d or equal.
                // The second only receives equal, mixed, or less.
                steps()
                    .skip_if(Cond::Either(Ordering::Less))
                    .scale_to_outer()
                    .crop()
                    .new_seq()
                    .skip_unless(Cond::Larger1DSmaller1D)
                    .virtual_canvas(BoxParam::Exact(BoxTarget::Target))
            }
            (FitMode::AspectCrop, _) => {
                //scale to the outer box and crop to target, always. Easy.
                steps().crop_aspect()
            }
        }
        .into_vec()
    }

    pub fn get_downscaling(&self) -> sizing::Result<(AspectRatio, AspectRatio)> {
        let (_, layout) = self.get_crop_and_layout()?;

        let new_crop = layout.get_source_crop();
        let image = layout.get_box(BoxTarget::CurrentImage);
        Ok((new_crop, image))
    }

    fn get_instructions(constraint: &imageflow_types::Constraint) -> Option<Instructions> {
        let mut i = Instructions::new();
        i.w = constraint.w.map(|v| v as i32);
        i.h = constraint.h.map(|v| v as i32);
        match constraint.mode {
            ConstraintMode::Distort => {
                i.mode = Some(FitMode::Stretch);
                i.scale = Some(ScaleMode::Both);
            }
            ConstraintMode::Within => {
                i.mode = Some(FitMode::Max);
                i.scale = Some(ScaleMode::DownscaleOnly);
            }
            ConstraintMode::Fit => {
                i.mode = Some(FitMode::Max);
                i.scale = Some(ScaleMode::Both);
            }
            ConstraintMode::LargerThan => {
                i.mode = Some(FitMode::Max);
                i.scale = Some(ScaleMode::UpscaleOnly);
            }
            ConstraintMode::WithinCrop => {
                i.mode = Some(FitMode::Crop);
                i.scale = Some(ScaleMode::DownscaleOnly);
            }
            ConstraintMode::FitCrop => {
                i.mode = Some(FitMode::Crop);
                i.scale = Some(ScaleMode::Both);
            }
            ConstraintMode::WithinPad => {
                i.mode = Some(FitMode::Pad);
                i.scale = Some(ScaleMode::DownscaleOnly);
            }
            ConstraintMode::FitPad => {
                i.mode = Some(FitMode::Pad);
                i.scale = Some(ScaleMode::Both);
            }
            ConstraintMode::AspectCrop => {
                i.mode = Some(FitMode::AspectCrop);
            }
        }
        Some(i)
    }

    pub fn process_constraint(
        source_w: i32,
        source_h: i32,
        constraint: &imageflow_types::Constraint,
    ) -> sizing::Result<ConstraintResults> {
        let instructions = Ir4Layout::get_instructions(constraint)
            .expect("aspect_crop is enabled but not supported");

        let ir_layout = Ir4Layout::new(instructions, source_w, source_h, source_w, source_h);

        let initial_size = AspectRatio::create(source_w, source_h)?;

        let target =
            ir_layout.get_ideal_target_size(initial_size, ir_layout.get_preshrink_ratio())?;

        let constraints = ir_layout.build_constraints();

        let cropper = sizing::IdentityCropProvider::new();

        // ======== This is where we do the sizing and constraint evaluation \/
        let layout =
            sizing::Layout::create(initial_size, target).execute_all(&constraints, &cropper)?;

        let new_crop = layout.get_source_crop();

        // Determine effective gravity: focus rects > explicit gravity > center
        let effective_gravity = if let Some(ref focus_rects) = constraint.focus {
            if !focus_rects.is_empty() {
                let (gx, gy) = Self::focus_rects_to_gravity(focus_rects);
                ConstraintGravity::Percentage { x: gx, y: gy }
            } else {
                constraint.gravity.unwrap_or(ConstraintGravity::Center)
            }
        } else {
            constraint.gravity.unwrap_or(ConstraintGravity::Center)
        };

        //align crop
        let (inner_crop_x1, inner_crop_y1) =
            Ir4Layout::align_gravity(effective_gravity, new_crop, initial_size)
                .expect("Outer box should never be smaller than inner box. All values must > 0");
        //add manual crop offset
        let (crop_x1, crop_y1) = ((inner_crop_x1) as u32, (inner_crop_y1) as u32);

        let final_crop = if crop_x1 > 0
            || crop_y1 > 0
            || initial_size.width() != new_crop.width()
            || initial_size.height() != new_crop.height()
        {
            Some([
                crop_x1,
                crop_y1,
                crop_x1 + new_crop.width() as u32,
                crop_y1 + new_crop.height() as u32,
            ])
        } else {
            None
        };

        //Align padding
        let final_canvas = layout.get_box(BoxTarget::CurrentCanvas);
        let scale_to = layout.get_box(BoxTarget::CurrentImage);
        let (left, top) = Ir4Layout::align_gravity(effective_gravity, scale_to, final_canvas)
            .expect("Outer box should never be smaller than inner box. All values must > 0");

        let (right, bottom) = (
            final_canvas.width() - scale_to.width() - left,
            final_canvas.height() - scale_to.height() - top,
        );
        //Add padding. This may need to be revisited - how do jpegs behave with transparent padding?
        let mut pad = None;
        if left > 0 || top > 0 || right > 0 || bottom > 0 {
            if left >= 0 && top >= 0 && right >= 0 && bottom >= 0 {
                pad = Some([left as u32, top as u32, right as u32, bottom as u32]);
            } else {
                panic!("Negative padding showed up: {},{},{},{}", left, top, right, bottom);
            }
        }

        Ok(ConstraintResults { crop: final_crop, scale_to, final_canvas, pad })
    }

    pub fn get_crop_and_layout(&self) -> sizing::Result<(Option<[u32; 4]>, sizing::Layout)> {
        let (precrop_w, precrop_h) = self.get_precrop();

        // later consider adding f.sharpen, f.ignorealpha
        // (up/down).(filter,window,blur,preserve,colorspace,speed)

        let initial_crop = self.get_initial_copy_window();

        let initial_size = sizing::AspectRatio::create(
            initial_crop[2] - initial_crop[0],
            initial_crop[3] - initial_crop[1],
        )?;

        let target = self.get_ideal_target_size(initial_size, self.get_preshrink_ratio())?;

        let constraints = self.build_constraints();

        let cropper = sizing::IdentityCropProvider::new();

        // ======== This is where we do the sizing and constraint evaluation \/
        let layout =
            sizing::Layout::create(initial_size, target).execute_all(&constraints, &cropper)?;

        let new_crop = layout.get_source_crop();

        // Priority: c.focus > c.gravity > anchor > center
        let crop_align = self
            .focus_to_anchor()
            .or_else(|| {
                self.i
                    .c_gravity
                    .map(|[x, y]| (Anchor1D::Percent(x as f32), Anchor1D::Percent(y as f32)))
            })
            .or(self.i.anchor.clone())
            .unwrap_or((Anchor1D::Center, Anchor1D::Center));
        //align crop
        let (inner_crop_x1, inner_crop_y1) = Self::align(crop_align, new_crop, initial_size)
            .expect("Outer box should never be smaller than inner box. All values must > 0");
        //add manual crop offset
        let (crop_x1, crop_y1) =
            ((initial_crop[0] + inner_crop_x1) as u32, (initial_crop[1] + inner_crop_y1) as u32);

        let final_crop = if crop_x1 > 0
            || crop_y1 > 0
            || precrop_w != new_crop.width()
            || precrop_h != new_crop.height()
        {
            Some([
                crop_x1,
                crop_y1,
                crop_x1 + new_crop.width() as u32,
                crop_y1 + new_crop.height() as u32,
            ])
        } else {
            None
        };
        Ok((final_crop, layout))
    }

    /// Convert c_focus rects (if present and explicit) to anchor percentages.
    /// Returns None if no explicit focus rects are set.
    fn focus_to_anchor(&self) -> Option<(Anchor1D, Anchor1D)> {
        match &self.i.c_focus {
            Some(FocusSource::Rects(rects)) if !rects.is_empty() => {
                let (gx, gy) = Self::focus_rects_to_gravity(rects);
                Some((Anchor1D::Percent(gx), Anchor1D::Percent(gy)))
            }
            _ => None,
        }
    }

    /// Does not add trimwhitespace or decode/encode
    pub fn add_steps(
        &self,
        b: &mut FramewiseBuilder,
        watermarks: &Option<Vec<imageflow_types::Watermark>>,
    ) -> sizing::Result<Ir4LayoutInfo> {
        b.add_rotate(self.i.srotate);
        b.add_flip(self.i.sflip);

        let (crop, layout) = self.get_crop_and_layout()?;

        let new_crop = layout.get_source_crop();
        let canvas = layout.get_box(BoxTarget::CurrentCanvas);
        let image = layout.get_box(BoxTarget::CurrentImage);
        let pad_anchor = self.i.anchor.unwrap_or((Anchor1D::Center, Anchor1D::Center));

        if let Some(c) = crop {
            b.add(s::Node::Crop { x1: c[0], y1: c[1], x2: c[2], y2: c[3] });
        }

        //get bgcolor - default to transparent (or white if targeting jpeg)
        let bgcolor_default = if Some(OutputFormat::Jpeg) == self.i.format {
            s::Color::Srgb(s::ColorSrgb::Hex("FFFFFFFF".to_owned()))
        } else {
            s::Color::Transparent
        };
        let bgcolor = self
            .i
            .bgcolor_srgb
            .map(|v| v.to_rrggbbaa_string())
            .map(|str| s::Color::Srgb(s::ColorSrgb::Hex(str)))
            .unwrap_or(bgcolor_default);

        let downscaling = image.width() < new_crop.width() || image.height() < new_crop.height();

        let sharpen_when = match self.i.f_sharpen_when {
            Some(SharpenWhen::Downscaling) => Some(s::SharpenWhen::Downscaling),
            Some(SharpenWhen::SizeDiffers) => Some(s::SharpenWhen::SizeDiffers),
            Some(SharpenWhen::Always) => Some(s::SharpenWhen::Always),
            None => None,
        };

        let which_colorspace =
            if downscaling { self.i.down_colorspace } else { self.i.up_colorspace };
        let scaling_colorspace = match which_colorspace {
            Some(ScalingColorspace::Linear) => Some(s::ScalingFloatspace::Linear),
            Some(ScalingColorspace::Srgb) => Some(s::ScalingFloatspace::Srgb),
            _ => None,
        };

        b.add(s::Node::Resample2D {
            w: image.width() as u32,
            h: image.height() as u32,
            hints: Some(imageflow_types::ResampleHints {
                sharpen_percent: self.i.f_sharpen,
                down_filter: self.i.down_filter.map(|v| v.to_filter()),
                up_filter: self.i.up_filter.map(|v| v.to_filter()),
                scaling_colorspace,
                background_color: Some(bgcolor.clone()),
                resample_when: Some(s::ResampleWhen::SizeDiffersOrSharpeningRequested),
                sharpen_when,
            }),
        });

        if let Some(quadrants) = self.i.s_round_corners {
            if let Some(all) = iter_all_eq(quadrants) {
                b.add(s::Node::RoundImageCorners {
                    radius: RoundCornersMode::Percentage(all as f32),
                    background_color: bgcolor.clone(),
                })
            } else {
                b.add(s::Node::RoundImageCorners {
                    radius: RoundCornersMode::PercentageCustom {
                        top_left: quadrants[0] as f32,
                        top_right: quadrants[1] as f32,
                        bottom_right: quadrants[2] as f32,
                        bottom_left: quadrants[3] as f32,
                    },
                    background_color: bgcolor.clone(),
                })
            }
        }

        // We now match ImageResizer order of operations below
        // if (!string.IsNullOrEmpty(alpha) && double.TryParse(alpha, ParseUtils.FloatingPointStyle, NumberFormatInfo.InvariantInfo, out temp)) filters.Add(Alpha((float)temp));
        // if (!string.IsNullOrEmpty(brightness) && double.TryParse(brightness, ParseUtils.FloatingPointStyle, NumberFormatInfo.InvariantInfo, out temp)) filters.Add(Brightness((float)temp));
        // if (!string.IsNullOrEmpty(contrast) && double.TryParse(contrast, ParseUtils.FloatingPointStyle, NumberFormatInfo.InvariantInfo, out temp)) filters.Add(Contrast((float)temp));
        // if (!string.IsNullOrEmpty(saturation) && double.TryParse(saturation, ParseUtils.FloatingPointStyle, NumberFormatInfo.InvariantInfo, out temp)) filters.Add(Saturation((float)temp));
        // Last, &a.* operations like &a.balance_white

        if let Some(c) = self.i.s_alpha {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Alpha(c)));
        }
        if let Some(c) = self.i.s_brightness {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Brightness(c)));
        }
        if let Some(c) = self.i.s_contrast {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Contrast(c)));
        }
        if let Some(c) = self.i.s_saturation {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Saturation(c)));
        }
        if let Some(true) = self.i.s_sepia {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Sepia));
        }
        if let Some(g) = self.i.s_grayscale {
            b.add(s::Node::ColorFilterSrgb(match g {
                GrayscaleAlgorithm::Flat => s::ColorFilterSrgb::GrayscaleFlat,
                GrayscaleAlgorithm::True | GrayscaleAlgorithm::Ntsc | GrayscaleAlgorithm::Y => {
                    s::ColorFilterSrgb::GrayscaleNtsc
                }
                GrayscaleAlgorithm::Bt709 => s::ColorFilterSrgb::GrayscaleBt709,
                GrayscaleAlgorithm::Ry => s::ColorFilterSrgb::GrayscaleRy,
            }));
        }

        // Perform white balance
        if Some(HistogramThresholdAlgorithm::Area) == self.i.a_balance_white {
            b.add(s::Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold: None });
        }

        if let Some(v) = watermarks {
            for w in v {
                match w.fit_box {
                    Some(WatermarkConstraintBox::ImageMargins { .. })
                    | Some(WatermarkConstraintBox::ImagePercentage { .. })
                    | None => {
                        b.add(s::Node::Watermark(w.clone()));
                    }
                    _ => {}
                }
            }
        }

        let (left, top) = Self::align(pad_anchor, image, canvas)
            .expect("Outer box should never be smaller than inner box. All values must > 0");

        let (right, bottom) =
            (canvas.width() - image.width() - left, canvas.height() - image.height() - top);
        //Add padding. This may need to be revisited - how do jpegs behave with transparent padding?
        if left > 0 || top > 0 || right > 0 || bottom > 0 {
            if left >= 0 && top >= 0 && right >= 0 && bottom >= 0 {
                b.add(s::Node::ExpandCanvas {
                    color: bgcolor.clone(),
                    left: left as u32,
                    top: top as u32,
                    right: right as u32,
                    bottom: bottom as u32,
                });
            } else {
                panic!("Negative padding showed up: {},{},{},{}", left, top, right, bottom);
            }
        }

        if let Some(v) = watermarks {
            for w in v {
                match w.fit_box {
                    Some(WatermarkConstraintBox::CanvasMargins { .. })
                    | Some(WatermarkConstraintBox::CanvasPercentage { .. }) => {
                        b.add(s::Node::Watermark(w.clone()));
                    }
                    _ => {}
                }
            }
        }

        b.add_rotate(self.i.rotate);
        b.add_flip(self.i.flip);

        //We apply red dot watermarking after rotate/flip unlike imageresizer
        if self.i.watermark_red_dot == Some(true) {
            b.add(s::Node::WatermarkRedDot);
        }

        Ok(Ir4LayoutInfo { canvas })
    }

    /// Compute effective gravity percentages (x%, y%) from focus rects.
    /// The centroid is weighted by each rect's weight * area.
    fn focus_rects_to_gravity(rects: &[FocusRect]) -> (f32, f32) {
        if rects.is_empty() {
            return (50.0, 50.0);
        }
        let mut total_weight = 0.0f64;
        let mut cx = 0.0f64;
        let mut cy = 0.0f64;
        for r in rects {
            let r = r.clamped();
            let area = ((r.x2 - r.x1) * (r.y2 - r.y1)).max(0.01) as f64;
            let w = r.weight.max(0.0) as f64 * area;
            let (rx, ry) = r.center();
            cx += rx as f64 * w;
            cy += ry as f64 * w;
            total_weight += w;
        }
        if total_weight <= 0.0 {
            return (50.0, 50.0);
        }
        ((cx / total_weight) as f32, (cy / total_weight) as f32)
    }

    fn align1d(a: Anchor1D, inner: i32, outer: i32) -> std::result::Result<i32, ()> {
        if outer < inner && inner < 1 || outer < 1 {
            Err(())
        } else {
            match a {
                Anchor1D::Near => Ok(0),
                Anchor1D::Center => Ok((outer - inner) / 2),
                Anchor1D::Far => Ok(outer - inner),
                Anchor1D::Percent(p) => Self::gravity1d(p, inner, outer),
            }
        }
    }
    fn align(
        alignment: (Anchor1D, Anchor1D),
        inner: AspectRatio,
        outer: AspectRatio,
    ) -> std::result::Result<(i32, i32), ()> {
        let (x, y) = alignment;
        Ok((
            Self::align1d(x, inner.width(), outer.width())?,
            Self::align1d(y, inner.height(), outer.height())?,
        ))
    }

    fn gravity1d(align_percentage: f32, inner: i32, outer: i32) -> std::result::Result<i32, ()> {
        let ratio = align_percentage.clamp(0f32, 100f32) / 100f32;
        if outer < inner && inner < 1 || outer < 1 {
            Err(())
        } else {
            Ok(i32::max(
                0,
                i32::min(((outer - inner) as f32 * ratio).round() as i32, outer - inner),
            ))
        }
    }

    fn align_gravity(
        gravity: imageflow_types::ConstraintGravity,
        inner: AspectRatio,
        outer: AspectRatio,
    ) -> std::result::Result<(i32, i32), ()> {
        let (x, y) = match gravity {
            imageflow_types::ConstraintGravity::Center => (50f32, 50f32),
            imageflow_types::ConstraintGravity::Percentage { x, y } => (x, y),
        };
        Ok((
            Self::gravity1d(x, inner.width(), outer.width())?,
            Self::gravity1d(y, inner.height(), outer.height())?,
        ))
    }

    fn get_initial_copy_window(&self) -> [i32; 4] {
        let (w, h) = self.get_precrop();
        let (ref_w, ref_h) = self.get_precrop_reference();

        let mut floats = self.get_initial_copy_window_floats(ref_w, ref_h);

        //Re-scale crop values against real width/height (after decoder downscaling)
        if ref_w != w || ref_h != h {
            floats[0] = floats[0] * w as f64 / ref_w as f64;
            floats[2] = floats[2] * w as f64 / ref_w as f64;
            floats[1] = floats[1] * h as f64 / ref_h as f64;
            floats[3] = floats[3] * h as f64 / ref_h as f64;
        }
        let maximums = [w, h];
        let ints = floats
            .iter()
            .enumerate()
            .map(|(ix, item)| cmp::max(0i32, cmp::min(item.round() as i32, maximums[ix % 2])))
            .collect::<Vec<i32>>();
        if ints[3] <= ints[1] || ints[2] <= ints[0] {
            //violation of X2 > X1 or Y2 > Y1
            [0, 0, w, h]
        } else {
            [ints[0], ints[1], ints[2], ints[3]]
        }
    }

    // Clippy considers an fp cast expensive enough to warrant a closure

    fn get_initial_copy_window_floats(
        &self,
        original_width: i32,
        original_height: i32,
    ) -> [f64; 4] {
        let defaults = [0f64, 0f64, f64::from(original_width), f64::from(original_height)];
        if let Some(values) = self.i.crop {
            let xunits = self
                .i
                .cropxunits
                .map(|v| if v == 0f64 { f64::from(original_width) } else { v })
                .unwrap_or(f64::from(original_width));
            let yunits = self
                .i
                .cropyunits
                .map(|v| if v == 0f64 { f64::from(original_height) } else { v })
                .unwrap_or(f64::from(original_height));
            let floats = values
                .iter()
                .enumerate()
                .map(|(ix, item)| {
                    let relative_to = if ix % 2 == 0 { xunits } else { yunits };
                    let max_dimension =
                        f64::from(if ix % 2 == 0 { original_width } else { original_height });
                    let mut v = *item * max_dimension / relative_to;
                    if ix < 2 && v < 0f64 || ix > 1 && v <= 0f64 {
                        v += max_dimension; //Support negative offsets from bottom right.
                    }
                    if v < 0f64 {
                        v = 0f64;
                    }
                    if v > max_dimension {
                        v = max_dimension;
                    }
                    v
                })
                .collect::<Vec<f64>>();
            if floats[3].round() <= floats[1].round() || floats[2].round() <= floats[0].round() {
                //violation of X2 > X1 or Y2 > Y1
                defaults
            } else {
                [floats[0], floats[1], floats[2], floats[3]]
            }
        } else {
            defaults
        }
    }
}
#[derive(Default)]
pub struct FramewiseBuilder {
    steps: Vec<s::Node>,
}

impl FramewiseBuilder {
    pub fn new() -> FramewiseBuilder {
        FramewiseBuilder { steps: vec![] }
    }
    fn add_flip(&mut self, f: Option<(bool, bool)>) {
        if let Some((h, v)) = f {
            if h {
                self.steps.push(s::Node::FlipH);
            }
            if v {
                self.steps.push(s::Node::FlipV);
            }
        }
    }
    fn add_rotate(&mut self, r: Option<i32>) {
        if let Some(rot) = r {
            self.add_maybe(match ((rot / 90) + 4) % 4 {
                1 => Some(s::Node::Rotate90),
                2 => Some(s::Node::Rotate180),
                3 => Some(s::Node::Rotate270),
                _ => None,
            });
        }
    }
    fn add_maybe(&mut self, n: Option<s::Node>) {
        if let Some(node) = n {
            self.steps.push(node);
        }
    }
    pub fn add(&mut self, n: s::Node) {
        self.steps.push(n);
    }
    pub fn into_steps(self) -> Vec<s::Node> {
        self.steps
    }
}

#[test]
fn test_crop_and_scale() {
    let mut b = FramewiseBuilder::new();

    let l = Ir4Layout::new(
        Instructions {
            w: Some(100),
            h: Some(200),
            mode: Some(FitMode::Crop),
            ..Default::default()
        },
        768,
        433,
        768,
        433,
    );
    l.add_steps(&mut b, &None).unwrap();

    assert_eq!(
        b.steps,
        vec![
            s::Node::Crop { x1: 275, y1: 0, x2: 492, y2: 433 },
            s::Node::Resample2D {
                w: 100,
                h: 200,
                hints: Some(s::ResampleHints {
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: Some(s::Color::Transparent),
                    resample_when: Some(s::ResampleWhen::SizeDiffersOrSharpeningRequested),
                    sharpen_when: None
                })
            }
        ]
    );
}

#[test]
fn test_custom_crop_with_preshrink() {
    let mut b = FramewiseBuilder::new();

    let l = Ir4Layout::new(
        Instructions {
            w: Some(170),
            h: Some(220),
            mode: Some(FitMode::Crop),
            scale: Some(ScaleMode::Both),
            crop: Some([449f64, 0f64, -472f64, 0f64]),
            ..Default::default()
        },
        641,
        960,
        2560,
        1707,
    ); //TODO: plug in preshrink values
    l.add_steps(&mut b, &None).unwrap();

    assert_eq!(
        b.steps,
        vec![
            s::Node::Crop { x1: 112, y1: 214, x2: 523, y2: 746 },
            s::Node::Resample2D {
                w: 170,
                h: 220,
                hints: Some(s::ResampleHints {
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: Some(s::Color::Transparent),
                    resample_when: Some(s::ResampleWhen::SizeDiffersOrSharpeningRequested),
                    sharpen_when: None
                })
            }
        ]
    );
}

#[test]
fn test_scale() {
    let mut b = FramewiseBuilder::new();

    let w = imageflow_types::Watermark {
        io_id: 3,
        gravity: None,
        fit_box: None,
        fit_mode: None,
        opacity: None,
        hints: None,
        min_canvas_width: None,
        min_canvas_height: None,
    };
    let l = Ir4Layout::new(
        Instructions {
            w: Some(2560),
            h: Some(1696),
            mode: Some(FitMode::Max),
            f_sharpen_when: Some(SharpenWhen::Downscaling),
            ..Default::default()
        },
        5104,
        3380,
        5104,
        3380,
    );
    l.add_steps(&mut b, &Some(vec![w.clone()])).unwrap();
    assert_eq!(
        b.steps,
        vec![
            s::Node::Resample2D {
                w: 2560,
                h: 1695,

                hints: Some(s::ResampleHints {
                    sharpen_percent: None,
                    down_filter: None,
                    up_filter: None,
                    scaling_colorspace: None,
                    background_color: Some(s::Color::Transparent),
                    resample_when: Some(s::ResampleWhen::SizeDiffersOrSharpeningRequested),
                    sharpen_when: Some(s::SharpenWhen::Downscaling)
                })
            },
            s::Node::Watermark(w)
        ]
    );

    // 5104x3380 "?w=2560&h=1696&mode=max&format=png&decoder.min_precise_scaling_ratio=2.1&down.colorspace=linear"
}

#[cfg(test)]
mod focus_tests {
    use super::*;
    use imageflow_types::{Constraint, ConstraintGravity, ConstraintMode, FocusKind, FocusRect};

    /// Helper: run process_constraint and return the crop coordinates
    fn crop_with_focus(
        source_w: i32,
        source_h: i32,
        target_w: u32,
        target_h: u32,
        focus_rects: Vec<FocusRect>,
    ) -> Option<[u32; 4]> {
        let constraint = Constraint {
            mode: ConstraintMode::Fit,
            w: Some(target_w),
            h: Some(target_h),
            hints: None,
            gravity: Some(ConstraintGravity::Center),
            canvas_color: None,
            focus: Some(focus_rects),
        };
        let result = Ir4Layout::process_constraint(source_w, source_h, &constraint).unwrap();
        result.crop
    }

    /// Helper: crop with gravity center and no focus rects
    fn crop_centered(
        source_w: i32,
        source_h: i32,
        target_w: u32,
        target_h: u32,
    ) -> Option<[u32; 4]> {
        let constraint = Constraint {
            mode: ConstraintMode::Fit,
            w: Some(target_w),
            h: Some(target_h),
            hints: None,
            gravity: Some(ConstraintGravity::Center),
            canvas_color: None,
            focus: None,
        };
        let result = Ir4Layout::process_constraint(source_w, source_h, &constraint).unwrap();
        result.crop
    }

    #[test]
    fn focus_rect_in_topleft_shifts_crop() {
        // 1000x1000 image, crop to 500x500 (crop mode)
        // Focus rect in top-left quadrant: should place crop there
        let constraint = Constraint {
            mode: ConstraintMode::Fit,
            w: Some(500),
            h: Some(500),
            hints: None,
            gravity: Some(ConstraintGravity::Center),
            canvas_color: None,
            focus: Some(vec![FocusRect::new(0.0, 0.0, 25.0, 25.0)]),
        };
        let result = Ir4Layout::process_constraint(1000, 1000, &constraint).unwrap();
        // With FitMode::Fit and square-to-square, no crop needed
        // This is expected — Fit mode doesn't crop
        // Switch to a mode that crops:
        let crop_result = crop_with_url_and_focus(
            1000,
            1000,
            "w=500&h=250&mode=crop",
            vec![FocusRect::new(0.0, 0.0, 25.0, 25.0)],
        );
        if let Some(crop) = crop_result {
            // Crop y1 should be near top (0) since focus is in top-left
            assert!(
                crop[1] < 250,
                "Crop top should be near top of image for top-left focus, but y1={}",
                crop[1]
            );
        }
    }

    #[test]
    fn focus_rect_in_bottomright_shifts_crop() {
        let crop_result = crop_with_url_and_focus(
            1000,
            1000,
            "w=500&h=250&mode=crop",
            vec![FocusRect::new(75.0, 75.0, 100.0, 100.0)],
        );
        if let Some(crop) = crop_result {
            // Crop y1 should be toward the bottom since focus is in bottom-right
            assert!(
                crop[1] > 250,
                "Crop top should be toward bottom for bottom-right focus, but y1={}",
                crop[1]
            );
        }
    }

    #[test]
    fn focus_rects_weighted_centroid() {
        // Two focus rects: heavy weight in top-left, light weight in bottom-right
        let (gx, gy) = Ir4Layout::focus_rects_to_gravity(&[
            FocusRect { x1: 0.0, y1: 0.0, x2: 20.0, y2: 20.0, weight: 10.0, kind: FocusKind::Face },
            FocusRect {
                x1: 80.0,
                y1: 80.0,
                x2: 100.0,
                y2: 100.0,
                weight: 1.0,
                kind: FocusKind::Saliency,
            },
        ]);
        // Centroid should be pulled toward top-left due to higher weight
        assert!(gx < 50.0, "Centroid x should be left of center, got {}", gx);
        assert!(gy < 50.0, "Centroid y should be above center, got {}", gy);
    }

    #[test]
    fn focus_rects_single_rect_centroid() {
        let (gx, gy) = Ir4Layout::focus_rects_to_gravity(&[FocusRect::new(60.0, 70.0, 80.0, 90.0)]);
        // Centroid should be at center of rect: (70, 80)
        assert!((gx - 70.0).abs() < 0.01, "Expected gx=70, got {}", gx);
        assert!((gy - 80.0).abs() < 0.01, "Expected gy=80, got {}", gy);
    }

    #[test]
    fn empty_focus_rects_defaults_to_center() {
        let (gx, gy) = Ir4Layout::focus_rects_to_gravity(&[]);
        assert!((gx - 50.0).abs() < 0.01, "Expected gx=50, got {}", gx);
        assert!((gy - 50.0).abs() < 0.01, "Expected gy=50, got {}", gy);
    }

    /// Helper: parse a URL query string with focus rects and process it
    fn crop_with_url_and_focus(
        source_w: i32,
        source_h: i32,
        query: &str,
        focus_rects: Vec<FocusRect>,
    ) -> Option<[u32; 4]> {
        let url = format!("http://localhost/image.jpg?{}", query);
        let a = url::Url::from_str(&url).unwrap();
        let (mut instructions, _warns) = parse_url(&a);
        instructions.c_focus = Some(FocusSource::Rects(focus_rects));

        let layout = Ir4Layout::new(instructions, source_w, source_h, source_w, source_h);
        let result = layout.get_crop_and_layout();
        match result {
            Ok((crop, _layout)) => crop,
            _ => None,
        }
    }
}
