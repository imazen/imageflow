use imageflow_helpers::preludes::from_std::*;
use ::std;
use ::url::Url;
use ::imageflow_types as s;
use ::option_filter::OptionFilterExt;

macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(FlipStringsVariants), IterVariantNames!(FlipStringsNames))]
pub enum FlipStrings{
    None,
    H,
    X,
    V,
    Y,
    Both,
    XY
}

}

macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(OutputFormatStringsVariants), IterVariantNames!(OutputFormatStringsNames))]
pub enum OutputFormatStrings {
    Jpg,
    Jpe,
    Jif,
    Jfif,
    Jfi,
    Exif,
    Jpeg,
    Png,
    Gif
}
}

macro_attr! {


#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(ScaleModeStringsVariants), IterVariantNames!(ScaleModeStringsNames))]
pub enum ScaleModeStrings{
    Down,
    DownscaleOnly,
    Up,
    UpscaleOnly,
    Both,
    Canvas,
    UpscaleCanvas
}
}
macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(FitModeVariants), IterVariantNames!(FitModeNames))]
/// How to resolve aspect ratio differences between the requested size and the original image's size.
pub enum FitModeStrings {
    /// Fit mode will be determined by other settings, such as &amp;carve=true, &amp;stretch=fill, and &amp;crop=auto. If none are specified and width/height are specified , &amp;mode=pad will be used. If maxwidth/maxheight are used, &amp;mode=max will be used.
    None,

    /// Width and height are considered maximum values. The resulting image may be smaller to maintain its aspect ratio. The image may also be smaller if the source image is smaller
    Max,
    /// Width and height are considered exact values - padding is used if there is an aspect ratio difference. Use &amp;anchor to override the MiddleCenter default.
    Pad,
    /// Width and height are considered exact values - cropping is used if there is an aspect ratio difference. Use &amp;anchor to override the MiddleCenter default.
    Crop,
    /// Width and height are considered exact values - seam carving is used if there is an aspect ratio difference. Requires the SeamCarving plugin to be installed, otherwise behaves like 'pad'.
    Carve,
    /// Width and height are considered exact values - if there is an aspect ratio difference, the image is stretched.
    Stretch,
}
}
macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(ServerCacheModeVariants), IterVariantNames!(ServerCacheModeNames))]
/// When to disk cache the image
pub enum ServerCacheMode {
    /// Request no disk caching of the resulting image.
    No,
    /// Request that the resulting image always be disk cached on the server, even if no modifications are made.
    Always,
    /// Default caching behavior. Modified images are disk cached, unmodified images are not.
    Default

}}
macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(ProcessWhenVariants), IterVariantNames!(ProcessWhenNames))]
/// When to process and re-encode the file.
pub enum ProcessWhen {
    /// Request no processing of the image or file (generally used with cache=always).
    /// The file contents will be used as-is.
    No,
    /// Require the file or image to be processed. Will cause non-image files to fail with an ImageCorruptedException.
    Always,
    /// Default. Only files with both a supported image extension and resizing settings specified in the querystring will be processed.
    Default
}


}



pub static IR4_KEYS: [&'static str;58] = ["mode", "anchor", "flip", "sflip", "scale", "cache", "process",
    "quality", "zoom", "crop", "cropxunits", "cropyunits",
    "w", "h", "width", "height", "maxwidth", "maxheight", "format", "thumbnail",
     "autorotate", "srotate", "rotate", "ignoreicc", //really? : "precise_scaling_ratio",
    "stretch",
    "frame", "page", "subsampling", "colors",
    "404", "bgcolor", "paddingcolor", "bordercolor", "preset", "floatspace", "jpeg_idct_downscale_linear", "watermark",
    "s.invert", "s.sepia", "s.grayscale", "s.alpha", "s.brightness", "s.contrast", "s.saturation", "trim.threshold",
    "trim.percentpadding", "a.blur", "a.sharpen", "a.removenoise", "dither",
    "encoder", "decoder", "builder", "s.roundcorners.", "paddingwidth", "paddingheight", "margin", "borderwidth"];


