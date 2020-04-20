use imageflow_helpers::preludes::from_std::*;
use std;
use crate::sizing::{steps, BoxParam, BoxTarget, AspectRatio, Cond, Step, Layout, LayoutError, BoxKind};
use crate::sizing;

#[derive(Copy, Clone, PartialEq, Debug)]
enum Strategy {
    /// Downscale the image until it fits within the box (less than both and matching at least one dimension).
/// Never upscale, even if the image is smaller in both dimensions.
///
/// `down.fitw=scale(w, auto) down.fith=scale(auto,h) up.fit=none`
/// `down.fit=proportional(target), up.fit=none`
/// `down.fit=proportional(ibox), up.fit=none`
    Max,
    /// Downscale minimally until the image fits one of the dimensions (it may exceed the other). Never upscale.
///
/// `down.fitw=scale(max(w, obox.w), auto) down.fith=scale(auto,max(h, obox.h) up.fit=none`
/// `down.fit=proportional(obox), up.fit=none`
    #[allow(dead_code)]
    MaxAny,
    /// Upscale minimally until one dimension matches. Never downscale, if larger.
/// `up.fit=scale(max(d, ibox.other), auto) down.fit=none`
/// `up.fit=proportional(ibox), down.fit=none`
    #[allow(dead_code)]
    MinAny,
    /// Upscale minimally until the image meets or exceeds both specified dimensions. Never downscale.
/// `up.fit=scale(d, auto) up.fit=none`
/// `up.fit=proportional(obox), down.fit=none`
    #[allow(dead_code)]
    Min,
    /// Downscale the image and pad to meet aspect ratio. If smaller in both dimensions, give up and leave as-is.
/// `down.fit=proportional(ibox), pad(target), up.fit=none` - won't work, second dimension will classify as upscale.
/// `down.fit=proportional(ibox), pad2d(target), up.fit=none`
    PadDownscaleOnly,
    /// Downscale the image and crop to meet aspect ratio. If smaller in both dimensions, give up and leave as-is.
/// `down.fit=proportional(obox),crop(target) up.fit=none`
    CropDownscaleOnly,

    /// Downscale & pad. If smaller, pad to achieve desired aspect ratio.
    PadOrAspect,

    /// Downscale & crop. If smaller, crop to achieve desired aspect ratio.
/// `down.fit=proportional(obox),crop(target) up.fit=cropaspect(target)`
    CropOrAspect,
    /// Downscale & crop. If smaller, pad to achieve desired aspect ratio.
/// `down.fit=proportional(obox),crop(target) up.fit=padaspect(target)`
    #[allow(dead_code)]
    CropOrAspectPad,

    // perhaps a lint for pad (or distort) in down. but not up. ?

    /// Minimally pad to match desired aspect ratio. Downscale or upscale to exact dimensions provided.
/// `always.fit.xy=proportional(ibox),pad(target)`
    #[allow(dead_code)]
    ExactPadAllowUpscaling,
    /// Minimally crop to match desired aspect ratio. Downscale or upscale to exact dimensions provided.
/// `up.fit.xy=proportional(ibox),crop(target)`, `down.fit.xy=proportional(obox), crop(target)
    #[allow(dead_code)]
    ExactCropAllowUpscaling,

    /// `always.fit.xy=distort(target)`
    Distort,
    /// `down.fit.xy=proportional(obox),cropcareful(target).scale(ibox)`
    #[allow(dead_code)]
    CropCarefulDownscale,
    /// `down.fit.xy=proportional(obox),cropcareful(target),proportional(target),pad2d(target)` -doesn't work; second dimension never executes pad. (unless we run smaller dimension first)
/// `down.fit.xy=proportional(obox),cropcareful(target),proportional(target),pad2d(target.aspect)` -doesn't work; second dimension never executes pad. (unless we run smaller dimension first)
    #[allow(dead_code)]
    CropCarefulPadDownscale,
    /// `down.fit.xy=proportional(obox),cropcareful(target),proportional(target),pad(target) up.xy.fit=cropcareful(targetaspect),pad(targetaspect)`
    #[allow(dead_code)]
    CropCarefulDownscaleOrForceAspect,

    // When cropping an image to achieve aspect ratio makes it smaller than the desired box, thereby changing the rules...
    // Ie, box of 20x30, image of 60x20. Crop to 14x20 leaves 6x10 gap.
    // Results should match downscaling rules, right?
    //
    // Alternate options: pad mismatched dimension: crop to 20x20, add 10px vertical padding.
    // Alternate: crop to 14x20 and upscale to 20x30


}

