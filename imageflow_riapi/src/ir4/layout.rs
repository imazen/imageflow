use imageflow_helpers::preludes::from_std::*;
use ::std;
use ::imageflow_types as s;
use ::sizing;
use ::sizing::prelude::*;
use ::ir4::parsing::*;


pub struct Ir4Layout{
    i: Instructions,
    /// source width
    w: i32,
    h: i32
}

pub struct Ir4LayoutInfo{
    pub canvas: AspectRatio
}
impl Ir4Layout{

    pub fn new(i: Instructions,

               w: i32,
               h: i32) -> Ir4Layout{
        Ir4Layout{
            i: i, w: w, h: h
        }
    }



    fn get_precrop(&self) -> (i32,i32){
        if((self.i.srotate.unwrap_or(0) / 90 + 4) % 2) == 0 {
            (self.w, self.h)
        } else {
            (self.h, self.w)
        }
    }

    fn get_wh_from_all(&self, source: AspectRatio) -> sizing::Result<(Option<i32>, Option<i32>)>{
        let mut w = self.i.w.unwrap_or(-1);
        let mut h = self.i.h.unwrap_or(-1);
        let mut mw = self.i.legacy_max_width.unwrap_or(-1);
        let mut mh = self.i.legacy_max_height.unwrap_or(-1);


        //Eliminate cases where both a value and a max value are specified: use the smaller value for the width/height
        if mw > 0 && w > 0 { w = cmp::min(mw, w); mw = -1; }
        if mh > 0 && h > 0 { h = cmp::min(mh, h); mh = -1; }

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

        Ok((if w < 1 { None } else { Some(w) }, if h < 1 { None } else { Some(h)}))
    }

    fn get_ideal_target_size(&self, source: AspectRatio) -> sizing::Result<AspectRatio>{


        let (w,h) = match self.get_wh_from_all(source)? {
            (Some(w), Some(h)) => (w,h),
            (None, None) => (source.w, source.h),
            (Some(w), None) => (w, source.height_for(w, None)?),
            (None, Some(h)) => (source.width_for(h, None)?,h)
        };

        //if all dimensions are absent, support zoom=x + scale=canvas | scale=both
        // and exit
        //No more than 1/80000 or 80000/1
        let zoom = Self::float_max(0.000_08f64, Self::float_min(self.i.zoom.unwrap_or(1f64), 80_000f64).unwrap()).unwrap();

        //Apply zoom directly to target dimensions. This differs from IR4 but should be easier to reason about.
        let w = Self::float_max(1f64, Self::float_min((f64::from(w) * zoom).round(), f64::from(std::i32::MAX)).unwrap()).unwrap() as i32;
        let h = Self::float_max(1f64, Self::float_min((f64::from(h) * zoom).round(), f64::from(std::i32::MAX)).unwrap()).unwrap() as i32;

        AspectRatio::create(w,h)
    }

    fn float_min(a: f64, b: f64) -> Option<f64>{
        let a_comparable = !a.is_nan() && !a.is_infinite();
        let b_comparable = !b.is_nan() && !b.is_infinite();
        if a_comparable && b_comparable {
            Some( if a < b { a } else { b })
        }else if a_comparable {
            Some(a)
        }else if b_comparable {
            Some(b)
        }else{
            None
        }
    }
    fn float_max(a: f64, b: f64) -> Option<f64>{
        let a_comparable = !a.is_nan() && !a.is_infinite();
        let b_comparable = !b.is_nan() && !b.is_infinite();
        if a_comparable && b_comparable {
            Some( if a > b { a } else { b })
        }else if a_comparable {
            Some(a)
        }else if b_comparable {
            Some(b)
        }else{
            None
        }
    }