#[derive(PartialEq,Debug, Clone)]
pub enum ParseWarning{
    // We don't really support comma concatenation like ImageResizer (in theory) did
    DuplicateKey((String, String)),
    // Not an IR4
    KeyNotRecognized((String, String)),
    KeyNotSupported((String, String)),
    ValueInvalid((&'static str, String))
}

pub fn parse_url(url: &Url) -> (Instructions, Vec<ParseWarning>) {
    let mut warnings = Vec::new();
    let mut map = HashMap::new();
    for (key, value) in url.query_pairs() {
        let k = key.to_lowercase(); //Trim whitespace?
        let v = value.into_owned();
        if map.contains_key(&k) {
            warnings.push(ParseWarning::DuplicateKey((k, v)));
        } else {
            if !IR4_KEYS.contains(&k.as_str()) {
                warnings.push(ParseWarning::KeyNotRecognized((k, v)));
            } else {
                map.insert(k, v.to_owned());
            }
        }
    }
    let i = Instructions::delete_from_map(&mut map, Some(&mut warnings));
    for (k, v) in map.drain() {
        warnings.push(ParseWarning::KeyNotSupported((k, v)));
    }
        (i, warnings)
}


impl Instructions{
    pub fn delete_from_map(map: &mut HashMap<String,String>, warnings: Option<&mut Vec<ParseWarning>>) -> Instructions {
        let mut p = Parser { m: map, w: warnings, delete_supported: true };
        let mut i = Instructions::new();
        i.w = p.parse_i32("width").or(p.parse_i32("w"));
        i.h = p.parse_i32("height").or(p.parse_i32("h"));
        i.legacy_max_height = p.parse_i32("maxheight");
        i.legacy_max_width = p.parse_i32("maxwidth");
        i.flip = p.parse_flip("flip").map(|v| v.clean());
        i.sflip = p.parse_flip("sflip").or(p.parse_flip("sourceFlip")).map(|v| v.clean());

        let mode_string = p.parse_fit_mode("mode");
        if mode_string == Some(FitModeStrings::Carve){
           p.warn(ParseWarning::ValueInvalid(("mode", "carve".to_owned())).to_owned());
        }

        i.mode = mode_string.and_then(|v| v.clean())
            .or(p.parse_test_pair("stretch", "fill").and_then(|b| if b { Some(FitMode::Stretch) } else { None }))
            .or(p.parse_test_pair("crop", "auto").and_then(|b| if b { Some(FitMode::Crop) } else { None }));

        i.scale = p.parse_scale("scale").map(|v| v.clean());

        //Actually supported!
//        if i.scale == Some(ScaleMode::UpscaleOnly){
//            warnings.push(ParseWarning::ValueInvalid("scale", "upscaleonly".to_owned()));
//        }

        i.format = p.parse_format("format").or(p.parse_format("thumbnail")).map(|v| v.clean());
        i.srotate = p.parse_rotate("srotate");
        i.rotate = p.parse_rotate("rotate");
        i.autorotate = p.parse_bool("autorotate");
        i.ignoreicc = p.parse_bool("ignoreicc");
        i.crop = p.parse_crop_strict("crop").or_else(|| p.parse_crop("crop"));
        i.cropxunits = p.parse_f64("cropxunits");
        i.cropyunits = p.parse_f64("cropyunits");
        i.quality = p.parse_i32("quality");
        i.zoom = p.parse_f64("zoom");
        i.bgcolor_srgb = p.parse_color_srgb("bgcolor").or_else(||p.parse_color_srgb("bgcolor"));
        i.jpeg_subsampling = p.parse_subsampling("subsampling");
        i.f_sharpen = p.parse_f64("f.sharpen");
        i.anchor = p.parse_anchor("anchor");

        let _ = p.parse_test_pair("fastscale", "true");


        i
    }

    pub fn to_framewise(&self) -> s::Framewise{
        s::Framewise::example_graph()
    }
    pub fn new() -> Instructions{
        Default::default()
    }
}

//
struct Parser<'a>{
    m: &'a mut HashMap<String,String>,
    w: Option<&'a mut Vec<ParseWarning>>,
    /// We leave pairs in the map if we do not support them (or we support them, but they are invalid)
    delete_supported: bool
}
impl<'a> Parser<'a>{