fn strategy_to_steps(s: Strategy) -> Option<Vec<Step>> {
    let vec = match s {
        Strategy::Distort => { steps().distort(BoxParam::Exact(BoxTarget::Target)) },
        // Like imageresizer max
        Strategy::Max => { steps().skip_unless(Cond::Either(Ordering::Greater)).scale_to_inner()}
        Strategy::MaxAny => { steps().skip_unless(Cond::Larger2D).scale_to_outer() }
        Strategy::MinAny => { steps().skip_unless(Cond::Smaller2D).scale_to_inner()}
        Strategy::Min => { steps().skip_unless(Cond::Either(Ordering::Less)).scale_to_outer() },

        Strategy::PadOrAspect => { steps().skip_if(Cond::Both(Ordering::Less)).scale_to_inner().pad().new_seq().pad_aspect() },
        Strategy::PadDownscaleOnly => { steps().skip_if(Cond::Both(Ordering::Less)).scale_to_inner().pad() },

        //Scale_to_outer can reduce the width, then crop the height, causing both coordinates to be smaller
        //TODO: perhaps combine scale_to_outer and crop() into a single operation to prevent this?
        Strategy::CropOrAspect => { steps().skip_if(Cond::Either(Ordering::Less)).fill_crop()
            .new_seq().skip_unless(Cond::Either(Ordering::Less)).crop_aspect() },


        //I think we need multiple parts, as we don't offer a way to compare against the obox
        Strategy::CropDownscaleOnly => { steps().skip_if(Cond::Either(Ordering::Less)).fill_crop().new_seq().skip_unless(Cond::Larger1DSmaller1D).crop_intersection() },
        //        Strategy::CropCarefulDownscale => StepSet::AnyLarger(vec![Step::ScaleToOuter,
        //        Step::PartialCropAspect, Step::ScaleToInner]),
        //        Strategy::ExactCropAllowUpscaling => StepSet::Always(vec![Step::ScaleToOuter,
        //        Step::Crop]),
        //        Strategy::ExactPadAllowUpscaling => StepSet::Always(vec![Step::ScaleToInner,
        //        Step::Pad]),
        _ => steps()
    }.into_vec();
    if vec.is_empty(){
        None
    }else{
        Some(vec)
    }

}


fn kit_for_strategy(s: Strategy) -> Kit{
    Kit{
        steps: strategy_to_steps(s).unwrap(),
        expectations: vec![],
        file: file!(),
        line: line!()
    }.add_defaults()
}


fn step_kits() -> Vec<Kit>{
    let mut kits = Vec::new();

    let no_crop = Expect::That{ a: ExpectVal::SourceCrop, is: Cond::Equal, b: ExpectVal::Source};
    let no_4_side_cropping = Expect::That{ a: ExpectVal::SourceCrop, is: Cond::Either(Ordering::Equal), b: ExpectVal::Source};
    let no_padding = Expect::CanvasAgainstImage(Cond::Equal);
    // let no_scaling = Expect::That{ a: ExpectVal::SourceCrop, is: Cond::Equal, b: ExpectVal::Image};
    let no_upscaling = Expect::That{ a: ExpectVal::SourceCrop, is: Cond::Neither(Ordering::Less), b: ExpectVal::Image};
    // let no_downscaling = Expect::That{ a: ExpectVal::SourceCrop, is: Cond::Neither(Ordering::Greater), b: ExpectVal::Image};
    let never_larger_than_target = Expect::CanvasAgainstTarget(Cond::Neither(Ordering::Greater));
    let no_4_side_padding = Expect::CanvasAgainstImage(Cond::Either(Ordering::Equal));

    kits.push(kit_for_strategy(Strategy::Distort)
        .assert(Expect::CanvasMatchesTarget)
        .assert(Expect::ImageAgainstTarget(Cond::Equal))
        .assert(no_crop).assert(no_padding));

    kits.push(kit_for_strategy(Strategy::Max)
        .assert(never_larger_than_target)
        .assert(no_crop).assert(no_padding).assert(no_upscaling)
        .when(When::SourceAgainstTarget(Cond::Neither(Ordering::Greater))).expect(Expect::CanvasMatchesSource)
        .when(When::SourceAgainstTarget(Cond::Either(Ordering::Greater))).expect(Expect::Whatever));

    kits.push(kit_for_strategy(Strategy::PadOrAspect)
        .assert(never_larger_than_target)
        .assert(no_crop)
        .assert(no_upscaling)
        .assert(no_4_side_padding)
        .when(When::SourceAgainstTarget(Cond::Either(Ordering::Equal))).expect(Expect::CanvasMatchesTarget)
        .when(When::CanvasAgainstTarget(Cond::Equal)).expect(Expect::Whatever)
        .when(When::SourceAgainstTarget(Cond::Smaller2D)).expect(Expect::Whatever)
        );

    kits.push(kit_for_strategy(Strategy::PadDownscaleOnly)
        .assert(never_larger_than_target)
        .assert(no_crop)
        .assert(no_upscaling)
        .assert(no_4_side_padding)
        .when(When::SourceAgainstTarget(Cond::Either(Ordering::Equal))).expect(Expect::CanvasMatchesTarget)
        .when(When::CanvasAgainstTarget(Cond::Equal)).expect(Expect::Whatever)
        .when(When::SourceAgainstTarget(Cond::Smaller2D)).expect(Expect::CanvasMatchesSource)
    );

    //Off-by-one - one pixel cropping on sides it ?shouldn't?
    kits.push(kit_for_strategy(Strategy::CropOrAspect)
        .assert(never_larger_than_target)
        .assert(no_4_side_cropping).or_warn()
        .assert(no_upscaling).or_warn()
        .assert(no_padding)
        .when(When::SourceAgainstTarget(Cond::Neither(Ordering::Less))).expect(Expect::CanvasMatchesTarget)
        .when(When::SourceAgainstTarget(Cond::Either(Ordering::Less))).expect(Expect::Whatever)
    );

    kits.push(kit_for_strategy(Strategy::CropDownscaleOnly)
        .assert(never_larger_than_target)
        .assert(no_4_side_cropping).or_warn()
        .assert(no_upscaling).or_warn()
        .assert(no_padding)
        .when(When::SourceAgainstTarget(Cond::Neither(Ordering::Less))).expect(Expect::CanvasMatchesTarget)
        .when(When::SourceAgainstTarget(Cond::Neither(Ordering::Greater))).expect(Expect::CanvasMatchesSource)
    );
    //::sizing::steps().
    //kits.push(steps().skip_unless(Cond::Neither(Ordering::Greater)).


    //
    //    let from_strategies = [Strategy::Distort, Strategy::Max, Strategy::MaxAny, Strategy::Min, Strategy::MinAny];
    //    for s in from_strategies.into_iter(){
    //        kits.push( strategy_to_steps(*s).unwrap());
    //    }
    for bad_kitty in kits.iter().filter(|k| k.expectations.iter().any(|e| e.when == When::Placeholder || e.expect == Expect::Placeholder)){
        panic!("{:?}\nYou forgot a .when, or a .expect, or something. you need an IF and a THEN, 'kay?", bad_kitty);
    }
    //kits.push(steps().into_vec());
    kits
}