    // Build constraint set, just from "mode" and "scale" (and the absence of width/height, for overriding "mode").
    // Keep in mind that crop=auto and scale=fill were normalized when parsing Instructions.
    fn build_constraints(&self) -> Vec<Step>{
        // if both w/width and h/height are absent, force mode=max regardless of current setting
        let mode = if self.i.w.is_none() && self.i.h.is_none(){
            FitMode::Max
        } else{
            self.i.mode.unwrap_or(FitMode::Pad)
        };


        match (mode,self.i.scale.unwrap_or(ScaleMode::DownscaleOnly)){
            //Max is a misnomer. It scales up proportionally, as well. With scale=canvas, it produces padding.
            (FitMode::Max, ScaleMode::DownscaleOnly) => {
                //scale to ibox, unless original is not larger than the box
                steps().skip_unless(Cond::Either(Ordering::Greater)).scale_to_inner()
            },
            (FitMode::Max, ScaleMode::UpscaleOnly) => {
                //if original is equal or less than both target dimensions, scale up within. Otherwise retain original size/aspect.
                steps().skip_unless(Cond::Neither(Ordering::Greater)).scale_to_inner()
            },
            (FitMode::Max, ScaleMode::Both) => {
                //scale to the inner box, always. Surprising?
                steps().scale_to_inner()
            },
            (FitMode::Max, ScaleMode::UpscaleCanvas) => {
                //Don't upscale the inner box.
                //Pad to the inner box of the target.
                steps().skip_unless(Cond::Either(Ordering::Greater)).scale_to_inner()
                    .new_seq().virtual_canvas(BoxParam::BoxOf{ target: BoxTarget::Target, ratio_source: BoxTarget::CurrentCanvas, kind: BoxKind::Inner})
            },
            (FitMode::Pad, ScaleMode::DownscaleOnly) => {
                //scale within box and pad, unless original is not larger than the box.
                steps().skip_unless(Cond::Either(Ordering::Greater)).scale_to_inner().pad()
                //If the image is smaller, we lose aspect ratio and it reverts to normal. Surprising?
            },
            (FitMode::Pad, ScaleMode::UpscaleOnly) => {
                //if original is equal or less than both target dimensions, scale up and pad. Otherwise retain original size/aspect.
                steps().skip_unless(Cond::Neither(Ordering::Greater)).scale_to_inner().pad()
            },
            (FitMode::Pad, ScaleMode::Both) => {
                //scale to the inner box and pad to target, always.
                steps().scale_to_inner().pad()
            },
            (FitMode::Pad, ScaleMode::UpscaleCanvas) => {
                //Don't upscale the inner box.
                //Pad to the inner box of the target.
                steps().skip_unless(Cond::Either(Ordering::Greater)).scale_to_inner()
                    .new_seq().pad()
            },
            (FitMode::Stretch, ScaleMode::DownscaleOnly) => {
                steps().skip_unless(Cond::Either(Ordering::Greater)).distort(BoxParam::Exact(BoxTarget::Target))
            },
            (FitMode::Stretch, ScaleMode::UpscaleOnly) => {
                //if original is equal or less than both target dimensions, distort. Otherwise retain original size/aspect.
                steps().skip_unless(Cond::Neither(Ordering::Greater)).distort(BoxParam::Exact(BoxTarget::Target))
            },
            (FitMode::Stretch, ScaleMode::Both) => {
                steps().distort(BoxParam::Exact(BoxTarget::Target))
            },
            (FitMode::Stretch, ScaleMode::UpscaleCanvas) => {
                //Don't upscale the inner box.
                //Pad to the inner box of the target.
                steps().skip_unless(Cond::Either(Ordering::Greater)).distort(BoxParam::Exact(BoxTarget::Target))
                    .new_seq().pad()
            },
            (FitMode::Crop, ScaleMode::DownscaleOnly) => {
                //We can't compare against the obox, so we have to use a partwise constraint
                //The first doesn't affect Large1DSmaller1D scenarios, only Larger2d or equal.
                //The second only receives equal, mixed, or less. It deals with mixed, as the only
                //batch requiring work.
                steps().skip_if(Cond::Either(Ordering::Less)).scale_to_outer().crop()
                    .new_seq().skip_unless(Cond::Larger1DSmaller1D).crop_intersection()
            },
            (FitMode::Crop, ScaleMode::UpscaleOnly) => {
                // mode=crop&scale=up only takes effect when no target dimension is smaller than the
                // source.
                steps().skip_unless(Cond::Neither(Ordering::Greater)).scale_to_outer().crop()
            },
            (FitMode::Crop, ScaleMode::Both) => {
                //scale to the outer box and crop to target, always. Easy.
                steps().scale_to_outer().crop()
            },
            (FitMode::Crop, ScaleMode::UpscaleCanvas) => {
                // We can't compare against the obox, so we have to use a partwise constraint
                // The first doesn't affect Large1DSmaller1D scenarios, only Larger2d or equal.
                // The second only receives equal, mixed, or less.
                steps().skip_if(Cond::Either(Ordering::Less)).scale_to_outer().crop()
                    .new_seq().skip_unless(Cond::Larger1DSmaller1D).virtual_canvas(BoxParam::Exact(BoxTarget::Target))
            },
        }.into_vec()
    }


