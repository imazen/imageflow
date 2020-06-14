//! `imageflow_riapi::sizing::` provides logic and constraint evaluation for determining sizes
//! of things in a layout. I.e, `WxH` of source imagery to copy. `WxH` of canvas, including padding, `WxH` of image within canvas.
//! It intentionally avoids dealing with positioning. The idea is that sizing determines output image size, and is therefore
//! something a user tunes separately from alignment issues within the canvas. For face/region of interest cropping
//! and careful cropping (resort to padding before cropping off a face, for example), it cannot know. Therefore it
//! accepts a `PartialCropProvider` to determine how close to the desired crop the provider is willing to go.
//! The provider is only used to adjust the layout sizes, but users should also have the provider handle alignment of crops.
use imageflow_helpers::preludes::from_std::*;
use std;

// Serves as a size *and* an aspect ratio. There's benefit to keeping these together.
// Rounding errors are problematic when they cause an off-by-one versus target width/height or original width/height.
// So aspect ratios include the fraction they were derived from, and implementors should round to these if one of the 2 dimensions matches.
#[derive(Copy, Clone,  Eq, PartialOrd, Ord)]
pub struct AspectRatio {
    pub w: i32, //Make private! We loose validation
    pub h: i32,
}

pub mod prelude{
    pub use super::{AspectRatio, steps, BoxKind, LayoutError, PartialCropProvider, IdentityCropProvider, Cond, BoxParam, BoxTarget, Step, Step1D};
}

impl AspectRatio {
    pub fn create(w: i32, h: i32) -> Result<AspectRatio> {
        if w < 1 || h < 1 {
            //panic!("");
            Err(LayoutError::InvalidDimensions { w, h })
        } else {
            Ok(AspectRatio {
                w,
                h,
            })
        }
    }

    pub fn ratio_f64(&self) -> f64{
        f64::from(self.w) / f64::from(self.h)
    }
    pub fn width(&self) -> i32{
        self.w
    }
    pub fn height(&self) -> i32{
        self.h
    }


    pub fn aspect_wider_than(&self, other: &AspectRatio) -> bool {
        other.ratio_f64() > self.ratio_f64()
    }
    pub fn transpose(&self) -> Result<AspectRatio>{
        AspectRatio::create(self.h, self.w)
    }

    /// Using own ratio, calculate height for given width
    pub fn height_for(&self, w: i32, potential_rounding_target: Option<&AspectRatio>) -> Result<i32> {
        AspectRatio::proportional(self.ratio_f64(), true, w, self.h, potential_rounding_target.map(|r| r.h).unwrap_or(self.h))
    }
    /// Using own ratio, calculate height for given width
    pub fn width_for(&self, h: i32, potential_rounding_target: Option<&AspectRatio>) -> Result<i32> {
        AspectRatio::proportional(self.ratio_f64(), false, h, self.w, potential_rounding_target.map(|r| r.w).unwrap_or(self.w))
    }

    pub fn proportional(ratio: f64, inverse: bool, basis: i32, snap_a: i32, snap_b: i32) -> Result<i32> {
        let float = if inverse {
            f64::from(basis) / ratio
        } else {
            ratio * f64::from(basis)
        };

        if (float - f64::from(snap_a)).abs() < 1f64 {
            Ok(snap_a)
        } else if (float - f64::from(snap_b)).abs() < 1f64 {
            Ok(snap_b)
        } else {
            let rounded = float.round();
            // We replace 0 with 1.
            if rounded <= f64::from(std::i32::MIN) || rounded >= f64::from(std::i32::MAX) {
                Err(LayoutError::ValueScalingFailed{
                    ratio,
                    basis,
                    invalid_result: rounded
                })
            } else{
                Ok(rounded as i32)
            }
        }.and_then(|v|
            if v < 0{
                Err(LayoutError::ValueScalingFailed{
                    ratio,
                    basis,
                    invalid_result: float
                })
            }else if v == 0{
                Ok(1)
            } else {
                Ok(v)
            }
        )

//        if let Err(ref e) = res {
//            panic!("{:?} during proportional({},{},{},{},{})", e, ratio, inverse, basis, snap_a, snap_b);
//        }
//        res
    }