fn next_pow2(v: u32) -> u32{
    let mut v = v;
    v-=1;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v+1
}


#[derive(PartialEq,Debug,Copy,Clone)]
enum NumberKind{
    Larger,
    Smaller
}


fn spot_in_primes(v: i32) -> usize{
    match SMALL_PRIMES.binary_search(&v){
        Ok(ix) => ix,
        Err(ix) => ix
    }
}
fn next_prime(v: i32, kind: NumberKind) -> Option<i32>{
    let ix = spot_in_primes(v);
    if ix >= SMALL_PRIMES.len() {
        None
    }else{
        let cur = SMALL_PRIMES[ix];
        if kind == NumberKind::Larger{
            if cur == v{
                next_prime(v + 1, kind) //TODO: Problem for 2->3. next_prime(2,Larger) will return 5, likely.
            }else{
                Some(cur)
            }
        }else{
            if ix == 0{
                if cur == v{
                    None
                }else{
                    Some(cur)
                }
            }else{
                Some(SMALL_PRIMES[ix -1])
            }
        }

    }
}
/// Returns a variety of changes to the given number (as well as some fixed values)
/// If given i32::MAX, returns the number of variations
fn vary_number(v: i32, variation_kind: u8) -> Option<i32>{
    if v < 1 { return None };
    match variation_kind{
        0 => Some(v),
        1 => next_prime(v, NumberKind::Larger),
        2 => next_prime(v, NumberKind::Smaller),
        3 => Some(next_pow2(v as u32) as i32),
        4 => v.checked_add(1),
        5 => v.checked_sub(1),
        6 => v.checked_mul(3),
        7 => v.checked_mul(10),
        8 => v.checked_mul(2),
        9 => v.checked_div(2),
        10 =>  v.checked_div(3),
        11 =>  v.checked_div(10),
        12 => Some(::std::i32::MAX),
        13 => Some(1),
        14 => Some(2),
        15 => Some(3),
        16 => Some(4),
        17 => Some(5),
        18 => Some(7),
        19 => Some(16),
        20 => Some(9),
        21 => Some(10),
        22 => Some((next_pow2(v as u32) / 2) as i32),
        23 => v.checked_add(66),
        ::std::u8::MAX => Some(24), // Return the upper bound number of variations
        _ => None
    }.and_then(|v| if v > 0 { Some(v) } else { None })
}

//Not used or ever tested
//fn unshift_delete_with_swap(vec: &mut Vec<AspectRatio>, count: usize ){
//    //If we have fewer than count * 2 items, then some swap_removes will fail
//    let swappable = vec.len() - count;
//    for ix in 0..swappable{
//        let _ = vec.swap_remove(ix);
//    }
//    for _ in swappable..count{
//        let _ = vec.pop();
//    }
//}

fn r(w: i32, h: i32) -> AspectRatio {
    AspectRatio::create(w, h).unwrap()
}

///Clears both vectors
fn generate_aspects(into: &mut Vec<AspectRatio>, temp: &mut Vec<AspectRatio>, seed: AspectRatio) {
    into.clear();
    temp.clear();


    // We use into as the first temp vec
    let n = into;
    n.reserve(80);
    n.push(seed);
    n.push(r(1, ::std::i32::MAX));
    n.push(r(1, 10));
    n.push(r(1, 3));
    n.push(r(5, 7));
    n.push(r(1, 1));
    n.push(r(4, 3));
    n.push(r(3, 2));
    n.push(r(16, 9));
    n.push(r(4, 5));

    // temp as the second
    let n_boxes = temp;
    n_boxes.reserve(n.len() * 4 + 2);
    n_boxes.push(seed);
    let _ = seed.transpose().map(|v| n_boxes.push(v));
    for aspect in n.as_slice().iter() {
        let transposed_aspect = aspect.transpose().unwrap();
        let _ = transposed_aspect.box_of(&seed, BoxKind::Outer).map(|v| n_boxes.push(v));
        let _ = transposed_aspect.box_of(&seed, BoxKind::Inner).map(|v| n_boxes.push(v));
        let _ = aspect.box_of(&seed, BoxKind::Outer).map(|v| n_boxes.push(v));
        let _ = aspect.box_of(&seed, BoxKind::Inner).map(|v| n_boxes.push(v));
    }
    n_boxes.sort();
    n_boxes.dedup();


    //Clear and reuse the first vector
    n.clear();
    let vary_count = vary_number(1,::std::u8::MAX).unwrap();
    n.reserve(n_boxes.len() * vary_count as usize * vary_count as usize);
    for base_ver in n_boxes {
        let (w, h) = base_ver.size();
        for vary_w in 0..30 {
            for vary_h in 0..30 {
                let new_w = vary_number(w, vary_w);
                let new_h = vary_number(h, vary_h);
                if new_w.is_some() && new_h.is_some() {
                    n.push(r(new_w.unwrap(), new_h.unwrap()));
                }
            }
        }
    }
    n.sort();
    n.dedup();
}



