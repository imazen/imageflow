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
        line: line!(),
        strategy: s
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
    n.push(r(1200, 400));
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
        [(1,1), (1,3), (3,1), (7,3),(90,45),(10,10),(100,33),(1621,883),(971,967), (17,1871), (512,512)].iter().map(|&(w,h)| AspectRatio::create(w, h).unwrap()).collect()
    }else{
        [(1,1), (1,3), (3,1), (7,3),(90,45),(10,10),(100,33),(1621,883),(971,967), (17,1871), (512,512)].iter().map(|&(w,h)| AspectRatio::create(w, h).unwrap()).collect()
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
    strategy: Strategy,
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

// to fix this, 99 is wrong
// initial_size: 1200x400
// target: 100x33
// constraints: [SkipUnless(Either(Greater)), ScaleToInner, Pad]
// layout: Layout { source_max: 1200x400, source: 1200x400, target: 100x33, canvas: 100x33, image: 99x33 }

#[test]
fn test_rounding_99 () {
    let cropper = sizing::IdentityCropProvider::new();
    let result = Layout::create(r(1200,400), r(100,33)).execute_all(&steps().scale_to_inner().into_vec(), &cropper).unwrap();
    assert_eq!(result.get_box(BoxTarget::CurrentCanvas), r(100,33), "canvas");
    assert_eq!(result.get_box(BoxTarget::CurrentImage), r(100,33), "image");
    assert_eq!(result.get_source_crop(), r(1200,400), "source_crop");
}

#[derive(Copy,Clone,Debug, PartialEq)]
struct ShrinkWithinTest{
    origin: AspectRatio,
    target_within: AspectRatio,
    w: Option<i32>,
    h: Option<i32>,
    loss_w: f64,
    loss_h: f64
}

// auto convert from (i32,i32, Option<i32>, Option<i32>)
impl From<(i32,i32, i32, i32)> for ShrinkWithinTest{
    fn from(tuple: (i32, i32, i32, i32)) -> Self{
        let w = if tuple.2 < 1 { None } else { Some(tuple.2) };
        let h = if tuple.3 < 1 { None } else { Some(tuple.3) };
        ShrinkWithinTest::new(AspectRatio::create(tuple.0, tuple.1).unwrap(), w, h).unwrap()
    }
}

impl ShrinkWithinTest{
    fn new(origin: AspectRatio, w: Option<i32>, h: Option<i32>) -> Option<Self>{

        if w.is_some() && h.is_some() {
            panic!("Cannot specify both width and height");
        }
        if w.is_none() && h.is_none() {
            panic!("Must specify at least one dimension");
        }
        let mut target_within = origin;
        if w.is_some(){
            if w.unwrap() > origin.width() {
                return None;
            }
            target_within = AspectRatio::create(w.unwrap(), origin.height_for(w.unwrap(), None).unwrap()).unwrap();
        }
        if h.is_some(){
            if h.unwrap() > origin.height() {
                return None;
            }
            target_within = AspectRatio::create(origin.width_for(h.unwrap(), None).unwrap(), h.unwrap()).unwrap();
        }
        let loss_w = origin.rounding_loss_based_on_target_width(target_within.width());
        let loss_h = origin.rounding_loss_based_on_target_height(target_within.height());
        let v = Self{origin, target_within, w, h, loss_w, loss_h};
        Some(v)
    }

}

impl fmt::Display for ShrinkWithinTest{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let char = if self.w.is_some() { "w" } else { "h" };
        let dim = if self.w.is_some() { self.w.unwrap() } else { self.h.unwrap() };
        write!(f, "{:?} -> {}={} -> [{:?}] (loss w={:.2} h={:.2})", self.origin,  char,dim, self.target_within, self.loss_w, self.loss_h)
    }
}

// st

fn test_shrink_within(test: ShrinkWithinTest, cropper: &sizing::IdentityCropProvider, steps: &[Step]) -> Option<(AspectRatio,AspectRatio,AspectRatio)>{
    let result = Layout::create(test.origin, test.target_within)
            .execute_all(steps, cropper).unwrap();

    let canvas = result.get_box(BoxTarget::CurrentCanvas);
    let image = result.get_box(BoxTarget::CurrentImage);
    let crop = result.get_source_crop();


    let mut failed = false;
    if canvas != image{
        //eprintln!("canvas and image mismatch: {:?} != {:?}", result.get_box(BoxTarget::CurrentCanvas), result.get_box(BoxTarget::CurrentImage));
        failed = true;
    }
    if crop != test.origin {
        //eprintln!("source crop mismatch: {:?} != {:?}", result.get_source_crop(), origin);
        failed = true;
    }

    if test.w.is_some() && canvas.width() != test.w.unwrap() {
        failed = true;
    }
    if test.h.is_some() && canvas.height() != test.h.unwrap() {
        failed = true;
    }
    if failed {
        return Some((canvas, image, crop));
    }
    return None;
}

