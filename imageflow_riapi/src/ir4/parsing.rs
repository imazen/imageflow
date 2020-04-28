use imageflow_helpers::preludes::from_std::*;
use std;
use url::Url;
use imageflow_types as s;
#[allow(unused)] use option_filter::OptionFilterExt;
use imageflow_helpers::colors::*;

macro_attr! {


#[derive(Debug,Copy, Clone,PartialEq,
IterVariants!(SharpenWhenVariants), IterVariantNames!(SharpenWhenNames))]
pub enum SharpenWhen {
    Downscaling,
    SizeDiffers,
    Always
}

}

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
    Gif,
    Webp
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
    AspectCrop
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

macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(HistogramThresholdAlgorithmVariants), IterVariantNames!(HistogramThresholdAlgorithmNames))]

pub enum HistogramThresholdAlgorithm {
    Simple,
    Area,
    True,
    Gimp
}


}

macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(GrayscaleAlgorithmVariants), IterVariantNames!(GrayscaleAlgorithmNames))]

pub enum GrayscaleAlgorithm {
    Ntsc,
    True,
    Y,
    Ry,
    Flat,
    Bt709
}


}

macro_attr! {

#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(ScalingColorspaceVariants), IterVariantNames!(ScalingColorspaceNames))]

pub enum ScalingColorspace {
    Srgb,
    Linear,
    Gamma
}


}

pub static IR4_KEYS: [&'static str;72] = ["mode", "anchor", "flip", "sflip", "scale", "cache", "process",
    "quality", "zoom", "crop", "cropxunits", "cropyunits",
    "w", "h", "width", "height", "maxwidth", "maxheight", "format", "thumbnail",
     "autorotate", "srotate", "rotate", "ignoreicc", //really? : "precise_scaling_ratio",
    "stretch", "webp.lossless", "webp.quality",
    "frame", "page", "subsampling", "colors", "f.sharpen", "f.sharpen_when", "down.colorspace",
    "404", "bgcolor", "paddingcolor", "bordercolor", "preset", "floatspace",
    "jpeg_idct_downscale_linear", "watermark", "s.invert", "s.sepia", "s.grayscale", "s.alpha",
    "s.brightness", "s.contrast", "s.saturation",  "trim.threshold", "trim.percentpadding",
    "a.blur", "a.sharpen", "a.removenoise", "a.balancewhite", "dither","jpeg.progressive",
    "jpeg.turbo", "encoder", "decoder", "builder", "s.roundcorners.", "paddingwidth",
    "paddingheight", "margin", "borderwidth", "decoder.min_precise_scaling_ratio",
    "png.quality","png.min_quality", "png.quantization_speed", "png.libpng", "png.max_deflate"];