    pub fn get_downscaling(&self) ->  sizing::Result<(AspectRatio, AspectRatio)> {
        let (_, layout) = self.get_crop_and_layout()?;

        let new_crop = layout.get_source_crop();
        let image = layout.get_box(BoxTarget::CurrentImage);
        Ok((new_crop, image))
    }



    pub fn get_crop_and_layout(&self) -> sizing::Result<(Option<[u32;4]>,sizing::Layout)> {
        let (precrop_w, precrop_h) = self.get_precrop();

        // later consider adding f.sharpen, f.ignorealpha
        // (up/down).(filter,window,blur,preserve,colorspace,speed)

        let initial_crop = self.get_initial_copy_window(precrop_w, precrop_h);

        let initial_size = sizing::AspectRatio::create(initial_crop[2] - initial_crop[0], initial_crop[3] - initial_crop[1])?;

        let target = self.get_ideal_target_size(initial_size)?;

        let constraints = self.build_constraints();

        //We would change this for face or ROI support
        let cropper = sizing::IdentityCropProvider::new();

        // ======== This is where we do the sizing and constraint evaluation \/
        let layout = sizing::Layout::create(initial_size, target).execute_all(&constraints, &cropper)?;

        //println!("executed constraints {:?} to get layout {:?} from target {:?}", &constraints, &layout, &target);
        let new_crop = layout.get_source_crop();

        let align = self.i.anchor.unwrap_or((Anchor1D::Center, Anchor1D::Center));
        //align crop
        let (inner_crop_x1, inner_crop_y1) = Self::align(align, new_crop, initial_size).expect("Outer box should never be smaller than inner box. All values must > 0");
        //add manual crop offset
        let (crop_x1, crop_y1) = ((initial_crop[0] + inner_crop_x1) as u32, (initial_crop[1] + inner_crop_y1) as u32);

        //println!("Crop initial={:?}, new: {:?}, x1: {}, y1: {}", &initial_crop, &new_crop, crop_x1, crop_y1);
        let final_crop = if crop_x1 > 0 || crop_y1 > 0 || precrop_w != new_crop.width() || precrop_h != new_crop.height() {
            Some([crop_x1, crop_y1, crop_x1 + new_crop.width() as u32, crop_y1 + new_crop.height() as u32])
        }else{
            None
        };
        Ok((final_crop,layout))

    }