fn target_sizes(fewer: bool) -> Vec<AspectRatio>{
    if fewer{
        [(1,1), (1,3), (3,1), (7,3),(90,45),(10,10),(1621,883),(971,967), (17,1871), (512,512)].iter().map(|&(w,h)| AspectRatio::create(w, h).unwrap()).collect()
    }else{
        [(1,1), (1,3), (3,1), (7,3),(90,45),(10,10),(1621,883),(971,967), (17,1871), (512,512)].iter().map(|&(w,h)| AspectRatio::create(w, h).unwrap()).collect()
    }
}


macro_rules! w(
    ($($arg:tt)*) => { {
        let r = write!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stream");
    } }
);

#[derive(Copy,Clone,Debug,PartialEq, Eq, Hash)]
enum ExpectVal{
    #[allow(dead_code)]
    Val(AspectRatio),
    #[allow(dead_code)]
    Canvas,
    Source,
    SourceCrop,
    Image,
    #[allow(dead_code)]
    Target
}
#[derive(Copy,Clone,Debug,PartialEq, Eq, Hash)]
enum Expect {
    Placeholder,
    /// Just end the noise, okay?
    Whatever,
    Always,
    #[allow(dead_code)]
    Error,
    Ok,
    That{a: ExpectVal, is: Cond, b: ExpectVal},
    CanvasMatchesTarget,
    CanvasMatchesSource,
    #[allow(dead_code)]
    Canvas {
        against: AspectRatio,
        expect: Cond
    },
    #[allow(dead_code)]
    Image {
        against: AspectRatio,
        expect: Cond
    },
    #[allow(dead_code)]
    Source {
        against: AspectRatio,
        expect: Cond
    },
    #[allow(dead_code)]
    CanvasAgainstSource(Cond),
    #[allow(dead_code)]
    ImageAgainstSource(Cond),
    CanvasAgainstTarget(Cond),
    ImageAgainstTarget(Cond),
    CanvasAgainstImage(Cond),
    SourceAgainstTarget(Cond)
}
enum SimplifiedExpect {
    Bool(bool),
    That{a: ExpectVal, is: Cond, b: ExpectVal},
    Canvas {
        against: AspectRatio,
        expect: Cond
    },
    Image {
        against: AspectRatio,
        expect: Cond
    },
    Source {
        against: AspectRatio,
        expect: Cond
    },
}

#[derive(Copy,Clone,Debug,PartialEq)]
struct EvaluationContext{
    result: sizing::Result<Layout>,
    target: AspectRatio,
    source: AspectRatio
}
impl EvaluationContext{
    fn to_compact(&self) -> String{
        if let Ok(l) = self.result {
            format!("target: {:?}, source: {:?}, canvas: {:?}, image: {:?}, source_crop: {:?}", self.target, self.source, l.get_box(BoxTarget::CurrentCanvas), l.get_box(BoxTarget::CurrentImage), l.get_source_crop())
        }else{
            format!("target: {:?}, source: {:?}, result: {:?}", self.target, self.source, self.result)
        }

    }
}

impl ExpectVal {
    fn resolve(&self, c: &EvaluationContext) -> Option<AspectRatio> {
        match *self {
            ExpectVal::Canvas => c.result.map(|r| r.get_box(BoxTarget::CurrentCanvas)).ok(),
            ExpectVal::Image => c.result.map(|r| r.get_box(BoxTarget::CurrentImage)).ok(),
            ExpectVal::SourceCrop => c.result.map(|r| r.get_source_crop()).ok(),
            ExpectVal::Source => Some(c.source),
            ExpectVal::Target => Some(c.target),
            ExpectVal::Val(v) => Some(v)
        }
    }
}
impl Expect{

    fn simplify(&self, c: &EvaluationContext) -> SimplifiedExpect {
        match *self {
            Expect::Placeholder | Expect::Whatever | Expect::Always => SimplifiedExpect::Bool(true),
            Expect::Ok => SimplifiedExpect::Bool(c.result.is_ok()),
            Expect::Error => SimplifiedExpect::Bool(c.result.is_err()),
            _ if c.result.is_err() => SimplifiedExpect::Bool(false),
            Expect::CanvasMatchesTarget => SimplifiedExpect::Canvas { against: c.target, expect: Cond::Equal },
            Expect::CanvasMatchesSource => SimplifiedExpect::Canvas { against: c.source, expect: Cond::Equal },
            Expect::CanvasAgainstSource(cond) => SimplifiedExpect::Canvas { against: c.source, expect: cond },
            Expect::CanvasAgainstTarget(cond) => SimplifiedExpect::Canvas { against: c.target, expect: cond },
            Expect::CanvasAgainstImage(cond) => SimplifiedExpect::Canvas { against: c.result.as_ref().unwrap().get_box(BoxTarget::CurrentImage), expect: cond },
            Expect::ImageAgainstSource(cond) => SimplifiedExpect::Image { against: c.source, expect: cond },
            Expect::ImageAgainstTarget(cond) => SimplifiedExpect::Image { against: c.target, expect: cond },
            Expect::SourceAgainstTarget(cond) => SimplifiedExpect::Source { against: c.target, expect: cond },
            Expect::Canvas { against, expect } => SimplifiedExpect::Canvas { against: against, expect: expect },
            Expect::Image { against, expect } => SimplifiedExpect::Image { against: against, expect: expect },
            Expect::Source { against, expect } => SimplifiedExpect::Source { against: against, expect: expect },
            Expect::That{a, is, b} =>  SimplifiedExpect::That{a: a, is: is, b: b},
        }
    }