// skipping, takes minutes. 
#[ignore]
#[test]
fn test_double_rounding_errors_exhaustive () {

    println!("Starting double rounding error test");
    let mut worst_tests = Vec::new();
    let cropper = sizing::IdentityCropProvider::new();
    let steps = steps().scale_to_inner().into_vec();


    let mut fail_count = 0;
    let mut worst_w_loss = 0.0;
    let mut worst_h_loss = 0.0;

    let mut count = 0;
    let max_dim = 1400;
    let max_target_side = 400;  
    for origin_width in (1..max_dim).rev() {
        for origin_height in (1..max_dim).rev() {
            let origin = AspectRatio::create(origin_width, origin_height).unwrap();
            for target_side in (1..max_target_side).rev() {
               
                count += 2;

                let test_w = ShrinkWithinTest::new(origin, Some(target_side), None);
                let test_h = ShrinkWithinTest::new(origin, None, Some(target_side));
                if let Some(test_w) = test_w {
                    if  test_shrink_within(test_w, &cropper, &steps).is_some(){
                        fail_count += 1;
                        if test_w.loss_w > worst_w_loss + 0.01 {
                            worst_w_loss = test_w.loss_w;
                            worst_tests.push(test_w);
                        }
                        if test_w.loss_h > worst_h_loss + 0.01 {
                            worst_h_loss = test_w.loss_h;
                            worst_tests.push(test_w);
                        }
                    }
                }
                if let Some(test_h) = test_h {
                    if test_shrink_within(test_h, &cropper, &steps).is_some(){
                        fail_count += 1;
                        if test_h.loss_w > worst_w_loss + 0.01 {
                            worst_w_loss = test_h.loss_w;
                            worst_tests.push(test_h);
                        }
                        if test_h.loss_h > worst_h_loss + 0.01 {
                            worst_h_loss = test_h.loss_h;
                            worst_tests.push(test_h);
                        }
                    }
                }
            }
        }
    }

    eprintln!("Failed {} of {} double rounding error tests", fail_count, count);
    if fail_count > 0 {
        let array_length = worst_tests.len();
        let format_iter = worst_tests.iter().map(|t| format!("({},{}, {:?}, {:?})", t.origin.width(), t.origin.height() , t.w.unwrap_or(-1), t.h.unwrap_or(-1)));
        eprintln!("static SHRINK_WITHIN_TESTS:[(i32,i32,i32,i32);{}]=[{}];", array_length, format_iter.collect::<Vec<String>>().join(","));
    }

}

