use imageflow_helpers::preludes::from_std::*;
use ::std;
use ::url::Url;
use ::macro_attr;
use ::enum_derive;
use ::imageflow_types as s;

pub mod parsing;

use ::sizing;
use self::parsing::*;

pub struct Ir4Layout{
    i: Instructions,
    info: s::ImageInfo
}
impl Ir4Layout{

    fn fit_mode(&self) -> FitMode{
        self.i.mode.map(|m| if m == FitMode::Carve { FitMode::Stretch} else {m})
            .and_then(|m| if m == FitMode::None { None } else {Some(m)})
            .unwrap_or_else(|| {
            if self.i.w.is_none() && self.i.h.is_none() {
                FitMode::Max
            } else {
                // if stretch=fill -> Fill
                // if crop=auto -> Crop
                // Carve??
                FitMode::Pad
            }
        })
    }
    pub fn produce_framewise(info: s::ImageInfo, i: Instructions) -> s::Framewise{
        Ir4Layout{ i: i, info: info}.produce()
    }

    fn get_target_size(&self) -> (i32,i32){
//        let w = if self.i.w.is_some() && self.i.legacy_max_width.is_some(){
//            cmp::min(self.i.w.unwrap(),self.i.legacy_max_width.unwrap())
//        }else
//        w = i.width
//        h = i.height
//        mw = i.obsolete_maxwidth
//        mh = i.obsolete_maxheight
//
//
//        #Eliminate cases where both a value and a max value are specified: use the smaller value for the width/height
//        if !w.nil? && !mw.nil?
//        w = [w, mw].min
//        mw = nil
//        end
//        if !h.nil? && !mh.nil?
//        h = [h, mh].min
//        mh = nil
//        end
//
//        #Handle cases of width/maxheight and height/maxwidth as in legacy version
//        mh = [mh, w / image_ratio].min if !w.nil? && !mh.nil?
//        mw = [mw, h * image_ratio].min if !h.nil? && !mw.nil?
//
//        w = [w, mw].compact.max
//        h = [h, mh].compact.max
//
//        # Calculate missing value (a missing value is handled the same everywhere)
//        h = w / image_ratio if h.nil?
//        w = h * image_ratio if w.nil?
        let orig = sizing::AspectRatio::create(self.info.image_width, self.info.image_height).unwrap();

        if self.i.w.is_none() && self.i.h.is_none(){
            (self.info.image_width, self.info.image_height)
        }else if self.i.w.is_none(){
            (0,0)
        }else if self.i.h.is_none(){
            (0,0)
        }else {
            (self.i.w.unwrap(), self.i.h.unwrap())
        }




    }

    fn produce(&self) -> s::Framewise{

        let mut nodes = Vec::new();

        //TODO: later consider decoder scaling

        nodes.push(s::Node::Decode{io_id: 0, commands: None});

        // add srotate
        if  let Some((h,v)) = self.i.sflip{
            if h { nodes.push(s::Node::FlipH); }
            if v { nodes.push(s::Node::FlipV); }
        }

        let starting_size = sizing::AspectRatio::create(self.info.image_width, self.info.image_height).unwrap();
        // TODO: apply manual crop first

        //Maybe just panic if using UpscaleOnly??
        match (self.fit_mode(),self.i.scale.unwrap_or(ScaleMode::DownscaleOnly)){
            (FitMode::Max, ScaleMode::UpscaleOnly) => { panic!("")},
            (FitMode::Max, _) => {

            },
            (FitMode::Pad, ScaleMode::Both) => {},
            (FitMode::Pad, ScaleMode::DownscaleOnly) | (FitMode::Pad, ScaleMode::UpscaleCanvas) => {},
            (FitMode::Crop, ScaleMode::DownscaleOnly) => {},
            (FitMode::Stretch, ScaleMode::Both) => {},
            (FitMode::Stretch, ScaleMode::DownscaleOnly) => {}
            _ => {panic!("");}
        }
        //crop
        //scale
        //pad
//        ::sizing::steps().skip_if



        //add rotate
        //add flip
        if let Some((h,v)) = self.i.flip{
            if h { nodes.push(s::Node::FlipH); }
            if v { nodes.push(s::Node::FlipV); }
        }


        s::Framewise::Steps(nodes)
    }
}


//discards warnings
pub fn parse_to_framewise(info: s::ImageInfo, url: &Url) -> s::Framewise{
    let (i, warn) = parsing::parse_url(url);
    Ir4Layout::produce_framewise(info, i)
}


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


//use ieee754::Ieee754;
//extern crate ieee754;


// <meta http-equiv="Accept-CH" content="DPR, Viewport-Width, Width, Downlink">
// <link rel=preload
// lazySizes javascript
// Cuzillion

// https://jmperezperez.com/medium-image-progressive-loading-placeholder/
// http://httpwg.org/http-extensions/client-hints.html#the-save-data-hint
// https://developers.google.com/web/updates/tags/clienthints
//
// To verify
// min-w/max-w/min-h/max-h + fit=crop a la imgix https://docs.imgix.com/apis/url/size/min-h


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


enum StrategyIfLarger2D {
    DownscaleWithin,
    DownscaleCrop,
    DownscaleCropCare,
    DownscalePad,

    Distort
}

enum StrategyIfMismatch {
    DownscaleWithin,
    Crop1D,
    CropThenPad,
    CropCareThenPad,
    CropThenUpscale,
    CropCareThenUpscale,
    Distort
}

enum StrategyIfSmaller2D {
    Pad,
    // Upscaling can bring us into the mismatch zone.
    UpscaleThenPad,
    UpscaleThenCropCare,
    UpscaleThenCrop,
    Distort,
    DistortAspect,
    CropToAspect,
    CropCareToAspect,
    PadToAspect
}

enum ComparisonState {
    Larger2D,
    Smaller2D,
    Mismatch
}
/////////////////////////////////////////////////////////
// Given a ScaleAbove and ScaleBelow which can cause one dimension to match the provided box
// And the other to be larger (or smaller), but with minimal loss of information.
// This can be used to achieve a state where we can reason about aspect ratio changes from
// an initial state of
enum PostScaleState {
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


// AspectModifier trait
// - provide pixel buffer or rect, desired aspect ratio, and a dimension constraint?
// - get resultant crop and resultant canvas


/// Here we combine a 1 dimensional constraint and a target aspect ratio.
/// We assume that the biggest delta dimension is processed first. I.e, width if the source image has a wider aspect ratio, height if the source image has a higher aspect ratio.
///
enum TargetAndAspect {}

/// Strategies for single dimension constraints: I.e, Width OR Height, but not both
enum Constraint1DAndAspectStrategy {
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


enum ConstraintStrategy {
    Width {
        w: u32,
        strategy: OneConstraintStrategy
    },
    Height {
        h: u32,
        strategy: OneConstraintStrategy
    },
    /// Keep the original aspect ratio. If the image is smaller than both bounds, don't upscale or pad.
    WithinBox {
        w: u32,
        h: u32
    },
}