    fn is_true(&self, c: &EvaluationContext) -> bool{
        match self.simplify(c) {
            SimplifiedExpect::Canvas { against, expect } => c.result.map(|r| expect.matches(r.get_box(BoxTarget::CurrentCanvas).cmp_size(&against))).unwrap_or(false),
            SimplifiedExpect::Image { against, expect } =>  c.result.map(|r| expect.matches(r.get_box(BoxTarget::CurrentImage).cmp_size(&against))).unwrap_or(false),
            SimplifiedExpect::Source { against, expect } => expect.matches(c.source.cmp_size(&against)),
            SimplifiedExpect::That{a, is, b} => {
                a.resolve(c).and_then(|a_v| b.resolve(c).map(|b_v| is.matches(a_v.cmp_size(&b_v)))).unwrap_or(false)
            }
            SimplifiedExpect::Bool(v) => v
        }
    }
}

use self::Expect as When;
#[derive(Copy,Clone,Debug,PartialEq, Eq, Hash)]
enum ViolationAction{
    Warn,
    Panic,
    FailTest
}
#[derive(Copy,Clone,Debug,PartialEq, Eq,Hash)]
struct Expectation{
    when: When,
    expect: Expect,
    action: ViolationAction
}

#[derive(Clone,Debug,PartialEq)]
struct Kit{
    expectations: Vec<Expectation>,
    steps: Vec<Step>,
    file: &'static str,
    line: u32,
}

impl Kit{
    pub fn add(mut self, e: Expectation) -> Self{
        self.expectations.push(e);
        self
    }

    pub fn mut_last<F>(mut self, c: F) -> Self
        where F: Fn(&mut Expectation) -> ()
    {
        let last = self.expectations.pop().map(|mut e| { c(&mut e); e });
        if let Some(item) = last{
            self.expectations.push(item);
        }
        self
    }
    pub fn or_warn(self) -> Self{
        self.mut_last(|e| e.action = ViolationAction::Warn)
    }
    pub fn or_panic(self) -> Self{
        self.mut_last(|e| e.action = ViolationAction::Panic)
    }
    pub fn peek(&self) -> Option<Expectation>{
        if self.expectations.len() > 0 {
            Some(self.expectations[self.expectations.len() - 1])
        }else{
            None
        }
    }
    pub fn expect(self, expect: Expect) -> Self{
        if self.peek().map(|e| e.expect == Expect::Placeholder) == Some(true) {
            self.mut_last(|e| e.expect = expect)
        }else{
            self.add(Expectation{when: When::Placeholder, expect: expect, action: ViolationAction::FailTest})
        }
    }
    pub fn when(self, when: When) -> Self{
        if self.peek().map(|e| e.when == When::Placeholder) == Some(true) {
            self.mut_last(|e| e.when = when)
        }else{
            self.add(Expectation{when: when, expect: Expect::Placeholder, action: ViolationAction::FailTest})
        }
    }
    pub fn when_source(self, c: Cond) -> Self{
        self.when(When::SourceAgainstTarget(c))
    }

    pub fn assert(mut self, e: Expect) -> Self{
        self.expectations.push(Expectation{when: When::Always, expect: e, action: ViolationAction::Panic});
        self
    }

    pub fn add_defaults(self) -> Self{
        self.assert(Expect::Ok)
            .expect(Expect::CanvasMatchesTarget).when_source(Cond::Equal).or_panic()
            .assert(Expect::CanvasAgainstImage(Cond::Neither(Ordering::Less)))
    }
}



#[test]
fn test_scale_to_outer(){
    let cropper = sizing::IdentityCropProvider::new();
    let result = Layout::create(r(2,4), r(1,3)).execute_all(&steps().scale_to_outer().into_vec(), &cropper).unwrap();
    assert_eq!(result.get_box(BoxTarget::CurrentCanvas), r(2,3));
    assert_eq!(result.get_source_crop(), r(2,4))
}

#[test]
fn test_scale_to_outer_and_crop(){
    let cropper = sizing::IdentityCropProvider::new();
    let result = Layout::create(r(2,4), r(1,3)).execute_all(&steps().fill_crop().into_vec(), &cropper).unwrap();
    assert_eq!(result.get_source_crop(), r(1,4))
}

#[test]
fn test_crop_aspect(){
    let cropper = sizing::IdentityCropProvider::new();
    let result = Layout::create(r(638,423), r(200,133)).execute_all(&steps().crop_aspect().into_vec(), &cropper).unwrap();
    assert_eq!(result.get_source_crop(), r(636,423))
}