    fn warn(&mut self, warning: ParseWarning){
        if self.w.is_some() {
            self.w.as_mut().unwrap().push(warning);
        }
    }
    fn warning_parse<F,T,E>(&mut self, key: &'static str, f: F) -> Option<T>
        where F: Fn(&str) -> std::result::Result<(T,Option<ParseWarning>, bool),E>{
        //Coalesce null and whitespace values to None
        let (r, supported) = {
            let v = self.m.get(key).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());

            if let Some(s) = v {
                match f(&s) {
                    Err(_) => {
                        self.warn(ParseWarning::ValueInvalid((key, s.to_owned())));
                        (None, false) // We assume an error means the value wasn't supported
                    },
                    Ok((v,w,supported)) => {
                        if w.is_some(){
                           self.warn(w.unwrap());
                        }
                            (Some(v), supported)
                    }
                }
            } else {
                (None, true) //We support (ignore) null and whitespace values in IR4
            }
        };
        if supported && self.delete_supported && self.m.contains_key(key) {
            self.m.remove(key);
        }
        r
    }
    fn parse<F,T,E>(&mut self, key: &'static str, f: F) -> Option<T>
            where F: Fn(&str) -> std::result::Result<T,E>{
        self.warning_parse(key, |s| f(s).map(|v| (v, None, true)) )
    }

    fn parse_test_pair(&mut self, key: &'static str, value: &'static str) -> Option<bool> {
        self.warning_parse(key, |s| -> std::result::Result<(bool, Option<ParseWarning>, bool), ()> {
            if s.eq_ignore_ascii_case(value) {
                Ok((true, None, true))
            } else {
                Ok((false, None, false))
            }
        })
    }

    fn parse_crop_strict(&mut self, key: &'static str) -> Option<[f64;4]> {
        self.warning_parse(key, |s| {
            let values = s.split(',').map(|v| v.trim().parse::<f64>()).collect::<Vec<std::result::Result<f64,::std::num::ParseFloatError>>>();
            if let Some(&Err(ref e)) = values.iter().find(|v| v.is_err()) {
                Err(ParseCropError::InvalidNumber(e.clone()))
            } else if values.len() != 4 {
                Err(ParseCropError::InvalidNumberOfValues("Crops must contain exactly 4 values, separated by commas"))
            } else {
                Ok(([*values[0].as_ref().unwrap(), *values[1].as_ref().unwrap(), *values[2].as_ref().unwrap(), *values[3].as_ref().unwrap()], None, true))
            }
        }
        )
    }


    fn parse_crop(&mut self, key: &'static str) -> Option<[f64;4]> {
        self.warning_parse(key, |s| {
            // TODO: We're also supposed to trim leading/trailing commas along with whitespace
            let str = s.replace("(", "").replace(")", "");
            // .unwrap_or(0) is ugly, but it's what IR4 does. :(
            let values = str.trim().split(',').map(|v| v.trim().parse::<f64>().unwrap_or(0f64)).collect::<Vec<f64>>();
            if values.len() == 4 {
                Ok(([values[0], values[1], values[2], values[3]], None, true))
            } else {
                Err(())
            }
        }
        )
    }

    fn parse_bool(&mut self, key: &'static str) -> Option<bool>{
        self.parse(key, |s|
            match s.to_lowercase().as_str(){
                "true" | "1" | "yes" | "on" => Ok(true),
                "false" | "0" | "no" | "off" => Ok(false),
                _ => Err(())
            }
        )
    }

    fn parse_i32(&mut self, key: &'static str) -> Option<i32>{
        self.parse(key, |s| s.parse::<i32>() )
    }
    fn parse_f64(&mut self, key: &'static str) -> Option<f64>{
        self.parse(key, |s| s.parse::<f64>() )
    }


    fn parse_subsampling(&mut self, key: &'static str) -> Option<i32>{
        self.parse(key, |s|
            s.parse::<i32>().map_err(|e| ()).and_then(|v|
                match v {
                    411 | 420 | 444 | 422 => Ok(v),
                    _ => Err(())
                }
            )
        )
    }

    fn parse_rotate(&mut self, key: &'static str) -> Option<i32>{
        self.warning_parse(key, |s|

            match s.parse::<f32>(){
                Ok(value) => {
                    let result = ((((value / 90f32).round() % 4f32) as i32 + 4) % 4) * 90;
                    if value % 90f32 > 0.1f32{
                        Ok((result, Some(ParseWarning::ValueInvalid((key, s.to_owned()))), false))
                    }else {
                        Ok((result, None, true))
                    }
                }
                Err(e) => Err(e)
            }

        )
    }

    fn parse_fit_mode(&mut self, key: &'static str) -> Option<FitModeStrings>{
        self.parse(key, |value| {
            for (k, v) in FitModeStrings::iter_variant_names().zip(FitModeStrings::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v)
                }
            }
            Err(())
        })
    }

