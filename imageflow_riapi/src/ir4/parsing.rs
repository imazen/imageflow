use super::srcset::apply_srcset_string;
use imageflow_helpers::colors::*;
use imageflow_helpers::preludes::from_std::fmt::Formatter;
use imageflow_helpers::preludes::from_std::*;
use imageflow_types as s;
use imageflow_types::json_messages::*;
use imageflow_types::BoolKeep;
use imageflow_types::Filter;
use imageflow_types::OutputImageFormat;
use imageflow_types::QualityProfile;
#[allow(unused)]
use option_filter::OptionFilterExt;
use std;
use std::num;
use std::result;
use url::Url;

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
    Webp,
    Avif,
    Jxl,
    Jpegxl,
    Auto,
    Keep,
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
IterVariants!(QualityProfileVariants), IterVariantNames!(QualityProfileNames))]
pub enum QualityProfileStrings {
        Lowest,
    Low,
        Med,
    Medium, //med
    Good,
    High,
    Highest,
    Lossless,
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


#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq,
IterVariants!(FilterVariants), IterVariantNames!(FilterNames))]
pub enum FilterStrings {
    Robidoux,
    Robidoux_Fast,
    RobidouxFast,
    Robidoux_Sharp,
    RobidouxSharp,
    Ginseng,
    GinsengSharp,
    Ginseng_Sharp,
    Lanczos,
    LanczosSharp,
    Lanczos_Sharp,
    Lanczos2,
    Lanczos_2,
    Lanczos2Sharp,
    Lanczos_2_Sharp,
    Cubic,
    CubicSharp,
    Cubic_Sharp,
    CatmullRom,
    Catmull_Rom,
    Mitchell,
    CubicBSpline,
    Cubic_B_Spline,
    Hermite,
    Jinc,
    Triangle,
    Linear,
    Box,
    Fastest,
    NCubic,
    N_Cubic,
    NCubicSharp,
    N_Cubic_Sharp,
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
#[rustfmt::skip]
pub static IR4_KEYS: [&str;100] = [
    "mode", "anchor", "flip", "sflip", "scale", "cache", "process",
    "quality", "jpeg.quality", "zoom", "crop", "cropxunits", "cropyunits",
    "w", "h", "width", "height", "maxwidth", "maxheight", "format", "thumbnail",
     "autorotate", "srotate", "rotate", "ignoreicc", "ignore_icc_errors", //really? : "precise_scaling_ratio",
    "stretch", "webp.lossless", "webp.quality", "watermark_red_dot",
    "frame", "page", "subsampling", "colors", "f.sharpen", "f.sharpen_when", "down.colorspace",
    "404", "bgcolor", "paddingcolor", "bordercolor", "preset", "floatspace",
    "jpeg_idct_downscale_linear", "watermark", "s.invert", "s.sepia", "s.grayscale", "s.alpha",
    "s.brightness", "s.contrast", "s.saturation",  "trim.threshold", "trim.percentpadding",
    "a.blur", "a.sharpen", "a.removenoise", "a.balancewhite", "dither","jpeg.progressive",
    "jpeg.turbo", "encoder", "decoder", "builder", "s.roundcorners", "paddingwidth",
    "paddingheight", "margin", "borderwidth", "decoder.min_precise_scaling_ratio",
    "png.quality","png.min_quality", "png.quantization_speed", "png.libpng", "png.max_deflate",
    "png.lossless", "up.filter", "down.filter", "dpr", "dppx", "up.colorspace", "srcset", "short","accept.webp",
    "accept.avif","accept.jxl", "accept.color_profiles", "c", "c.gravity", "qp", "qp.dpr", "qp.dppx",
    "avif.speed", "avif.quality", "jxl.effort", "jxl.distance", "jxl.quality", "jxl.lossless", "jpeg.li", "lossless"];

#[derive(PartialEq, Debug, Clone)]
pub enum ParseWarning {
    // We don't really support comma concatenation like ImageResizer (in theory) did
    DuplicateKey((String, String)),
    // Not an IR4
    KeyNotRecognized((String, String)),
    KeyNotSupported((String, String)),
    ValueInvalid((&'static str, String)),
}

impl ParseWarning {
    pub fn to_query_string_validation_issue(&self) -> QueryStringValidationIssue {
        match self {
            ParseWarning::DuplicateKey((k, v)) => QueryStringValidationIssue {
                message: format!("Duplicate key: {}", k),
                key: k.clone(),
                value: v.clone(),
                kind: QueryStringValidationIssueKind::DuplicateKeyError,
            },
            ParseWarning::KeyNotRecognized((k, v)) => QueryStringValidationIssue {
                message: format!("Key not recognized: {}", k),
                key: k.clone(),
                value: v.clone(),
                kind: QueryStringValidationIssueKind::UnrecognizedKey,
            },
            ParseWarning::KeyNotSupported((k, v)) => QueryStringValidationIssue {
                message: format!("Key not supported: {}", k),
                key: k.clone(),
                value: v.clone(),
                kind: QueryStringValidationIssueKind::IgnoredKey,
            },
            ParseWarning::ValueInvalid((k, v)) => QueryStringValidationIssue {
                message: format!("Value invalid: {}", v),
                key: k.to_string(),
                value: v.clone(),
                kind: QueryStringValidationIssueKind::InvalidValueError,
            },
        }
    }
}
pub fn parse_url(url: &Url) -> (Instructions, Vec<ParseWarning>) {
    let mut warnings = Vec::new();
    let mut map = HashMap::new();
    for (key, value) in url.query_pairs() {
        let k = key.to_lowercase(); //Trim whitespace?
        let v = value.into_owned();

        #[allow(clippy::map_entry)]
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

impl fmt::Display for Instructions {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.to_string_internal())
    }
}

pub(crate) fn iter_all_eq<T: PartialEq>(iter: impl IntoIterator<Item = T>) -> Option<T> {
    let mut iter = iter.into_iter();
    let first = iter.next()?;
    iter.all(|elem| elem == first).then_some(first)
}

impl Instructions {
    fn to_string_internal(self) -> String {
        let mut s = String::with_capacity(100);
        let mut vec = Vec::new();
        for (k, v) in self.to_map() {
            vec.push((k, v));
        }
        vec.sort_by_key(|&(a, _)| a);
        for (k, v) in vec {
            s.push_str(k);
            s.push('=');
            s.push_str(&v);
            s.push('&');
        }
        let len = s.len();
        if len > 0 {
            s.remove(len - 1);
        }
        s
    }

    #[allow(deprecated)]
    pub fn to_map(&self) -> HashMap<&'static str, String> {
        let mut m = HashMap::new();
        fn add<T>(m: &mut HashMap<&'static str, String>, key: &'static str, value: Option<T>)
        where
            T: fmt::Display,
        {
            if let Some(v) = value {
                m.insert(key, format!("{}", v));
            }
        }
        fn flip_str(f: Option<(bool, bool)>) -> Option<String> {
            match f {
                Some((true, true)) => Some("xy".to_owned()),
                Some((true, false)) => Some("x".to_owned()),
                Some((false, true)) => Some("y".to_owned()),
                _ => None,
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
        add(&mut m, "ignore_icc_errors", self.ignore_icc_errors);
        add(&mut m, "quality", self.quality);

        add(&mut m, "webp.quality", self.webp_quality);
        add(&mut m, "webp.lossless", self.webp_lossless);
        //.webp method (speed)
        add(&mut m, "jpeg.progressive", self.jpeg_progressive);
        add(&mut m, "jpeg.turbo", self.jpeg_turbo);
        add(&mut m, "jpeg.quality", self.jpeg_quality);
        add(&mut m, "jpeg.li", self.jpeg_li);
        add(&mut m, "png.quality", self.png_quality);
        add(&mut m, "png.min_quality", self.png_min_quality);
        add(&mut m, "png.quantization_speed", self.png_quantization_speed);
        add(&mut m, "png.libpng", self.png_libpng);
        add(&mut m, "png.max_deflate", self.png_max_deflate);
        add(&mut m, "png.lossless", self.png_lossless);
        add(&mut m, "avif.quality", self.avif_quality);
        add(&mut m, "avif.speed", self.avif_speed);
        add(&mut m, "jxl.effort", self.jxl_effort);
        add(&mut m, "jxl.distance", self.jxl_distance);
        add(&mut m, "jxl.quality", self.jxl_quality);
        add(&mut m, "jxl.lossless", self.jxl_lossless);

        add(&mut m, "zoom", self.zoom); // TODO: ImageResizer4 uses zoom, not dpr, but it feels outdated

        add(&mut m, "s.contrast", self.s_contrast);

        add(&mut m, "s.alpha", self.s_alpha);
        add(&mut m, "s.brightness", self.s_brightness);
        add(&mut m, "s.saturation", self.s_saturation);
        add(&mut m, "s.sepia", self.s_sepia);
        add(&mut m, "s.grayscale", self.s_grayscale.map(|v| format!("{:?}", v).to_lowercase()));
        add(
            &mut m,
            "a.balancewhite",
            self.a_balance_white.map(|v| format!("{:?}", v).to_lowercase()),
        );
        add(&mut m, "subsampling", self.jpeg_subsampling);
        add(&mut m, "bgcolor", self.bgcolor_srgb.map(|v| v.to_rrggbbaa_string().to_lowercase()));
        add(&mut m, "f.sharpen", self.f_sharpen);
        add(
            &mut m,
            "f.sharpen_when",
            self.f_sharpen_when.map(|v| format!("{:?}", v).to_lowercase()),
        );
        add(&mut m, "trim.percentpadding", self.trim_whitespace_padding_percent);
        add(&mut m, "trim.threshold", self.trim_whitespace_threshold);

        add(
            &mut m,
            "s.roundcorners",
            self.s_round_corners.map(|a| {
                if let Some(v) = iter_all_eq(a.iter()) {
                    format!("{}", v)
                } else {
                    format!("{},{},{},{}", a[0], a[1], a[2], a[3])
                }
            }),
        );
        if self.cropxunits == Some(100.0) && self.cropyunits == Some(100.0) {
            add(&mut m, "c", self.crop.map(|a| format!("{},{},{},{}", a[0], a[1], a[2], a[3])));
        } else {
            add(&mut m, "cropxunits", self.cropxunits);
            add(&mut m, "cropyunits", self.cropyunits);
            add(&mut m, "crop", self.crop.map(|a| format!("{},{},{},{}", a[0], a[1], a[2], a[3])));
        }

        add(&mut m, "anchor", self.anchor_string());
        add(&mut m, "c.gravity", self.gravity_string());
        add(&mut m, "qp.dpr", self.qp_dpr);
        add(&mut m, "qp", self.qp);

        add(
            &mut m,
            "down.colorspace",
            self.down_colorspace.map(|v| format!("{:?}", v).to_lowercase()),
        );
        add(&mut m, "up.colorspace", self.up_colorspace.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "down.filter", self.down_filter.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "up.filter", self.up_filter.map(|v| format!("{:?}", v).to_lowercase()));
        add(&mut m, "decoder.min_precise_scaling_ratio", self.min_precise_scaling_ratio);

        add(&mut m, "watermark_red_dot", self.watermark_red_dot);
        add(&mut m, "accept.webp", self.accept_webp);
        add(&mut m, "lossless", self.lossless);
        add(&mut m, "accept.avif", self.accept_avif);
        add(&mut m, "accept.jxl", self.accept_jxl);
        add(&mut m, "accept.color_profiles", self.accept_color_profiles);
        add(&mut m, "frame", self.frame);
        m
    }

    #[allow(deprecated)]
    pub fn delete_from_map(
        map: &mut HashMap<String, String>,
        warnings: Option<&mut Vec<ParseWarning>>,
    ) -> Instructions {
        let mut p = Parser { m: map, w: warnings, delete_supported: true };
        let mut i = Instructions::new();

        //Size and multipliers
        i.w = p.parse_i32("width").or_else(|| p.parse_i32("w"));
        i.h = p.parse_i32("height").or_else(|| p.parse_i32("h"));
        i.zoom = p.parse_dpr("zoom").or_else(|| p.parse_dpr("dpr")).or_else(|| p.parse_dpr("dppx"));

        i.legacy_max_height = p.parse_i32("maxheight");
        i.legacy_max_width = p.parse_i32("maxwidth");

        //flip-rotate
        i.flip = p.parse_flip("flip").map(|v| v.clean());
        i.sflip = p.parse_flip("sflip").or_else(|| p.parse_flip("sourceFlip")).map(|v| v.clean());
        i.srotate = p.parse_rotate("srotate");
        i.rotate = p.parse_rotate("rotate");
        i.autorotate = p.parse_bool("autorotate");

        // fit mode and scale
        let mode_string = p.parse_fit_mode("mode");
        if mode_string == Some(FitModeStrings::Carve) {
            p.warn(ParseWarning::ValueInvalid(("mode", "carve".to_owned())).to_owned());
        }
        // Side effects wanted for or()
        i.mode = mode_string
            .and_then(|v| v.clean())
            .or(p.parse_test_pair("stretch", "fill").and_then(|b| {
                if b {
                    Some(FitMode::Stretch)
                } else {
                    None
                }
            }))
            .or(p.parse_test_pair("crop", "auto").and_then(|b| {
                if b {
                    Some(FitMode::Crop)
                } else {
                    None
                }
            }));

        i.scale = p.parse_scale("scale").map(|v| v.clean());

        // icc profiles and resizing color space
        i.ignoreicc = p.parse_bool("ignoreicc");
        i.ignore_icc_errors = p.parse_bool("ignore_icc_errors");
        i.down_colorspace = p.parse_colorspace("down.colorspace");
        i.up_colorspace = p.parse_colorspace("up.colorspace");

        // Whitespace trimming
        //TODO: warn bounds (-1..1, 0..255)
        i.trim_whitespace_padding_percent = p.parse_f32("trim.percentpadding");
        i.trim_whitespace_threshold = p.parse_i32("trim.threshold");

        // parse c as crop, set cropxunits/yunits to 100
        if let Some(c) = p.parse_crop_strict("c") {
            i.cropxunits = Some(100.0);
            i.cropyunits = Some(100.0);
            i.crop = Some(c);
        } else {
            // legacy crop
            i.crop = p.parse_crop_strict("crop").or_else(|| p.parse_crop("crop"));
            i.cropxunits = p.parse_f64("cropxunits");
            i.cropyunits = p.parse_f64("cropyunits");
        }
        // crop gravity
        i.c_gravity = p.parse_gravity("c.gravity");
        // anchor for either crop or pad
        i.anchor = p.parse_anchor("anchor");

        // Effects
        i.s_round_corners = p.parse_round_corners("s.roundcorners");
        i.s_grayscale = p.parse_grayscale("s.grayscale");
        i.s_contrast = p.parse_f32("s.contrast");
        i.s_alpha = p.parse_f32("s.alpha");
        i.s_saturation = p.parse_f32("s.saturation");
        i.s_brightness = p.parse_f32("s.brightness");
        i.s_sepia = p.parse_bool("s.sepia");
        i.a_balance_white = match p.parse_white_balance("a.balancewhite") {
            Some(HistogramThresholdAlgorithm::True) | Some(HistogramThresholdAlgorithm::Area) => {
                Some(HistogramThresholdAlgorithm::Area)
            }
            None => None,
            Some(other) => {
                p.warn(ParseWarning::ValueInvalid((
                    "a.balancewhite",
                    format!("{:?}", other).to_lowercase(),
                )));
                Some(other)
            }
        };

        // resizing filter and sharpening
        i.f_sharpen = p.parse_f32("f.sharpen");
        i.f_sharpen_when = p.parse_sharpen_when("f.sharpen_when");
        i.down_filter = p.parse_filter("down.filter");
        i.up_filter = p.parse_filter("up.filter");
        i.min_precise_scaling_ratio = p.parse_f32("decoder.min_precise_scaling_ratio");
        let _ = p.parse_test_pair("fastscale", "true");

        // Removing alpha with a matte
        i.bgcolor_srgb = p.parse_color_srgb("bgcolor");

        // Format-specific tuning
        i.jpeg_quality = p.parse_i32("jpeg.quality");
        // ignore deprecation warning

        i.jpeg_subsampling = p.parse_subsampling("subsampling");
        i.jpeg_progressive = p.parse_bool("jpeg.progressive");
        i.jpeg_turbo = p.parse_bool("jpeg.turbo");
        i.jpeg_li = p.parse_bool("jpeg.li");
        i.webp_quality = p.parse_f32("webp.quality");
        i.webp_lossless = p.parse_bool_keep("webp.lossless");

        i.png_lossless = p.parse_bool_keep("png.lossless");
        i.png_min_quality = p.parse_u8("png.min_quality");
        i.png_quality = p.parse_u8("png.quality");
        i.png_quantization_speed = p.parse_u8("png.quantization_speed");
        i.png_libpng = p.parse_bool("png.libpng");
        i.png_max_deflate = p.parse_bool("png.max_deflate");
        i.avif_quality = p.parse_f32("avif.quality");
        i.avif_speed = p.parse_u8("avif.speed");
        i.jxl_quality = p.parse_f32("jxl.quality");
        i.jxl_effort = p.parse_u8("jxl.effort");
        i.jxl_distance = p.parse_f32("jxl.distance");
        i.jxl_lossless = p.parse_bool_keep("jxl.lossless");

        // Format selection
        i.accept_jxl = p.parse_bool("accept.jxl"); //Used for format=auto/lossless/lossy
        i.accept_webp = p.parse_bool("accept.webp"); //Used for format=auto/lossless/lossy
        i.accept_avif = p.parse_bool("accept.avif"); //Used for format=auto/lossless/lossy
        i.accept_color_profiles = p.parse_bool("accept.color_profiles");
        i.format = p.parse_format("format").or_else(|| p.parse_format("thumbnail"));
        i.lossless = p.parse_bool_keep("lossless");

        // This generic quality value was originally for Windows Jpeg encoder values
        // It now maps poorly to mozjpeg/lodepng/optipng/webp/etc
        i.quality = p.parse_i32("quality");

        // qp is its replacement
        i.qp = p.parse_quality_profile("qp", i.quality);
        i.qp_dpr = p.parse_qp_dpr("qp.dpr", i.zoom).or(p.parse_qp_dpr("qp.dppx", i.zoom));

        i.watermark_red_dot = p.parse_bool("watermark_red_dot");

        i.frame = p.parse_i32("frame").or_else(|| p.parse_i32("page"));

        p.apply_srcset(&mut i);

        i
    }

    fn anchor1d_numeric_string(a: Anchor1D) -> String {
        match a {
            Anchor1D::Near => "0".to_owned(),
            Anchor1D::Center => "50".to_owned(),
            Anchor1D::Far => "100".to_owned(),
            Anchor1D::Percent(v) => format!("{:.2}", v),
        }
    }
    fn anchor_string(&self) -> Option<String> {
        if let Some((h, v)) = self.anchor {
            match (h, v) {
                (Anchor1D::Percent(_), _) | (_, Anchor1D::Percent(_)) => {
                    return Some(format!(
                        "{},{}",
                        Self::anchor1d_numeric_string(h),
                        Self::anchor1d_numeric_string(v)
                    ))
                }
                _ => {}
            }

            let first = match v {
                Anchor1D::Near => "top",
                Anchor1D::Center => "middle",
                Anchor1D::Far => "bottom",
                Anchor1D::Percent(_) => unimplemented!(),
            };
            let last = match h {
                Anchor1D::Near => "left",
                Anchor1D::Center => "center",
                Anchor1D::Far => "right",
                Anchor1D::Percent(_) => unimplemented!(),
            };
            Some(format!("{}{}", first, last))
        } else {
            None
        }
    }
    fn gravity_string(&self) -> Option<String> {
        if let Some([x, y]) = self.c_gravity {
            Some(format!("{:.2},{:.2}", x, y))
        } else {
            None
        }
    }

    pub fn to_framewise(&self) -> s::Framewise {
        s::Framewise::example_graph()
    }
    pub fn new() -> Instructions {
        Default::default()
    }
}

//
struct Parser<'a> {
    m: &'a mut HashMap<String, String>,
    w: Option<&'a mut Vec<ParseWarning>>,
    /// We leave pairs in the map if we do not support them (or we support them, but they are invalid)
    delete_supported: bool,
}
impl<'a> Parser<'a> {
    fn remove(&mut self, key: &str) -> Option<String> {
        self.m.remove(key).map(|v| v.trim().to_owned())
    }

    fn apply_srcset(&mut self, i: &mut Instructions) {
        if let Some(srcset) = self.remove("srcset").or_else(|| self.remove("short")) {
            if let Some(w) = &mut self.w {
                apply_srcset_string(i, &srcset, w);
            } else {
                let mut w = Vec::new();
                apply_srcset_string(i, &srcset, &mut w);
            }
        }
    }

    fn warn(&mut self, warning: ParseWarning) {
        if let Some(w) = &mut self.w {
            w.push(warning);
        }
    }
    fn warning_parse<F, T, E>(&mut self, key: &'static str, f: F) -> Option<T>
    where
        F: Fn(&str) -> result::Result<(T, Option<ParseWarning>, bool), E>,
    {
        //Coalesce null and whitespace values to None
        let (r, supported) = {
            let v = self.m.get(key).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());

            if let Some(s) = v {
                match f(&s) {
                    Err(_) => {
                        self.warn(ParseWarning::ValueInvalid((key, s.to_owned())));
                        (None, false) // We assume an error means the value wasn't supported
                    }
                    Ok((v, w, supported)) => {
                        if let Some(w) = w {
                            self.warn(w);
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
    fn parse<F, T, E>(&mut self, key: &'static str, f: F) -> Option<T>
    where
        F: Fn(&str) -> result::Result<T, E>,
    {
        self.warning_parse(key, |s| f(s).map(|v| (v, None, true)))
    }

    fn parse_test_pair(&mut self, key: &'static str, value: &'static str) -> Option<bool> {
        self.warning_parse(key, |s| -> result::Result<(bool, Option<ParseWarning>, bool), ()> {
            if s.eq_ignore_ascii_case(value) {
                Ok((true, None, true))
            } else {
                Ok((false, None, false))
            }
        })
    }

    fn parse_crop_strict(&mut self, key: &'static str) -> Option<[f64; 4]> {
        self.warning_parse(key, |s| {
            let values = s
                .split(',')
                .map(|v| v.trim().parse::<f64>())
                .collect::<Vec<std::result::Result<f64, num::ParseFloatError>>>();
            if let Some(Err(e)) = values.iter().find(|v| v.is_err()) {
                Err(ParseCropError::InvalidNumber(e.clone()))
            } else if values.len() != 4 {
                Err(ParseCropError::InvalidNumberOfValues(
                    "Crops must contain exactly 4 values, separated by commas",
                ))
            } else {
                Ok((
                    [
                        *values[0].as_ref().unwrap(),
                        *values[1].as_ref().unwrap(),
                        *values[2].as_ref().unwrap(),
                        *values[3].as_ref().unwrap(),
                    ],
                    None,
                    true,
                ))
            }
        })
    }

    fn parse_crop(&mut self, key: &'static str) -> Option<[f64; 4]> {
        self.warning_parse(key, |s| {
            // TODO: We're also supposed to trim leading/trailing commas along with whitespace
            let str = s.replace("(", "").replace(")", "");
            // .unwrap_or(0) is ugly, but it's what IR4 does. :(
            let values = str
                .trim()
                .split(',')
                .map(|v| v.trim().parse::<f64>().unwrap_or(0f64))
                .collect::<Vec<f64>>();
            if values.len() == 4 {
                Ok(([values[0], values[1], values[2], values[3]], None, true))
            } else {
                Err(())
            }
        })
    }

    fn parse_round_corners(&mut self, key: &'static str) -> Option<[f64; 4]> {
        self.warning_parse(key, |s| {
            let values = s
                .split(',')
                .map(|v| v.trim().parse::<f64>())
                .collect::<Vec<std::result::Result<f64, num::ParseFloatError>>>();
            if let Some(Err(e)) = values.iter().find(|v| v.is_err()) {
                Err(ParseRoundCornersError::InvalidNumber(e.clone()))
            } else if values.len() == 4 {
                Ok((
                    [
                        *values[0].as_ref().unwrap(),
                        *values[1].as_ref().unwrap(),
                        *values[2].as_ref().unwrap(),
                        *values[3].as_ref().unwrap(),
                    ],
                    None,
                    true,
                ))
            } else if values.len() == 1 {
                let v = *values[0].as_ref().unwrap();
                Ok(([v, v, v, v], None, true))
            } else {
                Err(ParseRoundCornersError::InvalidNumberOfValues(
                    "s.roundcorners must contain exactly 1 value or 4 values, separated by commas",
                ))
            }
        })
    }

    fn parse_bool(&mut self, key: &'static str) -> Option<bool> {
        self.parse(key, |s| match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(()),
        })
    }
    fn parse_bool_keep(&mut self, key: &'static str) -> Option<BoolKeep> {
        self.parse(key, |s| match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(BoolKeep::True),
            "false" | "0" | "no" | "off" => Ok(BoolKeep::False),
            "keep" => Ok(BoolKeep::Keep),
            "preserve" => Ok(BoolKeep::Keep),
            _ => Err(()),
        })
    }
    fn parse_u8(&mut self, key: &'static str) -> Option<u8> {
        self.parse(key, |s| s.parse::<u8>())
    }
    fn parse_i32(&mut self, key: &'static str) -> Option<i32> {
        self.parse(key, |s| s.parse::<i32>())
    }
    fn parse_f64(&mut self, key: &'static str) -> Option<f64> {
        self.parse(key, |s| s.parse::<f64>())
    }
    fn parse_f32(&mut self, key: &'static str) -> Option<f32> {
        self.parse(key, |s| s.parse::<f32>())
    }
    fn parse_dpr(&mut self, key: &'static str) -> Option<f32> {
        self.parse(key, |s| s.trim_end_matches("x").parse::<f32>())
    }
    fn parse_qp_dpr(&mut self, key: &'static str, dpr: Option<f32>) -> Option<f32> {
        self.parse(key, |s| {
            if s.eq_ignore_ascii_case("dpr")
                || s.eq_ignore_ascii_case("dppx")
                || s.eq_ignore_ascii_case("zoom")
            {
                if let Some(zoom) = dpr {
                    Ok(Some(zoom))
                } else {
                    Ok(None)
                }
            } else {
                s.trim_end_matches("x").parse::<f32>().map(Some)
            }
        })?
    }

    fn parse_gravity(&mut self, key: &'static str) -> Option<[f64; 2]> {
        self.warning_parse(key, |s| {
            match Self::parse_f64_list::<2>(
                s,
                "c.gravity must contain exactly 2 decimal values, 0..100.0, separated by commas",
            ) {
                Ok(v) => Ok((v, None, true)),
                Err(e) => Err(e),
            }
        })
    }

    fn parse_quality_profile(
        &mut self,
        key: &'static str,
        quality: Option<i32>,
    ) -> Option<QualityProfile> {
        self.parse(key, |value| {
            // Copy the quality value to the qp field if it's set.
            if value.eq_ignore_ascii_case("quality") {
                return Ok(quality.map(|v| QualityProfile::Percent(v as f32)));
            }
            match QualityProfile::parse(value) {
                Some(v) => Ok(Some(v)),
                None => Err(QualityProfile::HELP_TEXT),
            }
        })?
    }

    fn parse_f64_list<const N: usize>(
        text: &str,
        wrong_count_message: &'static str,
    ) -> Result<[f64; N], ParseListError> {
        let mut array = [0f64; N];
        let mut i = 0;
        for s in text.split(',') {
            match s.trim().parse::<f64>() {
                Ok(v) => {
                    if i < N {
                        array[i] = v;
                    }
                }
                Err(e) => {
                    return Err(ParseListError::InvalidNumber((e.clone(), s.trim().to_owned())))
                }
            }
            i += 1;
        }
        if i != N {
            Err(ParseListError::InvalidNumberOfValues(wrong_count_message))
        } else {
            Ok(array)
        }
    }

    fn parse_subsampling(&mut self, key: &'static str) -> Option<i32> {
        self.parse(key, |s| {
            s.parse::<i32>().map_err(|_| ()).and_then(|v| match v {
                411 | 420 | 444 | 422 => Ok(v),
                _ => Err(()),
            })
        })
    }

    fn parse_rotate(&mut self, key: &'static str) -> Option<i32> {
        self.warning_parse(key, |s| match s.parse::<f32>() {
            Ok(value) => {
                let result = ((((value / 90f32).round() % 4f32) as i32 + 4) % 4) * 90;
                if value % 90f32 > 0.1f32 {
                    Ok((result, Some(ParseWarning::ValueInvalid((key, s.to_owned()))), false))
                } else {
                    Ok((result, None, true))
                }
            }
            Err(e) => Err(e),
        })
    }

    fn parse_colorspace(&mut self, key: &'static str) -> Option<ScalingColorspace> {
        self.parse(key, |value| {
            for (k, v) in
                ScalingColorspace::iter_variant_names().zip(ScalingColorspace::iter_variants())
            {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }

    fn parse_fit_mode(&mut self, key: &'static str) -> Option<FitModeStrings> {
        self.parse(key, |value| {
            for (k, v) in FitModeStrings::iter_variant_names().zip(FitModeStrings::iter_variants())
            {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }
    fn parse_filter(&mut self, key: &'static str) -> Option<FilterStrings> {
        self.parse(key, |value| {
            for (k, v) in FilterStrings::iter_variant_names().zip(FilterStrings::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }

    fn parse_sharpen_when(&mut self, key: &'static str) -> Option<SharpenWhen> {
        self.parse(key, |value| {
            for (k, v) in SharpenWhen::iter_variant_names().zip(SharpenWhen::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }

    fn parse_white_balance(&mut self, key: &'static str) -> Option<HistogramThresholdAlgorithm> {
        self.parse(key, |value| {
            for (k, v) in HistogramThresholdAlgorithm::iter_variant_names()
                .zip(HistogramThresholdAlgorithm::iter_variants())
            {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }

    fn parse_grayscale(&mut self, key: &'static str) -> Option<GrayscaleAlgorithm> {
        self.parse(key, |value| {
            for (k, v) in
                GrayscaleAlgorithm::iter_variant_names().zip(GrayscaleAlgorithm::iter_variants())
            {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }

    fn parse_scale(&mut self, key: &'static str) -> Option<ScaleModeStrings> {
        self.parse(key, |value| {
            for (k, v) in
                ScaleModeStrings::iter_variant_names().zip(ScaleModeStrings::iter_variants())
            {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }

    fn parse_flip(&mut self, key: &'static str) -> Option<FlipStrings> {
        self.parse(key, |value| {
            for (k, v) in FlipStrings::iter_variant_names().zip(FlipStrings::iter_variants()) {
                if k.eq_ignore_ascii_case(value) {
                    return Ok(v);
                }
            }
            Err(())
        })
    }
    fn parse_format(&mut self, key: &'static str) -> Option<OutputFormat> {
        self.parse(key, OutputFormat::from_str)
    }

    fn parse_color_srgb(&mut self, key: &'static str) -> Option<Color32> {
        self.parse(key, parse_color_hex_or_named)
    }

    fn parse_anchor(&mut self, key: &'static str) -> Option<(Anchor1D, Anchor1D)> {
        self.parse(key, |value| match value.to_lowercase().as_str() {
            "topleft" => Ok((Anchor1D::Near, Anchor1D::Near)),
            "topcenter" => Ok((Anchor1D::Center, Anchor1D::Near)),
            "topright" => Ok((Anchor1D::Far, Anchor1D::Near)),
            "middleleft" => Ok((Anchor1D::Near, Anchor1D::Center)),
            "middlecenter" => Ok((Anchor1D::Center, Anchor1D::Center)),
            "middleright" => Ok((Anchor1D::Far, Anchor1D::Center)),
            "bottomleft" => Ok((Anchor1D::Near, Anchor1D::Far)),
            "bottomcenter" => Ok((Anchor1D::Center, Anchor1D::Far)),
            "bottomright" => Ok((Anchor1D::Far, Anchor1D::Far)),
            other => {
                let gravity = Self::parse_f64_list::<2>(
                    other,
                    "Anchor must be a string or two numbers, separated by commas",
                );
                if let Ok([x, y]) = gravity {
                    Ok((Anchor1D::Percent(x as f32), Anchor1D::Percent(y as f32)))
                } else {
                    Err(())
                }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ParseCropError {
    InvalidNumber(std::num::ParseFloatError),
    InvalidNumberOfValues(&'static str),
}
#[derive(Debug, Clone, PartialEq)]
enum ParseListError {
    InvalidNumber((std::num::ParseFloatError, String)),
    InvalidNumberOfValues(&'static str),
}
#[derive(Debug, Clone, PartialEq)]
enum ParseRoundCornersError {
    InvalidNumber(std::num::ParseFloatError),
    InvalidNumberOfValues(&'static str),
}
impl QualityProfileStrings {
    pub fn clean(&self) -> QualityProfile {
        match *self {
            QualityProfileStrings::Lowest => QualityProfile::Lowest,
            QualityProfileStrings::Low => QualityProfile::Low,
            QualityProfileStrings::Medium => QualityProfile::Medium,
            QualityProfileStrings::Med => QualityProfile::Medium,
            QualityProfileStrings::Good => QualityProfile::Good,
            QualityProfileStrings::High => QualityProfile::High,
            QualityProfileStrings::Highest => QualityProfile::Highest,
            QualityProfileStrings::Lossless => QualityProfile::Lossless,
        }
    }
}
impl OutputFormatStrings {
    pub fn clean(&self) -> OutputFormat {
        match *self {
            OutputFormatStrings::Png => OutputFormat::Png,
            OutputFormatStrings::Gif => OutputFormat::Gif,
            OutputFormatStrings::Webp => OutputFormat::Webp,
            OutputFormatStrings::Auto => OutputFormat::Auto,
            OutputFormatStrings::Jpg => OutputFormat::Jpeg,
            OutputFormatStrings::Jpe => OutputFormat::Jpeg,
            OutputFormatStrings::Jif => OutputFormat::Jpeg,
            OutputFormatStrings::Jfif => OutputFormat::Jpeg,
            OutputFormatStrings::Jfi => OutputFormat::Jpeg,
            OutputFormatStrings::Exif => OutputFormat::Jpeg,
            OutputFormatStrings::Jpeg => OutputFormat::Jpeg,
            OutputFormatStrings::Avif => OutputFormat::Avif,
            OutputFormatStrings::Jxl => OutputFormat::Jxl,
            OutputFormatStrings::Jpegxl => OutputFormat::Jxl,
            OutputFormatStrings::Keep => OutputFormat::Keep,
        }
    }
}

impl FlipStrings {
    pub fn clean(&self) -> (bool, bool) {
        match *self {
            FlipStrings::None => (false, false),
            FlipStrings::X | FlipStrings::H => (true, false),
            FlipStrings::Y | FlipStrings::V => (false, true),
            FlipStrings::XY | FlipStrings::Both => (true, true),
        }
    }
}
impl FitModeStrings {
    pub fn clean(&self) -> Option<FitMode> {
        match *self {
            FitModeStrings::None => None,
            FitModeStrings::Max => Some(FitMode::Max),
            FitModeStrings::Pad => Some(FitMode::Pad),
            FitModeStrings::Crop => Some(FitMode::Crop),
            FitModeStrings::Carve | FitModeStrings::Stretch => Some(FitMode::Stretch),
            FitModeStrings::AspectCrop => Some(FitMode::AspectCrop),
        }
    }
}

impl FilterStrings {
    pub fn to_filter(&self) -> Filter {
        match *self {
            FilterStrings::Robidoux => Filter::Robidoux,
            FilterStrings::Robidoux_Sharp | FilterStrings::RobidouxSharp => Filter::RobidouxSharp,
            FilterStrings::Robidoux_Fast | FilterStrings::RobidouxFast => Filter::RobidouxFast,
            FilterStrings::Ginseng => Filter::Ginseng,
            FilterStrings::Ginseng_Sharp | FilterStrings::GinsengSharp => Filter::GinsengSharp,
            FilterStrings::Lanczos => Filter::Lanczos,
            FilterStrings::Lanczos_Sharp | FilterStrings::LanczosSharp => Filter::LanczosSharp,
            FilterStrings::Lanczos_2 | FilterStrings::Lanczos2 => Filter::Lanczos2,
            FilterStrings::Lanczos_2_Sharp | FilterStrings::Lanczos2Sharp => Filter::Lanczos2Sharp,
            FilterStrings::Cubic => Filter::Cubic,
            FilterStrings::Cubic_Sharp | FilterStrings::CubicSharp => Filter::CubicSharp,
            FilterStrings::Catmull_Rom | FilterStrings::CatmullRom => Filter::CatmullRom,
            FilterStrings::Mitchell => Filter::Mitchell,
            FilterStrings::Cubic_B_Spline | FilterStrings::CubicBSpline => Filter::CubicBSpline,
            FilterStrings::Hermite => Filter::Hermite,
            FilterStrings::Jinc => Filter::Jinc,
            FilterStrings::Triangle => Filter::Triangle,
            FilterStrings::Linear => Filter::Linear,
            FilterStrings::Box => Filter::Box,
            FilterStrings::Fastest => Filter::Fastest,
            FilterStrings::N_Cubic | FilterStrings::NCubic => Filter::NCubic,
            FilterStrings::N_Cubic_Sharp | FilterStrings::NCubicSharp => Filter::NCubicSharp,
        }
    }
}

impl ScaleModeStrings {
    pub fn clean(&self) -> ScaleMode {
        match *self {
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

//TODO, consider using f32 instead of f64 (26x) to halve the struct weight
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Instructions {
    pub w: Option<i32>,
    pub h: Option<i32>,
    pub legacy_max_width: Option<i32>,
    pub legacy_max_height: Option<i32>,
    pub mode: Option<FitMode>,
    pub scale: Option<ScaleMode>,
    pub flip: Option<(bool, bool)>,
    pub sflip: Option<(bool, bool)>,
    pub srotate: Option<i32>,
    pub rotate: Option<i32>,
    pub autorotate: Option<bool>,

    pub anchor: Option<(Anchor1D, Anchor1D)>,
    pub c_gravity: Option<[f64; 2]>,
    pub crop: Option<[f64; 4]>,
    pub s_round_corners: Option<[f64; 4]>,
    pub cropxunits: Option<f64>,
    pub cropyunits: Option<f64>,
    pub zoom: Option<f32>,

    pub trim_whitespace_threshold: Option<i32>,
    pub trim_whitespace_padding_percent: Option<f32>,
    pub a_balance_white: Option<HistogramThresholdAlgorithm>,
    pub s_alpha: Option<f32>,
    pub s_contrast: Option<f32>,
    pub s_saturation: Option<f32>,
    pub s_brightness: Option<f32>,
    pub s_sepia: Option<bool>,
    pub s_grayscale: Option<GrayscaleAlgorithm>,
    pub min_precise_scaling_ratio: Option<f32>,
    pub down_colorspace: Option<ScalingColorspace>,
    pub up_colorspace: Option<ScalingColorspace>,
    pub f_sharpen: Option<f32>,
    pub f_sharpen_when: Option<SharpenWhen>,
    pub up_filter: Option<FilterStrings>,
    pub down_filter: Option<FilterStrings>,
    pub watermark_red_dot: Option<bool>,

    pub bgcolor_srgb: Option<Color32>,

    /// Default=keep. When format=auto, the best enabled codec for the image/constraints is selected.
    pub format: Option<OutputFormat>,
    /// Affects format selection when format=auto.
    /// Uses lossless mode if the format supports it.
    /// Animation is considered more important than lossless when selecting the format.
    /// Maybe: Default=false, for jxl/webp, true for png. keep matches the source image.
    pub lossless: Option<BoolKeep>,

    // When format=auto, these enable codecs that are not classic web-safe
    pub accept_webp: Option<bool>,
    pub accept_avif: Option<bool>,
    pub accept_jxl: Option<bool>,

    // Allow custom color profiles (no guarantee we will produce them)
    pub accept_color_profiles: Option<bool>,
    /// Ignores color profiles (sometimes for some decoders)
    pub ignoreicc: Option<bool>,
    /// Ignores color profile errors (sometimes for some decoders)
    pub ignore_icc_errors: Option<bool>,

    /// Applies a quality profile, and (if format=auto or blank, chooses the best codec)
    pub qp: Option<QualityProfile>,
    /// Adjusts the quality profile, assuming a 150ppi display and 3x CSS pixel ratio. 3 is the default.
    /// lower values will increase quality, higher values will decrease quality.
    /// Useful when not using srcset/picture, just img src. Ex. <img width=400 src="img.jpg?srcset=qp-dpr-2,800w" />
    pub qp_dpr: Option<f32>,

    /// Traditionally the jpeg quality value, but used as a fallback for other formats.
    pub quality: Option<i32>,

    pub webp_quality: Option<f32>,
    //#[deprecated(since = "0.1.0", note = "replaced with shared &lossless")]
    pub webp_lossless: Option<BoolKeep>, // replace with shared &lossless

    /// jpeg_subsampling is Ignored!
    #[deprecated(since = "0.1.0", note = "Not implemented in imageflow")]
    pub jpeg_subsampling: Option<i32>,
    pub jpeg_progressive: Option<bool>,
    pub jpeg_turbo: Option<bool>,
    pub jpeg_li: Option<bool>, // TODO add to parsing/etc
    pub jpeg_quality: Option<i32>,

    pub png_quality: Option<u8>,
    pub png_min_quality: Option<u8>,
    pub png_quantization_speed: Option<u8>,
    pub png_libpng: Option<bool>,
    pub png_max_deflate: Option<bool>,
    //#[deprecated(since = "0.1.0", note = "replaced with shared &lossless")]
    pub png_lossless: Option<BoolKeep>,

    pub jxl_distance: Option<f32>, // recommend 0.5 to 3.0 (96.68 jpeg equiv), default 1, full range 0..25
    pub jxl_effort: Option<u8>,    //clamped to reasonable values 0..7, 8+ blocked
    pub jxl_quality: Option<f32>,  // similar to jpeg quality, 0..100
    //#[deprecated(since = "0.1.0", note = "replaced with shared &lossless")]
    pub jxl_lossless: Option<BoolKeep>, // replaced with shared &lossless

    pub avif_quality: Option<f32>,
    pub avif_speed: Option<u8>, // 3..10, 1 and 2 are blocked for being too slow.

    pub frame: Option<i32>,
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Anchor1D {
    Near,
    Center,
    Far,
    Percent(f32),
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum OutputFormat {
    Keep, // The default, don't change the format.
    Jpeg,
    Png,
    Gif,
    Webp,
    Avif,
    Jxl,
    Auto,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        for (k, v) in
            OutputFormatStrings::iter_variant_names().zip(OutputFormatStrings::iter_variants())
        {
            if k.eq_ignore_ascii_case(text) {
                return Ok(v.clean());
            }
        }
        Err(format!("Invalid output format: {}", text))
    }
}

impl OutputFormat {
    pub fn to_output_image_format(&self) -> Option<OutputImageFormat> {
        match self {
            OutputFormat::Keep => Some(OutputImageFormat::Keep),
            OutputFormat::Jpeg => Some(OutputImageFormat::Jpeg),
            OutputFormat::Png => Some(OutputImageFormat::Png),
            OutputFormat::Gif => Some(OutputImageFormat::Gif),
            OutputFormat::Webp => Some(OutputImageFormat::Webp),
            OutputFormat::Avif => Some(OutputImageFormat::Avif),
            OutputFormat::Jxl => Some(OutputImageFormat::Jxl),
            OutputFormat::Auto => None,
        }
    }
}
/// Controls whether the image is allowed to upscale, downscale, both, or if only the canvas gets to be upscaled.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ScaleMode {
    /// The default. Only downsamples images - never enlarges. If an image is smaller than 'width' and 'height', the image coordinates are used instead.
    DownscaleOnly,
    /// Only upscales (zooms) images - never downsamples except to meet web.config restrictions. If an image is larger than 'width' and 'height', the image coordinates are used instead.
    UpscaleOnly,
    /// Upscales and downscales images according to 'width' and 'height', within web.config restrictions.
    Both,
    /// When the image is smaller than the requested size, padding is added instead of stretching the image
    UpscaleCanvas,
}

#[cfg(test)]
fn debug_diff<T>(a: &T, b: &T, collapse_same: bool) -> String
where
    T: fmt::Debug,
    T: PartialEq,
{
    let mut t = String::new();
    if a != b {
        let text1 = format!("{:#?}", a);
        let text2 = format!("{:#?}", b);
        use ::difference::{Changeset, Difference};

        // compare both texts, the third parameter defines the split level
        let changeset = Changeset::new(&text1, &text2, "\n");

        let mut last_same = false;
        for i in 0..changeset.diffs.len() {
            match changeset.diffs[i] {
                Difference::Same(ref x) => {
                    if !last_same {
                        if collapse_same {
                            t.push_str("...\n");
                        } else {
                            t.push_str(&format!(" {}\n", x));
                        }
                        last_same = true;
                    }
                }
                Difference::Add(ref x) => {
                    t.push_str(&format!("+{}\n", x));
                    last_same = false;
                }
                Difference::Rem(ref x) => {
                    t.push_str(&format!("-{}\n", x));
                    last_same = false;
                }
            }
        }
    }
    t
}

#[test]
#[rustfmt::skip]
fn test_url_parsing() {
    #[track_caller]
    fn t(rel_url: &str, expected: Instructions, expected_warnings: Vec<ParseWarning>){
        let mut error_text = String::new();
        let url = format!("http://localhost/image.jpg?{}", rel_url);
        let a = Url::from_str(&url).unwrap();
        let (i, warns) = parse_url(&a);

        let match_failed = i != expected || warns != expected_warnings;

        if match_failed {
            error_text.push_str(&format!("Failed to parse as expected: {}\n", &rel_url));
        }else{
           return;
        }
            // eprintln!("{} -> {}", &url, i.to_string());
        if i.bgcolor_srgb != expected.bgcolor_srgb && i.bgcolor_srgb.is_some() && expected.bgcolor_srgb.is_some(){
            error_text.push_str(&format!("Expected bgcolor={}, actual={}\n", expected.bgcolor_srgb.unwrap().to_aarrggbb_string(), i.bgcolor_srgb.unwrap().to_aarrggbb_string()));
        }


        if i != expected {
            let error = debug_diff(&i, &expected, true);
            error_text.push_str("Instructions diff:\n");
            error_text.push_str(&error);
        }

        if warns != expected_warnings{
            let warns_diff = debug_diff(&warns, &expected_warnings, true);
            error_text.push_str("Warnings diff:\n");
            error_text.push_str(&warns_diff);
        }

        panic!("{}", error_text);

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
    t("format=webp", Instructions { format: Some(OutputFormat::Webp), ..Default::default() }, vec![]);
    t("format=avif", Instructions { format: Some(OutputFormat::Avif), ..Default::default() }, vec![]);
    t("format=jxl", Instructions { format: Some(OutputFormat::Jxl), ..Default::default() }, vec![]);
    t("format=keep", Instructions { format: Some(OutputFormat::Keep), ..Default::default() }, vec![]);
    t("format=auto&accept.webp=1", Instructions { format: Some(OutputFormat::Auto), accept_webp: Some(true), ..Default::default() }, vec![]);
    t("format=auto&accept.avif=1", Instructions { format: Some(OutputFormat::Auto), accept_avif: Some(true), ..Default::default() }, vec![]);
    t("lossless=false&accept.jxl=1", Instructions { lossless: Some(BoolKeep::False), accept_jxl: Some(true), ..Default::default() }, vec![]);
    t("lossless=keep&accept.jxl=1", Instructions { lossless: Some(BoolKeep::Keep), accept_jxl: Some(true), ..Default::default() }, vec![]);
    t("lossless=true&accept.jxl=1", Instructions { lossless: Some(BoolKeep::True), accept_jxl: Some(true), ..Default::default() }, vec![]);
    t("format=auto&accept.color_profiles=1", Instructions { format: Some(OutputFormat::Auto), accept_color_profiles: Some(true), ..Default::default() }, vec![]);
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




    t("qp=lowest", Instructions { qp:Some(QualityProfile::Lowest), ..Default::default() }, vec![]);
    t("qp=low", Instructions { qp:Some(QualityProfile::Low), ..Default::default() }, vec![]);
    t("qp=medium", Instructions { qp:Some(QualityProfile::Medium), ..Default::default() }, vec![]);
    t("qp=good", Instructions { qp:Some(QualityProfile::Good), ..Default::default() }, vec![]);
    t("qp=high", Instructions { qp:Some(QualityProfile::High), ..Default::default() }, vec![]);
    t("qp=highest", Instructions { qp:Some(QualityProfile::Highest), ..Default::default() }, vec![]);
    t("qp=lossless", Instructions { qp:Some(QualityProfile::Lossless), ..Default::default() }, vec![]);
    t("qp=100", Instructions { qp:Some(QualityProfile::Percent(100.0)), ..Default::default() }, vec![]);
    t("qp=0", Instructions { qp:Some(QualityProfile::Percent(0.0)), ..Default::default() }, vec![]);
    t("qp=37.2", Instructions { qp:Some(QualityProfile::Percent(37.2)), ..Default::default() }, vec![]);
    t("qp.dpr=4", Instructions { qp_dpr: Some(4.0), ..Default::default() }, vec![]);
    t("qp.dppx=2.75", Instructions { qp_dpr: Some(2.75), ..Default::default() }, vec![]);



    t("webp.lossless=true", Instructions { webp_lossless: Some(BoolKeep::True), ..Default::default() }, vec![]);
    t("webp.lossless=keep", Instructions { webp_lossless: Some(BoolKeep::Keep), ..Default::default() }, vec![]);
    t("jpeg.progressive=true", Instructions { jpeg_progressive: Some(true), ..Default::default() }, vec![]);
    t("ignoreicc=true", Instructions { ignoreicc: Some(true), ..Default::default() }, vec![]);
    t("ignore_icc_errors=true", Instructions { ignore_icc_errors: Some(true), ..Default::default() }, vec![]);
    t("jpeg.turbo=true", Instructions { jpeg_turbo: Some(true), ..Default::default() }, vec![]);
    t("jpeg.li=true", Instructions { jpeg_li: Some(true), ..Default::default() }, vec![]);
    t("png.quality=90", Instructions { png_quality: Some(90), ..Default::default() }, vec![]);
    t("png.min_quality=90", Instructions { png_min_quality: Some(90), ..Default::default() }, vec![]);
    t("png.quantization_speed=4", Instructions { png_quantization_speed: Some(4), ..Default::default() }, vec![]);
    t("png.lossless=true", Instructions { png_lossless: Some(BoolKeep::True), ..Default::default() }, vec![]);
    t("png.libpng=true", Instructions { png_libpng: Some(true), ..Default::default() }, vec![]);
    t("png.max_deflate=true", Instructions { png_max_deflate: Some(true), ..Default::default() }, vec![]);
    t("quality=85", Instructions { quality: Some(85), ..Default::default() }, vec![]);
    t("webp.quality=85", Instructions { webp_quality: Some(85f32), ..Default::default() }, vec![]);
    t("jpeg.quality=85", Instructions { jpeg_quality: Some(85), ..Default::default() }, vec![]);
    t("avif.quality=85", Instructions { avif_quality: Some(85f32), ..Default::default() }, vec![]);
    t("avif.speed=2", Instructions { avif_speed: Some(2), ..Default::default() }, vec![]);
    t("jxl.quality=85", Instructions { jxl_quality: Some(85f32), ..Default::default() }, vec![]);
    t("jxl.distance=0.4", Instructions { jxl_distance: Some(0.4f32), ..Default::default() }, vec![]);
    t("jxl.effort=4", Instructions { jxl_effort: Some(4), ..Default::default() }, vec![]);
    t("jxl.lossless=keep", Instructions { jxl_lossless: Some(BoolKeep::Keep), ..Default::default() }, vec![]);

    t("frame=2", Instructions { frame: Some(2), ..Default::default() }, vec![]);
    t("page=5", Instructions { frame: Some(5), ..Default::default() }, vec![]);

    t("zoom=0.02", Instructions { zoom: Some(0.02f32), ..Default::default() }, vec![]);
    t("trim.threshold=80&trim.percentpadding=0.02", Instructions { trim_whitespace_threshold: Some(80),  trim_whitespace_padding_percent: Some(0.02f32), ..Default::default() }, vec![]);
    t("w=10&f.sharpen=80.5", Instructions { w: Some(10), f_sharpen: Some(80.5f32), ..Default::default() }, vec![]);
    t("f.sharpen=80.5", Instructions { f_sharpen: Some(80.5f32), ..Default::default() }, vec![]);
    t("decoder.min_precise_scaling_ratio=3.5", Instructions { min_precise_scaling_ratio: Some(3.5f32), ..Default::default() }, vec![]);



    t("f.sharpen_when=always", Instructions{ f_sharpen_when: Some(SharpenWhen::Always), ..Default::default()}, vec![]);
    t("f.sharpen_when=downscaling", Instructions{ f_sharpen_when: Some(SharpenWhen::Downscaling), ..Default::default()}, vec![]);
    t("f.sharpen_when=sizediffers", Instructions{ f_sharpen_when: Some(SharpenWhen::SizeDiffers), ..Default::default()}, vec![]);

    t("s.sepia=true&s.brightness=0.1&s.saturation=-0.1&s.contrast=1&s.alpha=0", Instructions { s_alpha: Some(0f32), s_contrast: Some(1f32), s_sepia: Some(true), s_brightness: Some(0.1f32), s_saturation: Some(-0.1f32), ..Default::default() }, vec![]);

    t("s.grayscale=true",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::True), ..Default::default()}, vec![]);
    t("s.grayscale=flat",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Flat), ..Default::default()}, vec![]);
    t("s.grayscale=ntsc",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Ntsc), ..Default::default()}, vec![]);
    t("s.grayscale=ry",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Ry), ..Default::default()}, vec![]);
    t("s.grayscale=Y",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Y), ..Default::default()}, vec![]);
    t("s.grayscale=Bt709",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Bt709), ..Default::default()}, vec![]);

    t("bgcolor=é", Default::default(), vec![ParseWarning::ValueInvalid(("bgcolor", "é".into())), ParseWarning::KeyNotSupported(("bgcolor".into(), "é".into()))]);
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
    t("c=0,0,40,50", Instructions { crop: Some([0f64,0f64,40f64,50f64]), cropxunits: Some(100.0), cropyunits: Some(100.0), ..Default::default() }, vec![]);



    t("s.roundcorners= 0, 0,40 ,  50", Instructions { s_round_corners: Some([0f64,0f64,40f64,50f64]), ..Default::default() }, vec![]);
    t("s.roundcorners= 100", Instructions { s_round_corners: Some([100f64,100f64,100f64,100f64]), ..Default::default() }, vec![]);

    t("a.balancewhite=true",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Area), ..Default::default()}, vec![]);
    t("a.balancewhite=area",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Area), ..Default::default()}, vec![]);
    t("down.colorspace=linear",  Instructions{down_colorspace: Some(ScalingColorspace::Linear), ..Default::default()}, vec![]);
    t("down.colorspace=srgb",  Instructions{down_colorspace: Some(ScalingColorspace::Srgb), ..Default::default()}, vec![]);
    t("up.colorspace=linear",  Instructions{up_colorspace: Some(ScalingColorspace::Linear), ..Default::default()}, vec![]);
    t("up.colorspace=srgb",  Instructions{up_colorspace: Some(ScalingColorspace::Srgb), ..Default::default()}, vec![]);
    t("up.filter=mitchell",  Instructions{up_filter: Some(FilterStrings::Mitchell), ..Default::default()}, vec![]);
    t("down.filter=lanczos",  Instructions{down_filter: Some(FilterStrings::Lanczos), ..Default::default()}, vec![]);


    t("c.gravity=89,101",  Instructions{c_gravity: Some([89.0,101.0]), ..Default::default()}, vec![]);

    t("watermark_red_dot=true",  Instructions{watermark_red_dot: Some(true), ..Default::default()}, vec![]);

    let srcset_default = Instructions{mode: Some(FitMode::Max), ..Default::default()};
    t("srcset=100w",  Instructions{w: Some(100), ..srcset_default.to_owned()}, vec![]);
    t("srcset=100h",  Instructions{h: Some(100), ..srcset_default.to_owned()}, vec![]);
    t("srcset=2x",  Instructions{zoom: Some(2.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=webp-90",  Instructions{format: Some(OutputFormat::Webp), webp_quality: Some(90.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=png-90",  Instructions{format: Some(OutputFormat::Png), png_quality: Some(90), ..srcset_default.to_owned()}, vec![]);
    t("srcset=jpeg-90",  Instructions{format: Some(OutputFormat::Jpeg), jpeg_quality: Some(90), ..srcset_default.to_owned()}, vec![]);
    t("srcset=crop-10-20-80-90",  Instructions{cropxunits: Some(100.0), cropyunits: Some(100.0),crop: Some([10.0,20.0,80.0,90.0]), ..srcset_default.to_owned()}, vec![]);
    t("srcset=upscale",  Instructions{scale: Some(ScaleMode::Both), ..srcset_default.to_owned()}, vec![]);
    t("srcset=fit-crop",  Instructions{mode: Some(FitMode::Crop), ..srcset_default.to_owned()}, vec![]);
    t("srcset=fit-cover",  Instructions{mode: Some(FitMode::Crop), ..srcset_default.to_owned()}, vec![]);
    t("srcset=fit-pad",  Instructions{mode: Some(FitMode::Pad), ..srcset_default.to_owned()}, vec![]);
    t("srcset=fit-contain",  Instructions{mode: Some(FitMode::Max), ..srcset_default.to_owned()}, vec![]);
    t("srcset=fit-distort",  Instructions{mode: Some(FitMode::Stretch), ..srcset_default.to_owned()}, vec![]);
    t("srcset=",  Instructions{..Default::default()}, vec![]);
    t("srcset=sharp-20",  Instructions{f_sharpen: Some(20.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=sharpen-20",  Instructions{f_sharpen: Some(20.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=sharp-20",  Instructions{f_sharpen: Some(20.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=gif",  Instructions{format: Some(OutputFormat::Gif), ..srcset_default.to_owned()}, vec![]);
    t("srcset=png-lossless",  Instructions{format: Some(OutputFormat::Png), png_lossless: Some(BoolKeep::True), ..srcset_default.to_owned()}, vec![]);
    t("srcset=webp-lossless",  Instructions{format: Some(OutputFormat::Webp), webp_lossless: Some(BoolKeep::True), ..srcset_default.to_owned()}, vec![]);
    t("srcset=webp-keep",  Instructions{format: Some(OutputFormat::Webp), webp_lossless: Some(BoolKeep::Keep), ..srcset_default.to_owned()}, vec![]);
    t("srcset=jxl-keep",  Instructions{format: Some(OutputFormat::Jxl), jxl_lossless: Some(BoolKeep::Keep), ..srcset_default.to_owned()}, vec![]);
    t("srcset=jxl-90",  Instructions{format: Some(OutputFormat::Jxl), jxl_quality: Some(90.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=avif-90",  Instructions{format: Some(OutputFormat::Avif), avif_quality: Some(90.0), ..srcset_default.to_owned()}, vec![]);

    t("srcset=webp",  Instructions{format: Some(OutputFormat::Webp), ..srcset_default.to_owned()}, vec![]);
    t("srcset=webp&webp.quality=5",  Instructions{format: Some(OutputFormat::Webp), webp_quality: Some(5.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=webp-76,100w, 100h,2x ,sharp-20 ,fit-crop", Instructions{format: Some(OutputFormat::Webp), webp_quality: Some(76.0), w: Some(100), h: Some(100), zoom: Some(2.0), f_sharpen: Some(20.0), mode: Some(FitMode::Crop), ..srcset_default.to_owned()}, vec![]);
    t("srcset=lossless,100w,fit-crop", Instructions{format: Some(OutputFormat::Auto), lossless: Some(BoolKeep::True), w: Some(100), mode: Some(FitMode::Crop), ..srcset_default.to_owned()}, vec![]);
    t("srcset=lossy,100w,fit-crop", Instructions{format: Some(OutputFormat::Auto), lossless: Some(BoolKeep::False), w: Some(100), mode: Some(FitMode::Crop), ..srcset_default.to_owned()}, vec![]);
    t("srcset=auto,100w,fit-crop", Instructions{format: Some(OutputFormat::Auto), w: Some(100), mode: Some(FitMode::Crop), ..srcset_default.to_owned()}, vec![]);
    t("srcset=keep,100w,fit-crop", Instructions{format: Some(OutputFormat::Keep), w: Some(100), mode: Some(FitMode::Crop), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-low,qp-dpr-1x", Instructions{qp: Some(QualityProfile::Low), qp_dpr: Some(1.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-medium,qp-dpr-2x", Instructions{qp: Some(QualityProfile::Medium), qp_dpr: Some(2.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-mediumlow,qp-dpr-2x", Instructions{qp: Some(QualityProfile::MediumLow), qp_dpr: Some(2.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-good,qp-dpr-3x", Instructions{qp: Some(QualityProfile::Good), qp_dpr: Some(3.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-mediumhigh,qp-dpr-3x", Instructions{qp: Some(QualityProfile::Good), qp_dpr: Some(3.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-high,qp-dpr-4x", Instructions{qp: Some(QualityProfile::High), qp_dpr: Some(4.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-highest,qp-dpr-1x", Instructions{qp: Some(QualityProfile::Highest), qp_dpr: Some(1.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-lossless,qp-dpr-.7x", Instructions{qp: Some(QualityProfile::Lossless), qp_dpr: Some(0.7), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-100,qp-dpr-1x", Instructions{qp: Some(QualityProfile::Percent(100.0)), qp_dpr: Some(1.0), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-0,qp-dpr-0.5x", Instructions{qp: Some(QualityProfile::Percent(0.0)), qp_dpr: Some(0.5), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-37.2,qp-dpr-1.5x", Instructions{qp: Some(QualityProfile::Percent(37.2)), qp_dpr: Some(1.5), ..srcset_default.to_owned()}, vec![]);
    t("srcset=qp-lowest", Instructions{qp: Some(QualityProfile::Lowest), ..srcset_default.to_owned()}, vec![]);
    t("srcset=auto-lossless,qp-lowest", Instructions{format: Some(OutputFormat::Auto), lossless: Some(BoolKeep::True), qp: Some(QualityProfile::Lowest), ..srcset_default.to_owned()}, vec![]);

    t("anchor=50,25",  Instructions{anchor: Some((Anchor1D::Percent(50.0), Anchor1D::Percent(25.0))), ..Default::default()}, vec![]);
    t("anchor=bottomleft",  Instructions{anchor: Some((Anchor1D::Near, Anchor1D::Far)), ..Default::default()}, vec![]);
    t("srcset=png",  Instructions{format: Some(OutputFormat::Png), ..srcset_default.to_owned()}, vec![]);


    expect_warning("a.balancewhite","gimp",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Gimp), ..Default::default()});
    expect_warning("a.balancewhite","simple",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Simple), ..Default::default()});
    expect_warning("crop","(0,3,80, 90)",  Instructions { crop: Some([0f64,3f64,80f64,90f64]), ..Default::default() });
    expect_warning("crop","(0,3,happy, 90)",  Instructions { crop: Some([0f64,3f64,0f64,90f64]), ..Default::default() });
    expect_warning("crop","(  a0, 3, happy, 90)",  Instructions { crop: Some([0f64,3f64,0f64,90f64]), ..Default::default() });

    // expect_warning("srcset","crop,pad",  Instructions{mode: Some(FitMode::Pad), ..srcset_default.to_owned()});
    // expect_warning("srcset","pad,crop",  Instructions{mode: Some(FitMode::Crop), ..srcset_default.to_owned()});
    // expect_warning("srcset","png,gif",  Instructions{format: Some(OutputFormat::Gif), ..srcset_default.to_owned()});


}

#[rustfmt::skip]

#[test]
fn test_tostr(){
    fn t(expected_query: &str, from: Instructions){
        let b = from.to_string();
        if expected_query != b.as_str(){
            let mut text = format!("Expected: {}\n", expected_query);
            text.push_str(&format!("Actual: {}\n", &b.as_str()));
            text.push_str(&format!("Diff:\n{}", debug_diff(&expected_query, &b.as_str(), true)));
            text.push_str(&format!("Input Instructions:\n{}", &from.to_string()));
            panic!("{}", text);
        }
    }
    t("h=300&mode=max&w=200", Instructions { w: Some(200), h: Some(300), mode: Some(FitMode::Max), ..Default::default() });
    t("h=300&mode=crop&w=200", Instructions { w: Some(200), h: Some(300), mode: Some(FitMode::Crop), ..Default::default() });
    t("format=jpeg", Instructions { format: Some(OutputFormat::Jpeg), ..Default::default() });
    t("format=gif", Instructions { format: Some(OutputFormat::Gif), ..Default::default() });
    t("format=png", Instructions { format: Some(OutputFormat::Png), ..Default::default() });
    t("format=webp", Instructions { format: Some(OutputFormat::Webp), ..Default::default() });
    t("format=keep", Instructions { format: Some(OutputFormat::Keep), ..Default::default() });
    t("lossless=true", Instructions { lossless: Some(BoolKeep::True), ..Default::default() });
    t("accept.webp=true&format=auto", Instructions { format: Some(OutputFormat::Auto), accept_webp: Some(true), ..Default::default() });
    t("accept.avif=true&format=auto", Instructions { format: Some(OutputFormat::Auto), accept_avif: Some(true), ..Default::default() });
    t("accept.jxl=true&format=auto", Instructions { format: Some(OutputFormat::Auto), accept_jxl: Some(true), ..Default::default() });
    t("accept.color_profiles=true&format=auto", Instructions { format: Some(OutputFormat::Auto), accept_color_profiles: Some(true), ..Default::default() });
    t("lossless=keep", Instructions { lossless: Some(BoolKeep::Keep), ..Default::default() });
    t("lossless=false", Instructions { lossless: Some(BoolKeep::False), ..Default::default() });

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
    t("zoom=0.02", Instructions { zoom: Some(0.02f32), ..Default::default() });
    t("trim.percentpadding=0.02&trim.threshold=80", Instructions { trim_whitespace_threshold: Some(80),  trim_whitespace_padding_percent: Some(0.02f32), ..Default::default() });
    t("bgcolor=ff0000ff", Instructions { bgcolor_srgb: Some(Color32(0xffff0000)), ..Default::default() });
    t("bgcolor=8fbc8bff", Instructions { bgcolor_srgb: Some(Color32(0xff8fbc8b)), ..Default::default() });
    t("bgcolor=77889953", Instructions { bgcolor_srgb: Some(Color32(0x53778899)), ..Default::default() });
    t("bgcolor=ffffffff", Instructions { bgcolor_srgb: Some(Color32(0xffffffff)), ..Default::default() });
    t("crop=0,0,40,50", Instructions { crop: Some([0f64,0f64,40f64,50f64]), ..Default::default() });
    t("a.balancewhite=area",  Instructions{a_balance_white: Some(HistogramThresholdAlgorithm::Area), ..Default::default()});
    t("webp.quality=85", Instructions { webp_quality: Some(85f32), ..Default::default() });
    t("webp.lossless=true", Instructions { webp_lossless: Some(BoolKeep::True), ..Default::default() });
    t("webp.lossless=keep", Instructions { webp_lossless: Some(BoolKeep::Keep), ..Default::default() });
    t("up.colorspace=srgb",  Instructions{up_colorspace: Some(ScalingColorspace::Srgb), ..Default::default()});
    t("up.colorspace=linear",  Instructions{up_colorspace: Some(ScalingColorspace::Linear), ..Default::default()});
    t("down.colorspace=srgb",  Instructions{down_colorspace: Some(ScalingColorspace::Srgb), ..Default::default()});
    t("down.colorspace=linear",  Instructions{down_colorspace: Some(ScalingColorspace::Linear), ..Default::default()});
    t("decoder.min_precise_scaling_ratio=3.5", Instructions { min_precise_scaling_ratio: Some(3.5f32), ..Default::default() });

    t("s.roundcorners=0,0,40,50", Instructions { s_round_corners: Some([0f64,0f64,40f64,50f64]), ..Default::default() });
    t("s.roundcorners=100", Instructions { s_round_corners: Some([100f64,100f64,100f64,100f64]), ..Default::default() });


    t("f.sharpen=10", Instructions{ f_sharpen: Some(10f32), ..Default::default()});
    t("f.sharpen_when=always", Instructions{ f_sharpen_when: Some(SharpenWhen::Always), ..Default::default()});
    t("f.sharpen_when=downscaling", Instructions{ f_sharpen_when: Some(SharpenWhen::Downscaling), ..Default::default()});
    t("f.sharpen_when=sizediffers", Instructions{ f_sharpen_when: Some(SharpenWhen::SizeDiffers), ..Default::default()});
    t("s.grayscale=bt709",  Instructions{s_grayscale: Some(GrayscaleAlgorithm::Bt709), ..Default::default()});
    t("s.alpha=0&s.brightness=0.1&s.contrast=1&s.saturation=-0.1&s.sepia=true", Instructions { s_alpha: Some(0f32), s_contrast: Some(1f32), s_sepia: Some(true), s_brightness: Some(0.1f32), s_saturation: Some(-0.1f32), ..Default::default() });
    t("jpeg.progressive=true", Instructions { jpeg_progressive: Some(true), ..Default::default() });
    t("jpeg.turbo=true", Instructions { jpeg_turbo: Some(true), ..Default::default() });
    t("jpeg.li=true", Instructions { jpeg_li: Some(true), ..Default::default() });
    t("jpeg.quality=85", Instructions { jpeg_quality: Some(85), ..Default::default() });
    t("png.quality=90", Instructions { png_quality: Some(90), ..Default::default() });
    t("png.min_quality=90", Instructions { png_min_quality: Some(90), ..Default::default() });
    t("png.quantization_speed=4", Instructions { png_quantization_speed: Some(4), ..Default::default() });
    t("png.libpng=true", Instructions { png_libpng: Some(true), ..Default::default() });
    t("png.max_deflate=true", Instructions { png_max_deflate: Some(true), ..Default::default() });
    t("png.lossless=true", Instructions { png_lossless: Some(BoolKeep::True), ..Default::default()});
    t("up.filter=mitchell",  Instructions{up_filter: Some(FilterStrings::Mitchell), ..Default::default()});
    t("down.filter=lanczos",  Instructions{down_filter: Some(FilterStrings::Lanczos), ..Default::default()});
    t("anchor=bottomleft",  Instructions{anchor: Some((Anchor1D::Near, Anchor1D::Far)), ..Default::default()});
    t("ignoreicc=true", Instructions { ignoreicc: Some(true), ..Default::default() });
    t("ignore_icc_errors=true", Instructions { ignore_icc_errors: Some(true), ..Default::default() });
    t("watermark_red_dot=true",  Instructions{watermark_red_dot: Some(true), ..Default::default()});
    t("frame=3", Instructions { frame: Some(3), ..Default::default() });

    // Add missing cases from test_url_parsing
    t("accept.webp=true", Instructions { accept_webp: Some(true), ..Default::default() });
    t("accept.avif=true", Instructions { accept_avif: Some(true), ..Default::default() });
    t("accept.jxl=true", Instructions { accept_jxl: Some(true), ..Default::default() });
    t("accept.color_profiles=true", Instructions { accept_color_profiles: Some(true), ..Default::default() });
    t("c=0,0,40,50", Instructions { cropxunits: Some(100.0), cropyunits: Some(100.0), crop: Some([0f64, 0f64, 40f64, 50f64]), ..Default::default() });
    t("qp=lowest", Instructions { qp: Some(QualityProfile::Lowest), ..Default::default() });
    t("qp=low", Instructions { qp: Some(QualityProfile::Low), ..Default::default() });
    t("qp=medium", Instructions { qp: Some(QualityProfile::Medium), ..Default::default() });
    t("qp=good", Instructions { qp: Some(QualityProfile::Good), ..Default::default() });
    t("qp=high", Instructions { qp: Some(QualityProfile::High), ..Default::default() });
    t("qp=highest", Instructions { qp: Some(QualityProfile::Highest), ..Default::default() });
    t("qp=lossless", Instructions { qp: Some(QualityProfile::Lossless), ..Default::default() });
    t("qp=100", Instructions { qp: Some(QualityProfile::Percent(100.0)), ..Default::default() });
    t("qp=0", Instructions { qp: Some(QualityProfile::Percent(0.0)), ..Default::default() });
    t("qp=37", Instructions { qp: Some(QualityProfile::Percent(37.0)), ..Default::default() });
    t("qp.dpr=4", Instructions { qp_dpr: Some(4.0), ..Default::default() });
    t("qp.dpr=2.75", Instructions { qp_dpr: Some(2.75), ..Default::default() });

    //jxl*, avif*
    t("jxl.quality=85", Instructions { jxl_quality: Some(85f32), ..Default::default() });
    t("jxl.distance=0.4", Instructions { jxl_distance: Some(0.4f32), ..Default::default() });
    t("jxl.effort=4", Instructions { jxl_effort: Some(4), ..Default::default() });
    t("jxl.lossless=keep", Instructions { jxl_lossless: Some(BoolKeep::Keep), ..Default::default() });
    t("jxl.lossless=false", Instructions { jxl_lossless: Some(BoolKeep::False), ..Default::default() });
    t("avif.quality=85", Instructions { avif_quality: Some(85f32), ..Default::default() });
    t("avif.speed=2", Instructions { avif_speed: Some(2), ..Default::default() });


}