#[test]
fn test_steps() {
    let kits = step_kits();
    let target_sizes = target_sizes(false);

    //Reusable vectors for generating aspects
    let mut temp = Vec::new();
    let mut source_sizes = Vec::new();

    //Holds reusable maps/vectors for collecting info
    let mut current = invalid_group_data();

    let mut failed_kits = Vec::new();

    for kit in kits{
        let start_time = ::imageflow_helpers::timeywimey::precise_time_ns();
        let mut test_failed = false;
        for target in target_sizes.iter() {
            generate_aspects(&mut source_sizes, &mut temp, *target);

            let mut target_header_printed = false;

            //We want to filter into 9 groups, lt,eq,gt x w,h.
            source_sizes.sort_by_key(|a| a.cmp_size(&target) );

            current.invalidate();

            for source in source_sizes.iter() {

                let group = source.cmp_size(&target);
                if !current.valid_for(group){
                    current.end_report(&kit, &mut target_header_printed);
                    current = current.reset(group, *target, source_sizes.len());
                }

                let cropper = sizing::IdentityCropProvider::new();

                let result = Layout::create(*source, *target).execute_all(&kit.steps, &cropper);

                match result {
                    Err(LayoutError::ValueScalingFailed { .. }) if source.width() > 2147483646 || source.height() > 2147483646 => {},
                    r => {

                        if !current.report(&kit,  &mut target_header_printed, r, *source){
                            test_failed = true;
                        }
                    }
                }
            }
            current.end_report(&kit, &mut target_header_printed);

        }

        let duration = ::imageflow_helpers::timeywimey::precise_time_ns() - start_time;
        w!("\nSpent {:.0}ms testing {:?}\n\n", (duration  as f64) / 1000000., &kit.steps);

        if test_failed{
            failed_kits.push(kit);
        }
    }

    if !failed_kits.is_empty(){
        panic!("The following kits failed:\n {:#?}\n", &failed_kits);

    }
}


type ResultKey = (AspectRatio,AspectRatio);

struct GroupData<T> where T: fmt::Debug{
    name: T,
    padded_name: String,
    count: usize,
    target: AspectRatio,
    invalidated: bool,

    sources_for_all_groups: usize,
    identity_count: usize,
    target_count: usize,
    different_count: usize,

    applicable: HashMap<Expectation, usize>,
    unique: HashMap<ResultKey, usize>,
    truncate_unique: usize,

    failure_count: usize,
    failures: Vec<(EvaluationContext, Expectation)>,
    truncate_failures: usize,
}

fn invalid_group_data() -> GroupData<(Ordering,Ordering)> {
    let mut g = GroupData::new((Ordering::Equal, Ordering::Equal), AspectRatio::create(1, 1).unwrap(), 0);
    g.invalidate();
    g
}

impl<T> GroupData<T>
where T: std::fmt::Debug, T: std::cmp::PartialEq {
    fn new(value: T, target: AspectRatio, sources_for_all_groups: usize) -> GroupData<T> {
        let mut s = format!("{:?}", value);
        for _ in s.len()..20 {
            s.push_str(" ");
        }
        GroupData {
            name: value,
            padded_name: s,
            target: target,
            count: 0,
            identity_count: 0,
            target_count: 0,
            different_count: 0,
            failure_count: 0,
            unique: HashMap::new(),
            applicable: HashMap::new(),
            failures: Vec::new(),
            truncate_unique: 16,
            truncate_failures: 64,
            invalidated: false,
            sources_for_all_groups: sources_for_all_groups
        }
    }

    fn reset(mut self, value: T, target: AspectRatio, sources_for_all_groups: usize) -> GroupData<T> {
        self.invalidate();
        GroupData {
            unique: self.unique,
            failures: self.failures,
            applicable: self.applicable,
            ..GroupData::new(value, target, sources_for_all_groups)
        }
    }
    fn invalidate(&mut self) {
        self.unique.clear();
        self.applicable.clear();
        self.failures.clear();
        self.invalidated = true;
    }
    fn valid_for(&self, group: T) -> bool {
        !self.invalidated && self.name == group
    }

    fn print_header(&self, kit: &Kit){
        w!("\n======================================================\n");
        w!("  Targeting {}x{} using {:?}\n\n", self.target.width(), self.target.height(), &kit.steps);
        w!("  Testing {} source sizes\n\n", self.sources_for_all_groups);
    }
    fn end_report(&self, kit: &Kit, header_printed: &mut bool) {
        if self.invalidated {
            return; //Nothing to print
        }



        let met_target_covered = self.applicable.keys().any(|e| e.expect == Expect::CanvasMatchesTarget);
        let kept_size_covered = self.applicable.keys().any(|e| e.expect == Expect::CanvasMatchesSource);
        let shut_up_already = self.applicable.keys().any(|e| e.expect == Expect::Whatever);


        let silent = self.failures.is_empty() &&
            (self.target_count == self.count && met_target_covered) ||
            (self.identity_count == self.count && kept_size_covered) ||
            shut_up_already;


        if !silent {
            if !*header_printed {
                self.print_header(kit);
                *header_printed = true;
            }

            let (w, h) = self.target.size();
            w!("{:04} {} against ({},{}) - ", self.count, self.padded_name, w, h, );

            let unique_truncated = if self.unique.len() == self.truncate_unique { "+" } else { "" };
            let failures_truncated = if self.failures.len() == self.truncate_failures { "+" } else { "" };

            if self.target_count == self.count {
                w!("met target, a {}x{} canvas - ({})\n", w, h, self.count);
            } else if self.identity_count == self.count {
                w!("kept size - ({})\n", self.count);
            } else if self.different_count == self.count {
                w!("{}{} unique of {}\n",  self.unique.len(), &unique_truncated, self.count);
            } else if self.failures.len() > 0 {
                w!("{}{} failures, {}{} unique of {} different, {} met target, {} maintained canvas size\n", self.failures.len(), failures_truncated, self.unique.len(), &unique_truncated, self.different_count, self.target_count, self.identity_count);
            } else {
                w!("{}{} unique of {} different, {} met target, {} maintained canvas size\n", self.unique.len(), &unique_truncated, self.different_count, self.target_count, self.identity_count);
            }


            if !self.failures.is_empty() {
                let display_limit = 10;
                w!("Displaying {} of {}{} failures\n", display_limit, self.failures.len(), &failures_truncated);
                for &(k, v) in self.failures.iter().take(display_limit) {
                    w!("({},{}) produced {:?}, violating {:?}\n", k.source.width(), k.source.height(), k.result, v);
                }
                w!("\n");
            }
            if !self.unique.is_empty() {
                let display_limit = 10;
                w!("Displaying {} of {}{} unique results\n", display_limit, self.unique.len(), &unique_truncated);
                for (k, v) in self.unique.iter().take(display_limit) {
                    if k.0 == k.1 {
                        w!("({},{}) - from {} unique source sizes. aspect: {}\n", k.0.width(), k.0.height(), v, k.0.ratio_f64());
                    } else {
                        w!("({},{}) canvas, ({},{}) image - from {} unique source sizes. {:?} {:?}\n", k.0.width(), k.0.height(), k.1.width(), k.1.height(),  v, k.0, k.1);
                        //w!("{} source sizes produced canvas {:?} image {:?}\n", v, k.0, k.1);
                    }
                }
                w!("\n");
            }
        }
    }
    /// Expects the canvas to be the first in the ResultKey tuple
    ///
    fn report(&mut self, kit: &Kit, header_printed: &mut bool, result: sizing::Result<Layout>, source: AspectRatio) -> bool {
        self.count += 1;


        if let Ok(layout) = result {
            let canvas = layout.get_box(BoxTarget::CurrentCanvas);
            let image = layout.get_box(BoxTarget::CurrentImage);

            let unique_key = (canvas, image);

            if canvas == self.target {
                self.target_count += 1;
            } else if canvas == source {
                self.identity_count += 1;
            } else {
                self.different_count += 1;
                if self.unique.contains_key(&unique_key) || self.unique.len() < self.truncate_unique {
                    *self.unique.entry(unique_key).or_insert(0) += 1;
                }
            }
        }

        // Unused rules for a group

        //Evaluate rules and panic right away as requested
        let ctx = EvaluationContext {
            result: result,
            target: self.target,
            source: source
        };

        let mut failed_test = false;
        let mut fail_panic = false;
        let mut fail_any = false;

        for e in kit.expectations.iter().filter(|e| e.when.is_true(&ctx)) {
            *self.applicable.entry(*e).or_insert(0) +=1;
            if !e.expect.is_true(&ctx) {
                fail_any = true;
                if e.action != ViolationAction::Warn {
                    failed_test = true;
                }
                if e.action == ViolationAction::Panic {
                    fail_panic = true;
                }


                if self.failures.len() < self.truncate_failures {
                    self.failures.push((ctx, *e));
                }
            }
        }

        if fail_any {
            self.failure_count += 1;
        }

        if fail_panic {
            if !*header_printed {
                self.print_header(kit);
                *header_printed = true;
            }
            let failed = kit.expectations.iter().filter(|e| e.when.is_true(&ctx) && !e.expect.is_true(&ctx)).cloned().collect::<Vec<Expectation>>();
            panic!("\n  at {}/{}:{}\n\n{}\n\n{:#?}", env!("CARGO_MANIFEST_DIR"), kit.file, kit.line, ctx.to_compact(), &failed);
        }


        !failed_test
    }
}