    /// Create a ibox (inner box) or obox (outer box) using own ratio, but other's min/max box.
/// One dimension of the produced box will always match with `target`
    pub fn box_of(&self, target: &AspectRatio, kind: BoxKind) -> Result<AspectRatio> {
        if target.aspect_wider_than(self) == (kind == BoxKind::Inner) {
            //calculate height
            AspectRatio::create(target.w, self.height_for(target.w, Some(target))?)
        } else {
            AspectRatio::create(self.width_for(target.h, Some(target))?, target.h)
        }
    }

    pub fn size(&self) -> (i32, i32) {
        (self.w, self.h)
    }

    pub fn exceeds_any(&self, other: &AspectRatio) -> bool {
        self.w > other.w || self.h > other.h
    }
    pub fn exceeds_2d(&self, other: &AspectRatio) -> bool {
        self.w > other.w && self.h > other.h
    }


    pub fn intersection(&self, other: &AspectRatio) -> Result<AspectRatio> {
        AspectRatio::create(cmp::min(self.w, other.w), cmp::min(self.h, other.h))
    }

    pub fn distort_with(&self, other_old: &AspectRatio, other_new: &AspectRatio) -> Result<AspectRatio> {
        let new_w = mult_fraction(self.w, other_new.w, other_old.w)?;
        let new_h = mult_fraction(self.h, other_new.h, other_old.h)?;
        AspectRatio::create(new_w, new_h)
    }

    /// Returns a tuple of Ordering values for width/height respectively.
    pub fn cmp_size(&self, other: &AspectRatio) -> (Ordering, Ordering) {
        (self.width().cmp(&other.width()), self.height().cmp(&other.height()))
    }
}


impl std::hash::Hash for AspectRatio {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.w.hash(state);
        self.h.hash(state);
    }
}

impl PartialEq for AspectRatio{
    fn eq(&self, other: &AspectRatio) -> bool {
        self.w == other.w && self.h == other.h
    }
}
impl fmt::Debug for AspectRatio {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}x{}", self.w, self.h)
    }
}


fn mult_fraction(value: i32, num: i32, denom: i32) -> Result<i32> {
    Ok((i64::from(value) * i64::from(num) / i64::from(denom)) as i32)
}

#[test]
fn test_box_of_() {
    test_box_of();
}


#[cfg(test)]
fn test_box_of() {
    fn ratio(w: i32, h: i32) -> AspectRatio {
        AspectRatio::create(w, h).unwrap()
    }

    assert_eq!(Ok(ratio(4, 4)), ratio(8, 8).box_of(&ratio(4, 8), BoxKind::Inner));
    assert_eq!(Ok(ratio(8, 8)), ratio(32, 32).box_of(&ratio(4, 8), BoxKind::Outer));
    assert_eq!(Ok(ratio(3, 5)), ratio(20, 30).box_of(&ratio(3, 2), BoxKind::Outer));
}


#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BoxKind {
    Inner,
    Outer
}

#[derive(Copy, Clone, PartialEq, Debug, Hash)]
pub struct Layout {
    source_max: AspectRatio,
    source: AspectRatio,
    target: AspectRatio,
    canvas: AspectRatio,
    image: AspectRatio
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum LayoutError {
    NotImplemented,
    InvalidDimensions {
        w: i32,
        h: i32
    },
    ImpossiblePad {
        target: AspectRatio,
        current: AspectRatio
    },
    ImpossibleCrop {
        target: AspectRatio,
        current: AspectRatio
    },
    ValueScalingFailed {
        ratio: f64,
        basis: i32,
        invalid_result: f64
    },
    /// The result depends on the bitmap contents, and can't be calculated based on input size alone
    ContentDependent
}

pub type Result<T> = ::std::result::Result<T, LayoutError>;

impl Layout {
    pub fn scale_canvas(self, target: AspectRatio, sizing: BoxKind) -> Result<Layout> {
        let new_canvas = self.canvas.box_of(&target, sizing)?;
        Ok(Layout {
            image: self.image.distort_with(&self.canvas, &new_canvas)?,
            canvas: new_canvas,
            ..self
        })
    }
    pub fn fill_crop(self, target: AspectRatio) -> Result<Layout> {
        let new_source = target.box_of(&self.source, BoxKind::Inner)?;
        Ok(Layout {
            source: new_source,
            image: target,
            canvas: target,
            ..self
        })

    }

    //Also distorts 'image' in a corresponding fashion.
    pub fn distort_canvas(self, target: AspectRatio) -> Result<Layout> {
        let new_canvas = target;
        Ok(Layout {
            image: self.image.distort_with(&self.canvas, &new_canvas)?,
            canvas: new_canvas,
            ..self
        })
    }


