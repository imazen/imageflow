extern crate imageflow_types;
extern crate imageflow_helpers;
use imageflow_helpers as hlp;
use imageflow_types as s;
use imageflow_helpers::preludes::from_std::*;
extern crate url;
use url::Url;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

static IR4_KEYS: [&'static str;58] = ["mode", "anchor", "flip", "sflip", "scale", "cache", "process", "frame", "page", "quality", "subsampling", "colors", "zoom",
"crop", "cropxunits", "cropyunits", "w", "h", "width", "height", "maxwidth", "maxheight", "format", "thumbnail",
"precise_scaling_ratio", "autorotate", "srotate", "rotate", "ignoreicc", "404", "bgcolor", "paddingcolor", "bordercolor", "preset", "floatspace", "jpeg_idct_downscale_linear", "watermark",
"s.invert", "s.sepia", "s.grayscale", "s.alpha", "s.brightness", "s.contrast", "s.saturation", "trim.threshold", "trim.percentpadding", "a.blur", "a.sharpen", "a.removenoise", "dither",
"encoder", "decoder", "builder", "s.roundcorners.", "paddingwidth", "paddingheight", "margin", "borderwidth"];

static IF_KEYS: [&'static str;8] = ["w","h", "width", "height", "mode", "quality", "format", "autorotate"];

//pub fn parse_url(url: Url) -> s::Framewise{
//    url.query_pairs().filter(|(k,v)| IR4_keys.contains(k.to_lowercase()))
//
//    // Let various strategies attempt to handle all the querypairs that intersect with IR4/IF
//    // Leftovers cause a warning
//
//
//    //scale=up should cause a warning
//}



enum ConstraintMode{
    Max,
    Crop,
    Pad
}
enum Scaling{
    Down,
    Both,
    Canvas
}
enum AspectHandling{
    CropToRatioWhenSmaller,
    PadToRatioWhenSmaller,
    RevertToOrigin,
}
struct ConstraintParams{
    w: Option<u32>,
    h: Option<u32>,
    scale: Scaling,
    aspect_handling: AspectHandling,
    mode: ConstraintMode
}

/// When a constraint requires cropping, how should it be selected?
enum CropSelection{
    Center
}
/// When a constraint requires padding, where shall it be added?
enum PaddingLocation{
    Even
}

/// Strategies for single dimension constraints: I.e, Width OR Height, but not both
enum OneConstraintStrategy{
    /// The constraint is an upper bound; the image will never be upscaled,
    /// and downscaling will preserve the original aspect ratio
    Max,
    /// (no use case) The constraint is a lower bound. If the image is smaller than this value, it will be upscaled, preserving the original aspect ratio.
    Min,
    /// (no use case)  If the original image is too small, the canvas will be extended in that one dimension, following the PaddingLocation rules.
    ExactViaCanvas1D,
    /// (no use case) If too small, the canvas will be extended in both directions as required to match both the original aspect ratio and the desired dimension.
    ExactViaCanvas,
    /// (no use case) If too small, the image will be upscaled so that the given dimension is an exact match.
    ExactViaUpscaling,
    /// (no use case) If too small, distort. Otherwise, preserve aspect ratio.
    ExactViaDistortion,
    /// (no use case) The image's aspect ratio will be ignored, and scaled in a single dimension to the given value.
    ExactAlwaysDistort,
    /// (no use case) Don't scale the image; rather, crop or pad it in a single dimension.
    /// always.fit=crop(target).pad(target)
    ExactAlwaysCropOrPad
}

// Boxes
// from fbox
// target tbox
// from box scaled within target with 1 dimension matching: ibox
// from box enclosing target with 1 dimension matching :obox

enum TwoConstraintStrategy{
    /// Downscale the image until it fits within the box (less than both and matching at least one dimension).
    /// Never upscale, even if the image is smaller in both dimensions.
    ///
    /// `down.fitw=scale(w, auto) down.fith=scale(auto,h) up.fit=none`
    /// `down.fit=proportional(target), up.fit=none`
    /// `down.fit=proportional(ibox), up.fit=none`
    Max,
    /// Downscale minimally until the image fits one of the dimensions (it may exceed the other). Never uspcale.
    ///
    /// `down.fitw=scale(max(w, obox.w), auto) down.fith=scale(auto,max(h, obox.h) up.fit=none`
    /// `down.fit=proportional(obox), up.fit=none`
    /// `down.fit=proportional(target), losslessup.fit=proportional(target)` #if shorter dimension is done last
    Max1D,
    /// Upscale minimally until one dimension matches. Never downscale, if larger.
    /// `up.fit=scale(max(d, ibox.other), auto) down.fit=none`
    /// `up.fit=proportional(ibox), down.fit=none`
    Min1D,
    /// Upscale minimally until the image meets or exceeds both specified dimensions. Never downscale.
    /// `up.fit=scale(d, auto) up.fit=none`
    /// `up.fit=proportional(obox), down.fit=none`
    Min2D,
    /// Downscale the image and pad to meet aspect ratio. If smaller in both dimensions, give up and leave as-is.
    /// `down.fit=proportional(ibox), pad(target), up.fit=none` - won't work, second dimension will classify as upscale.
    /// `down.fit=proportional(ibox), pad2d(target.aspect), up.fit=none`
    PadUnlessSmaller,
    /// Downscale the image and crop to meet aspect ratio. If smaller in both dimensions, give up and leave as-is.
    /// `down.fit=proportional(obox),crop(target) up.fit=none`
    CropUnlessSmaller,

    /// Downscale & pad. If smaller, pad to achieve desired aspect ratio.
    PadOrAspect,

    /// Downscale & crop. If smaller, crop to achieve desired aspect ratio.
    /// `down.fit=proportional(obox),crop(target) up.fit=cropaspect(target)`
    CropOrAspect,
    /// Downscale & crop. If smaller, pad to achieve desired aspect ratio.
    /// `down.fit=proportional(ibox),crop(target) up.fit=padaspect(target)`
    CropOrAspectPad,

// perhaps a lint for pad (or distort) in down. but not up. ?

    /// Minimally pad to match desired aspect ratio. Downscale or upscale to exact dimensions provided.
    /// `always.fit.xy=proportional(ibox),pad(target)`
    ExactPadAllowUpscaling,
    /// Minimally crop to match desired aspect ratio. Downscale or upscale to exact dimensions provided.
    /// `always.fit.xy=proportional(ibox),crop(target)`
    ExactCropAllowUpscaling,

    /// `always.fit.xy=distort(target)`
    Distort,
    /// `down.fit.xy=proportional(obox),cropcareful(target)`
    CropCarefulDownscale,
    /// `down.fit.xy=proportional(obox),cropcareful(target),proportional(target),pad(target)` -doesn't work; second dimension never executes pad. (unless we run smaller dimension first)
    /// `down.fit.xy=proportional(obox),cropcareful(target),proportional(target),pad(target.aspect)` -doesn't work; second dimension never executes pad. (unless we run smaller dimension first)
    CropCarefulPadDownscale,
    /// `down.fit.xy=proportional(obox),cropcareful(target),proportional(target),pad(target) up.xy.fit=cropcareful(targetaspect),pad(targetaspect)`
    CropCarefulDownscaleOrForceAspect,


    // When cropping an image to achieve aspect ratio makes it smaller than the desired box, thereby changing the rules...
    // Ie, box of 20x30, image of 60x20. Crop to 14x20 leaves 6x10 gap.
    // Results should match downscaling rules, right?
    //
    // Alternate options: pad mismatched dimension: crop to 20x20, add 10px vertical padding.
    // Alternate: crop to 14x20 and upscale to 20x30

    // Aspect ratio comparison can be Wider, Taller, Same
    // Size comparison can be Larger2D, LargerW, LargerH, Smaller2D, Same
    // LargerW implies Wider, you can't have a LargerH and a Wider aspect ratio .
    //

    // What if we separate dimensions?
    // If dimension constraints are ordered with clairvoyance
    // Then we could crop width to 20 (as 'downscaling'), and pad height to 20 (as 'upscaling' solution"). If 'upscaling' is the default, then we have to crop more.
    // This gets really hard to reason about, as each dimension's constraint also has to consider aspect ratio.
    // TODO: ValueAndAspectStrategy
    // Or we can specify '2-constraint' strategies separately?
    // With separate strategies, any aspect ratio changes can turn a larger2d or smaller2d into a Mismatch.

}

enum StrategyIfLarger2D{
    DownscaleWithin,
    DownscaleCrop,
    DownscaleCropCare,
    DownscalePad,

    Distort
}
enum StrategyIfMismatch{
    DownscaleWithin,
    Crop1D,
    CropThenPad,
    CropCareThenPad,
    CropThenUpscale,
    CropCareThenUpscale,
    Distort
}
enum StrategyIfSmaller2D{
    Pad,
    // Upscaling can bring us into the mismatch zone.
    UpscaleThenPad,
    UpscaleThenCropCare,
    UpscaleThenCrop,
    Distort,
    DistortToAspect,
    CropToAspect,
    CropCareToAspect,
    PadToAspect
}
enum ComparisonState{
    Larger2D,
    Smaller2D,
    Mismatch
}
/////////////////////////////////////////////////////////
// Given a ScaleAbove and ScaleBelow which can cause one dimension to match the provided box
// And the other to be larger (or smaller), but with minimal loss of information.
// This can be used to achieve a state where we can reason about aspect ratio changes from
// an initial state of
enum PostScaleState{
    Larger1D,
    Smaller1D,
    Larger1DSmaller1D,
    //The image is either larger2d or smaller2d, and we are not targeting scaling.
    DontCare
}
// After exhausting our acceptable aspect ratio modification strategies,
// We may have (a) success, or (b) partial (or no) progress towards the aspect ratio goal.
// If successful, we should be able to scale the rest of the way IF desired. (say we added padding coming from Larger2D or cropped from Smaller2D). WithinBox and
// When partially successful,
//
// We may 1. Downscale within bounds, Downscale to exact bounds,


enum AspectRatioStrategy{

}


// Rounding errors are problematic when they cause an off-by-one versus target width/height or original width/height.
// So aspect ratios include the fraction they were derived from, and implementors should round to these if one of the 2 dimensions matches.
struct AspectRatio{
    ratio: f64,
    w: u64,
    h: u64,
}

// AspectModifier trait
// - provide pixel buffer or rect, desired aspect ratio, and a dimension constraint?
// - get resultant crop and resultant canvas


/// Here we combine a 1 dimensional constraint and a target aspect ratio.
/// We assume that the biggest delta dimension is processed first. I.e, width if the source image has a wider aspect ratio, height if the source image has a higher aspect ratio.
///
enum TargetAndAspect{

}

/// Strategies for single dimension constraints: I.e, Width OR Height, but not both
enum Constraint1DAndAspectStrategy{

    /// The constraint is an upper bound; the image will never be upscaled,
    /// and downscaling will preserve the original aspect ratio
    Max,
    /// Ensures that at least one dimension is equal to or smaller than the provided box (alt. dimension estimated via aspect ratio). aka. MinimalSupersetNoUspcaling
    /// (fits outside box)
    MaxAny,
    /// (no use case) The constraint is a lower bound. If the image is smaller than this value, it will be upscaled, preserving the original aspect ratio.
    /// (like maxany (if done twice), fits outside)
    Min,
    /// (like max done twice, fits within box).
    MinAny,
    /// (no use case)  If the original image is too small, the canvas will be extended in that one dimension, following the PaddingLocation rules.
    ExactViaCanvas1D,
    /// (no use case) If too small, the canvas will be extended in both directions as required to match both the original aspect ratio and the desired dimension.
    ExactViaCanvas,
    /// (no use case) If too small, the image will be upscaled so that the given dimension is an exact match.
    ExactViaUpscaling,
    /// (no use case) If too small, distort. Otherwise, preserve aspect ratio.
    ExactViaDistortion,
    /// (no use case) The image's aspect ratio will be ignored, and scaled in a single dimension to the given value.
    ExactAlwaysDistort,
    /// (no use case) Don't scale the image; rather, crop or pad it in a single dimension.
    ExactAlwaysCropOrPad
}


enum ConstraintStrategy{
    Width{ w: u32, strategy: OneConstraintStrategy },
    Height{ h: u32, strategy: OneConstraintStrategy },
    /// Keep the original aspect ratio. If the image is smaller than both bounds, don't upscale or pad.
    WithinBox{w: u32, h: u32},

}
// 1x and 2x are enough. Specifying size on HTML means we must, at least preserve the aspect ratio``
// If crop or pad.
// width/height/max
// width/height/crop - when larger than original - should crop minimally to aspect ratio by default.
// width/height/pad - when larger than original - should pad minimally to aspect ratio.
// aspectforce=crop|pad?
// scale=both|canvas


// when width or height are specified alone, there is no expectation of aspect ratio.

// toosmall=preserveaspect,upscale,expandcanvas


// http://calendar.perfplanet.com/2015/why-arent-your-images-using-chroma-subsampling/