    fn parse_scale(&mut self, key: &'static str) -> Option<ScaleModeStrings>{
        self.parse(key, |value| {
            for (k, v) in ScaleModeStrings::iter_variant_names().zip(ScaleModeStrings::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v)
                }
            }
            Err(())
        })
    }

    fn parse_flip(&mut self, key: &'static str) -> Option<FlipStrings>{
        self.parse(key, |value| {
            for (k, v) in FlipStrings::iter_variant_names().zip(FlipStrings::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v)
                }
            }
            Err(())
        })
    }
    fn parse_format(&mut self, key: &'static str) -> Option<OutputFormatStrings>{
        self.parse(key, |value| {
            for (k, v) in OutputFormatStrings::iter_variant_names().zip(OutputFormatStrings::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v)
                }
            }
            Err(())
        })
    }

    fn parse_argb(a :&str, r: &str, g: &str, b: &str) -> Result<u32,std::num::ParseIntError>{
        [a,r,g,b].iter().map(|s|{
            match s.len() {
                0 => Ok(255),
                1 => u8::from_str_radix(s, 16).map(|v| (v << 4) | v),
                2 => u8::from_str_radix(s, 16),
                _ => { panic! {"segments may be zero to two characters, but no more"}; }
            }.map(|v| v as u32)
        }).fold(Ok(0u32), |acc, item| {
            if let Ok(argb) = acc{
                if let Ok(v) = item {
                    Ok(argb.checked_shl(8).expect("4 8-bit shifts cannot overflow u32 when starting with zero") | v)
                }else{
                    item
                }
            }else{
                acc
            }
        })
    }


    /// #AARRGGBB #RRGGBB #RGB #ARGB named - with and without leading #, case insensitive
    fn parse_color_srgb(&mut self, key: &'static str) -> Option<u32>{
        self.parse(key, |value| {
            let value = match &value[0..1] { "#" => &value[1..], _ => &value};
            let u32_result = u32::from_str_radix(value, 16);
            if u32_result.is_ok(){
                let why = "Any substring of a valid hexadecimal string should also be a valid hexadecimal string";
                match value.len(){
                    3 => Ok(Self::parse_argb("", &value[0..1], &value[1..2], &value[2..3]).expect(why)),
                    4 => Ok(Self::parse_argb(&value[0..1], &value[1..2], &value[2..3], &value[3..4]).expect(why)),
                    6 => Ok(Self::parse_argb("", &value[0..2], &value[2..4], &value[4..6]).expect(why)),
                    8 => Ok(Self::parse_argb(&value[0..2], &value[2..4], &value[4..6], &value[6..8]).expect(why)),
                    _ => Err(ParseColorError::FormatIncorrect("CSS Colors must be in the form [#]RGB, [#]ARGB, [#]AARRGGBB, or [#]RRGGBB, or be a named CSS color. "))
                }
            } else {
                match COLORS.get(value.to_lowercase().as_str()){
                    Some(v) => Ok(*v),
                    None => Err(ParseColorError::ColorNotRecognized(u32_result.unwrap_err()))
                }
            }
        })
    }

    fn parse_anchor(&mut self, key: &'static str) -> Option<(Anchor1D,Anchor1D)> {
        self.parse(key, |value| {
            match value.to_lowercase().as_str() {
                "topleft" => Ok((Anchor1D::Near, Anchor1D::Near)),
                "topcenter" => Ok((Anchor1D::Center, Anchor1D::Near)),
                "topright" => Ok((Anchor1D::Far, Anchor1D::Near)),
                "middleleft" => Ok((Anchor1D::Near, Anchor1D::Center)),
                "middlecenter" => Ok((Anchor1D::Center, Anchor1D::Center)),
                "middleright" => Ok((Anchor1D::Far, Anchor1D::Center)),
                "bottomleft" => Ok((Anchor1D::Near, Anchor1D::Far)),
                "bottomcenter" => Ok((Anchor1D::Center, Anchor1D::Far)),
                "bottomright" => Ok((Anchor1D::Far, Anchor1D::Far)),
                _ => Err(())
            }
        })
    }

}