    /// Does not add trimwhitespace or decode/encode
    pub fn add_steps(&self, b: &mut FramewiseBuilder) -> sizing::Result<Ir4LayoutInfo> {
        b.add_rotate(self.i.srotate);
        b.add_flip(self.i.sflip);


        let (crop, layout) = self.get_crop_and_layout()?;

        let new_crop = layout.get_source_crop();
        let canvas = layout.get_box(BoxTarget::CurrentCanvas);
        let image = layout.get_box(BoxTarget::CurrentImage);
        let align = self.i.anchor.unwrap_or((Anchor1D::Center, Anchor1D::Center));

        if let Some(c) = crop{
            b.add(s::Node::Crop { x1: c[0], y1: c[1], x2: c[2], y2: c[3] });
        }

        //Scale
        if image.width() != new_crop.width() || image.height() != new_crop.height() || self.i.f_sharpen.unwrap_or(0f64) > 0f64 {
            let downscaling = image.width() < new_crop.width() || image.height() < new_crop.height();
            b.add(s::Node::Resample2D {
                w: image.width() as u32,
                h: image.height() as u32,
                down_filter: None,
                up_filter: None,
                scaling_colorspace: match self.i.down_colorspace {
                    Some(ScalingColorspace::Linear) if downscaling => Some(s::ScalingFloatspace::Linear),
                    Some(ScalingColorspace::Srgb) if downscaling => Some(s::ScalingFloatspace::Srgb),
                    _ => None

                },
                hints: Some(s::ResampleHints { sharpen_percent: self.i.f_sharpen.map(|v| v as f32) })
            });
        }


        // Perform white balance
        if Some(HistogramThresholdAlgorithm::Area) == self.i.a_balance_white{
            b.add( s::Node::WhiteBalanceHistogramAreaThresholdSrgb {
                threshold: None
            });
        }

        if let Some(c) = self.i.s_contrast {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Contrast(c as f32)));
        }
        if let Some(c) = self.i.s_alpha {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Alpha(c as f32)));
        }
        if let Some(c) = self.i.s_brightness {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Brightness(c as f32)));
        }
        if let Some(c) = self.i.s_saturation {
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Saturation(c as f32)));
        }
        if let Some(true) = self.i.s_sepia{
            b.add(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Sepia));
        }
        if let Some(g) = self.i.s_grayscale {
            b.add(s::Node::ColorFilterSrgb(match g{
                GrayscaleAlgorithm::Flat => s::ColorFilterSrgb::GrayscaleFlat,
                GrayscaleAlgorithm::True |
                GrayscaleAlgorithm::Ntsc |
                GrayscaleAlgorithm::Y => s::ColorFilterSrgb::GrayscaleNtsc,
                GrayscaleAlgorithm::Bt709 => s::ColorFilterSrgb::GrayscaleBt709,
                GrayscaleAlgorithm::Ry => s::ColorFilterSrgb::GrayscaleRy
            }));
        }

        //get bgcolor - default to transparent white
        let bgcolor = self.i.bgcolor_srgb.map(|v| v.to_rrggbbaa_string()).map(|str| s::Color::Srgb(s::ColorSrgb::Hex(str)));

        let default_bgcolor = s::Color::Srgb(s::ColorSrgb::Hex("FFFFFF00".to_owned()));

        let (left, top) = Self::align(align, image, canvas).expect("Outer box should never be smaller than inner box. All values must > 0");

        let (right, bottom) = (canvas.width() - image.width() - left, canvas.height() - image.height() - top);
        //Add padding. This may need to be revisited - how do jpegs behave with transparent padding?
        if left > 0 || top > 0 || right > 0 || bottom > 0 {
            if left >= 0 && top >= 0 && right >= 0 && bottom >= 0 {
                b.add(s::Node::ExpandCanvas { color: bgcolor.clone().unwrap_or(default_bgcolor), left: left as u32, top: top as u32, right: right as u32, bottom: bottom as u32 });
            } else {
                panic!("Negative padding showed up: {},{},{},{}", left, top, right, bottom);
            }
        }


        b.add_rotate(self.i.rotate);
        b.add_flip(self.i.flip);

        Ok(Ir4LayoutInfo {
            canvas: canvas
        })
    }


    fn align1d(a: Anchor1D, inner: i32, outer: i32) -> std::result::Result<i32, ()>{
        if outer < inner && inner < 1 || outer < 1 {
            Err(())
        }else{
            Ok(match a{
                Anchor1D::Near => 0,
                Anchor1D::Center => (outer - inner) /2,
                Anchor1D::Far => outer - inner
            })
        }
    }
    fn align(alignment: (Anchor1D, Anchor1D), inner: AspectRatio, outer: AspectRatio) -> std::result::Result<(i32,i32),()>{
        let (x,y) = alignment;
        Ok((Self::align1d(x,inner.width(), outer.width())?, Self::align1d(y, inner.height(), outer.height())?))
    }

    fn get_initial_copy_window(&self, w: i32, h: i32) -> [i32;4]{
        let floats = self.get_initial_copy_window_floats(w,h);
        let maximums = [w, h];
        let ints = floats.iter().enumerate().map(|(ix, item)| {
            cmp::max(0i32, cmp::min(item.round() as i32, maximums[ix % 2]))
        }).collect::<Vec<i32>>();
        if ints[3] <= ints[1] || ints[2] <= ints[0]{
            //violation of X2 > X1 or Y2 > Y1
            [0,0, w, h]
        }else {
            [ints[0], ints[1], ints[2], ints[3]]
        }
    }

    fn get_initial_copy_window_floats(&self, original_width: i32, original_height: i32) -> [f64;4]{
        let defaults = [0f64, 0f64, f64::from(original_width), f64::from(original_height)];
        if let Some(values) = self.i.crop{
            let xunits = self.i.cropxunits.map(|v| if v == 0f64 {f64::from(original_width)} else { v }).unwrap_or(f64::from(original_width));
            let yunits = self.i.cropyunits.map(|v| if v == 0f64 {f64::from(original_height)} else { v }).unwrap_or(f64::from(original_height));
            let floats = values.iter().enumerate().map(|(ix, item)| {
                let relative_to = if ix % 2 == 0 { xunits } else { yunits} as f64;
                let max_dimension = f64::from(if ix % 2 == 0 {original_width } else {original_height});
                let mut v = *item * max_dimension / relative_to;
                if ix < 2 && v < 0f64 || ix > 1 && v <= 0f64{
                    v += max_dimension; //Support negative offsets from bottom right.
                }
                v
            }).collect::<Vec<f64>>();
            if floats[3] <= floats[1] || floats[2] <= floats[0] {
                //violation of X2 > X1 or Y2 > Y1
                defaults
            }else{
                [floats[0], floats[1], floats[2], floats[3]]
            }

        }else{
            defaults
        }
    }

}
#[derive(Default)]
pub struct FramewiseBuilder{
    steps: Vec<s::Node>
}