#[test]
fn test_shrink_within_specific(){
    let cropper = sizing::IdentityCropProvider::new();
    let steps = steps().scale_to_inner().into_vec();
    let mut failed = Vec::new();
    for tuple in SHRINK_WITHIN_TESTS{
        let test = ShrinkWithinTest::from(tuple);
        if let Some((canvas, image, crop)) = test_shrink_within(test, &cropper, &steps){
            failed.push((test,canvas, image, crop));
        }
    }
    if !failed.is_empty(){
        eprintln!(" {:?} (of {}) failed ShrinkWithinTest tests:", failed.len(), SHRINK_WITHIN_TESTS.len());
        for (test,canvas, image, crop) in failed.iter(){
            eprintln!("{} -> canvas {:?} image {:?} crop {:?}", test, canvas, image, crop);
        }
        eprintln!(" {:?} (of {}) failed ShrinkWithinTest tests:", failed.len(), SHRINK_WITHIN_TESTS.len());
        assert!(false, "Failed {} of {} ShrinkWithinTest tests", failed.len(), SHRINK_WITHIN_TESTS.len());
    }
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
            eprintln!("kit strategy: {:?}", kit.strategy);
            eprintln!("kit file: {:?}", kit.file);
            eprintln!("kit line: {:?}", kit.line);
            eprintln!("failed: {:?}", failed);
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

static SHRINK_WITHIN_TESTS:[(i32,i32,i32,i32);1185]=[(1399,697, 280, -1),(1399,689, 200, -1),(1399,685, 193, -1),(1399,683, 212, -1),(1399,673, 396, -1),(1399,671, 270, -1),(1399,665, 365, -1),(1399,659, 190, -1),(1399,656, 193, -1),(1399,652, 162, -1),(1399,643, 260, -1),(1399,643, 260, -1),(1399,637, 291, -1),(1399,628, 362, -1),(1399,628, 362, -1),(1399,622, 343, -1),(1399,614, 270, -1),(1399,614, 270, -1),(1399,607, 363, -1),(1399,600, 232, -1),(1399,600, 232, -1),(1399,594, 305, -1),(1399,587, 342, -1),(1399,585, 391, -1),(1399,582, 256, -1),(1399,577, 217, -1),(1399,569, 193, -1),(1399,568, 383, -1),(1399,564, 222, -1),(1399,560, 346, -1),(1399,556, 39, -1),(1399,554, 125, -1),(1399,551, 179, -1),(1399,545, 163, -1),(1399,540, 307, -1),(1399,537, 353, -1),(1399,534, 93, -1),(1399,530, 260, -1),(1399,526, 254, -1),(1399,526, 254, -1),(1399,520, 265, -1),(1399,516, 61, -1),(1399,512, 291, -1),(1399,512, 97, -1),(1399,508, 263, -1),(1399,500, 270, -1),(1399,497, 190, -1),(1399,497, 114, -1),(1399,493, 271, -1),(1399,489, 216, -1),(1399,481, 397, -1),(1399,480, 290, -1),(1399,480, 290, -1),(1399,474, 152, -1),(1399,468, 139, -1),(1399,464, 300, -1),(1399,459, 32, -1),(1399,456, 158, -1),(1399,450, 300, -1),(1399,449, 148, -1),(1399,445, 11, -1),(1399,440, 310, -1),(1399,438, 107, -1),(1399,435, 320, -1),(1399,431, 297, -1),(1399,427, 172, -1),(1399,424, 325, -1),(1399,419, 202, -1),(1399,417, 52, -1),(1399,413, 188, -1),(1399,408, 108, -1),(1399,406, 143, -1),(1399,401, 232, -1),(1399,397, 259, -1),(1399,394, 158, -1),(1399,392, 298, -1),(1399,389, 196, -1),(1399,387, 338, -1),(1399,384, 388, -1),(1399,380, 289, -1),(1399,377, 154, -1),(1399,372, 220, -1),(1399,370, 259, -1),(1399,367, 223, -1),(1399,364, 98, -1),(1399,362, 114, -1),(1399,360, 68, -1),(1399,359, 189, -1),(1399,355, 333, -1),(1399,351, 277, -1),(1399,348, 203, -1),(1399,346, 374, -1),(1399,345, 221, -1),(1399,341, 240, -1),(1399,338, 387, -1),(1399,335, 332, -1),(1399,333, 355, -1),(1399,330, 248, -1),(1399,328, 386, -1),(1399,326, 324, -1),(1399,324, 326, -1),(1399,321, 146, -1),(1399,319, 182, -1),(1399,317, 267, -1),(1399,314, 274, -1),(1399,313, 257, -1),(1399,310, 264, -1),(1399,309, 206, -1),(1399,307, 180, -1),(1399,305, 383, -1),(1399,304, 237, -1),(1399,301, 244, -1),(1399,299, 386, -1),(1399,299, 255, -1),(1399,295, 377, -1),(1399,295, 230, -1),(1399,293, 74, -1),(1399,291, 387, -1),(1399,290, 41, -1),(1399,288, 85, -1),(1399,286, 203, -1),(1399,283, 393, -1),(1399,279, 183, -1),(1399,279, 178, -1),(1399,278, 234, -1),(1399,277, 250, -1),(1399,275, 379, -1),(1399,272, 162, -1),(1399,272, 54, -1),(1399,269, 169, -1),(1399,269, 91, -1),(1399,269, 13, -1),(1399,267, 317, -1),(1399,264, 310, -1),(1399,263, 125, -1),(1399,261, 335, -1),(1399,259, 397, -1),(1399,258, 122, -1),(1399,257, 313, -1),(1399,256, 194, -1),(1399,255, 299, -1),(1399,253, 235, -1),(1399,252, 297, -1),(1399,250, 277, -1),(1399,248, 330, -1),(1399,247, 337, -1),(1399,246, 327, -1),(1399,245, 197, -1),(1399,244, 43, -1),(1399,242, 211, -1),(1399,241, 357, -1),(1399,240, 341, -1),(1399,238, 385, -1),(1399,237, 304, -1),(1399,236, 329, -1),(1399,234, 278, -1),(1399,233, 9, -1),(1399,231, 324, -1),(1399,230, 295, -1),(1399,229, 168, -1),(1399,228, 316, -1),(1399,225, 115, -1),(1399,224, 153, -1),(1399,223, 367, -1),(1399,221, 345, -1),(1399,220, 124, -1),(1399,219, 214, -1),(1399,218, 369, -1),(1399,216, 204, -1),(1399,216, 68, -1),(1399,215, 244, -1),(1399,214, 219, -1),(1399,213, 266, -1),(1399,211, 242, -1),(1399,209, 251, -1),(1399,208, 380, -1),(1399,206, 309, -1),(1399,205, 58, -1),(1399,204, 120, -1),(1399,203, 286, -1),(1399,201, 261, -1),(1399,199, 355, -1),(1399,198, 378, -1),(1399,197, 316, -1),(1399,197, 245, -1),(1399,196, 389, -1),(1399,195, 391, -1),(1399,194, 256, -1),(1399,191, 260, -1),(1399,190, 335, -1),(1399,189, 396, -1),(1399,189, 359, -1),(1399,187, 288, -1),(1399,186, 267, -1),(1399,185, 397, -1),(1399,184, 19, -1),(1399,183, 172, -1),(1399,182, 319, -1),(1399,181, 228, -1),(1399,180, 307, -1),(1399,179, 254, -1),(1399,178, 279, -1),(1399,177, 328, -1),(1399,176, 155, -1),(1399,174, 205, -1),(1399,173, 376, -1),(1399,172, 305, -1),(1399,172, 61, -1),(1399,170, 144, -1),(1399,168, 229, -1),(1399,167, 289, -1),(1399,165, 284, -1),(1399,165, 89, -1),(1399,164, 354, -1),(1399,163, 339, -1),(1399,162, 367, -1),(1399,161, 265, -1),(1399,160, 153, -1),(1399,158, 394, -1),(1399,157, 147, -1),(1399,157, 49, -1),(1399,156, 139, -1),(1399,155, 176, -1),(1399,154, 377, -1),(1399,153, 224, -1),(1399,153, 96, -1),(1399,152, 69, -1),(1399,152, 23, -1),(1399,151, 88, -1),(1399,149, 399, -1),(1399,149, 61, -1),(1399,148, 345, -1),(1399,147, 157, -1),(1399,146, 321, -1),(1399,145, 82, -1),(1399,144, 306, -1),(1399,144, 170, -1),(1399,144, 34, -1),(1399,143, 44, -1),(1399,142, 399, -1),(1399,141, 253, -1),(1399,139, 317, -1),(1399,139, 156, -1),(1399,138, 370, -1),(1399,137, 291, -1),(1399,137, 97, -1),(1399,136, 324, -1),(1399,136, 180, -1),(1399,136, 36, -1),(1399,134, 214, -1),(1399,133, 163, -1),(1399,133, 142, -1),(1399,131, 315, -1),(1399,131, 283, -1),(1399,130, 382, -1),(1399,129, 244, -1),(1399,128, 388, -1),(1399,127, 369, -1),(1399,127, 358, -1),(1399,126, 272, -1),(1399,125, 263, -1),(1399,124, 220, -1),(1399,123, 381, -1),(1399,122, 258, -1),(1399,121, 237, -1),(1399,120, 204, -1),(1399,119, 335, -1),(1399,119, 288, -1),(1399,119, 241, -1),(1399,118, 326, -1),(1399,117, 269, -1),(1399,116, 211, -1),(1399,115, 371, -1),(1399,114, 362, -1),(1399,113, 229, -1),(1399,112, 331, -1),(1399,111, 397, -1),(1399,110, 337, -1),(1399,110, 248, -1),(1399,108, 395, -1),(1399,108, 136, -1),(1399,107, 268, -1),(1399,105, 393, -1),(1399,104, 343, -1),(1399,103, 292, -1),(1399,102, 336, -1),(1399,102, 144, -1),(1399,101, 367, -1),(1399,101, 90, -1),(1399,99, 332, -1),(1399,99, 219, -1),(1399,98, 364, -1),(1399,97, 310, -1),(1399,97, 137, -1),(1399,96, 357, -1),(1399,96, 255, -1),(1399,96, 153, -1),(1399,96, 51, -1),(1399,95, 346, -1),(1399,94, 305, -1),(1399,94, 186, -1),(1399,93, 203, -1),(1399,93, 188, -1),(1399,92, 190, -1),(1399,92, 114, -1),(1399,92, 38, -1),(1399,91, 392, -1),(1399,90, 272, -1),(1399,89, 275, -1),(1399,89, 165, -1),(1399,89, 55, -1),(1399,88, 310, -1),(1399,87, 217, -1),(1399,87, 201, -1),(1399,86, 366, -1),(1399,86, 122, -1),(1399,85, 288, -1),(1399,84, 358, -1),(1399,83, 396, -1),(1399,82, 179, -1),(1399,82, 162, -1),(1399,82, 145, -1),(1399,81, 354, -1),(1399,80, 341, -1),(1399,79, 363, -1),(1399,78, 278, -1),(1399,77, 227, -1),(1399,77, 118, -1),(1399,76, 322, -1),(1399,76, 230, -1),(1399,76, 138, -1),(1399,76, 46, -1),(1399,75, 345, -1),(1399,74, 293, -1),(1399,73, 297, -1),(1399,72, 340, -1),(1399,72, 204, -1),(1399,72, 68, -1),(1399,71, 325, -1),(1399,71, 266, -1),(1399,69, 375, -1),(1399,69, 152, -1),(1399,68, 360, -1),(1399,68, 216, -1),(1399,68, 72, -1),(1399,67, 261, -1),(1399,66, 392, -1),(1399,65, 398, -1),(1399,65, 355, -1),(1399,65, 312, -1),(1399,65, 269, -1),(1399,64, 295, -1),(1399,64, 142, -1),(1399,63, 344, -1),(1399,63, 233, -1),(1399,63, 122, -1),(1399,62, 327, -1),(1399,62, 282, -1),(1399,61, 172, -1),(1399,60, 338, -1),(1399,59, 391, -1),(1399,59, 320, -1),(1399,58, 253, -1),(1399,58, 229, -1),(1399,58, 205, -1),(1399,57, 233, -1),(1399,57, 184, -1),(1399,56, 387, -1),(1399,55, 394, -1),(1399,55, 267, -1),(1399,55, 89, -1),(1399,54, 272, -1),(1399,53, 277, -1),(1399,52, 390, -1),(1399,52, 121, -1),(1399,51, 288, -1),(1399,51, 96, -1),(1399,49, 385, -1),(1399,49, 328, -1),(1399,49, 271, -1),(1399,49, 214, -1),(1399,49, 157, -1),(1399,48, 335, -1),(1399,48, 306, -1),(1399,48, 102, -1),(1399,47, 372, -1),(1399,47, 253, -1),(1399,46, 380, -1),(1399,46, 228, -1),(1399,46, 76, -1),(1399,45, 295, -1),(1399,45, 264, -1),(1399,45, 233, -1),(1399,45, 202, -1),(1399,44, 302, -1),(1399,43, 374, -1),(1399,43, 309, -1),(1399,43, 244, -1),(1399,42, 383, -1),(1399,41, 392, -1),(1399,41, 358, -1),(1399,41, 324, -1),(1399,41, 290, -1),(1399,40, 367, -1),(1399,39, 376, -1),(1399,39, 269, -1),(1399,38, 276, -1),(1399,38, 92, -1),(1399,37, 397, -1),(1399,36, 369, -1),(1399,36, 136, -1),(1399,34, 390, -1),(1399,34, 349, -1),(1399,34, 308, -1),(1399,34, 267, -1),(1399,34, 226, -1),(1399,34, 185, -1),(1399,34, 144, -1),(1399,33, 360, -1),(1399,33, 233, -1),(1399,32, 371, -1),(1399,32, 284, -1),(1399,32, 153, -1),(1399,31, 383, -1),(1399,31, 338, -1),(1399,31, 293, -1),(1399,31, 248, -1),(1399,31, 203, -1),(1399,30, 396, -1),(1399,30, 303, -1),(1399,29, 361, -1),(1399,29, 313, -1),(1399,29, 265, -1),(1399,29, 217, -1),(1399,28, 374, -1),(1399,27, 388, -1),(1399,27, 233, -1),(1399,26, 349, -1),(1399,26, 242, -1),(1399,25, 363, -1),(1399,24, 378, -1),(1399,24, 320, -1),(1399,24, 262, -1),(1399,24, 204, -1),(1399,23, 395, -1),(1399,23, 152, -1),(1399,22, 349, -1),(1399,22, 286, -1),(1399,21, 366, -1),(1399,21, 233, -1),(1399,20, 384, -1),(1399,19, 331, -1),(1399,19, 184, -1),(1399,18, 349, -1),(1399,18, 272, -1),(1399,17, 370, -1),(1399,17, 288, -1),(1399,16, 393, -1),(1399,16, 306, -1),(1399,15, 326, -1),(1399,15, 233, -1),(1399,14, 349, -1),(1399,13, 376, -1),(1399,13, 269, -1),(1399,12, 291, -1),(1399,11, 317, -1),(1399,11, 190, -1),(1399,10, 349, -1),(1399,9, 388, -1),(1399,9, 233, -1),(1399,8, 262, -1),(1399,7, 299, -1),(1399,6, 349, -1),(1399,5, 399, -1),(1398,5, 399, -1),(1397,5, 399, -1),(1396,5, 399, -1),(1395,5, 399, -1),(1394,5, 399, -1),(1393,5, 399, -1),(1392,5, 399, -1),(1391,5, 399, -1),(1390,5, 399, -1),(1389,5, 399, -1),(1388,5, 399, -1),(1387,5, 399, -1),(1386,5, 399, -1),(1385,5, 399, -1),(1384,5, 399, -1),(1383,5, 399, -1),(1382,5, 399, -1),(1381,5, 399, -1),(1380,5, 399, -1),(1379,5, 399, -1),(1378,5, 399, -1),(1377,5, 399, -1),(1376,5, 399, -1),(1375,5, 399, -1),(1374,5, 399, -1),(1373,5, 399, -1),(1372,5, 399, -1),(1371,5, 399, -1),(1370,5, 399, -1),(1369,5, 399, -1),(1368,5, 399, -1),(1367,5, 399, -1),(1366,5, 399, -1),(1365,5, 399, -1),(1364,5, 399, -1),(1363,5, 399, -1),(1362,5, 399, -1),(1361,5, 399, -1),(1360,5, 399, -1),(1359,5, 399, -1),(1358,5, 399, -1),(1357,5, 399, -1),(1356,5, 399, -1),(1355,5, 399, -1),(1354,5, 399, -1),(1353,5, 399, -1),(1352,5, 399, -1),(1351,5, 399, -1),(1350,5, 399, -1),(1349,5, 399, -1),(1348,5, 399, -1),(1347,5, 399, -1),(1346,5, 399, -1),(1345,5, 399, -1),(1344,5, 399, -1),(1343,5, 399, -1),(1342,5, 399, -1),(1341,5, 399, -1),(1340,5, 399, -1),(1339,5, 399, -1),(1338,5, 399, -1),(1337,5, 399, -1),(1336,5, 399, -1),(1335,5, 399, -1),(1334,5, 399, -1),(1333,5, 399, -1),(1332,5, 399, -1),(1331,5, 399, -1),(697,1399, -1, 280),(689,1399, -1, 200),(683,1399, -1, 212),(674,1398, -1, 28),(667,1398, -1, 284),(660,1399, -1, 124),(654,1399, -1, 123),(647,1399, -1, 40),(641,1399, -1, 287),(635,1398, -1, 142),(629,1398, -1, 10),(623,1398, -1, 46),(618,1399, -1, 103),(612,1399, -1, 8),(606,1399, -1, 202),(600,1399, -1, 232),(594,1399, -1, 305),(588,1397, -1, 177),(582,1399, -1, 256),(577,1399, -1, 217),(571,1396, -1, 11),(567,1399, -1, 359),(563,1399, -1, 41),(557,1399, -1, 162),(552,1399, -1, 313),(547,1399, -1, 211),(541,1399, -1, 128),(536,1399, -1, 338),(532,1398, -1, 293),(527,1399, -1, 73),(521,1398, -1, 377),(517,1399, -1, 115),(513,1397, -1, 241),(509,1399, -1, 224),(505,1396, -1, 217),(502,1398, -1, 110),(498,1397, -1, 108),(495,1399, -1, 366),(490,1398, -1, 398),(485,1397, -1, 301),(482,1398, -1, 364),(479,1399, -1, 92),(474,1399, -1, 152),(470,1398, -1, 58),(467,1399, -1, 349),(463,1399, -1, 210),(459,1399, -1, 32),(456,1399, -1, 158),(452,1398, -1, 283),(449,1399, -1, 148),(445,1399, -1, 11),(442,1398, -1, 68),(439,1398, -1, 164),(436,1398, -1, 101),(433,1399, -1, 63),(430,1399, -1, 353),(426,1399, -1, 133),(423,1399, -1, 339),(419,1399, -1, 202),(417,1399, -1, 52),(413,1399, -1, 188),(409,1396, -1, 285),(406,1399, -1, 143),(402,1397, -1, 384),(400,1399, -1, 348),(397,1399, -1, 111),(393,1399, -1, 283),(391,1399, -1, 195),(388,1399, -1, 384),(385,1398, -1, 187),(383,1399, -1, 305),(380,1399, -1, 289),(377,1399, -1, 154),(374,1398, -1, 271),(372,1399, -1, 220),(370,1399, -1, 259),(368,1398, -1, 340),(366,1399, -1, 86),(364,1397, -1, 71),(361,1398, -1, 91),(359,1399, -1, 189),(356,1397, -1, 155),(355,1399, -1, 333),(351,1399, -1, 277),(349,1398, -1, 6),(347,1398, -1, 280),(345,1399, -1, 221),(341,1399, -1, 240),(338,1399, -1, 387),(335,1399, -1, 332),(333,1399, -1, 355),(330,1399, -1, 248),(328,1399, -1, 386),(326,1399, -1, 324),(324,1399, -1, 326),(322,1398, -1, 89),(320,1398, -1, 391),(318,1397, -1, 380),(317,1399, -1, 267),(315,1397, -1, 51),(313,1399, -1, 257),(311,1398, -1, 227),(310,1399, -1, 264),(309,1399, -1, 206),(307,1399, -1, 180),(305,1399, -1, 383),(304,1399, -1, 237),(301,1399, -1, 244),(299,1399, -1, 386),(299,1399, -1, 255),(296,1398, -1, 196),(294,1397, -1, 354),(292,1399, -1, 103),(291,1397, -1, 12),(289,1399, -1, 380),(288,1399, -1, 17),(286,1399, -1, 203),(284,1397, -1, 273),(283,1399, -1, 393),(281,1397, -1, 261),(280,1398, -1, 347),(278,1399, -1, 390),(277,1399, -1, 250),(275,1399, -1, 379),(273,1397, -1, 284),(272,1399, -1, 54),(270,1398, -1, 277),(269,1399, -1, 65),(267,1399, -1, 317),(265,1398, -1, 182),(263,1399, -1, 125),(262,1398, -1, 8),(261,1399, -1, 201),(259,1399, -1, 397),(258,1399, -1, 122),(257,1399, -1, 313),(256,1399, -1, 194),(255,1399, -1, 299),(253,1399, -1, 235),(252,1399, -1, 297),(251,1398, -1, 220),(250,1399, -1, 277),(248,1399, -1, 330),(247,1399, -1, 337),(246,1399, -1, 327),(245,1399, -1, 197),(244,1399, -1, 43),(242,1399, -1, 211),(241,1399, -1, 357),(240,1399, -1, 341),(238,1399, -1, 385),(238,1398, -1, 326),(237,1399, -1, 304),(236,1399, -1, 329),(234,1399, -1, 278),(233,1399, -1, 9),(232,1397, -1, 280),(230,1399, -1, 295),(229,1399, -1, 168),(228,1399, -1, 316),(226,1398, -1, 300),(225,1398, -1, 146),(224,1399, -1, 153),(223,1399, -1, 367),(221,1399, -1, 345),(220,1399, -1, 124),(219,1399, -1, 214),(218,1399, -1, 369),(216,1399, -1, 204),(216,1399, -1, 68),(215,1399, -1, 244),(214,1399, -1, 219),(213,1399, -1, 266),(212,1397, -1, 313),(211,1399, -1, 242),(209,1399, -1, 251),(208,1399, -1, 380),(207,1398, -1, 260),(206,1399, -1, 309),(205,1399, -1, 58),(204,1399, -1, 120),(203,1399, -1, 286),(202,1398, -1, 218),(201,1399, -1, 261),(200,1398, -1, 346),(199,1396, -1, 235),(198,1399, -1, 378),(197,1399, -1, 316),(197,1399, -1, 245),(196,1399, -1, 389),(195,1399, -1, 391),(194,1399, -1, 256),(193,1398, -1, 134),(191,1399, -1, 260),(191,1398, -1, 172),(189,1399, -1, 396),(189,1399, -1, 359),(188,1398, -1, 145),(187,1398, -1, 385),(186,1399, -1, 267),(185,1399, -1, 397),(184,1399, -1, 19),(183,1399, -1, 172),(182,1399, -1, 319),(181,1399, -1, 228),(180,1399, -1, 307),(179,1399, -1, 254),(178,1399, -1, 279),(177,1399, -1, 328),(176,1399, -1, 155),(175,1396, -1, 347),(174,1399, -1, 205),(173,1399, -1, 376),(173,1398, -1, 101),(172,1399, -1, 305),(172,1399, -1, 61),(171,1397, -1, 241),(170,1399, -1, 144),(169,1398, -1, 335),(168,1399, -1, 229),(167,1399, -1, 289),(166,1398, -1, 240),(166,1398, -1, 80),(165,1398, -1, 72),(164,1399, -1, 354),(163,1399, -1, 339),(162,1399, -1, 367),(162,1397, -1, 332),(161,1398, -1, 178),(160,1399, -1, 153),(159,1396, -1, 259),(158,1399, -1, 394),(157,1399, -1, 147),(157,1399, -1, 49),(156,1399, -1, 139),(155,1399, -1, 176),(154,1399, -1, 377),(153,1399, -1, 224),(153,1399, -1, 96),(152,1399, -1, 69),(152,1399, -1, 23),(151,1399, -1, 88),(150,1398, -1, 219),(149,1399, -1, 399),(149,1399, -1, 61),(149,1396, -1, 89),(148,1399, -1, 345),(148,1398, -1, 392),(147,1399, -1, 157),(146,1399, -1, 321),(145,1399, -1, 82),(144,1399, -1, 306),(144,1399, -1, 170),(144,1399, -1, 34),(143,1399, -1, 44),(142,1399, -1, 399),(141,1399, -1, 253),(140,1396, -1, 344),(139,1399, -1, 317),(139,1399, -1, 156),(138,1399, -1, 370),(138,1397, -1, 329),(137,1399, -1, 291),(137,1399, -1, 97),(136,1399, -1, 324),(136,1399, -1, 180),(136,1399, -1, 36),(135,1398, -1, 88),(135,1397, -1, 119),(134,1399, -1, 214),(134,1398, -1, 193),(133,1399, -1, 142),(132,1398, -1, 323),(131,1399, -1, 315),(131,1399, -1, 283),(130,1399, -1, 382),(130,1398, -1, 371),(129,1399, -1, 244),(128,1399, -1, 388),(127,1399, -1, 369),(127,1399, -1, 358),(126,1399, -1, 272),(125,1399, -1, 263),(124,1399, -1, 220),(123,1399, -1, 381),(122,1399, -1, 258),(121,1399, -1, 237),(120,1399, -1, 204),(119,1399, -1, 335),(119,1399, -1, 288),(119,1399, -1, 241),(118,1399, -1, 326),(118,1398, -1, 77),(117,1399, -1, 269),(117,1397, -1, 197),(116,1399, -1, 211),(116,1398, -1, 235),(115,1399, -1, 371),(115,1398, -1, 79),(114,1399, -1, 362),(113,1399, -1, 229),(113,1398, -1, 167),(112,1399, -1, 331),(111,1399, -1, 397),(110,1399, -1, 337),(110,1399, -1, 248),(109,1398, -1, 109),(108,1399, -1, 136),(107,1399, -1, 268),(106,1398, -1, 389),(106,1398, -1, 178),(106,1397, -1, 112),(105,1399, -1, 393),(105,1397, -1, 153),(104,1399, -1, 343),(103,1399, -1, 292),(102,1399, -1, 336),(102,1399, -1, 144),(101,1399, -1, 367),(101,1399, -1, 90),(100,1396, -1, 342),(99,1399, -1, 332),(99,1399, -1, 219),(98,1399, -1, 364),(97,1399, -1, 310),(97,1399, -1, 137),(96,1399, -1, 357),(96,1399, -1, 255),(96,1399, -1, 153),(96,1399, -1, 51),(95,1399, -1, 346),(95,1397, -1, 272),(95,1396, -1, 360),(94,1399, -1, 305),(94,1399, -1, 186),(94,1398, -1, 290),(93,1399, -1, 203),(93,1399, -1, 188),(93,1397, -1, 353),(92,1399, -1, 190),(92,1399, -1, 114),(92,1399, -1, 38),(91,1399, -1, 392),(91,1397, -1, 284),(90,1399, -1, 272),(89,1399, -1, 275),(89,1399, -1, 165),(89,1399, -1, 55),(88,1399, -1, 310),(87,1399, -1, 217),(87,1399, -1, 201),(86,1399, -1, 366),(86,1399, -1, 122),(85,1399, -1, 288),(85,1398, -1, 74),(84,1399, -1, 358),(84,1398, -1, 208),(83,1399, -1, 396),(83,1398, -1, 261),(83,1398, -1, 160),(82,1399, -1, 162),(82,1399, -1, 145),(81,1399, -1, 354),(81,1398, -1, 302),(80,1399, -1, 341),(79,1399, -1, 363),(78,1399, -1, 278),(77,1399, -1, 227),(77,1399, -1, 118),(77,1398, -1, 354),(77,1398, -1, 118),(76,1399, -1, 230),(76,1399, -1, 138),(76,1399, -1, 46),(75,1399, -1, 345),(75,1396, -1, 214),(75,1394, -1, 381),(75,1393, -1, 65),(74,1399, -1, 293),(74,1398, -1, 85),(73,1399, -1, 297),(73,1398, -1, 67),(72,1399, -1, 340),(72,1399, -1, 204),(72,1399, -1, 68),(71,1399, -1, 325),(71,1399, -1, 266),(70,1396, -1, 329),(70,1395, -1, 269),(70,1394, -1, 229),(69,1399, -1, 375),(69,1399, -1, 152),(69,1398, -1, 314),(68,1399, -1, 360),(68,1399, -1, 216),(68,1399, -1, 72),(67,1399, -1, 261),(66,1399, -1, 392),(66,1398, -1, 180),(65,1399, -1, 398),(65,1399, -1, 355),(65,1399, -1, 312),(65,1399, -1, 269),(64,1399, -1, 295),(64,1399, -1, 142),(64,1398, -1, 273),(64,1397, -1, 251),(63,1399, -1, 344),(63,1399, -1, 233),(63,1399, -1, 122),(63,1398, -1, 122),(63,1397, -1, 255),(62,1399, -1, 327),(62,1399, -1, 282),(62,1398, -1, 124),(61,1399, -1, 172),(60,1399, -1, 338),(60,1398, -1, 198),(59,1399, -1, 391),(59,1399, -1, 320),(59,1398, -1, 154),(58,1399, -1, 229),(58,1399, -1, 205),(57,1399, -1, 233),(57,1399, -1, 184),(57,1398, -1, 282),(56,1399, -1, 387),(56,1398, -1, 337),(55,1399, -1, 267),(55,1399, -1, 89),(54,1399, -1, 272),(53,1399, -1, 277),(53,1398, -1, 356),(53,1398, -1, 145),(53,1397, -1, 224),(53,1395, -1, 329),(52,1399, -1, 390),(52,1399, -1, 121),(52,1397, -1, 94),(51,1399, -1, 288),(51,1399, -1, 96),(50,1397, -1, 377),(50,1396, -1, 321),(50,1395, -1, 265),(49,1399, -1, 328),(49,1399, -1, 271),(49,1399, -1, 214),(49,1399, -1, 157),(48,1399, -1, 335),(48,1399, -1, 306),(48,1399, -1, 102),(47,1399, -1, 372),(47,1399, -1, 253),(46,1399, -1, 380),(46,1399, -1, 228),(46,1399, -1, 76),(45,1399, -1, 295),(45,1399, -1, 264),(45,1399, -1, 233),(45,1399, -1, 202),(45,1397, -1, 357),(44,1399, -1, 302),(43,1399, -1, 374),(43,1399, -1, 309),(43,1399, -1, 244),(42,1399, -1, 383),(41,1399, -1, 392),(41,1399, -1, 358),(41,1399, -1, 324),(41,1399, -1, 290),(40,1399, -1, 367),(40,1398, -1, 332),(39,1399, -1, 269),(38,1399, -1, 276),(38,1399, -1, 92),(37,1399, -1, 397),(36,1399, -1, 369),(36,1399, -1, 136),(35,1398, -1, 379),(35,1397, -1, 379),(35,1396, -1, 339),(34,1399, -1, 308),(34,1399, -1, 267),(34,1399, -1, 226),(34,1399, -1, 185),(34,1399, -1, 144),(33,1399, -1, 360),(33,1399, -1, 233),(33,1398, -1, 360),(32,1399, -1, 371),(32,1399, -1, 284),(32,1399, -1, 153),(31,1399, -1, 383),(31,1399, -1, 338),(31,1399, -1, 293),(31,1399, -1, 248),(31,1399, -1, 203),(31,1398, -1, 248),(30,1399, -1, 396),(30,1399, -1, 303),(30,1396, -1, 349),(29,1399, -1, 361),(29,1399, -1, 313),(29,1399, -1, 265),(29,1399, -1, 217),(28,1399, -1, 374),(28,1398, -1, 374),(28,1397, -1, 374),(28,1396, -1, 324),(28,1395, -1, 274),(27,1399, -1, 388),(27,1399, -1, 233),(27,1397, -1, 388),(26,1399, -1, 349),(26,1399, -1, 242),(26,1397, -1, 188),(25,1399, -1, 363),(25,1398, -1, 363),(25,1397, -1, 363),(25,1396, -1, 307),(25,1393, -1, 195),(24,1399, -1, 378),(24,1399, -1, 320),(24,1399, -1, 262),(24,1399, -1, 204),(23,1399, -1, 395),(23,1399, -1, 152),(22,1399, -1, 349),(22,1399, -1, 286),(21,1399, -1, 366),(21,1399, -1, 233),(20,1399, -1, 384),(20,1398, -1, 384),(20,1397, -1, 384),(20,1396, -1, 314),(19,1399, -1, 331),(19,1399, -1, 184),(18,1399, -1, 349),(18,1399, -1, 272),(17,1399, -1, 370),(17,1399, -1, 288),(16,1399, -1, 393),(16,1399, -1, 306),(15,1399, -1, 326),(15,1399, -1, 233),(14,1399, -1, 349),(14,1398, -1, 349),(14,1397, -1, 349),(14,1395, -1, 249),(13,1399, -1, 376),(13,1399, -1, 269),(12,1399, -1, 291),(12,1398, -1, 291),(12,1397, -1, 291),(11,1399, -1, 317),(11,1399, -1, 190),(11,1398, -1, 190),(11,1397, -1, 317),(11,1396, -1, 317),(11,1395, -1, 317),(10,1399, -1, 349),(10,1398, -1, 349),(10,1397, -1, 349),(9,1399, -1, 388),(9,1399, -1, 233),(8,1399, -1, 262),(8,1398, -1, 262),(7,1399, -1, 299),(7,1398, -1, 299),(7,1397, -1, 299),(7,1396, -1, 299),(6,1399, -1, 349),(6,1398, -1, 349),(6,1397, -1, 349),(5,1399, -1, 399),(5,1398, -1, 399),(5,1397, -1, 399),(5,1396, -1, 399),(5,1395, -1, 399),(5,1394, -1, 399),(5,1393, -1, 399),(5,1392, -1, 399),(5,1391, -1, 399),(5,1390, -1, 399),(5,1389, -1, 399),(5,1388, -1, 399),(5,1387, -1, 399),(5,1386, -1, 399),(5,1385, -1, 399),(5,1384, -1, 399),(5,1383, -1, 399),(5,1382, -1, 399),(5,1381, -1, 399),(5,1380, -1, 399),(5,1379, -1, 399),(5,1378, -1, 399),(5,1377, -1, 399),(5,1376, -1, 399),(5,1375, -1, 399),(5,1374, -1, 399),(5,1373, -1, 399),(5,1372, -1, 399),(5,1371, -1, 399),(5,1370, -1, 399),(5,1369, -1, 399),(5,1368, -1, 399),(5,1367, -1, 399),(5,1366, -1, 399),(5,1365, -1, 399),(5,1364, -1, 399),(5,1363, -1, 399),(5,1362, -1, 399),(5,1361, -1, 399),(5,1360, -1, 399),(5,1359, -1, 399),(5,1358, -1, 399),(5,1357, -1, 399),(5,1356, -1, 399),(5,1355, -1, 399),(5,1354, -1, 399),(5,1353, -1, 399),(5,1352, -1, 399),(5,1351, -1, 399),(5,1350, -1, 399),(5,1349, -1, 399),(5,1348, -1, 399),(5,1347, -1, 399),(5,1346, -1, 399),(5,1345, -1, 399),(5,1344, -1, 399),(5,1343, -1, 399),(5,1342, -1, 399),(5,1341, -1, 399),(5,1340, -1, 399),(5,1339, -1, 399),(5,1338, -1, 399),(5,1337, -1, 399),(5,1336, -1, 399),(5,1335, -1, 399),(5,1334, -1, 399),(5,1333, -1, 399),(5,1332, -1, 399),(5,1331, -1, 399)];