/*         [InlineData(1600, 1200, "w=90;h=45;mode=crop;scale=canvas", 90,45,  90,45, 0,200,1600,1000 )]
        [InlineData(1600, 1200, "w=10;h=10;mode=crop",10,10,10,10, 200, 0, 1400, 1200)]
[InlineData(1600, 1200, "w=10;h=10;mode=max", 10, 8, 10, 8, 0, 0, 1600, 1200)]
*/



static SMALL_PRIMES:[i32;1000] = [
2, 3, 5, 7,11,13,17,19,23,29
,31,37,41,43,47,53,59,61,67,71
,73,79,83,89,97 ,101, 103, 107, 109, 113
, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173
, 179, 181, 191, 193, 197, 199, 211, 223, 227, 229
, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281
, 283, 293, 307, 311, 313, 317, 331, 337, 347, 349
, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409
, 419, 421, 431, 433, 439, 443, 449, 457, 461, 463
, 467, 479, 487, 491, 499, 503, 509, 521, 523, 541
, 547, 557, 563, 569, 571, 577, 587, 593, 599, 601
, 607, 613, 617, 619, 631, 641, 643, 647, 653, 659
, 661, 673, 677, 683, 691, 701, 709, 719, 727, 733
, 739, 743, 751, 757, 761, 769, 773, 787, 797, 809
, 811, 821, 823, 827, 829, 839, 853, 857, 859, 863
, 877, 881, 883, 887, 907, 911, 919, 929, 937, 941
, 947, 953, 967, 971, 977, 983, 991, 997,1009,1013
,1019,1021,1031,1033,1039,1049,1051,1061,1063,1069
,1087,1091,1093,1097,1103,1109,1117,1123,1129,1151
,1153,1163,1171,1181,1187,1193,1201,1213,1217,1223
,1229,1231,1237,1249,1259,1277,1279,1283,1289,1291
,1297,1301,1303,1307,1319,1321,1327,1361,1367,1373
,1381,1399,1409,1423,1427,1429,1433,1439,1447,1451
,1453,1459,1471,1481,1483,1487,1489,1493,1499,1511
,1523,1531,1543,1549,1553,1559,1567,1571,1579,1583
,1597,1601,1607,1609,1613,1619,1621,1627,1637,1657
,1663,1667,1669,1693,1697,1699,1709,1721,1723,1733
,1741,1747,1753,1759,1777,1783,1787,1789,1801,1811
,1823,1831,1847,1861,1867,1871,1873,1877,1879,1889
,1901,1907,1913,1931,1933,1949,1951,1973,1979,1987
,1993,1997,1999,2003,2011,2017,2027,2029,2039,2053
,2063,2069,2081,2083,2087,2089,2099,2111,2113,2129
,2131,2137,2141,2143,2153,2161,2179,2203,2207,2213
,2221,2237,2239,2243,2251,2267,2269,2273,2281,2287
,2293,2297,2309,2311,2333,2339,2341,2347,2351,2357
,2371,2377,2381,2383,2389,2393,2399,2411,2417,2423
,2437,2441,2447,2459,2467,2473,2477,2503,2521,2531
,2539,2543,2549,2551,2557,2579,2591,2593,2609,2617
,2621,2633,2647,2657,2659,2663,2671,2677,2683,2687
,2689,2693,2699,2707,2711,2713,2719,2729,2731,2741
,2749,2753,2767,2777,2789,2791,2797,2801,2803,2819
,2833,2837,2843,2851,2857,2861,2879,2887,2897,2903
,2909,2917,2927,2939,2953,2957,2963,2969,2971,2999
,3001,3011,3019,3023,3037,3041,3049,3061,3067,3079
,3083,3089,3109,3119,3121,3137,3163,3167,3169,3181
,3187,3191,3203,3209,3217,3221,3229,3251,3253,3257
,3259,3271,3299,3301,3307,3313,3319,3323,3329,3331
,3343,3347,3359,3361,3371,3373,3389,3391,3407,3413
,3433,3449,3457,3461,3463,3467,3469,3491,3499,3511
,3517,3527,3529,3533,3539,3541,3547,3557,3559,3571
,3581,3583,3593,3607,3613,3617,3623,3631,3637,3643
,3659,3671,3673,3677,3691,3697,3701,3709,3719,3727
,3733,3739,3761,3767,3769,3779,3793,3797,3803,3821
,3823,3833,3847,3851,3853,3863,3877,3881,3889,3907
,3911,3917,3919,3923,3929,3931,3943,3947,3967,3989
,4001,4003,4007,4013,4019,4021,4027,4049,4051,4057
,4073,4079,4091,4093,4099,4111,4127,4129,4133,4139
,4153,4157,4159,4177,4201,4211,4217,4219,4229,4231
,4241,4243,4253,4259,4261,4271,4273,4283,4289,4297
,4327,4337,4339,4349,4357,4363,4373,4391,4397,4409
,4421,4423,4441,4447,4451,4457,4463,4481,4483,4493
,4507,4513,4517,4519,4523,4547,4549,4561,4567,4583
,4591,4597,4603,4621,4637,4639,4643,4649,4651,4657
,4663,4673,4679,4691,4703,4721,4723,4729,4733,4751
,4759,4783,4787,4789,4793,4799,4801,4813,4817,4831
,4861,4871,4877,4889,4903,4909,4919,4931,4933,4937
,4943,4951,4957,4967,4969,4973,4987,4993,4999,5003
,5009,5011,5021,5023,5039,5051,5059,5077,5081,5087
,5099,5101,5107,5113,5119,5147,5153,5167,5171,5179
,5189,5197,5209,5227,5231,5233,5237,5261,5273,5279
,5281,5297,5303,5309,5323,5333,5347,5351,5381,5387
,5393,5399,5407,5413,5417,5419,5431,5437,5441,5443
,5449,5471,5477,5479,5483,5501,5503,5507,5519,5521
,5527,5531,5557,5563,5569,5573,5581,5591,5623,5639
,5641,5647,5651,5653,5657,5659,5669,5683,5689,5693
,5701,5711,5717,5737,5741,5743,5749,5779,5783,5791
,5801,5807,5813,5821,5827,5839,5843,5849,5851,5857
,5861,5867,5869,5879,5881,5897,5903,5923,5927,5939
,5953,5981,5987,6007,6011,6029,6037,6043,6047,6053
,6067,6073,6079,6089,6091,6101,6113,6121,6131,6133
,6143,6151,6163,6173,6197,6199,6203,6211,6217,6221
,6229,6247,6257,6263,6269,6271,6277,6287,6299,6301
,6311,6317,6323,6329,6337,6343,6353,6359,6361,6367
,6373,6379,6389,6397,6421,6427,6449,6451,6469,6473
,6481,6491,6521,6529,6547,6551,6553,6563,6569,6571
,6577,6581,6599,6607,6619,6637,6653,6659,6661,6673
,6679,6689,6691,6701,6703,6709,6719,6733,6737,6761
,6763,6779,6781,6791,6793,6803,6823,6827,6829,6833
,6841,6857,6863,6869,6871,6883,6899,6907,6911,6917
,6947,6949,6959,6961,6967,6971,6977,6983,6991,6997
,7001,7013,7019,7027,7039,7043,7057,7069,7079,7103
,7109,7121,7127,7129,7151,7159,7177,7187,7193,7207
,7211,7213,7219,7229,7237,7243,7247,7253,7283,7297
,7307,7309,7321,7331,7333,7349,7351,7369,7393,7411
,7417,7433,7451,7457,7459,7477,7481,7487,7489,7499
,7507,7517,7523,7529,7537,7541,7547,7549,7559,7561
,7573,7577,7583,7589,7591,7603,7607,7621,7639,7643
,7649,7669,7673,7681,7687,7691,7699,7703,7717,7723
,7727,7741,7753,7757,7759,7789,7793,7817,7823,7829
,7841,7853,7867,7873,7877,7879,7883,7901,7907,7919];