    pub fn virtual_canvas(self, target: AspectRatio) -> Result<Layout> {
        let new_image = self.image.intersection(&target)?;
        let new_source = new_image.box_of(&self.source, BoxKind::Inner)?;
        Ok(Layout {
            source: new_source,
            image: new_image,
            canvas: target,
            ..self
        })
    }
    pub fn pad_canvas(self, target: AspectRatio) -> Result<Layout> {
        if self.canvas.exceeds_any(&target) {
            return Err(LayoutError::ImpossiblePad { target: target, current: self.canvas });
        }
        Ok(Layout {
            canvas: target,
            ..self
        })
    }

    pub fn crop(self, target: AspectRatio) -> Result<Layout> {
        if target.exceeds_any(&self.canvas) {
            return Err(LayoutError::ImpossibleCrop { target: target, current: self.canvas });
        }
        let new_image = self.image.intersection(&target)?;
        let new_source = new_image.box_of(&self.source, BoxKind::Inner)?;
        Ok(Layout {
            source: new_source,
            image: new_image,
            canvas: target,
            ..self
        })
    }
    pub fn get_box(&self, which: BoxTarget) -> AspectRatio {
        match which {
            BoxTarget::Target => self.target,
            BoxTarget::CurrentCanvas => self.canvas,
            BoxTarget::CurrentImage => self.image
        }
    }
    pub fn get_source_crop(&self) -> AspectRatio{
        self.source
    }

    pub fn resolve_box_param(&self, p: BoxParam) -> Result<AspectRatio> {
        match p {
            BoxParam::Exact(which) => Ok(self.get_box(which)),
            BoxParam::BoxOf { target, kind, ratio_source } => self.get_box(ratio_source).box_of(&self.get_box(target), kind)
        }
    }

    /// Modifies the other target dimension so that it isn't in play - then later restores it.
    pub fn execute_1d<T: PartialCropProvider>(self, horizontal: bool, step: Step1D, cropper: &T) -> Result<Layout> {
        let target_2d = self.target;
        let target_1d = if horizontal { AspectRatio::create(self.target.w, self.canvas.h) } else { AspectRatio::create(self.canvas.w, self.target.h) }?;
        let canvas = self.canvas;
        let layout1d = Layout {
            target: target_1d,
            ..self
        };
        let step_2d = match step {
            Step1D::ScaleProportional if canvas.aspect_wider_than(&target_1d) && horizontal => Step::ScaleToInner,
            Step1D::ScaleProportional if canvas.aspect_wider_than(&target_1d) && !horizontal => Step::ScaleToOuter,
            Step1D::ScaleProportional if target_1d.aspect_wider_than(&canvas) && horizontal => Step::ScaleToInner,
            Step1D::ScaleProportional if target_1d.aspect_wider_than(&canvas) && !horizontal => Step::ScaleToOuter,
            Step1D::ScaleProportional => Step::ScaleToInner,
            Step1D::Crop => Step::Crop,
            Step1D::PartialCrop => Step::PartialCrop,
            Step1D::Pad => Step::Pad,
            Step1D::Distort => Step::Distort(BoxParam::Exact(BoxTarget::Target)),
            Step1D::VirtualCanvas => Step::VirtualCanvas(BoxParam::Exact(BoxTarget::Target)),
        };
        let modified_layout = layout1d.execute_step(step_2d, cropper)?;
        Ok(Layout {
            target: target_2d,
            ..modified_layout
        })
    }
    pub fn execute_step<T: PartialCropProvider>(self, step: Step, cropper: &T) -> Result<Layout> {
        match step {
            Step::None | Step::BeginSequence | Step::SkipIf(_) | Step::SkipUnless(_) => Ok(self),
            Step::ScaleToOuter => self.scale_canvas(self.target, BoxKind::Outer),
            Step::FillCrop => self.fill_crop(self.target),
            Step::ScaleToInner => self.scale_canvas(self.target, BoxKind::Inner),
            Step::PadAspect => self.pad_canvas(self.target.box_of(&self.canvas, BoxKind::Outer)?),
            Step::Pad => self.pad_canvas(self.target),
            Step::CropAspect => self.crop(self.target.box_of(&self.canvas, BoxKind::Inner)?),
            Step::Crop => self.crop(self.target),
            Step::CropToIntersection => self.crop(self.image.intersection(&self.target)?),
            Step::VirtualCanvas(param) => self.virtual_canvas(self.resolve_box_param(param)?),
            Step::Distort(param) => self.distort_canvas(self.resolve_box_param(param)?),
            Step::PartialCropAspect => cropper.crop_size(self, self.target.box_of(&self.canvas, BoxKind::Inner)?),
            Step::PartialCrop => cropper.crop_size(self, self.target),
            Step::X(x_step) => self.execute_1d(true, x_step, cropper),
            Step::Y(y_step) => self.execute_1d(false, y_step, cropper),
        }
    }