impl FramewiseBuilder {

    pub fn new() -> FramewiseBuilder{
        FramewiseBuilder{ steps: vec![]}
    }
    fn add_flip(&mut self, f: Option<(bool, bool)>){
        if  let Some((h,v)) = f{
            if h { self.steps.push(s::Node::FlipH); }
            if v { self.steps.push(s::Node::FlipV); }
        }
    }
    fn add_rotate(&mut self, r: Option<i32>) {
        if let Some(rot) = r {
            self.add_maybe(match ((rot / 90) + 4) % 4 {
                1 => Some(s::Node::Rotate90),
                2 => Some(s::Node::Rotate180),
                3 => Some(s::Node::Rotate270),
                _ => None
            });
        }
    }
    fn add_maybe(&mut self, n : Option<s::Node>){
        if let Some(node) = n{
            self.steps.push(node);
        }
    }
    pub fn add(&mut self, n: s::Node){
        self.steps.push(n);
    }
    pub fn into_steps(self) -> Vec<s::Node>{
        self.steps
    }
}

#[test]
fn test_crop_and_scale(){
    let mut b = FramewiseBuilder::new();

    let l  = Ir4Layout::new(Instructions{w: Some(100), h: Some(200), mode: Some(FitMode::Crop), .. Default::default() }, 768, 433);
    l.add_steps(&mut b).unwrap();

    assert_eq!(b.steps, vec![s::Node::Crop { x1: 275, y1: 0, x2: 492, y2: 433 }, s::Node::Resample2D { w: 100, h: 200, down_filter: None, up_filter: None, scaling_colorspace: None, hints: Some(s::ResampleHints { sharpen_percent: None }) }]);
}


#[test]
fn test_scale(){
    let mut b = FramewiseBuilder::new();

    let l  = Ir4Layout::new(Instructions{w: Some(2560), h: Some(1696), mode: Some(FitMode::Max), .. Default::default() }, 5104, 3380);
    l.add_steps(&mut b).unwrap();
    assert_eq!(b.steps, vec![s::Node::Resample2D { w: 2560, h: 1696, down_filter: None, up_filter: None, scaling_colorspace: None, hints: Some(s::ResampleHints { sharpen_percent: None}) }]);

    // 5104x3380 "?w=2560&h=1696&mode=max&format=png&decoder.min_precise_scaling_ratio=2.1&down.colorspace=linear"


}
