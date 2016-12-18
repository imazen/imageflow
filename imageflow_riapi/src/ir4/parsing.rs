use imageflow_helpers::preludes::from_std::*;
use ::std;
use ::url::Url;
use ::macro_attr;
use ::enum_derive;
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
pub enum FitMode {
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



pub static IR4_KEYS: [&'static str;57] = ["mode", "anchor", "flip", "sflip", "scale", "cache", "process",
    "quality", "zoom", "crop", "cropxunits", "cropyunits",
    "w", "h", "width", "height", "maxwidth", "maxheight", "format", "thumbnail",
     "autorotate", "srotate", "rotate", "ignoreicc", //really? : "precise_scaling_ratio",

    "frame", "page", "subsampling", "colors",
    "404", "bgcolor", "paddingcolor", "bordercolor", "preset", "floatspace", "jpeg_idct_downscale_linear", "watermark",
    "s.invert", "s.sepia", "s.grayscale", "s.alpha", "s.brightness", "s.contrast", "s.saturation", "trim.threshold",
    "trim.percentpadding", "a.blur", "a.sharpen", "a.removenoise", "dither",
    "encoder", "decoder", "builder", "s.roundcorners.", "paddingwidth", "paddingheight", "margin", "borderwidth"];



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

enum KeyResult{
    NotValid,
    NotSupported,
    Supported,
}

impl Instructions{
    pub fn delete_from_map(map: &mut HashMap<String,String>, warnings: Option<&mut Vec<ParseWarning>>) -> Instructions {
        let mut p = Parser { m: map, w: warnings, delete_used: true };
        let mut i = Instructions::new();
        i.w = p.parse_i32("width").or(p.parse_i32("w"));
        i.h = p.parse_i32("height").or(p.parse_i32("h"));
        i.legacy_max_height = p.parse_i32("maxheight");
        i.legacy_max_width = p.parse_i32("maxwidth");
        i.flip = p.parse_flip("flip").map(|v| v.clean());
        i.sflip = p.parse_flip("sflip").map(|v| v.clean());
        i.mode = p.parse_fit_mode("mode");
        i.scale = p.parse_scale("scale").map(|v| v.clean());
        i.format = p.parse_format("format").or(p.parse_format("thumbnail")).map(|v| v.clean());

        i
    }

    pub fn to_framewise(&self) -> s::Framewise{
        s::Framewise::example_graph()
    }
    pub fn new() -> Instructions{
        Default::default()
    }
}

struct Parser<'a>{
    m: &'a mut HashMap<String,String>,
    w: Option<&'a mut Vec<ParseWarning>>,
    delete_used: bool
}
impl<'a> Parser<'a>{


    fn parse<F,T,E>(&mut self, key: &'static str, f: F) -> Option<T>
            where F: Fn(&str) -> std::result::Result<T,E>{
        //Coalesce null and whitespace values to None
        let r = {
            let v = self.m.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());

            if let Some(s) = v {
                match f(s) {
                    Err(e) => {
                        if self.w.is_some() {
                            self.w.as_mut().unwrap().push(ParseWarning::ValueInvalid((key, s.to_owned())));
                        }
                        None
                    },
                    Ok(v) =>
                        Some(v)
                }
            } else {
                None
            }
        };
        if self.m.contains_key(key) && self.delete_used{
            self.m.remove(key);
        }
        r

    }


    fn parse_i32(&mut self, key: &'static str) -> Option<i32>{
        self.parse(key, |s| s.parse::<i32>() )
    }

    fn parse_fit_mode(&mut self, key: &'static str) -> Option<FitMode>{
        self.parse(key, |value| {
            for (k, v) in FitMode::iter_variant_names().zip(FitMode::iter_variants()) {
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




///// Anchor location. Convertible to System.Drawing.ContentAlignment by casting.
//[Flags]
//pub enum AnchorLocation {
///// Content is vertically aligned at the top, and horizontally aligned on the left.
//TopLeft = 1,
//
///// Content is vertically aligned at the top, and horizontally aligned at the center.
//TopCenter = 2,
//
///// Content is vertically aligned at the top, and horizontally aligned on the right.
//TopRight = 4,
//
///// Content is vertically aligned in the middle, and horizontally aligned onthe left.
//MiddleLeft = 16,
//
///// Content is vertically aligned in the middle, and horizontally aligned at the center.
//MiddleCenter = 32,
//
///// Content is vertically aligned in the middle, and horizontally aligned on  the right.
//MiddleRight = 64,
//
///// Content is vertically aligned at the bottom, and horizontally aligned on the left.
//BottomLeft = 256,
//
///// Content is vertically aligned at the bottom, and horizontally aligned at  the center.
//BottomCenter = 512,
//
///// Content is vertically aligned at the bottom, and horizontally aligned on the right.
//BottomRight = 1024,
//}
//

//
//
//
//[Obsolete("Obsolete. Use Mode=Crop to specify automatic cropping. Set CropTopLeft and CropTopRight to specify custom coordinates. Will be removed in V3.5 or V4.")]
//pub enum CropMode {
///// Default. No cropping - uses letterboxing if strecth=proportionally and both width and height are specified.
//None,
///// [Deprecated] Use Mode=Crop. Minimally crops to preserve aspect ratio if stretch=proportionally.
//[Obsolete("Use Mode=Crop instead.")]
//Auto,
///// Crops using the custom crop rectangle. Letterboxes if stretch=proportionally and both widht and height are specified.
//Custom
//}
//
//[Obsolete("Obsolete. Specify 0 for a crop unit to indicate source pixel coordinates.  Will be removed in V3.5 or V4.")]
//pub enum CropUnits {
///// Indicates the crop units are pixels on the original image.
//SourcePixels,
///// Indicates a custom range is being specified for the values. Base 0.
//Custom
//
//
//}
//
//
///// Modes of converting the image to Grayscale. GrayscaleMode.Y usually produces the best resuts
//pub enum GrayscaleMode {
//[EnumString("false")]
//None = 0,
///// The reccomended value. Y and NTSC are identical.
//[EnumString("true")]
//Y = 1,
//
//NTSC = 1,
//RY = 2,
//BT709= 3,
//
///// Red, green, and blue are averaged to get the grayscale image. Usually produces poor results compared to other algorithms.
//Flat = 4
//}
///// The Jpeg subsampling mode to use. Requires FreeImageEncoder, FreeImageBuilder, WicEncoder, or WicBuilder.
//pub enum JpegSubsamplingMode {
///// The encoder's default subsampling method will be used.
//Default = 0,
///// 411 Subsampling - Only supported by FreeImageBuilder and FreeImageEncoder. Poor quality.
//[EnumString("411",true)]
//Y4Cb1Cr1 = 4,
///// 420 Subsampling - Commonly used in H262 and H264. Low quality compared to 422 and 444.
//[EnumString("420",true)]
//Y4Cb2Cr0 = 8,
///// 422 Subsampling - Great balance of quality and file size, commonly used in high-end video formats.
//[EnumString("422",true)]
//Y4Cb2Cr2 = 16,
///// 444 subsampling - Highest quality, largest file size.
//[EnumString("444",true)]
//HighestQuality =32,
///// 444 subsampling - Highest quality, largest file size.
//[EnumString("444",true)]
//Y4Cb4Cr4 = 32
//}