    pub fn evaluate_condition(&self, c: Cond) -> bool {
        c.matches(self.canvas.cmp_size(&self.target))
    }
    pub fn execute_all<T: PartialCropProvider>(self, steps: &[Step], cropper: &T) -> Result<Layout> {
        let mut lay = self;
        let mut skipping = false;
        for step in steps {
            match *step {
                Step::SkipIf(c) if lay.evaluate_condition(c) => {
                    //Skip to next Step::BeginSequence
                    skipping = true;
                },
                Step::SkipUnless(c) if !lay.evaluate_condition(c) => {
                    //Skip to next Step::BeginSequence
                    skipping = true;
                },
                Step::BeginSequence => {
                    skipping = false;
                },
                _ => {}
            }
            if !skipping {
                lay = lay.execute_step(*step, cropper)?;
            }
        }
        Ok(lay)
    }

    pub fn create(original: AspectRatio, target: AspectRatio) -> Layout {
        Layout {
            canvas: original,
            image: original,
            source: original,
            source_max: original,
            target
        }
    }
}

/// Implements `PartialCropProvider` but always crops fully - same as regular crop
#[derive(Default)]
pub struct IdentityCropProvider {}
impl IdentityCropProvider{
    pub fn new() -> IdentityCropProvider{
        IdentityCropProvider{}
    }
}
impl PartialCropProvider for IdentityCropProvider{
    fn crop_size(&self, lay: Layout, target: AspectRatio) -> Result<Layout> {
        lay.crop(target)
    }
}

pub trait PartialCropProvider {
    fn crop_size(&self, lay: Layout, target: AspectRatio) -> Result<Layout>;
}

/// After distortion, Crop/VirtualCanvas will undo stretching by cropping proportionally.


//(ratio(canvas|target|image), outer(canvas|target|image)|inner(canvas|target|image)
//scale uses the canvas ratio
// All others default to target ratio
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BoxTarget {
    Target,
    CurrentCanvas,
    CurrentImage,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BoxParam {
    Exact(BoxTarget),
    BoxOf {
        target: BoxTarget,
        kind: BoxKind,
        ratio_source: BoxTarget
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Cond {

    Is((Ordering, Ordering)),
    Not((Ordering, Ordering)),
    WidthIs(Ordering),
    HeightIs(Ordering),
    Both(Ordering),
    WidthNot(Ordering),
    HeightNot(Ordering),
    Neither(Ordering),
    Either(Ordering),
    /// larger than the target in both dimensions
    Larger2D,
    /// smaller than the target in both dimensions
    Smaller2D,
    /// larger than the target in one dimension and smaller in the other
    Larger1DSmaller1D,
    /// Exact match
    Equal,
    /// Neither dimension is a match
    Differs2D,
    ///Always true
    True,
}
impl Cond {
    pub fn matches(&self, cmp: (Ordering, Ordering)) -> bool{
        match *self{
            Cond::Is(pair) => pair == cmp,
            Cond::Not(pair) => pair != cmp,
            Cond::Larger2D => Cond::Both(Ordering::Greater).matches(cmp),
            Cond::Smaller2D => Cond::Both(Ordering::Less).matches(cmp),
            Cond::Equal => Cond::Both(Ordering::Equal).matches(cmp),
            Cond::True => true,
            Cond::Larger1DSmaller1D => cmp == (Ordering::Greater, Ordering::Less) || cmp == (Ordering::Less, Ordering::Greater),
            //Cond::Larger1D => Cond::Either(Ordering::Greater).matches(cmp),
            //Cond::Smaller1D => Cond::Either(Ordering::Less).matches(cmp),
            Cond::Differs2D => Cond::Neither(Ordering::Equal).matches(cmp),
            Cond::WidthIs(v) => cmp.0 == v,
            Cond::WidthNot(v) => cmp.0 != v,
            Cond::HeightIs(v) => cmp.1 == v,
            Cond::HeightNot(v) => cmp.1 != v,
            Cond::Both(v) => cmp.0 == v && cmp.1 == v,
            Cond::Neither(v) => cmp.0 != v && cmp.1 != v,
            Cond::Either(v) => cmp.0 == v || cmp.1 == v,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Step1D {
    Pad,
    Crop,
    PartialCrop,
    VirtualCanvas,
    Distort,
    ScaleProportional
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Step {
    None,
    ///Indicates the start of a new sequence.
    BeginSequence,
    /// Fast-forwards to the next sequence if the condition fails
    SkipIf(Cond),
    SkipUnless(Cond),
    /// Scales the canvas, using the canvas ratio. Use distort to use the target ratio (or another)
    ScaleToOuter,
    ScaleToInner,
    Distort(BoxParam),
    Pad,
    PadAspect,
    Crop, //What about intersect? Crop that doesn't fail out of bounds
    CropToIntersection,
    CropAspect,
    /// Use ScaleToOuterAndCrop instead of ScaleToOuter, then Crop, because the combination can reduce the dimensions below the outer box
    FillCrop,
    /// We can use a variety of hints, and we're not required to fully change the aspect ratio or achieve the target box
    PartialCrop,
    PartialCropAspect,
    /// Can simultaneously crop in one dimension and pad in another.
    VirtualCanvas(BoxParam),
    /// Work on widths
    X(Step1D),
    /// Work on heights
    Y(Step1D),

    // Uncrop (within source_max)?

    // PermitBlockAlignedEdges(4/8) - crop more, uncrop, scaleup, scaledown, unpad, distort?
}

pub struct StepsBuilder {
    steps: Vec<Step>
}

impl StepsBuilder {
    pub fn crop(mut self) -> StepsBuilder {
        self.steps.push(Step::Crop);
        self
    }
    pub fn crop_intersection(mut self) -> StepsBuilder {
        self.steps.push(Step::CropToIntersection);
        self
    }

    pub fn crop_aspect(mut self) -> StepsBuilder {
        self.steps.push(Step::CropAspect);
        self
    }
    pub fn crop_partial(mut self) -> StepsBuilder {
        self.steps.push(Step::PartialCrop);
        self
    }
    pub fn crop_partial_aspect(mut self) -> StepsBuilder {
        self.steps.push(Step::PartialCropAspect);
        self
    }
    pub fn pad(mut self) -> StepsBuilder {
        self.steps.push(Step::Pad);
        self
    }
    pub fn pad_aspect(mut self) -> StepsBuilder {
        self.steps.push(Step::PadAspect);
        self
    }
    pub fn new_seq(mut self) -> StepsBuilder {
        self.steps.push(Step::BeginSequence);
        self
    }
    pub fn skip_if(mut self, c: Cond) -> StepsBuilder {
        self.steps.push(Step::SkipIf(c));
        self
    }
    pub fn skip_unless(mut self, c: Cond) -> StepsBuilder {
        self.steps.push(Step::SkipUnless(c));
        self
    }
    pub fn scale_to_inner(mut self) -> StepsBuilder {
        self.steps.push(Step::ScaleToInner);
        self
    }
    pub fn scale_to_outer(mut self) -> StepsBuilder {
        self.steps.push(Step::ScaleToOuter);
        self
    }
    pub fn fill_crop(mut self) -> StepsBuilder {
        self.steps.push(Step::FillCrop);
        self
    }
    pub fn distort(mut self, t: BoxParam) -> StepsBuilder {
        self.steps.push(Step::Distort(t));
        self
    }
    pub fn virtual_canvas(mut self, t: BoxParam) -> StepsBuilder {
        self.steps.push(Step::VirtualCanvas(t));
        self
    }
    pub fn x(mut self, s: Step1D) -> StepsBuilder {
        self.steps.push(Step::X(s));
        self
    }
    pub fn y(mut self, s: Step1D) -> StepsBuilder {
        self.steps.push(Step::Y(s));
        self
    }
    pub fn into_vec(self) -> Vec<Step> {
        self.steps
    }
}

pub fn steps() -> StepsBuilder {
    StepsBuilder {
        steps: vec![]
    }
}