#[derive(Debug,Clone,PartialEq)]
enum ParseColorError{
    ColorNotRecognized(std::num::ParseIntError),
    FormatIncorrect(&'static str)
}

#[derive(Debug,Clone,PartialEq)]
enum ParseCropError{
    InvalidNumber(std::num::ParseFloatError),
    InvalidNumberOfValues(&'static str)
}

impl OutputFormatStrings{
    pub fn clean(&self) -> OutputFormat{
        match *self{
            OutputFormatStrings::Png => OutputFormat::Png,
            OutputFormatStrings::Gif => OutputFormat::Gif,
            _ => OutputFormat::Jpeg
        }
    }
}

impl FlipStrings{
    pub fn clean(&self) -> (bool,bool){
        match *self{
            FlipStrings::None => (false,false),
            FlipStrings::X | FlipStrings::H => (true, false),
            FlipStrings::Y | FlipStrings::V => (false, true),
            FlipStrings::XY | FlipStrings::Both => (true, true)
         }
    }
}
impl FitModeStrings{
    pub fn clean(&self) -> Option<FitMode>{
        match *self{
            FitModeStrings::None => None,
            FitModeStrings::Max => Some(FitMode::Max),
            FitModeStrings::Pad => Some(FitMode::Pad),
            FitModeStrings::Crop => Some(FitMode::Crop),
            FitModeStrings::Carve => Some(FitMode::Stretch),
            FitModeStrings::Stretch => Some(FitMode::Stretch)
        }
    }
}




impl ScaleModeStrings{
    pub fn clean(&self) -> ScaleMode{
        match *self{
            ScaleModeStrings::DownscaleOnly | ScaleModeStrings::Down => ScaleMode::DownscaleOnly,
            ScaleModeStrings::UpscaleOnly | ScaleModeStrings::Up => ScaleMode::UpscaleOnly,
            ScaleModeStrings::UpscaleCanvas | ScaleModeStrings::Canvas => ScaleMode::UpscaleCanvas,
            ScaleModeStrings::Both => ScaleMode::Both,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// How to resolve aspect ratio differences between the requested size and the original image's size.
pub enum FitMode {
    /// Width and height are considered maximum values. The resulting image may be smaller to maintain its aspect ratio. The image may also be smaller if the source image is smaller
    Max,
    /// Width and height are considered exact values - padding is used if there is an aspect ratio difference. Use &amp;anchor to override the MiddleCenter default.
    Pad,
    /// Width and height are considered exact values - cropping is used if there is an aspect ratio difference. Use &amp;anchor to override the MiddleCenter default.
    Crop,
    /// Width and height are considered exact values - if there is an aspect ratio difference, the image is stretched.
    Stretch,
}


#[derive(Default,Debug,Clone,Copy,PartialEq)]
pub struct Instructions{
    pub w: Option<i32>,
    pub h: Option<i32>,
    pub legacy_max_width: Option<i32>,
    pub legacy_max_height: Option<i32>,
    pub mode: Option<FitMode>,
    pub scale: Option<ScaleMode>,
    pub format: Option<OutputFormat>,
    pub flip: Option<(bool,bool)>,
    pub sflip: Option<(bool,bool)>,
    pub srotate: Option<i32>,
    pub rotate: Option<i32>,
    pub autorotate: Option<bool>,
    pub ignoreicc: Option<bool>,
    pub crop: Option<[f64;4]>,
    pub cropxunits: Option<f64>,
    pub cropyunits: Option<f64>,
    pub zoom: Option<f64>,
    pub quality: Option<i32>,
    pub f_sharpen: Option<f64>,
    pub bgcolor_srgb: Option<u32>,
    pub jpeg_subsampling: Option<i32>,
    pub anchor: Option<(Anchor1D, Anchor1D)>
}
#[derive(Debug,Copy, Clone,PartialEq)]
pub enum Anchor1D{
    Near,
    Center,
    Far
}

#[derive(Debug,Copy, Clone,PartialEq)]
pub enum OutputFormat{
    Jpeg,
    Png,
    Gif
}

/// Controls whether the image is allowed to upscale, downscale, both, or if only the canvas gets to be upscaled.
#[derive(Debug,Copy, Clone,PartialEq)]
pub enum ScaleMode {
    /// The default. Only downsamples images - never enlarges. If an image is smaller than 'width' and 'height', the image coordinates are used instead.
    DownscaleOnly,
    /// Only upscales (zooms) images - never downsamples except to meet web.config restrictions. If an image is larger than 'width' and 'height', the image coordinates are used instead.
    UpscaleOnly,
    /// Upscales and downscales images according to 'width' and 'height', within web.config restrictions.
    Both,
    /// When the image is smaller than the requested size, padding is added instead of stretching the image
    UpscaleCanvas
}

fn debug_diff<T>(a : &T, b: &T) where T: std::fmt::Debug, T: PartialEq{
    if a != b {
        let text1 = format!("{:#?}", a);
        let text2 = format!("{:#?}", b);
        use ::difference::{diff, Difference};

        // compare both texts, the third parameter defines the split level
        let (_dist, changeset) = diff(&text1, &text2, "\n");

        let mut t = ::std::io::stderr();

        for i in 0..changeset.len() {
            match changeset[i] {
                Difference::Same(ref x) => {
                    let _ = writeln!(t, " {}", x);
                },
                Difference::Add(ref x) => {
                    let _ = writeln!(t, "+{}", x);
                },
                Difference::Rem(ref x) => {
                    let _ = writeln!(t, "-{}", x);
                }
            }
        }
    }
}
#[test]
fn test_url_parsing() {
    fn t(rel_url: &str, expected: Instructions, expected_warnings: Vec<ParseWarning>){
        let url = format!("http://localhost/image.jpg?{}", rel_url);
        let a = Url::from_str(&url).unwrap();
        let (i, warns) = parse_url(&a);
        if i.bgcolor_srgb != expected.bgcolor_srgb && i.bgcolor_srgb.is_some() && expected.bgcolor_srgb.is_some(){
            let _ = write!(::std::io::stderr(), "Expected bgcolor={:08X}, actual={:08X}\n", expected.bgcolor_srgb.unwrap(), i.bgcolor_srgb.unwrap());
        }
        debug_diff(&i, &expected);
        assert_eq!(i, expected);
        assert_eq!(warns, expected_warnings);
    }
    fn expect_warning(key: &'static str, value: &str, expected: Instructions){
        let mut expect_warnings = Vec::new();
        expect_warnings.push(ParseWarning::ValueInvalid((key, value.to_owned())));
        let url = format!("{}={}", key, value);
        t(&url, expected, expect_warnings)
    }

    t("w=200&h=300&mode=max", Instructions { w: Some(200), h: Some(300), mode: Some(FitMode::Max), ..Default::default() }, vec![]);
    t("w=200&h=300&mode=crop", Instructions { w: Some(200), h: Some(300), mode: Some(FitMode::Crop), ..Default::default() }, vec![]);
    t("format=jpeg", Instructions { format: Some(OutputFormat::Jpeg), ..Default::default() }, vec![]);
    t("format=png", Instructions { format: Some(OutputFormat::Png), ..Default::default() }, vec![]);
    t("maxwidth=1&maxheight=3", Instructions { legacy_max_height: Some(3), legacy_max_width: Some(1), ..Default::default() }, vec![]);
    t("scale=down", Instructions {scale: Some(ScaleMode::DownscaleOnly), ..Default::default() }, vec![]);
    t("width=20&Height=300&scale=Canvas", Instructions { w: Some(20), h: Some(300), scale: Some(ScaleMode::UpscaleCanvas), ..Default::default() }, vec![]);
    t("sflip=XY&flip=h", Instructions { sflip: Some((true,true)), flip: Some((true,false)), ..Default::default() }, vec![]);
    t("sflip=None&flip=V", Instructions { sflip: Some((false,false)), flip: Some((false,true)), ..Default::default() }, vec![]);
    t("sflip=None&flip=V", Instructions { sflip: Some((false,false)), flip: Some((false,true)), ..Default::default() }, vec![]);
    t("srotate=360&rotate=-90", Instructions { srotate: Some(0), rotate: Some(270), ..Default::default() }, vec![]);
    t("srotate=-20.922222&rotate=-46.2", Instructions { srotate: Some(0), rotate: Some(270), ..Default::default() }, vec![]);
    t("autorotate=false&ignoreicc=true", Instructions { autorotate: Some(false), ignoreicc: Some(true) , ..Default::default() }, vec![]);
    t("mode=max&stretch=fill", Instructions { mode: Some(FitMode::Max), ..Default::default() }, vec![]);
    t("stretch=fill", Instructions { mode: Some(FitMode::Stretch), ..Default::default() }, vec![]);
    t("crop=auto", Instructions { mode: Some(FitMode::Crop), ..Default::default() }, vec![]);
    t("thumbnail=exif", Instructions { format: Some(OutputFormat::Jpeg), ..Default::default() }, vec![]);
    t("cropxunits=2.3&cropyunits=100", Instructions { cropxunits: Some(2.3f64), cropyunits: Some(100f64), ..Default::default() }, vec![]);
    t("quality=85", Instructions { quality: Some(85), ..Default::default() }, vec![]);
    t("zoom=0.02", Instructions { zoom: Some(0.02f64), ..Default::default() }, vec![]);


    t("bgcolor=red", Instructions { bgcolor_srgb: Some(0xffff0000), ..Default::default() }, vec![]);
    t("bgcolor=f00", Instructions { bgcolor_srgb: Some(0xffff0000), ..Default::default() }, vec![]);
    t("bgcolor=ff00", Instructions { bgcolor_srgb: Some(0xffff0000), ..Default::default() }, vec![]);
    t("bgcolor=ff0000", Instructions { bgcolor_srgb: Some(0xffff0000), ..Default::default() }, vec![]);
    t("bgcolor=ffff0000", Instructions { bgcolor_srgb: Some(0xffff0000), ..Default::default() }, vec![]);

    t("bgcolor=darkseagreen", Instructions { bgcolor_srgb: Some(0xff8fbc8b), ..Default::default() }, vec![]);
    t("bgcolor=8fbc8b", Instructions { bgcolor_srgb: Some(0xff8fbc8b), ..Default::default() }, vec![]);
    t("bgcolor=ff8fbc8b", Instructions { bgcolor_srgb: Some(0xff8fbc8b), ..Default::default() }, vec![]);

    t("bgcolor=lightslategray", Instructions { bgcolor_srgb: Some(0xff778899), ..Default::default() }, vec![]);
    t("bgcolor=789", Instructions { bgcolor_srgb: Some(0xff778899), ..Default::default() }, vec![]);
    t("bgcolor=f789", Instructions { bgcolor_srgb: Some(0xff778899), ..Default::default() }, vec![]);
    t("bgcolor=778899", Instructions { bgcolor_srgb: Some(0xff778899), ..Default::default() }, vec![]);
    t("bgcolor=53778899", Instructions { bgcolor_srgb: Some(0x53778899), ..Default::default() }, vec![]);

    t("bgcolor=white", Instructions { bgcolor_srgb: Some(0xffffffff), ..Default::default() }, vec![]);
    t("bgcolor=fff", Instructions { bgcolor_srgb: Some(0xffffffff), ..Default::default() }, vec![]);
    t("bgcolor=ffff", Instructions { bgcolor_srgb: Some(0xffffffff), ..Default::default() }, vec![]);
    t("bgcolor=ffffff", Instructions { bgcolor_srgb: Some(0xffffffff), ..Default::default() }, vec![]);
    t("bgcolor=ffffffff", Instructions { bgcolor_srgb: Some(0xffffffff), ..Default::default() }, vec![]);

    t("crop=0,0,40,50", Instructions { crop: Some([0f64,0f64,40f64,50f64]), ..Default::default() }, vec![]);
    t("crop= 0, 0,40 ,  50", Instructions { crop: Some([0f64,0f64,40f64,50f64]), ..Default::default() }, vec![]);


    expect_warning("crop","(0,3,80, 90)",  Instructions { crop: Some([0f64,3f64,80f64,90f64]), ..Default::default() });

    expect_warning("crop","(0,3,happy, 90)",  Instructions { crop: Some([0f64,3f64,0f64,90f64]), ..Default::default() });

    expect_warning("crop","(  a0, 3, happy, 90)",  Instructions { crop: Some([0f64,3f64,0f64,90f64]), ..Default::default() });

}

macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

lazy_static!{
    static ref COLORS: HashMap<&'static str, u32> = create_css_color_map();
}

fn create_css_color_map() -> HashMap<&'static str, u32> {
    map! {
        "transparent" => 0x00ffffff,
        "aliceblue" => 0xfff0f8ff,
        "antiquewhite" => 0xfffaebd7,
        "aqua" => 0xff00ffff,
        "aquamarine" => 0xff7fffd4,
        "azure" => 0xfff0ffff,
        "beige" => 0xfff5f5dc,
        "bisque" => 0xffffe4c4,
        "black" => 0xff000000,
        "blanchedalmond" => 0xffffebcd,
        "blue" => 0xff0000ff,
        "blueviolet" => 0xff8a2be2,
        "brown" => 0xffa52a2a,
        "burlywood" => 0xffdeb887,
        "cadetblue" => 0xff5f9ea0,
        "chartreuse" => 0xff7fff00,
        "chocolate" => 0xffd2691e,
        "coral" => 0xffff7f50,
        "cornflowerblue" => 0xff6495ed,
        "cornsilk" => 0xfffff8dc,
        "crimson" => 0xffdc143c,
        "cyan" => 0xff00ffff,
        "darkblue" => 0xff00008b,
        "darkcyan" => 0xff008b8b,
        "darkgoldenrod" => 0xffb8860b,
        "darkgray" => 0xffa9a9a9,
        "darkgrey" => 0xffa9a9a9,
        "darkgreen" => 0xff006400,
        "darkkhaki" => 0xffbdb76b,
        "darkmagenta" => 0xff8b008b,
        "darkolivegreen" => 0xff556b2f,
        "darkorange" => 0xffff8c00,
        "darkorchid" => 0xff9932cc,
        "darkred" => 0xff8b0000,
        "darksalmon" => 0xffe9967a,
        "darkseagreen" => 0xff8fbc8b,
        "darkslateblue" => 0xff483d8b,
        "darkslategray" => 0xff2f4f4f,
        "darkslategrey" => 0xff2f4f4f,
        "darkturquoise" => 0xff00ced1,
        "darkviolet" => 0xff9400d3,
        "deeppink" => 0xffff1493,
        "deepskyblue" => 0xff00bfff,
        "dimgray" => 0xff696969,
        "dimgrey" => 0xff696969,
        "dodgerblue" => 0xff1e90ff,
        "firebrick" => 0xffb22222,
        "floralwhite" => 0xfffffaf0,
        "forestgreen" => 0xff228b22,
        "fuchsia" => 0xffff00ff,
        "gainsboro" => 0xffdcdcdc,
        "ghostwhite" => 0xfff8f8ff,
        "gold" => 0xffffd700,
        "goldenrod" => 0xffdaa520,
        "gray" => 0xff808080,
        "grey" => 0xff808080,
        "green" => 0xff008000,
        "greenyellow" => 0xffadff2f,
        "honeydew" => 0xfff0fff0,
        "hotpink" => 0xffff69b4,
        "indianred" => 0xffcd5c5c,
        "indigo" => 0xff4b0082,
        "ivory" => 0xfffffff0,
        "khaki" => 0xfff0e68c,
        "lavender" => 0xffe6e6fa,
        "lavenderblush" => 0xfffff0f5,
        "lawngreen" => 0xff7cfc00,
        "lemonchiffon" => 0xfffffacd,
        "lightblue" => 0xffadd8e6,
        "lightcoral" => 0xfff08080,
        "lightcyan" => 0xffe0ffff,
        "lightgoldenrodyellow" => 0xfffafad2,
        "lightgray" => 0xffd3d3d3,
        "lightgrey" => 0xffd3d3d3,
        "lightgreen" => 0xff90ee90,
        "lightpink" => 0xffffb6c1,
        "lightsalmon" => 0xffffa07a,
        "lightseagreen" => 0xff20b2aa,
        "lightskyblue" => 0xff87cefa,
        "lightslategray" => 0xff778899,
        "lightslategrey" => 0xff778899,
        "lightslategrey" => 0xff778899,
        "lightsteelblue" => 0xffb0c4de,
        "lightyellow" => 0xffffffe0,
        "lime" => 0xff00ff00,
        "limegreen" => 0xff32cd32,
        "linen" => 0xfffaf0e6,
        "magenta" => 0xffff00ff,
        "maroon" => 0xff800000,
        "mediumaquamarine" => 0xff66cdaa,
        "mediumblue" => 0xff0000cd,
        "mediumorchid" => 0xffba55d3,
        "mediumpurple" => 0xff9370db,
        "mediumseagreen" => 0xff3cb371,
        "mediumslateblue" => 0xff7b68ee,
        "mediumspringgreen" => 0xff00fa9a,
        "mediumturquoise" => 0xff48d1cc,
        "mediumvioletred" => 0xffc71585,
        "midnightblue" => 0xff191970,
        "mintcream" => 0xfff5fffa,
        "mistyrose" => 0xffffe4e1,
        "moccasin" => 0xffffe4b5,
        "navajowhite" => 0xffffdead,
        "navy" => 0xff000080,
        "oldlace" => 0xfffdf5e6,
        "olive" => 0xff808000,
        "olivedrab" => 0xff6b8e23,
        "orange" => 0xffffa500,
        "orangered" => 0xffff4500,
        "orchid" => 0xffda70d6,
        "palegoldenrod" => 0xffeee8aa,
        "palegreen" => 0xff98fb98,
        "paleturquoise" => 0xffafeeee,
        "palevioletred" => 0xffdb7093,
        "papayawhip" => 0xffffefd5,
        "peachpuff" => 0xffffdab9,
        "peru" => 0xffcd853f,
        "pink" => 0xffffc0cb,
        "plum" => 0xffdda0dd,
        "powderblue" => 0xffb0e0e6,
        "purple" => 0xff800080,
        "red" => 0xffff0000,
        "rosybrown" => 0xffbc8f8f,
        "royalblue" => 0xff4169e1,
        "saddlebrown" => 0xff8b4513,
        "salmon" => 0xfffa8072,
        "sandybrown" => 0xfff4a460,
        "seagreen" => 0xff2e8b57,
        "seashell" => 0xfffff5ee,
        "sienna" => 0xffa0522d,
        "silver" => 0xffc0c0c0,
        "skyblue" => 0xff87ceeb,
        "slateblue" => 0xff6a5acd,
        "slategray" => 0xff708090,
        "slategrey" => 0xff708090,
        "slategrey" => 0xff708090,
        "snow" => 0xfffffafa,
        "springgreen" => 0xff00ff7f,
        "steelblue" => 0xff4682b4,
        "tan" => 0xffd2b48c,
        "teal" => 0xff008080,
        "thistle" => 0xffd8bfd8,
        "tomato" => 0xffff6347,
        "turquoise" => 0xff40e0d0,
        "violet" => 0xffee82ee,
        "wheat" => 0xfff5deb3,
        "white" => 0xffffffff,
        "whitesmoke" => 0xfff5f5f5,
        "yellow" => 0xffffff00,
        "yellowgreen" => 0xff9acd32,
        "rebeccapurple"	=> 0xff663399
       }
}