#[derive(PartialEq,Debug, Clone)]
pub enum ParseWarning{
    // We don't really support comma concatenation like ImageResizer (in theory) did
    DuplicateKey((String, String)),
    // Not an IR4
    KeyNotRecognized((String, String)),
    KeyNotSupported((String, String)),
    ValueInvalid((&'static str, String))
}

#[cfg_attr(feature = "cargo-clippy", allow(map_entry))]
pub fn parse_url(url: &Url) -> (Instructions, Vec<ParseWarning>) {
    let mut warnings = Vec::new();
    let mut map = HashMap::new();
    for (key, value) in url.query_pairs() {
        let k = key.to_lowercase(); //Trim whitespace?
        let v = value.into_owned();


        if map.contains_key(&k) {
            warnings.push(ParseWarning::DuplicateKey((k, v)));
        } else if !IR4_KEYS.contains(&k.as_str()) {
            warnings.push(ParseWarning::KeyNotRecognized((k, v)));
        } else {
            map.insert(k, v.to_owned());
        }
    }
    let i = Instructions::delete_from_map(&mut map, Some(&mut warnings));
    for (k, v) in map.drain() {
        warnings.push(ParseWarning::KeyNotSupported((k, v)));
    }
        (i, warnings)
}


impl Instructions{

    pub fn to_string(&self) -> String{
        let mut s = String::with_capacity(100);
        let mut vec = Vec::new();
        for (k,v) in self.to_map() {
            vec.push((k, v));
        }
        vec.sort_by_key(|&(a,_)| a);
        for (k,v) in vec{
            s.push_str(k);
            s.push_str("=");
            s.push_str(&v);
            s.push_str("&");
        }
        let len = s.len();
        if len > 0{
            s.remove(len - 1);
        }
        s
    }

    pub fn to_map(&self) -> HashMap<&'static str,String>{
        let mut m = HashMap::new();
        fn add<T>(m: &mut HashMap<&'static str,String>, key: &'static str, value: Option<T>) where T: std::fmt::Display{
            if value.is_some(){
                m.insert(key, format!("{}", value.unwrap()));
            }
        }
        fn flip_str(f: Option<(bool, bool)>) -> Option<String>{
            match f{
                Some((true, true)) => Some("xy".to_owned()),
                Some((true, false)) => Some("x".to_owned()),
                Some((false, true)) => Some("y".to_owned()),
                _ => None
            }
        }
        add(&mut m, "w", self.w);
        add(&mut m, "h", self.h);
        add(&mut m, "maxwidth", self.legacy_max_width);
        add(&mut m, "maxheight", self.legacy_max_height);
        add(&mut m, "flip", flip_str(self.flip));
        add(&mut m, "sflip", flip_str(self.sflip));
        add(&mut m, "mode", self.mode.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "scale", self.scale.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "format", self.format.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "srotate", self.srotate);
        add(&mut m, "rotate", self.rotate);
        add(&mut m, "autorotate", self.autorotate);
        add(&mut m, "ignoreicc", self.ignoreicc);
        add(&mut m, "cropxunits", self.cropxunits);
        add(&mut m, "cropyunits", self.cropyunits);
        add(&mut m, "quality", self.quality);
        add(&mut m, "webp.quality", self.webp_quality);
        add(&mut m, "webp.lossless", self.webp_lossless);
        add(&mut m, "zoom", self.zoom);

        add(&mut m, "s.contrast", self.s_contrast);

        add(&mut m, "s.alpha", self.s_alpha);
        add(&mut m, "s.brightness", self.s_brightness);
        add(&mut m, "s.saturation", self.s_saturation);
        add(&mut m, "s.sepia", self.s_sepia);
        add(&mut m, "jpeg.progressive", self.jpeg_progressive);
        add(&mut m, "jpeg.turbo", self.jpeg_turbo);
        add(&mut m, "png.quality", self.png_quality);
        add(&mut m, "png.min_quality", self.png_min_quality);
        add(&mut m, "png.quantization_speed", self.png_quantization_speed);
        add(&mut m, "png.libpng", self.png_libpng);
        add(&mut m, "png.max_deflate", self.png_max_deflate);
        add(&mut m, "s.grayscale", self.s_grayscale.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "a.balancewhite", self.a_balance_white.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "subsampling", self.jpeg_subsampling);
        add(&mut m, "bgcolor", self.bgcolor_srgb.and_then(|v| Some(v.to_rrggbbaa_string().to_lowercase())));
        add(&mut m, "f.sharpen", self.f_sharpen);
        add(&mut m, "f.sharpen_when", self.f_sharpen_when.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "trim.percentpadding", self.trim_whitespace_padding_percent);
        add(&mut m, "trim.threshold", self.trim_whitespace_threshold);

        add(&mut m, "crop", self.crop.map(|a| format!("{},{},{},{}", a[0],a[1],a[2],a[3])));
        add(&mut m, "anchor", self.anchor_string());


        add(&mut m, "down.colorspace", self.down_colorspace.map(|v| format!("{:?}", v).to_lowercase()));

        add(&mut m, "decoder.min_precise_scaling_ratio", self.min_precise_scaling_ratio);
        m
    }

    #[cfg_attr(feature = "cargo-clippy", allow(or_fun_call))]
    pub fn delete_from_map(map: &mut HashMap<String,String>, warnings: Option<&mut Vec<ParseWarning>>) -> Instructions {
        let mut p = Parser { m: map, w: warnings, delete_supported: true };
        let mut i = Instructions::new();
        i.f_sharpen = p.parse_f64("f.sharpen");
        i.f_sharpen_when = p.parse_sharpen_when("f.sharpen_when");

        i.w = p.parse_i32("width").or_else(|| p.parse_i32("w"));
        i.h = p.parse_i32("height").or_else(|| p.parse_i32("h"));
        i.legacy_max_height = p.parse_i32("maxheight");
        i.legacy_max_width = p.parse_i32("maxwidth");
        i.flip = p.parse_flip("flip").map(|v| v.clean());
        i.sflip = p.parse_flip("sflip").or_else(|| p.parse_flip("sourceFlip")).map(|v| v.clean());

        let mode_string = p.parse_fit_mode("mode");
        if mode_string == Some(FitModeStrings::Carve){
           p.warn(ParseWarning::ValueInvalid(("mode", "carve".to_owned())).to_owned());
        }

        // Side effects wanted for or()
        i.mode = mode_string.and_then(|v| v.clean())
            .or(p.parse_test_pair("stretch", "fill").and_then(|b| if b { Some(FitMode::Stretch) } else { None }))
            .or(p.parse_test_pair("crop", "auto").and_then(|b| if b { Some(FitMode::Crop) } else { None }));

        i.scale = p.parse_scale("scale").map(|v| v.clean());


        i.format = p.parse_format("format").or_else(|| p.parse_format("thumbnail")).map(|v| v.clean());
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

        i.webp_quality = p.parse_f64("webp.quality");
        i.webp_lossless = p.parse_bool("webp.lossless");
        i.png_min_quality = p.parse_u8("png.min_quality");
        i.png_quality = p.parse_u8("png.quality");
        i.png_quantization_speed= p.parse_u8("png.quantization_speed");
        i.png_libpng = p.parse_bool("png.libpng");
        i.png_max_deflate = p.parse_bool("png.max_deflate");
        i.anchor = p.parse_anchor("anchor");


        i.min_precise_scaling_ratio = p.parse_f64("decoder.min_precise_scaling_ratio");

        //TODO: warn bounds (-1..1, 0..255)
        i.trim_whitespace_padding_percent = p.parse_f64("trim.percentpadding");
        i.trim_whitespace_threshold = p.parse_i32("trim.threshold");

        i.s_grayscale = p.parse_grayscale("s.grayscale");
        i.s_contrast = p.parse_f64("s.contrast");
        i.s_alpha = p.parse_f64("s.alpha");
        i.s_saturation = p.parse_f64("s.saturation");
        i.s_brightness = p.parse_f64("s.brightness");
        i.s_sepia = p.parse_bool("s.sepia");
        i.a_balance_white = match p.parse_white_balance("a.balancewhite"){
            Some(HistogramThresholdAlgorithm::True) |
            Some(HistogramThresholdAlgorithm::Area) => Some(HistogramThresholdAlgorithm::Area),
            None => None,
            Some(other) => {
                p.warn(ParseWarning::ValueInvalid(("a.balancewhite", format!("{:?}", other).to_lowercase())));
                Some(other)
            }
        };

        i.down_colorspace = p.parse_colorspace("down.colorspace");


        let _ = p.parse_test_pair("fastscale", "true");
        i.jpeg_progressive = p.parse_bool("jpeg.progressive");
        i.jpeg_turbo = p.parse_bool("jpeg.turbo");

        i
    }

    fn anchor_string(&self) -> Option<String>{
        if let Some((v,h)) = self.anchor{
            let first = match v{
                Anchor1D::Near => "top",
                Anchor1D::Center => "middle",
                Anchor1D::Far => "bottom"
            };
            let last = match h{
                Anchor1D::Near => "left",
                Anchor1D::Center => "center",
                Anchor1D::Far => "right"
            };
            Some(format!("{}{}", first, last))
        }else{
            None
        }
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
    fn parse_u8(&mut self, key: &'static str) -> Option<u8>{
        self.parse(key, |s| s.parse::<u8>() )
    }
    fn parse_i32(&mut self, key: &'static str) -> Option<i32>{
        self.parse(key, |s| s.parse::<i32>() )
    }
    fn parse_f64(&mut self, key: &'static str) -> Option<f64>{
        self.parse(key, |s| s.parse::<f64>() )
    }


    fn parse_subsampling(&mut self, key: &'static str) -> Option<i32>{
        self.parse(key, |s|
            s.parse::<i32>().map_err(|_| ()).and_then(|v|
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

fn parse_colorspace(&mut self, key: &'static str) -> Option<ScalingColorspace> {
    self.parse(key, |value| {
        for (k, v) in ScalingColorspace::iter_variant_names().zip(ScalingColorspace::iter_variants()) {
            if k.eq_ignore_ascii_case(value) {
                return Ok(v)
            }
        }
        Err(())
    })
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

    fn parse_sharpen_when(&mut self, key: &'static str) -> Option<SharpenWhen>{
        self.parse(key, |value| {
            for (k, v) in SharpenWhen::iter_variant_names().zip(SharpenWhen::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v)
                }
            }
            Err(())
        })
    }

    fn parse_white_balance(&mut self, key: &'static str) -> Option<HistogramThresholdAlgorithm>{
        self.parse(key, |value| {
            for (k, v) in HistogramThresholdAlgorithm::iter_variant_names().zip(HistogramThresholdAlgorithm::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v)
                }
            }
            Err(())
        })
    }


    fn parse_grayscale(&mut self, key: &'static str) -> Option<GrayscaleAlgorithm>{
        self.parse(key, |value| {
            for (k, v) in GrayscaleAlgorithm::iter_variant_names().zip(GrayscaleAlgorithm::iter_variants()) {
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

    fn parse_color_srgb(&mut self, key: &'static str) -> Option<Color32>{
        self.parse(key, |value| {
            parse_color_hex_or_named(value)
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
enum ParseCropError{
    InvalidNumber(std::num::ParseFloatError),
    InvalidNumberOfValues(&'static str)
}

impl OutputFormatStrings{
    pub fn clean(&self) -> OutputFormat{
        match *self{
            OutputFormatStrings::Png => OutputFormat::Png,
            OutputFormatStrings::Gif => OutputFormat::Gif,
            OutputFormatStrings::Webp => OutputFormat::Webp,
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
            FitModeStrings::Carve |
            FitModeStrings::Stretch => Some(FitMode::Stretch),
            FitModeStrings::AspectCrop => Some(FitMode::AspectCrop)
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
    /// Width and height are considered a target aspect ratio for cropping
    AspectCrop,
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
    pub webp_quality: Option<f64>,
    pub webp_lossless: Option<bool>,
    pub f_sharpen: Option<f64>,
    pub f_sharpen_when: Option<SharpenWhen>,
    pub bgcolor_srgb: Option<Color32>,
    pub jpeg_subsampling: Option<i32>,
    pub anchor: Option<(Anchor1D, Anchor1D)>,
    pub trim_whitespace_threshold: Option<i32>,
    pub trim_whitespace_padding_percent: Option<f64>,
    pub a_balance_white: Option<HistogramThresholdAlgorithm>,
    pub s_alpha: Option<f64>,
    pub s_contrast: Option<f64>,
    pub s_saturation: Option<f64>,
    pub s_brightness: Option<f64>,
    pub s_sepia: Option<bool>,
    pub s_grayscale: Option<GrayscaleAlgorithm>,
    pub min_precise_scaling_ratio: Option<f64>,
    pub down_colorspace: Option<ScalingColorspace>,
    pub jpeg_progressive: Option<bool>,
    pub jpeg_turbo: Option<bool>,
    pub png_quality: Option<u8>,
    pub png_min_quality: Option<u8>,
    pub png_quantization_speed: Option<u8>,
    pub png_libpng: Option<bool>,
    pub png_max_deflate: Option<bool>
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
    Gif,
    Webp
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


#[cfg(test)]
fn debug_diff<T>(a : &T, b: &T) where T: std::fmt::Debug, T: PartialEq{
    if a != b {
        let text1 = format!("{:#?}", a);
        let text2 = format!("{:#?}", b);
        use ::difference::{Changeset, Difference};

        // compare both texts, the third parameter defines the split level
        let changeset = Changeset::new(&text1, &text2, "\n");

        let mut t = ::std::io::stderr();

        for i in 0..changeset.diffs.len() {
            match changeset.diffs[i] {
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
        // eprintln!("{} -> {}", &url, i.to_string());
        if i.bgcolor_srgb != expected.bgcolor_srgb && i.bgcolor_srgb.is_some() && expected.bgcolor_srgb.is_some(){
            let _ = write!(::std::io::stderr(), "Expected bgcolor={}, actual={}\n", expected.bgcolor_srgb.unwrap().to_aarrggbb_string(), i.bgcolor_srgb.unwrap().to_aarrggbb_string());
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
    t("format=gif", Instructions { format: Some(OutputFormat::Gif), ..Default::default() }, vec![]);
    t("height=200&format=gif", Instructions { format: Some(OutputFormat::Gif), h: Some(200), ..Default::default() }, vec![]);
    t("maxwidth=1&maxheight=3", Instructions { legacy_max_height: Some(3), legacy_max_width: Some(1), ..Default::default() }, vec![]);
    t("scale=down", Instructions {scale: Some(ScaleMode::DownscaleOnly), ..Default::default() }, vec![]);
    t("width=20&Height=300&scale=Canvas", Instructions { w: Some(20), h: Some(300), scale: Some(ScaleMode::UpscaleCanvas), ..Default::default() }, vec![]);
    t("sflip=XY&flip=h", Instructions { sflip: Some((true,true)), flip: Some((true,false)), ..Default::default() }, vec![]);
    t("sflip=None&flip=V", Instructions { sflip: Some((false,false)), flip: Some((false,true)), ..Default::default() }, vec![]);
    t("sflip=None&flip=V", Instructions { sflip: Some((false,false)), flip: Some((false,true)), ..Default::default() }, vec![]);
    t("srotate=360&rotate=-90", Instructions { srotate: Some(0), rotate: Some(270), ..Default::default() }, vec![]);
    t("srotate=-20.922222&rotate=-46.2", Instructions { srotate: Some(0), rotate: Some(270), ..Default::default() }, vec![]);
    t("autorotate=false&ignoreicc=true", Instructions { autorotate: Some(false), ignoreicc: Some(true) , ..Default::default() }, vec![]);
    t("mode=aspectcrop", Instructions { mode: Some(FitMode::AspectCrop), ..Default::default() }, vec![]);
    t("mode=max&stretch=fill", Instructions { mode: Some(FitMode::Max), ..Default::default() }, vec![]);
    t("stretch=fill", Instructions { mode: Some(FitMode::Stretch), ..Default::default() }, vec![]);
    t("crop=auto", Instructions { mode: Some(FitMode::Crop), ..Default::default() }, vec![]);
    t("thumbnail=exif", Instructions { format: Some(OutputFormat::Jpeg), ..Default::default() }, vec![]);
    t("cropxunits=2.3&cropyunits=100", Instructions { cropxunits: Some(2.3f64), cropyunits: Some(100f64), ..Default::default() }, vec![]);
    t("quality=85", Instructions { quality: Some(85), ..Default::default() }, vec![]);
    t("webp.quality=85", Instructions { webp_quality: Some(85f64), ..Default::default() }, vec![]);
    t("webp.lossless=true", Instructions { webp_lossless: Some(true), ..Default::default() }, vec![]);
    t("jpeg.progressive=true", Instructions { jpeg_progressive: Some(true), ..Default::default() }, vec![]);
    t("jpeg.turbo=true", Instructions { jpeg_turbo: Some(true), ..Default::default() }, vec![]);
    t("png.quality=90", Instructions { png_quality: Some(90), ..Default::default() }, vec![]);
    t("png.min_quality=90", Instructions { png_min_quality: Some(90), ..Default::default() }, vec![]);
    t("png.quantization_speed=4", Instructions { png_quantization_speed: Some(4), ..Default::default() }, vec![]);
    t("png.libpng=true", Instructions { png_libpng: Some(true), ..Default::default() }, vec![]);
    t("png.max_deflate=true", Instructions { png_max_deflate: Some(true), ..Default::default() }, vec![]);
    t("zoom=0.02", Instructions { zoom: Some(0.02f64), ..Default::default() }, vec![]);
    t("trim.threshold=80&trim.percentpadding=0.02", Instructions { trim_whitespace_threshold: Some(80),  trim_whitespace_padding_percent: Some(0.02f64), ..Default::default() }, vec![]);
    t("w=10&f.sharpen=80.5", Instructions { w: Some(10), f_sharpen: Some(80.5f64), ..Default::default() }, vec![]);

    t("f.sharpen=80.5", Instructions { f_sharpen: Some(80.5f64), ..Default::default() }, vec![]);
    t("f.sharpen_when=always", Instructions{ f_sharpen_when: Some(SharpenWhen::Always), ..Default::default()}, vec![]);
    t("f.sharpen_when=downscaling", Instructions{ f_sharpen_when: Some(SharpenWhen::Downscaling), ..Default::default()}, vec![]);
    t("f.sharpen_when=sizediffers", Instructions{ f_sharpen_when: Some(SharpenWhen::SizeDiffers), ..Default::default()}, vec![]);

    t("s.sepia=true&s.brightness=0.1&s.saturation=-0.1&s.contrast=1&s.alpha=0", Instructions { s_alpha: Some(0f64), s_contrast: Some(1f64), s_sepia: Some(true), s_brightness: Some(0.1f64), s_saturation: Some(-0.1f64), ..Default::default() }, vec![]);

    t("s.grayscale=true",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::True), ..Default::default()}, vec![]);
    t("s.grayscale=flat",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Flat), ..Default::default()}, vec![]);
    t("s.grayscale=ntsc",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Ntsc), ..Default::default()}, vec![]);
    t("s.grayscale=ry",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Ry), ..Default::default()}, vec![]);
    t("s.grayscale=Y",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Y), ..Default::default()}, vec![]);
    t("s.grayscale=Bt709",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Bt709), ..Default::default()}, vec![]);

    t("bgcolor=red", Instructions { bgcolor_srgb: Some(Color32(0xffff0000)), ..Default::default() }, vec![]);
    t("bgcolor=f00", Instructions { bgcolor_srgb: Some(Color32(0xffff0000)), ..Default::default() }, vec![]);
    t("bgcolor=f00f", Instructions { bgcolor_srgb: Some(Color32(0xffff0000)), ..Default::default() }, vec![]);
    t("bgcolor=ff0000", Instructions { bgcolor_srgb: Some(Color32(0xffff0000)), ..Default::default() }, vec![]);
    t("bgcolor=ff0000ff", Instructions { bgcolor_srgb: Some(Color32(0xffff0000)), ..Default::default() }, vec![]);

    t("bgcolor=darkseagreen", Instructions { bgcolor_srgb: Some(Color32(0xff8fbc8b)), ..Default::default() }, vec![]);
    t("bgcolor=8fbc8b", Instructions { bgcolor_srgb: Some(Color32(0xff8fbc8b)), ..Default::default() }, vec![]);
    t("bgcolor=8fbc8bff", Instructions { bgcolor_srgb: Some(Color32(0xff8fbc8b)), ..Default::default() }, vec![]);

    t("bgcolor=lightslategray", Instructions { bgcolor_srgb: Some(Color32(0xff778899)), ..Default::default() }, vec![]);
    t("bgcolor=789", Instructions { bgcolor_srgb: Some(Color32(0xff778899)), ..Default::default() }, vec![]);
    t("bgcolor=789f", Instructions { bgcolor_srgb: Some(Color32(0xff778899)), ..Default::default() }, vec![]);
    t("bgcolor=778899", Instructions { bgcolor_srgb: Some(Color32(0xff778899)), ..Default::default() }, vec![]);
    t("bgcolor=77889953", Instructions { bgcolor_srgb: Some(Color32(0x53778899)), ..Default::default() }, vec![]);

    t("bgcolor=white", Instructions { bgcolor_srgb: Some(Color32(0xffffffff)), ..Default::default() }, vec![]);
    t("bgcolor=fff", Instructions { bgcolor_srgb: Some(Color32(0xffffffff)), ..Default::default() }, vec![]);
    t("bgcolor=ffff", Instructions { bgcolor_srgb: Some(Color32(0xffffffff)), ..Default::default() }, vec![]);
    t("bgcolor=ffffff", Instructions { bgcolor_srgb: Some(Color32(0xffffffff)), ..Default::default() }, vec![]);
    t("bgcolor=ffffffff", Instructions { bgcolor_srgb: Some(Color32(0xffffffff)), ..Default::default() }, vec![]);

    t("crop=0,0,40,50", Instructions { crop: Some([0f64,0f64,40f64,50f64]), ..Default::default() }, vec![]);
    t("crop= 0, 0,40 ,  50", Instructions { crop: Some([0f64,0f64,40f64,50f64]), ..Default::default() }, vec![]);

    t("a.balancewhite=true",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Area), ..Default::default()}, vec![]);
    t("a.balancewhite=area",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Area), ..Default::default()}, vec![]);
    t("down.colorspace=linear",  Instructions{down_colorspace: Some(ScalingColorspace::Linear), ..Default::default()}, vec![]);
    t("down.colorspace=srgb",  Instructions{down_colorspace: Some(ScalingColorspace::Srgb), ..Default::default()}, vec![]);

    expect_warning("a.balancewhite","gimp",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Gimp), ..Default::default()});
    expect_warning("a.balancewhite","simple",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Simple), ..Default::default()});
    expect_warning("crop","(0,3,80, 90)",  Instructions { crop: Some([0f64,3f64,80f64,90f64]), ..Default::default() });
    expect_warning("crop","(0,3,happy, 90)",  Instructions { crop: Some([0f64,3f64,0f64,90f64]), ..Default::default() });
    expect_warning("crop","(  a0, 3, happy, 90)",  Instructions { crop: Some([0f64,3f64,0f64,90f64]), ..Default::default() });

}

#[test]
fn test_tostr(){
    fn t(expected_query: &str, from: Instructions){
        let b = from.to_string();
        debug_diff(&expected_query, &b.as_str());
        assert_eq!(&expected_query, &b.as_str());
    }
    t("h=300&mode=max&w=200", Instructions { w: Some(200), h: Some(300), mode: Some(FitMode::Max), ..Default::default() });
    t("h=300&mode=crop&w=200", Instructions { w: Some(200), h: Some(300), mode: Some(FitMode::Crop), ..Default::default() });
    t("format=jpeg", Instructions { format: Some(OutputFormat::Jpeg), ..Default::default() });
    t("format=gif", Instructions { format: Some(OutputFormat::Gif), ..Default::default() });
    t("format=png", Instructions { format: Some(OutputFormat::Png), ..Default::default() });
    t("scale=downscaleonly", Instructions {scale: Some(ScaleMode::DownscaleOnly), ..Default::default() });
    t("h=300&scale=upscalecanvas&w=20", Instructions { w: Some(20), h: Some(300), scale: Some(ScaleMode::UpscaleCanvas), ..Default::default() });
    t("flip=x&sflip=xy", Instructions { sflip: Some((true,true)), flip: Some((true,false)), ..Default::default() });
    t("flip=y", Instructions { sflip: Some((false,false)), flip: Some((false,true)), ..Default::default() });
    t("rotate=270&srotate=0", Instructions { srotate: Some(0), rotate: Some(270), ..Default::default() });
    t("autorotate=false&ignoreicc=true", Instructions { autorotate: Some(false), ignoreicc: Some(true) , ..Default::default() });
    t("mode=max", Instructions { mode: Some(FitMode::Max), ..Default::default() });
    t("mode=aspectcrop", Instructions { mode: Some(FitMode::AspectCrop), ..Default::default() });
    t("cropxunits=2.3&cropyunits=100", Instructions { cropxunits: Some(2.3f64), cropyunits: Some(100f64), ..Default::default() });
    t("quality=85", Instructions { quality: Some(85), ..Default::default() });
    t("zoom=0.02", Instructions { zoom: Some(0.02f64), ..Default::default() });
    t("trim.percentpadding=0.02&trim.threshold=80", Instructions { trim_whitespace_threshold: Some(80),  trim_whitespace_padding_percent: Some(0.02f64), ..Default::default() });
    t("bgcolor=ff0000ff", Instructions { bgcolor_srgb: Some(Color32(0xffff0000)), ..Default::default() });
    t("bgcolor=8fbc8bff", Instructions { bgcolor_srgb: Some(Color32(0xff8fbc8b)), ..Default::default() });
    t("bgcolor=77889953", Instructions { bgcolor_srgb: Some(Color32(0x53778899)), ..Default::default() });
    t("bgcolor=ffffffff", Instructions { bgcolor_srgb: Some(Color32(0xffffffff)), ..Default::default() });
    t("crop=0,0,40,50", Instructions { crop: Some([0f64,0f64,40f64,50f64]), ..Default::default() });
    t("a.balancewhite=area",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Area), ..Default::default()});
    t("webp.quality=85", Instructions { webp_quality: Some(85f64), ..Default::default() });
    t("webp.lossless=true", Instructions { webp_lossless: Some(true), ..Default::default() });
    t("down.colorspace=srgb",  Instructions{down_colorspace: Some(ScalingColorspace::Srgb), ..Default::default()});
    t("down.colorspace=linear",  Instructions{down_colorspace: Some(ScalingColorspace::Linear), ..Default::default()});
    t("f.sharpen=10", Instructions{ f_sharpen: Some(10f64), ..Default::default()});
    t("f.sharpen_when=always", Instructions{ f_sharpen_when: Some(SharpenWhen::Always), ..Default::default()});
    t("f.sharpen_when=downscaling", Instructions{ f_sharpen_when: Some(SharpenWhen::Downscaling), ..Default::default()});
    t("f.sharpen_when=sizediffers", Instructions{ f_sharpen_when: Some(SharpenWhen::SizeDiffers), ..Default::default()});
    t("s.grayscale=bt709",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Bt709), ..Default::default()});
    t("s.alpha=0&s.brightness=0.1&s.contrast=1&s.saturation=-0.1&s.sepia=true", Instructions { s_alpha: Some(0f64), s_contrast: Some(1f64), s_sepia: Some(true), s_brightness: Some(0.1f64), s_saturation: Some(-0.1f64), ..Default::default() });
    t("jpeg.progressive=true", Instructions { jpeg_progressive: Some(true), ..Default::default() });
    t("jpeg.turbo=true", Instructions { jpeg_turbo: Some(true), ..Default::default() });
    t("png.quality=90", Instructions { png_quality: Some(90), ..Default::default() });
    t("png.min_quality=90", Instructions { png_min_quality: Some(90), ..Default::default() });
    t("png.quantization_speed=4", Instructions { png_quantization_speed: Some(4), ..Default::default() });
    t("png.libpng=true", Instructions { png_libpng: Some(true), ..Default::default() });
    t("png.max_deflate=true", Instructions { png_max_deflate: Some(true), ..Default::default() });
}
