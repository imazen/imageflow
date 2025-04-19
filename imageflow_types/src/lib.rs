//! # `imageflow_types`
//!
//! Responsible for the schema of the JSON API, as well as for providing types used internally.
//! (There is a lot of overlap, as there can be, in early versions).
//!
//! ## `snake_case` vs `camelCase`
//!
//! We don't currently do any style transformations, but we have tests to try to ensure they're
//! always possible.
//! Here are the transformation rules we use to verify all key names we select can be round-tripped
//! between styles:
//!
//! #### `camelCase` to `snake_case`
//!
//! 1. Add a leading underscore to every group of numbers unless preceded by a lowercase x or y.
//!    `/([^xy])([0-9]+)/ with "$1_$2"/`
//! 2. Add a leading underscore before every uppercase letter: `/[A-Z]/ with "_$0"`
//! 3. Strip leading underscores from string `/(\A|\s+)_+/ with "$1"`
//! 4. Collapse all duplicate underscores `replace("__", "_")`
//! 5. Lowercase the resulting string
//!
//! #### `snake_case` to `camelCase`
//!
//!  1. Uppercase every letter following an underscore or word boundary.
//!     `Regex::new(r"(_|\b)([a-z])").unwrap().replace_all(&s, |c: &Captures| c[0].to_uppercase())`
//!  2. Lowercase first character of string
//!  3. Delete all underscores from string


pub mod build_env_info;
pub mod version;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static; //Used by build_env_info.rs

extern crate imageflow_helpers;
extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate rgb;
extern crate imgref;

use std::fmt;

use imgref::ImgRef;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
#[cfg(feature = "schema-export")]
use utoipa::ToSchema;

// Placeholder derive macro when json-schema feature is not enabled
#[cfg(not(feature = "json-schema"))]
#[macro_export]
macro_rules! JsonSchema { () => {}; }

// Placeholder trait when json-schema feature is not enabled
#[cfg(not(feature = "json-schema"))]
pub trait JsonSchema {}

// Implement the placeholder trait for all types when the feature is off
#[cfg(not(feature = "json-schema"))]
impl<T> JsonSchema for T {}



//use std::str::FromStr;
pub mod collections;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PixelLayout{
    BGR,
    BGRA,
    Gray
}

impl PixelLayout{
    pub fn channels(&self) -> usize{
        match self{
            PixelLayout::BGR => 3,
            PixelLayout::BGRA => 4,
            PixelLayout::Gray => 1
        }
    }
}

/// Memory layout for pixels.
/// sRGB w/ gamma encoding assumed for 8-bit channels.
#[repr(C)]
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum PixelFormat {
    #[serde(rename="bgra_32")]
    Bgra32 = 4,
    #[serde(rename="bgr_32")]
    Bgr32 = 70,
    #[serde(rename="bgr_24")]

    Bgr24 = 3,
    #[serde(rename="gray_8")]
    Gray8 = 1,
}

impl PixelFormat {
    pub fn pixel_layout(&self) -> PixelLayout {
        match self{
            PixelFormat::Bgra32 => PixelLayout::BGRA,
            PixelFormat::Bgr32 => PixelLayout::BGRA,
            PixelFormat::Bgr24 => PixelLayout::BGR,
            PixelFormat::Gray8 => PixelLayout::Gray,
        }
    }
    pub fn alpha_meaningful(&self) -> bool{
        self == &PixelFormat::Bgra32
    }

    pub fn debug_name(&self) -> &'static str {
        match self {
            PixelFormat::Bgr24 => "bgra24",
            PixelFormat::Gray8 => "gray8",
            PixelFormat::Bgra32 => "bgra32",
            PixelFormat::Bgr32 => "bgr32",
            // _ => "?"
        }
    }

}

impl PixelFormat{
    /// The number of bytes required to store the given pixel type
    pub fn bytes(&self) -> usize {
        match *self{
            PixelFormat::Gray8 => 1,
            PixelFormat::Bgr24 => 3,
            PixelFormat::Bgra32 |
            PixelFormat::Bgr32 => 4
        }
    }
}

/// Internal 2d representation of pixel slices
pub enum PixelBuffer<'a> {
    Bgra32(ImgRef<'a, rgb::alt::BGRA8>),
    Bgr32(ImgRef<'a, rgb::alt::BGRA8>), // there's no BGRX support in the rgb crate
    Bgr24(ImgRef<'a, rgb::alt::BGR8>),
    Gray8(ImgRef<'a, rgb::alt::Gray<u8>>),
}

/// Named interpolation function+configuration presets
#[repr(C)]
#[derive(Copy, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum Filter {
    #[serde(rename="robidoux_fast")]
    RobidouxFast = 1,
    #[serde(rename="robidoux")]
    Robidoux = 2,
    #[serde(rename="robidoux_sharp")]
    RobidouxSharp = 3,
    #[serde(rename="ginseng")]
    Ginseng = 4,
    #[serde(rename="ginseng_sharp")]
    GinsengSharp = 5,
    #[serde(rename="lanczos")]
    Lanczos = 6,
    #[serde(rename="lanczos_sharp")]
    LanczosSharp = 7,
    #[serde(rename="lanczos_2")]
    Lanczos2 = 8,
    #[serde(rename="lanczos_2_sharp")]
    Lanczos2Sharp = 9,
    // #[serde(rename="cubic_fast")]
    // CubicFast = 10,
    #[serde(rename="cubic")]
    Cubic = 11,
    #[serde(rename="cubic_sharp")]
    CubicSharp = 12,
    #[serde(rename="catmull_rom")]
    CatmullRom = 13,
    #[serde(rename="mitchell")]
    Mitchell = 14,
    #[serde(rename="cubic_b_spline")]
    CubicBSpline = 15,
    #[serde(rename="hermite")]
    Hermite = 16,
    #[serde(rename="jinc")]
    Jinc = 17,
    // #[serde(rename="raw_lanczos_3")]
    // RawLanczos3 = 18,
    // #[serde(rename="raw_lanczos_3_sharp")]
    // RawLanczos3Sharp = 19,
    // #[serde(rename="raw_lanczos_2")]
    // RawLanczos2 = 20,
    // #[serde(rename="raw_lanczos_2_sharp")]
    // RawLanczos2Sharp = 21,
    #[serde(rename="triangle")]
    Triangle = 22,
    #[serde(rename="linear")]
    Linear = 23,
    #[serde(rename="box")]
    Box = 24,
    // #[serde(rename="catmull_rom_fast")]
    // CatmullRomFast = 25,
    // #[serde(rename="catmull_rom_fast_sharp")]
    // CatmullRomFastSharp = 26,

    #[serde(rename="fastest")]
    Fastest = 27,
    // #[serde(rename="mitchell_fast")]
    // MitchellFast = 28,
    #[serde(rename="n_cubic")]
    NCubic = 29,
    #[serde(rename="n_cubic_sharp")]
    NCubicSharp = 30,
}
//
// impl FromStr for Filter {
//     type Err = &'static str;
//
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         match &*s.to_ascii_lowercase() {
//             "robidouxfast" => Ok(Filter::RobidouxFast),
//             "robidoux" => Ok(Filter::Robidoux),
//             "robidouxsharp" => Ok(Filter::RobidouxSharp),
//             "ginseng" => Ok(Filter::Ginseng),
//             "ginsengsharp" => Ok(Filter::GinsengSharp),
//             "lanczos" => Ok(Filter::Lanczos),
//             "lanczossharp" => Ok(Filter::LanczosSharp),
//             "lanczos2" => Ok(Filter::Lanczos2),
//             "lanczos2sharp" => Ok(Filter::Lanczos2Sharp),
//             "cubicfast" => Ok(Filter::CubicFast),
//             "cubic_0_1" => Ok(Filter::Cubic),
//             "cubicsharp" => Ok(Filter::CubicSharp),
//             "catmullrom" |
//             "catrom" => Ok(Filter::CatmullRom),
//             "mitchell" => Ok(Filter::Mitchell),
//             "cubicbspline" |
//             "bspline" => Ok(Filter::CubicBSpline),
//             "hermite" => Ok(Filter::Hermite),
//             "jinc" => Ok(Filter::Jinc),
//             "rawlanczos3" => Ok(Filter::RawLanczos3),
//             "rawlanczos3sharp" => Ok(Filter::RawLanczos3Sharp),
//             "rawlanczos2" => Ok(Filter::RawLanczos2),
//             "rawlanczos2sharp" => Ok(Filter::RawLanczos2Sharp),
//             "triangle" => Ok(Filter::Triangle),
//             "linear" => Ok(Filter::Linear),
//             "box" => Ok(Filter::Box),
//             "catmullromfast" => Ok(Filter::CatmullRomFast),
//             "catmullromfastsharp" => Ok(Filter::CatmullRomFastSharp),
//             "fastest" => Ok(Filter::Fastest),
//             "mitchellfast" => Ok(Filter::MitchellFast),
//             "ncubic" => Ok(Filter::NCubic),
//             "ncubicsharp" => Ok(Filter::NCubicSharp),
//             _ => Err("no match"),
//         }
//     }
// }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum PngBitDepth {
    #[serde(rename="png_32")]
    Png32,
    #[serde(rename="png_24")]
    Png24,
}

/// The color space to blend/combine pixels in. Downscaling is best done in linear light.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ScalingFloatspace {
    #[serde(rename="srgb")]
    Srgb,
    #[serde(rename="linear")]
    Linear, // gamma = 2,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum OutputImageFormat{
    Webp,
    Jpeg,
    Jpg,
    Png,
    Avif,
    Jxl,
    Gif,
    Keep
}

impl fmt::Display for OutputImageFormat{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self{
            OutputImageFormat::Webp => write!(f, "webp"),
            OutputImageFormat::Jpeg => write!(f, "jpg"),
            OutputImageFormat::Jpg => write!(f, "jpg"),
            OutputImageFormat::Png => write!(f, "png"),
            OutputImageFormat::Avif => write!(f, "avif"),
            OutputImageFormat::Jxl => write!(f, "jxl"),
            OutputImageFormat::Gif => write!(f, "gif"),
            OutputImageFormat::Keep => write!(f, "keep"),
        }
    }
}

impl OutputImageFormat{
    pub fn from_str(s: &str) -> Option<OutputImageFormat>{
        // case insensitive
        match s{
            _ if s.eq_ignore_ascii_case("webp") => Some(OutputImageFormat::Webp),
            _ if s.eq_ignore_ascii_case("image/webp") => Some(OutputImageFormat::Webp),
            _ if s.eq_ignore_ascii_case("jpg") => Some(OutputImageFormat::Jpeg),
            _ if s.eq_ignore_ascii_case("image/jpg") => Some(OutputImageFormat::Jpeg),
            _ if s.eq_ignore_ascii_case("jpeg") => Some(OutputImageFormat::Jpeg),
            _ if s.eq_ignore_ascii_case("png") => Some(OutputImageFormat::Png),
            _ if s.eq_ignore_ascii_case("image/png") => Some(OutputImageFormat::Png),
            _ if s.eq_ignore_ascii_case("avif") => Some(OutputImageFormat::Avif),
            _ if s.eq_ignore_ascii_case("image/avif") => Some(OutputImageFormat::Avif),
            _ if s.eq_ignore_ascii_case("jxl") => Some(OutputImageFormat::Jxl),
            _ if s.eq_ignore_ascii_case("image/jxl") => Some(OutputImageFormat::Jxl),
            _ if s.eq_ignore_ascii_case("gif") => Some(OutputImageFormat::Gif),
            _ if s.eq_ignore_ascii_case("image/gif") => Some(OutputImageFormat::Gif),
            _ if s.eq_ignore_ascii_case("keep") => Some(OutputImageFormat::Keep),
            _ => None,
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub struct EncoderHints{
    //pub jxl: Option<JxlEncoderHints>,
    pub webp: Option<WebpEncoderHints>,
    pub jpeg: Option<JpegEncoderHints>,
    pub png: Option<PngEncoderHints>,
    //pub avif: Option<AvifEncoderHints>,
    pub gif: Option<GifEncoderHints>,
}

// #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
// #[serde(rename_all = "lowercase")]
// pub struct JxlEncoderHints{
//     pub quality: Option<f32>,
//     pub lossless: Option<bool>,
//     //pub effort: Option<u8>,
//     pub distance: Option<f32>,
// }

// #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
// #[serde(rename_all = "lowercase")]
// pub struct AvifEncoderHints{
//     pub quality: Option<f32>,
//     pub speed: Option<u8>,
// }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub struct GifEncoderHints{

}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub struct WebpEncoderHints{
    pub quality: Option<f32>,
    pub lossless: Option<BoolKeep>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub struct JpegEncoderHints{
    pub quality: Option<f32>,
    pub progressive: Option<bool>, // Default to allow {jpeg_progressive}
    pub mimic: Option<JpegEncoderStyle>,
    // mozjpeg always optimizes huffman
    // And we don't use libjpeg turbo anymore
    // We don't allow custom subsampling do we?
    //pub hint_optimize_huffman_coding: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum JpegEncoderStyle{
    Jpegli,
    LibjpegTurbo,
    Mozjpeg,
    /// Default is mozjpeg now, might be jpegli later
    Default,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub struct PngEncoderHints{
    pub quality: Option<f32>,
    pub min_quality: Option<f32>,
    pub quantization_speed: Option<u8>,
    pub mimic: Option<PngEncoderStyle>,
    pub hint_max_deflate: Option<bool>,
    pub lossless: Option<BoolKeep>,
    // We are dropping libpng, thus dropping zlib_compression: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum PngEncoderStyle{
    Libpng,
    Lodepng,
    Pngquant,
    Default,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub struct AllowedFormats{
    pub webp: Option<bool>,
    pub jxl: Option<bool>,
    //pub jxl_animated: Option<bool>,
    pub avif: Option<bool>,
    pub jpeg: Option<bool>,
    pub jpeg_progressive: Option<bool>,
    pub jpeg_xyb: Option<bool>,
    pub png: Option<bool>,
    // pub png_animated: Option<bool>,
    pub gif: Option<bool>,
    pub all: Option<bool>,
    /// Enables all formats that are 'safe' to display for all browsers.
    /// Enables jpeg, gif, png.
    /// WebP is at 97%, but is still excluded.
    pub web_safe: Option<bool>,
    /// Enables jpeg, gif, png, webp, color_profiles
    pub modern_web_safe: Option<bool>,
    /// Allows use of color profiles other than sRGB. This is now widely supported.
    pub color_profiles: Option<bool>,
}

impl AllowedFormats{
    pub fn expand_sets(self) -> AllowedFormats{
        if self.all == Some(true) {
            return AllowedFormats::all();
        }
        if self.web_safe == Some(true) {
            return self.merge(AllowedFormats::web_safe());
        }
        if self.modern_web_safe == Some(true) {
            return self.merge(AllowedFormats::modern_web_safe());
        }
        self
    }
    fn and(a: Option<bool>, b: Option<bool>) -> Option<bool>{
        match (a, b) {
            (Some(a), Some(b)) => Some(a && b),
            _ => None,
        }
    }
    fn or(a: Option<bool>, b: Option<bool>) -> Option<bool>{
        match (a, b) {
            (Some(a), Some(b)) => Some(a || b),
            _ => None,
        }
    }
    pub fn merge(self, other: AllowedFormats) -> Self{
        AllowedFormats{
            webp: Self::or(self.webp, other.webp),
            jxl: Self::or(self.jxl, other.jxl),
            avif: Self::or(self.avif, other.avif),
            jpeg: Self::or(self.jpeg, other.jpeg),
            jpeg_xyb: Self::or(self.jpeg_xyb, other.jpeg_xyb),
            jpeg_progressive: Self::or(self.jpeg_progressive, other.jpeg_progressive),
            png: Self::or(self.png, other.png),
            gif: Self::or(self.gif, other.gif),
            all: Self::or(self.all, other.all),
            web_safe: Self::or(self.web_safe, other.web_safe),
            modern_web_safe: Self::or(self.modern_web_safe, other.modern_web_safe),
            color_profiles: Self::or(self.color_profiles, other.color_profiles),
        }
    }
    pub fn intersect(self, other: AllowedFormats) -> Self{
        AllowedFormats{
            webp: Self::and(self.webp, other.webp),
            jxl: Self::and(self.jxl, other.jxl),
            avif: Self::and(self.avif, other.avif),
            jpeg: Self::and(self.jpeg, other.jpeg),
            jpeg_xyb: Self::and(self.jpeg_xyb, other.jpeg_xyb),
            jpeg_progressive: Self::and(self.jpeg_progressive, other.jpeg_progressive),
            png: Self::and(self.png, other.png),
            gif: Self::and(self.gif, other.gif),
            all: None,
            web_safe: None,
            modern_web_safe: None,
            color_profiles: Self::and(self.color_profiles, other.color_profiles),
        }
    }
    pub fn any_formats_enabled(&self) -> bool{
        self.webp == Some(true) || self.jxl == Some(true) || self.avif == Some(true) || self.jpeg == Some(true)  || self.png == Some(true) || self.gif == Some(true)
    }
    pub fn none() -> Self{
        AllowedFormats{
            webp: None,
            jxl: None,
            avif: None,
            jpeg: None,
            jpeg_progressive: None,
            jpeg_xyb: None,
            png: None,
            gif: None,
            all: None,
            web_safe: None,
            modern_web_safe: None,
            color_profiles: None,
        }
    }
    pub fn all() -> Self{
        AllowedFormats{
            webp: Some(true),
            avif: Some(true),
            jpeg: Some(true),
            png: Some(true),
            gif: Some(true),
            all: Some(true),
            web_safe: Some(true),
            modern_web_safe: Some(true),
            color_profiles: Some(true),
            jpeg_progressive: Some(true),
            jxl: Some(true),
            jpeg_xyb: Some(true),
        }
    }
    pub fn modern_web_safe() -> Self{
        AllowedFormats{
            webp: Some(true),
            modern_web_safe: Some(true),
            color_profiles: Some(true),
            jpeg_progressive: Some(true),
            jpeg_xyb: Some(true),
            ..AllowedFormats::web_safe()
        }
    }
    pub fn web_safe() -> Self{
        AllowedFormats{
            jpeg: Some(true),
            png: Some(true),
            gif: Some(true),
            web_safe: Some(true),
            ..AllowedFormats::none()
        }
    }
    pub fn lossless() -> Self{
        AllowedFormats{
            webp: Some(true),
            png: Some(true),
            jxl: Some(true),
            color_profiles: Some(true),
            ..AllowedFormats::none()
        }
    }
    pub fn lossy() -> Self{
        AllowedFormats::all() // Every format, even png, can be lossy
    }
    pub fn png() -> Self{
        AllowedFormats{
            png: Some(true),
            ..AllowedFormats::none()
        }
    }
    pub fn jpeg(allow_progressive: Option<bool>, allow_xyb: Option<bool>) -> Self{
        AllowedFormats{
            jpeg: Some(true),
            jpeg_progressive: allow_progressive,
            jpeg_xyb: allow_xyb,
            ..AllowedFormats::none()
        }
    }
    pub fn gif() -> Self{
        AllowedFormats{
            gif: Some(true),
            ..AllowedFormats::none()
        }
    }
    pub fn webp() -> Self{
        AllowedFormats{
            webp: Some(true),
            color_profiles: Some(true),
            ..AllowedFormats::none()
        }
    }
    pub fn jxl() -> Self{
        AllowedFormats{
            jxl: Some(true),
            color_profiles: Some(true),
            ..AllowedFormats::none()
        }
    }
    pub fn avif() -> Self{
        AllowedFormats{
            avif: Some(true),
            color_profiles: Some(true),
            ..AllowedFormats::none()
        }
    }

    pub fn set_color_profiles(mut self, value: bool) -> Self{
        self.color_profiles = Some(value);
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum BoolKeep{
    Keep,
    True,
    False
}
impl std::fmt::Display for BoolKeep{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoolKeep::Keep => write!(f, "keep"),
            BoolKeep::True => write!(f, "true"),
            BoolKeep::False => write!(f, "false"),
        }
    }
}
//impl bool to BoolKeep
impl From<bool> for BoolKeep{
    fn from(value: bool) -> Self {
        if value { BoolKeep::True } else { BoolKeep::False }
    }
}
impl BoolKeep{
    pub fn resolve(self, default: bool) -> bool{
        match self{
            BoolKeep::Keep => default,
            BoolKeep::True => true,
            BoolKeep::False => false,
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum QualityProfile{
    Lowest,
    Low,
    MediumLow,
    Medium,
    Good, //TODO rename to MediumHigh?
    High,
    Highest,
    Lossless,
    Percent(f32)
}

impl fmt::Display for QualityProfile{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self{
            QualityProfile::Lowest => write!(f, "lowest"),
            QualityProfile::Low => write!(f, "low"),
            QualityProfile::MediumLow => write!(f, "medium-low"),
            QualityProfile::Medium => write!(f, "medium"),
            QualityProfile::Good => write!(f, "good"),
            QualityProfile::High => write!(f, "high"),
            QualityProfile::Highest => write!(f, "highest"),
            QualityProfile::Lossless => write!(f, "lossless"),
            QualityProfile::Percent(v) => write!(f, "{:.0}", v)
        }
    }
}

impl QualityProfile{

    /// Returns the quality profile as a string, or None if it is not a valid quality profile
    pub fn from_str(text: &str) -> Option<QualityProfile> {

        match text{
            _ if text.eq_ignore_ascii_case("lowest") => Some(QualityProfile::Lowest),
            _ if text.eq_ignore_ascii_case("low") => Some(QualityProfile::Low),
            _ if text.eq_ignore_ascii_case("medium-low") => Some(QualityProfile::MediumLow),
            _ if text.eq_ignore_ascii_case("mediumlow") => Some(QualityProfile::MediumLow),
            _ if text.eq_ignore_ascii_case("medium") => Some(QualityProfile::Medium),
            _ if text.eq_ignore_ascii_case("good") => Some(QualityProfile::Good),
            _ if text.eq_ignore_ascii_case("medium-high") => Some(QualityProfile::Good),
            _ if text.eq_ignore_ascii_case("mediumhigh") => Some(QualityProfile::Good),
            _ if text.eq_ignore_ascii_case("high") => Some(QualityProfile::High),
            _ if text.eq_ignore_ascii_case("highest") => Some(QualityProfile::Highest),
            _ if text.eq_ignore_ascii_case("lossless") => Some(QualityProfile::Lossless),
            v =>{
                if let Ok(v) = v.parse::<f32>() {
                    return Some(QualityProfile::Percent(f32::min(100.0,f32::max(0.0,v))))
                }
                None
            }
        }
    }

    // Const error string
    pub const HELP_TEXT: &'static str = "Quality profile (qp) must be a number (0..100) or one of the following: lowest, low, med, medium, good, high, highest, lossless";

}

/// Encoder presets (each with optional configuration). These are exposed by the JSON API.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum EncoderPreset {
    /// Requires a quality profile to be specified
    /// Specify "allow" to enable specific formats (like webp, avif, jxl) and features
    /// (like jpeg_progressive, jpeg_xyb, color_profiles), (or sets thereof, like web_safe and modern_web_safe).
    Auto{
        /// A quality profile to use. Will provide default 'allowed' formats, but 'allowed' takes priority.
        quality_profile: QualityProfile,
        /// Adjusts the quality profile, assuming a 150ppi display and 3x CSS pixel ratio. 3 is the default.
        /// lower values will increase quality, higher values will decrease quality.
        /// Useful when not using srcset/picture, just img src. Ex. <img width=400 src="img.jpg?srcset=qp-dpr-2,800w" />
        quality_profile_dpr: Option<f32>,
        /// Applies a matte - background color to the image to eliminate transparency.
        matte: Option<Color>,
        /// Whether to use, disable, or keep lossless encoding.
        lossless: Option<BoolKeep>,
        // Which formats and features can be used
        allow: Option<AllowedFormats>,
        // max_effort, or budget, someday
    },
    /// Requires a file format to be specified, and allows for specific encoder hints.
    /// Specific format features can be specified in 'allow', such as jxl_animation, avif_animation, etc.
    Format{
        format: OutputImageFormat,
        /// A quality profile to use..
        quality_profile: Option<QualityProfile>,
        /// Adjusts the quality profile, assuming a 150ppi display and 3x CSS pixel ratio. 3 is the default.
        /// lower values will increase quality, higher values will decrease quality.
        /// Useful when not using srcset/picture, just img src. Ex. <img width=400 src="img.jpg?srcset=qp-dpr-2,800w" />
        quality_profile_dpr: Option<f32>,
        /// Applies a matte - background color to the image to eliminate transparency.
        matte: Option<Color>,
        /// Whether to use, disable, or keep lossless encoding.
        lossless: Option<BoolKeep>,
        // Which formats and features can be used
        allow: Option<AllowedFormats>,
        // max_effort, or budget, someday
        encoder_hints: Option<EncoderHints>,
    },
    LibjpegTurbo {
        quality: Option<i32>,
        progressive: Option<bool>,
        optimize_huffman_coding: Option<bool>,
        matte: Option<Color>
    },
    Libpng {
        depth: Option<PngBitDepth>,
        matte: Option<Color>,
        zlib_compression: Option<i32>,
    },
    Pngquant {
        quality: Option<u8>,
        minimum_quality: Option<u8>,
        speed: Option<u8>,
        maximum_deflate: Option<bool>
    },
    Lodepng {
        maximum_deflate: Option<bool>
    },
    Mozjpeg {
        quality: Option<u8>,
        progressive: Option<bool>,
        matte: Option<Color>
    },
    WebPLossy{
        quality: f32
    },
    WebPLossless,
    Gif,
}


impl EncoderPreset {
    pub fn libpng32() -> EncoderPreset {
        EncoderPreset::Libpng {
            depth: Some(PngBitDepth::Png32),
            matte: None,
            zlib_compression: None, // Use default
        }
    }
    pub fn libjpeg_turbo() -> EncoderPreset {
        EncoderPreset::LibjpegTurbo {
            quality: Some(100),
            optimize_huffman_coding: None,
            progressive: None,
            matte: None
        }
    }
    pub fn libjpeg_turbo_q(quality: Option<i32>) -> EncoderPreset {
        EncoderPreset::LibjpegTurbo {
            quality,
            optimize_huffman_coding: None,
            progressive: None,
            matte: None
        }
    }
}

/// Represents an sRGB color value.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ColorSrgb {
    /// Hex in RRGGBBAA (css) form or variant thereof
    #[serde(rename="hex")]
    Hex(String),
}

/// Represents arbitrary colors (not color space specific)
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum Color {
    #[serde(rename="transparent")]
    Transparent,
    #[serde(rename="black")]
    Black,
    #[serde(rename="srgb")]
    Srgb(ColorSrgb),
}

use imageflow_helpers::colors::*;
use rgb::alt::BGRA8;
impl Color {

    pub fn to_u32_bgra(&self) -> std::result::Result<u32, ParseColorError> {
        self.to_color_32().map(|c| c.to_bgra_le() )
    }

    pub fn to_bgra8(&self) -> std::result::Result<BGRA8, ParseColorError> {
        self.to_color_32().map(|c| c.to_bgra8())
    }

    pub fn to_u32_rgba_big_endian(&self) -> std::result::Result<u32, ParseColorError> {
        self.to_color_32().map(|c| c.to_abgr_le() )
    }

    /// Parse a Color into a 32-bit sRGBA value.
    pub fn to_color_32(&self) -> std::result::Result<Color32, ParseColorError> {
        match *self {
            Color::Srgb(ref srgb) => {
                match *srgb {
                    ColorSrgb::Hex(ref hex_srgb) => {
                        parse_color_hex(hex_srgb)
                    }
                }
            }
            Color::Black => Ok(Color32::black()),
            Color::Transparent => Ok(Color32::transparent_black()),
        }
    }

    pub fn is_transparent(&self) -> bool{
        self.to_color_32().unwrap_or(Color32::black()).is_transparent()
    }

    pub fn is_opaque(&self) -> bool{
        self.to_color_32().unwrap_or(Color32::black()).is_opaque()
    }
}

#[cfg(test)]
fn assert_eq_hex(a: u32, b: u32){
    if a != b{
        println!("{:08X} != {:08X} (expected)", a, b);
    }
    assert_eq!(a,b);
}
#[test]
fn test_color() {

    assert_eq_hex(Color::Srgb(ColorSrgb::Hex("FFAAEEDD".to_owned())).to_u32_rgba_big_endian().unwrap(),
               0xFFAAEEDD);
    assert_eq_hex(Color::Srgb(ColorSrgb::Hex("FFAAEE".to_owned())).to_u32_rgba_big_endian().unwrap(),
               0xFFAAEEFF);
}

#[test]
fn test_bgra() {

    assert_eq_hex(Color::Srgb(ColorSrgb::Hex("FFAAEEDD".to_owned())).to_color_32().unwrap().to_bgra_le(),
               0xDDFFAAEE);
    assert_eq_hex(Color::Srgb(ColorSrgb::Hex("FFAAEE".to_owned())).to_color_32().unwrap().to_bgra_le(),
               0xFFFFAAEE);
    assert_eq_hex(Color::Srgb(ColorSrgb::Hex("000000FF".to_owned())).to_color_32().unwrap().to_bgra_le(),
               0xFF000000);


}


#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ResampleWhen{
    #[serde(rename="size_differs")]
    SizeDiffers,
    #[serde(rename="size_differs_or_sharpening_requested")]
    SizeDiffersOrSharpeningRequested,
    #[serde(rename="always")]
    Always
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum SharpenWhen{
    #[serde(rename="downscaling")]
    Downscaling,
    #[serde(rename="upscaling")]
    Upscaling,
    #[serde(rename="size_differs")]
    SizeDiffers,
    #[serde(rename="always")]
    Always
}

#[derive(Serialize, Deserialize,  Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct ResampleHints {
    pub sharpen_percent: Option<f32>,
    pub down_filter: Option<Filter>,
    pub up_filter: Option<Filter>,
    pub scaling_colorspace: Option<ScalingFloatspace>,
    pub background_color: Option<Color>,
    pub resample_when: Option<ResampleWhen>,
    pub sharpen_when: Option<SharpenWhen>
}

impl ResampleHints {
    pub fn new() -> ResampleHints {
        ResampleHints {
            sharpen_percent: None,
            down_filter: None,
            up_filter: None,
            scaling_colorspace: None,
            background_color: None,
            resample_when: None,
            sharpen_when: None
        }
    }
    pub fn with_bi_filter(self, filter: Filter) -> ResampleHints {
        ResampleHints {
            down_filter: Some(filter),
            up_filter: Some(filter),
            .. self
        }
    }
    pub fn with_floatspace(self, space: ScalingFloatspace) -> ResampleHints {
        ResampleHints {
            scaling_colorspace: Some(space),
            .. self
        }
    }

    pub fn with(filter: Option<Filter>, sharpen_percent: Option<f32>) -> ResampleHints {
        ResampleHints {
            sharpen_percent,
            down_filter: filter,
            up_filter: filter,
            resample_when: None,
            scaling_colorspace: None,
            background_color: None,
            sharpen_when: None
        }
    }

}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum CommandStringKind{
    #[serde(rename="ir4")]
    ImageResizer4
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ConstraintMode {
    /// Distort the image to exactly the given dimensions.
    /// If only one dimension is specified, behaves like `fit`.
    #[serde(rename = "distort")]
    Distort,
    /// Ensure the result fits within the provided dimensions. No upscaling.
    #[serde(rename = "within")]
    Within,
    /// Fit the image within the dimensions, upscaling if needed
    #[serde(rename = "fit")]
    Fit,
    /// Ensure the image is larger than the given dimensions
    #[serde(rename = "larger_than")]
    LargerThan,
    /// Crop to desired aspect ratio if image is larger than requested, then downscale. Ignores smaller images.
    /// If only one dimension is specified, behaves like `within`.
    #[serde(rename = "within_crop")]
    WithinCrop,
    /// Crop to desired aspect ratio, then downscale or upscale to fit.
    /// If only one dimension is specified, behaves like `fit`.
    #[serde(rename = "fit_crop")]
    FitCrop,
    /// Crop to desired aspect ratio, no upscaling or downscaling. If only one dimension is specified, behaves like Fit.
    #[serde(rename = "aspect_crop")]
    AspectCrop,
    /// Pad to desired aspect ratio if image is larger than requested, then downscale. Ignores smaller images.
    /// If only one dimension is specified, behaves like `within`
    #[serde(rename = "within_pad")]
    WithinPad,
    /// Pad to desired aspect ratio, then downscale or upscale to fit
    /// If only one dimension is specified, behaves like `fit`.
    #[serde(rename = "fit_pad")]
    FitPad,
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum WatermarkConstraintMode {
    /// Distort the image to exactly the given dimensions.
    /// If only one dimension is specified, behaves like `fit`.
    #[serde(rename = "distort")]
    Distort,
    /// Ensure the result fits within the provided dimensions. No upscaling.
    #[serde(rename = "within")]
    Within,
    /// Fit the image within the dimensions, upscaling if needed
    #[serde(rename = "fit")]
    Fit,
    /// Crop to desired aspect ratio if image is larger than requested, then downscale. Ignores smaller images.
    /// If only one dimension is specified, behaves like `within`.
    #[serde(rename = "within_crop")]
    WithinCrop,
    /// Crop to desired aspect ratio, then downscale or upscale to fit.
    /// If only one dimension is specified, behaves like `fit`.
    #[serde(rename = "fit_crop")]
    FitCrop,
}
impl From<WatermarkConstraintMode> for ConstraintMode{
    fn from(mode: WatermarkConstraintMode) -> Self {
        match mode{
            WatermarkConstraintMode::Distort => ConstraintMode::Distort,
            WatermarkConstraintMode::Within => ConstraintMode::Within,
            WatermarkConstraintMode::Fit => ConstraintMode::Fit,
            WatermarkConstraintMode::WithinCrop => ConstraintMode::WithinCrop,
            WatermarkConstraintMode::FitCrop => ConstraintMode::FitCrop,
        }
    }
}



#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ConstraintGravity {
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "percentage")]
    Percentage{x: f32, y: f32}
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Constraint {
    pub mode: ConstraintMode,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub hints: Option<ResampleHints>,
    pub gravity: Option<ConstraintGravity>,
    pub canvas_color: Option<Color>
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum WatermarkConstraintBox{
    #[serde(rename = "image_percentage")]
    ImagePercentage{ x1: f32, y1: f32, x2: f32, y2: f32},
    #[serde(rename = "image_margins")]
    ImageMargins{ left: u32, top: u32, right: u32, bottom: u32},
    #[serde(rename = "canvas_percentage")]
    CanvasPercentage{ x1: f32, y1: f32, x2: f32, y2: f32},
    #[serde(rename = "canvas_margins")]
    CanvasMargins{ left: u32, top: u32, right: u32, bottom: u32},

}


#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Watermark{
    pub io_id: i32,
    pub fit_box: Option<WatermarkConstraintBox>,
    pub fit_mode: Option<WatermarkConstraintMode>,
    pub gravity: Option<ConstraintGravity>,
    pub min_canvas_width: Option<u32>,
    pub min_canvas_height: Option<u32>,
    pub opacity: Option<f32>,
    pub hints: Option<ResampleHints>,
}

/// Blend pixels (if transparent) or replace?
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum CompositingMode {
    #[serde(rename="compose")]
    Compose,
    #[serde(rename="overwrite")]
    Overwrite
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct FrameSizeLimit{
    pub w: u32,
    pub h: u32,
    pub megapixels: f32
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct ExecutionSecurity{
    pub max_decode_size: Option<FrameSizeLimit>,
    pub max_frame_size: Option<FrameSizeLimit>,
    pub max_encode_size: Option<FrameSizeLimit>
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum RoundCornersMode {
    #[serde(rename = "percentage")]
    Percentage(f32),
    #[serde(rename = "pixels")]
    Pixels(f32),
    #[serde(rename = "circle")]
    Circle,
    #[serde(rename = "percentage_custom")]
    PercentageCustom{top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32 },
    #[serde(rename = "pixels_custom")]
    PixelsCustom{top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32 },
}

/// Represents a image operation. Currently used both externally (for JSON API) and internally.
/// The most important data type
#[allow(unreachable_patterns)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum Node {
    #[serde(rename="flip_v")]
    FlipV,
    #[serde(rename="flip_h")]
    FlipH,
    #[serde(rename="crop")]
    Crop { x1: u32, y1: u32, x2: u32, y2: u32 },
    #[serde(rename="crop_whitespace")]
    CropWhitespace { threshold: u32, percent_padding: f32 },

    #[serde(rename="create_canvas")]
    CreateCanvas {
        format: PixelFormat,
        w: usize,
        h: usize,
        color: Color,
    },
    #[serde(rename="command_string")]
    CommandString{
        kind: CommandStringKind,
        value: String,
        decode: Option<i32>,
        encode: Option<i32>,
        watermarks: Option<Vec<Watermark>>
    },
    #[serde(rename="constrain")]
    Constrain(Constraint),
    #[serde(rename="copy_rect_to_canvas")]
    CopyRectToCanvas {
        from_x: u32,
        from_y: u32,
        w: u32,
        h: u32,
        x: u32,
        y: u32,
    },
    #[serde(rename="round_image_corners")]
    RoundImageCorners {
        radius: RoundCornersMode,
        background_color: Color
    },
    #[serde(rename="decode")]
    Decode {
        io_id: i32,
        commands: Option<Vec<DecoderCommand>>,
    },
    #[serde(rename="encode")]
    Encode {
        io_id: i32,
        preset: EncoderPreset,
    },
    #[serde(rename="fill_rect")]
    FillRect {
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        color: Color,
    },
    #[serde(rename="expand_canvas")]
    ExpandCanvas {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
        color: Color,
    },
    #[serde(rename="region_percent")]
    RegionPercent {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        background_color: Color,
    },
    #[serde(rename="region")]
    Region {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        background_color: Color,
    },
    #[serde(rename="transpose")]
    Transpose,
    #[serde(rename="rotate_90")]
    Rotate90,
    #[serde(rename="rotate_180")]
    Rotate180,
    #[serde(rename="rotate_270")]
    Rotate270,
    #[serde(rename="apply_orientation")]
    ApplyOrientation { flag: i32 },
    #[serde(rename="resample_2d")]
    Resample2D {
        w: u32,
        h: u32,
        hints: Option<ResampleHints>,
    },
    #[serde(rename="draw_image_exact")]
    DrawImageExact {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        blend: Option<CompositingMode>,
        hints: Option<ResampleHints>,
    },
//    #[serde(rename="resample_1d")]
//    Resample1D {
//        scale_to_width: u32,
//        transpose_on_write: bool,
//        interpolation_filter: Option<Filter>,
//        scaling_colorspace: Option<ScalingFloatspace>,
//    },
    #[serde(rename="watermark")]
    Watermark (Watermark),
    #[serde(rename="watermark_red_dot")]
    WatermarkRedDot,
    #[serde(rename="white_balance_histogram_area_threshold_srgb")]
    WhiteBalanceHistogramAreaThresholdSrgb{
        threshold: Option<f32>
    },
    #[serde(rename="color_matrix_srgb")]
    ColorMatrixSrgb{
        matrix: [[f32;5];5]
    },
    #[serde(rename="color_filter_srgb")]
    ColorFilterSrgb (ColorFilterSrgb),
    // TODO: Block use except from FFI/unit test use
    #[serde(rename="flow_bitmap_key_ptr")]
    FlowBitmapKeyPtr {
        //TODO: Rename this
        ptr_to_bitmap_key: usize,
    },
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ColorFilterSrgb {
    #[serde(rename="grayscale_ntsc")]
    GrayscaleNtsc,
    #[serde(rename="grayscale_flat")]
    GrayscaleFlat,
    #[serde(rename="grayscale_bt709")]
    GrayscaleBt709,
    #[serde(rename="grayscale_ry")]
    GrayscaleRy,
    #[serde(rename="sepia")]
    Sepia,
    #[serde(rename="invert")]
    Invert,
//    #[serde(rename="color_shift")]
//    ColorShift(Color),
    #[serde(rename="alpha")]
    Alpha(f32),
    #[serde(rename="contrast")]
    Contrast(f32),
    #[serde(rename="brightness")]
    Brightness(f32),
    #[serde(rename="saturation")]
    Saturation(f32),
}

/// Operation nodes are connected by edges. Many operations require both an input and a canvas node.
///
/// In the future, some operations may have multiple inputs, and new edge types may be introduced for non-bitmap data.
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum EdgeKind {
    #[serde(rename="input")]
    Input,
    #[serde(rename="canvas")]
    Canvas,
}

/// Operation nodes are connected by edges. JSON only.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Edge {
    pub from: i32,
    pub to: i32,
    pub kind: EdgeKind,
}


/// An operation graph; should be directed and acyclic. JSON only.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Graph {
    pub nodes: std::collections::HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

/// We must mark IO objects as data sources or data destinations.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[repr(C)]
pub enum IoDirection {
    #[serde(rename="out")]
    Out = 8,
    #[serde(rename="in")]
    In = 4,
}

/// Describes (or contains) a data source or destination
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum IoEnum {
    #[serde(rename="bytes_hex")]
    BytesHex(String),
    #[serde(rename="base_64")]
    Base64(String),
    #[serde(rename="byte_array")]
    ByteArray(Vec<u8>),
    // TODO: A PathBuf might be more appropriate
    #[serde(rename="file")]
    Filename(String),
    #[serde(rename="output_buffer")]
    OutputBuffer,
    #[serde(rename="output_base_64")]
    OutputBase64,
    /// To be replaced before execution
    #[serde(rename="placeholder")]
    Placeholder
}

impl IoEnum{
    pub fn into_input(self, io_id: i32) -> IoObject{
        IoObject{
            io_id,
            direction: IoDirection::In,
            io: self
        }
    }
    pub fn into_output(self, io_id: i32) -> IoObject{
        IoObject{
            io_id,
            direction: IoDirection::Out,
            io: self
        }
    }
}

/// Data source or destination (including IO ID).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct IoObject {
    pub io_id: i32,
    pub direction: IoDirection,
    pub io: IoEnum,
}

/// Represents an operation graph or series (series is simpler to think about and suitable for most tasks).
/// Operation graphs *may* be applied to each frame in the source data - thus 'Framewise'.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum Framewise {
    #[serde(rename="graph")]
    Graph(Graph),
    #[serde(rename="steps")]
    Steps(Vec<Node>),
}

impl Framewise {
    pub fn clone_nodes(&self) -> Vec<&Node> {
        match *self {
            Framewise::Graph(ref graph) => graph.nodes.values().collect::<Vec<&Node>>(),
            Framewise::Steps(ref nodes) => nodes.iter().collect::<Vec<&Node>>(),
        }
    }

    fn io_ids_and_directions(&self) -> Vec<(i32, IoDirection)>{
        let mut vec = self.clone_nodes().into_iter().map(|n|{
            match *n{
                Node::Decode{io_id, ..} => Some((io_id, IoDirection::In)),
                Node::Encode{io_id, ..} => Some((io_id, IoDirection::Out)),
                _ => None
            }
        }).filter(|v| v.is_some()).map(|v| v.unwrap()).collect::<Vec<(i32, IoDirection)>>();
        vec.sort_by(|&(a,_), &(b,_)| a.cmp(&b));
        vec
    }

    pub fn wrap_in_build_0_1(self) -> Build001{
        let io_vec = self.io_ids_and_directions().into_iter().map(|(id, dir)|
            IoObject{
                direction: dir,
                io_id: id,
                io: IoEnum::Placeholder
            }
        ).collect::<Vec<IoObject>>();
        Build001 {
            builder_config: None,
            framewise: self,
            io: io_vec,
        }
    }
}

/// TODO: clean up!
/// Contains flags that instruct how job execution is recorded during execution.
/// v0.0.1
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Build001GraphRecording {
    pub record_graph_versions: Option<bool>,
    pub record_frame_images: Option<bool>,
    pub render_last_graph: Option<bool>,
    pub render_graph_versions: Option<bool>,
    pub render_animated_graph: Option<bool>,
}

impl Build001GraphRecording {
    pub fn debug_defaults() -> Build001GraphRecording {
        Build001GraphRecording {
            record_graph_versions: Some(true),
            record_frame_images: Some(true),
            render_last_graph: Some(true),
            render_animated_graph: Some(false),
            render_graph_versions: Some(false),
        }
    }
    pub fn off() -> Build001GraphRecording {
        Build001GraphRecording {
            record_graph_versions: Some(false),
            record_frame_images: Some(false),
            render_last_graph: Some(false),
            render_animated_graph: Some(false),
            render_graph_versions: Some(false),
        }
    }
}


#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Build001Config {
    // pub process_all_gif_frames: Option<bool>,
    pub graph_recording: Option<Build001GraphRecording>,
    pub security: Option<ExecutionSecurity>,
}

/// Represents a complete build job, combining IO objects with a framewise operation graph.
/// TODO: cleanup builder_config.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Build001 {
    pub builder_config: Option<Build001Config>,
    pub io: Vec<IoObject>,
    pub framewise: Framewise,
}

impl Build001{


    /// Replaces the specified IO object by io_id.  Panics if no such io_id found
    pub fn replace_io(self, io_id: i32, value: IoEnum) -> Build001{
        let value_ref = &value;
        let new_io_vec = self.io.into_iter().map(|obj| {
            if obj.io_id == io_id {
                IoObject { direction: obj.direction, io_id: io_id, io: value_ref.to_owned() }
            }else {obj}
        }).collect::<Vec<IoObject>>();
        if !new_io_vec.as_slice().iter().any(|obj| obj.io_id == io_id){
            panic!("No existing IoObject with io_id {} found to replace!",io_id);
        }
        Build001{
            builder_config: self.builder_config,
            io: new_io_vec,
            framewise: self.framewise
        }
    }

}


////////////// Examples

impl IoEnum {
    pub fn example_byte_array() -> IoEnum {
        let tiny_png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
                           0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
                           0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
                           0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
                           0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
                           0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
        IoEnum::ByteArray(tiny_png)
    }
    pub fn example_byte_array_truncated() -> IoEnum {
        IoEnum::ByteArray(vec![0x89, 0x50, 0x4E, 0x47])
    }
    pub fn example_bytes_hex() -> IoEnum {
        IoEnum::BytesHex("89504E470D0A1A0A0000000D49484452000000010000000108060000001F15C4890000000A49444154789C63000100000500010D0A2DB40000000049454E44AE426082".to_owned())
    }
    pub fn example_base64() -> IoEnum {
        IoEnum::Base64("iVBORw0KGgoAAAANSUhEUgAAAMgAAADICAYAAACtWK6eAAABiUlEQVR42u3TgRAAQAgAsA/qkaKLK48EIug2h8XP6gesQhAQBAQBQUAQEAQEAUFAEBAEEAQEAUFAEBAEBAFBQBAQBAQRBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQBBQBAQBAQBQUAQEAQEAUEAQUAQEAQEAUFAEBAEBAFBQBBAEBAEBAFBQBAQBAQBQUAQQBAQBAQBQUAQEAQEAUFAEBAEEAQEAUFAEBAEBAFBQBAQBAQRBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQQBQUAQEAQEAUFAEBAEBAFBAEFAEBAEBAFBQBAQBAQBQUAQQBAQBAQBQUAQEAQEAUFAEEAQEAQEAUFAEBAEBAFBQBAQBBAEBAFBQBAQBAQBQUAQEAQQBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQQBQUAQEAQEAUFAEBAEBAFBAEFAEBAEBAFBQBAQBAQBQUAQQUAQEAQEAUFAEBAEBIGLBkZ+sahOjkyUAAAAAElFTkSuQmCC".to_owned())
    }
}

impl Build001 {
    pub fn example_with_steps() -> Build001 {
        Build001 {
            builder_config: None,
            io: vec![
            IoObject {direction: IoDirection::In,
                io_id: 0,
                io: IoEnum::Placeholder
            },
            IoObject {

                direction: IoDirection::In,
                io_id: 92,
                io: IoEnum::example_base64(),
            },
            IoObject {
                io: IoEnum::Filename("output.png".to_owned()),
                io_id: 1,

                direction: IoDirection::Out
            },
            IoObject {
                io: IoEnum::OutputBuffer,
                io_id: 2,

                direction: IoDirection::Out
            },
            IoObject {
                io: IoEnum::OutputBase64,
                io_id: 3,

                direction: IoDirection::Out
            }
            ],
            framewise: Framewise::example_graph()
        }
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Execute001 {
    pub graph_recording: Option<Build001GraphRecording>,
    pub security: Option<ExecutionSecurity>,
    pub framewise: Framewise,
}

impl Framewise {
    pub fn example_steps() -> Framewise {
        Framewise::Steps(vec![Node::Decode {
                                  io_id: 0,
                                  commands: Some(vec![
                DecoderCommand::JpegDownscaleHints(JpegIDCTDownscaleHints{
                    width: 800 , height: 600,
                    gamma_correct_for_srgb_during_spatial_luma_scaling: Some(false),
                    scale_luma_spatially: Some(false)})]),
                              },
                              Node::ApplyOrientation { flag: 7 },
                              Node::ExpandCanvas {
                                  left: 10,
                                  top: 10,
                                  right: 10,
                                  bottom: 10,
                                  color: Color::Srgb(ColorSrgb::Hex("FFEECCFF".to_owned())),
                              },
                              Node::Crop {
                                  x1: 10,
                                  y1: 10,
                                  x2: 650,
                                  y2: 490,
                              },
                              Node::FillRect {
                                  x1: 0,
                                  y1: 0,
                                  x2: 8,
                                  y2: 8,
                                  color: Color::Transparent,
                              },
                              Node::FlipV,
                              Node::FlipH,
                              Node::Rotate90,
                              Node::Rotate180,
                              Node::Rotate270,
                              Node::Transpose,
                              Node::Resample2D {
                                  w: 100,
                                  h: 75,
                                  hints: Some(ResampleHints {
                                      sharpen_percent: Some(10f32),
                                      down_filter: Some(Filter::Robidoux),
                                      up_filter: Some(Filter::Ginseng),
                                      scaling_colorspace: Some(ScalingFloatspace::Linear),
                                      background_color: Some(Color::Srgb(ColorSrgb::Hex("FFEEAACC".to_owned()))),
                                      //prefer_1d_twice: None,
                                      resample_when: Some(ResampleWhen::SizeDiffersOrSharpeningRequested),
                                      sharpen_when: Some(SharpenWhen::Downscaling)
                                  }),
                              },
                              Node::Resample2D {
                                  w: 200,
                                  h: 150,
                                  hints: Some(ResampleHints {
                                      sharpen_percent: None,
                                      down_filter: None,
                                      up_filter: None,
                                      scaling_colorspace: Some(ScalingFloatspace::Srgb),
                                      background_color: None,
                                      resample_when: None,
                                      sharpen_when: None
                                  }),
                              },
                              Node::Encode {
                                  io_id: 1,
                                  preset: EncoderPreset::LibjpegTurbo { quality: Some(90), optimize_huffman_coding: Some(true), progressive: Some(true), matte: None},
                              }])
    }
    pub fn example_graph() -> Framewise {

        let mut nodes = std::collections::HashMap::new();
        nodes.insert("0".to_owned(),
                     Node::Decode {
                         io_id: 0,
                         commands: None,
                     });
        nodes.insert("1".to_owned(),
                     Node::CreateCanvas {
                         w: 200,
                         h: 200,
                         format: PixelFormat::Bgr32,
                         color: Color::Transparent,
                     });
        nodes.insert("2".to_owned(),
                     Node::CopyRectToCanvas {
                         x: 0,
                         y: 0,
                         from_x: 0,
                         from_y: 0,
                         w: 100,
                         h: 100,
                     });
        nodes.insert("3".to_owned(),
                     Node::Resample2D {
                         w: 100,
                         h: 100,
                         hints: Some(ResampleHints {
                             sharpen_percent: Some(10f32),
                             down_filter: Some(Filter::Robidoux),
                             up_filter: Some(Filter::Ginseng),
                             scaling_colorspace: Some(ScalingFloatspace::Linear),
                             background_color: Some(Color::Srgb(ColorSrgb::Hex("FFEEAACC".to_owned()))),
                             //prefer_1d_twice: None,
                             resample_when: Some(ResampleWhen::SizeDiffersOrSharpeningRequested),
                             sharpen_when: Some(SharpenWhen::Downscaling)
                         }),
                     });
        nodes.insert("4".to_owned(),
                     Node::Encode {
                         io_id: 1,
                         preset: EncoderPreset::Libpng {
                             matte: Some(Color::Srgb(ColorSrgb::Hex("999999".to_owned()))),
                             zlib_compression: None,
                             depth: Some(PngBitDepth::Png24),
                         },
                     });
        nodes.insert("5".to_owned(),
                     Node::Encode {
                         io_id: 2,
                         preset: EncoderPreset::LibjpegTurbo { quality: Some(90), optimize_huffman_coding: Some(true), progressive: Some(true), matte: None},
                     });

        Framewise::Graph(Graph {
            edges: vec![Edge {
                            from: 0,
                            to: 2,
                            kind: EdgeKind::Input,
                        },
                        Edge {
                            from: 1,
                            to: 2,
                            kind: EdgeKind::Canvas,
                        },
                        Edge {
                            from: 2,
                            to: 3,
                            kind: EdgeKind::Input,
                        },
                        Edge {
                            from: 3,
                            to: 4,
                            kind: EdgeKind::Input,
                        },
                        Edge {
                            from: 3,
                            to: 5,
                            kind: EdgeKind::Input,
                        }],
            nodes,
        })
    }
}
impl Execute001 {
    pub fn example_steps() -> Execute001 {
        Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::example_steps(),
        }
    }
    pub fn example_graph() -> Execute001 {
        Execute001 {
            graph_recording: None,
            security: None,
            framewise: Framewise::example_graph(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct GetImageInfo001 {
    pub io_id: i32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct GetVersionInfo{

}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct GetQueryStringSchema{

}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct ListQueryStringKeys{

}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct ValidateQueryString{
    pub query_string: String,
}

impl GetImageInfo001 {
    pub fn example_get_image_info() -> GetImageInfo001 {
        GetImageInfo001 { io_id: 0 }
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct JpegIDCTDownscaleHints {
    pub width: i64,
    pub height: i64,
    pub scale_luma_spatially: Option<bool>,
    pub gamma_correct_for_srgb_during_spatial_luma_scaling: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct WebPDecoderHints {
    pub width: i32,
    pub height: i32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum DecoderCommand {
    #[serde(rename="jpeg_downscale_hints")]
    JpegDownscaleHints(JpegIDCTDownscaleHints),
    #[serde(rename="webp_decoder_hints")]
    WebPDecoderHints(WebPDecoderHints),
    #[serde(rename="discard_color_profile")]
    DiscardColorProfile,
    #[serde(rename="ignore_color_profile_errors")]
    IgnoreColorProfileErrors
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct TellDecoder001 {
    pub io_id: i32,
    pub command: DecoderCommand,
}

impl TellDecoder001 {
    pub fn example_hints() -> TellDecoder001 {
        TellDecoder001 {
            io_id: 2,
            command: DecoderCommand::JpegDownscaleHints(JpegIDCTDownscaleHints {
                width: 1000,
                height: 1000,
                scale_luma_spatially: Some(true),
                gamma_correct_for_srgb_during_spatial_luma_scaling: Some(true),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct ImageInfo {
    pub preferred_mime_type: String,
    pub preferred_extension: String,
    // Warning, one cannot count frames in a GIF without scanning the whole thing.
//    pub frame_count: usize,
//    pub current_frame_index: i64,
    pub image_width: i32,
    pub image_height: i32,
    pub frame_decodes_into: PixelFormat,
    pub lossless: bool,
    pub multiple_frames: bool
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ResultBytes {
    #[serde(rename="base_64")]
    Base64(String),
    #[serde(rename="byte_array")]
    ByteArray(Vec<u8>),
    #[serde(rename="physical_file")]
    PhysicalFile(String),
    #[serde(rename="elsewhere")]
    Elsewhere,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct EncodeResult {
    pub preferred_mime_type: String,
    pub preferred_extension: String,

    pub io_id: i32,
    pub w: i32,
    pub h: i32,

    pub bytes: ResultBytes,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct DecodeResult {
    pub preferred_mime_type: String,
    pub preferred_extension: String,

    pub io_id: i32,
    pub w: i32,
    pub h: i32,
}



#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct NodePerf{
    pub wall_microseconds: u64,
    pub name: String
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct BuildPerformance{
    pub frames: Vec<FramePerformance>,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct FramePerformance{
    pub nodes: Vec<NodePerf>,
    pub wall_microseconds: u64,
    pub overhead_microseconds: i64
}

//pub struct JobDebugInfo{
//    pub final_graph: String
//
//}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct JobResult {
    pub encodes: Vec<EncodeResult>,
    pub decodes: Vec<DecodeResult>,
    pub performance: Option<BuildPerformance>
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct VersionInfo{
    pub long_version_string: String,
    pub last_git_commit: String,
    pub dirty_working_tree: bool,
    pub build_date: String,
    pub git_tag: Option<String>,
    pub git_describe_always: String
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum ResponsePayload {
    #[serde(rename="image_info")]
    ImageInfo(ImageInfo),
    #[serde(rename="job_result")]
    JobResult(JobResult),
    #[serde(rename="build_result")]
    BuildResult(JobResult),
    #[serde(rename="version_info")]
    VersionInfo(VersionInfo),
    #[serde(rename="query_string_schema")]
    QueryStringSchema(json_messages::QueryStringSchema),
    #[serde(rename="query_string_validation_results")]
    QueryStringValidationResults(json_messages::QueryStringValidationResults),
    #[serde(rename="none")]
    None,
}

/// Contains the types that are exclusively used in the JSON endpoints
/// To prevent name collisions with other types
pub mod json_messages{

    #[cfg(feature = "schema-export")]
    use utoipa::ToSchema;

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringValidationResults{
        pub issues: Vec<QueryStringValidationIssue>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringValidationIssue{
        pub message: String,
        pub key: String,
        pub value: String,
        pub kind: QueryStringValidationIssueKind,
    }
    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub enum QueryStringValidationIssueKind{
        DuplicateKeyError,
        UnrecognizedKey,
        IgnoredKey,
        InvalidValueError,
        DeprecatedValueWarning,
        DeprecatedKeyWarning,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringSchema{
        pub key_names: Vec<String>,
        // pub keys: Vec<QueryStringSchemaKey>,
        // pub groups: Vec<QueryStringSchemaKeyGroup>,
        // pub markdown_pages: Vec<QueryStringSchemaMarkdownPage>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringSchemaMarkdownPage{
        pub slug: String,
        pub title: String,
        pub markdown: String,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub enum QueryStringDescription{
        #[serde(rename="markdown")]
        Markdown(String),
        #[serde(rename="text")]
        Text(String),
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringSchemaKeyGroup{
        pub id: String,
        pub name: String,
        pub description: QueryStringDescription,
        pub generated_markdown: Option<String>, // gener
        pub keys: Vec<String>,
        pub examples: Option<Vec<QueryStringSchemaExample>>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringSchemaKey{
        pub key: String,
        pub aliases: Option<Vec<String>>,
        pub description: QueryStringDescription,
        pub ignored_reason: Option<String>,
        pub deprecation_message: Option<String>,
        pub interacts_with: Option<Vec<String>>,
        pub related_keys: Option<Vec<String>>,
        pub conflicts_with_keys: Option<Vec<String>>,
        pub allowed_values: Vec<QueryStringSchemaValue>,
        pub examples: Option<Vec<QueryStringSchemaExample>>,
        pub generated_markdown: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringSchemaValue{
        pub example_value: Option<String>,
        pub value_syntax: Option<String>,
        pub data_validation: Option<QueryStringSchemaValueValidation>,
        pub description: QueryStringDescription,
        pub is_default: Option<bool>,
        pub ignored_reason: Option<String>,
        pub deprecation_message: Option<String>,
        pub examples: Option<Vec<QueryStringSchemaExample>>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub enum QueryStringSchemaValueValidation{
        // float, integer (both with optionalrange)
        // enum of strings
        // boolean
        // list of floats, specific count, optional range
        #[serde(rename="enum")]
        Enum { options: Vec<String>, case_sensitive: Option<bool> },
        // list of floats, specific count, optional range
        #[serde(rename="numeric_list")]
        NumberList { count: Option<usize>, ranges: Option<Vec<QueryStringSchemaValueRange>> },
        /// boolean values like 1, 0, true, false
        #[serde(rename="bool")]
        Bool,

        #[serde(rename="number")]
        Number(QueryStringSchemaValueRange),

        #[serde(rename="regex")]
        Regex { pattern: String },

        #[serde(rename="equals")]
        Equals { value: String, case_sensitive: Option<bool> },
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringSchemaValueRange{
        pub min: Option<f32>,
        pub max: Option<f32>,
        pub step_hint: Option<f32>,
        pub integer: Option<bool>,
    }


    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
    pub struct QueryStringSchemaExample{
        pub querystring: String,
        pub html_fragment: Option<String>,
        pub description: QueryStringDescription,
        pub generated_markdown: Option<String>,
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct Response001 {
    pub code: i64,
    pub success: bool,
    pub message: Option<String>,
    pub data: ResponsePayload,
}

impl Response001 {
    pub fn example_error() -> Response001 {
        Response001 {
            code: 500,
            success: false,
            message: Some("Invalid internal state".to_owned()),
            data: ResponsePayload::None,
        }
    }
    pub fn example_ok() -> Response001 {
        Response001 {
            code: 200,
            success: true,
            message: None,
            data: ResponsePayload::None,
        }
    }

    pub fn example_job_result_encoded(io_id: i32,
                                      w: i32,
                                      h: i32,
                                      mime: &'static str,
                                      ext: &'static str)
                                      -> Response001 {

        let frame_perf = FramePerformance{ nodes: vec![ NodePerf {wall_microseconds: 30_000, name: "decode".to_owned()}], overhead_microseconds: 100, wall_microseconds: 30_100};
        Response001 {
            code: 200,
            success: true,
            message: None,
            data: ResponsePayload::JobResult(JobResult {
                decodes: vec![],
                encodes: vec![EncodeResult {
                                  io_id,
                                  w,
                                  h,
                                  preferred_mime_type: mime.to_owned(),
                                  preferred_extension: ext.to_owned(),
                                  bytes: ResultBytes::Elsewhere,
                              }],
             performance: Some(BuildPerformance{
                 frames: vec![frame_perf]
             }),
            }),
        }
    }

    pub fn example_image_info() -> Response001 {
        Response001 {
            code: 200,
            success: true,
            message: None,
            data: ResponsePayload::ImageInfo(ImageInfo {
                multiple_frames: false,
//                current_frame_index: 0,
//                frame_count: 1,
                image_height: 480,
                image_width: 640,
                frame_decodes_into: PixelFormat::Bgr24,
                preferred_mime_type: "image/png".to_owned(),
                preferred_extension: "png".to_owned(),
                lossless: true,

            }),
        }
    }
}
//#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
pub fn rtf<'de,T>(value: T) -> usize
    where T: serde::Serialize,
          T: serde::Deserialize<'de>
{
    key_casing::print_keys_not_roundtrippable(&value)
}

#[test]
fn roundtrip_example_responses() {
    let failures = rtf(Response001::example_error()) + rtf(Response001::example_image_info()) +
                   rtf(Response001::example_ok()) +
                   rtf(Response001::example_job_result_encoded(0, 200, 200, "image/jpeg", "jpg")) +
                   rtf(Build001::example_with_steps()) +
                   rtf(Execute001::example_graph()) +
                   rtf(Execute001::example_steps());

    assert_eq!(0, failures);
}


#[allow(unused_macros)]
macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key, $val); )*
         map
    }}
}

#[test]
fn decode_graph() {
    let text = r#"{
        "nodes": {
            "0": {"decode": { "io_id": 1 } },
            "1": {"rotate_90" : null}

        },
        "edges": [
            {"from": 0, "to": 1, "kind": "input"}
        ]
    }"#;

    let obj: Graph = serde_json::from_str(&text).unwrap();
    let expected = Graph {
        nodes: hashmap![ "0".to_owned() => Node::Decode{ io_id: 1, commands: None},
                         "1".to_owned() => Node::Rotate90
        ],
        edges: vec![Edge {
                        from: 0,
                        to: 1,
                        kind: EdgeKind::Input,
                    }],
    };

    assert_eq!(obj, expected);
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum TestEnum {
    A,
    B { c: i32 },
}

#[test]
fn error_from_string() {
    let text = r#"{ "B": { "c": "hi" } }"#;

    let val: Result<TestEnum, serde_json::Error> = serde_json::from_str(text);

    let msg = match val {
        Err(e) => {
            format!("{:?}", e)
        }
        _ => {
            assert!(false);
            unreachable!()
        }
    };
    assert_eq!(msg, "Error(\"invalid type: string \\\"hi\\\", expected i32\", line: 1, column: 18)");
}

#[test]
fn error_from_value() {

    let text = r#"{ "B": { "c": "hi" } }"#;

    let val: serde_json::Value = serde_json::from_str(text).unwrap();

    let x: Result<TestEnum, serde_json::Error> = serde_json::from_value(val);

    let msg = match x {
        Err(e) => {
            format!("{:?}", e)
        }
        _ => {
            assert!(false);
            unreachable!()
        }
    };

    assert_eq!(msg, "Error(\"invalid type: string \\\"hi\\\", expected i32\", line: 0, column: 0)");
    // When parsing from a value, we cannot tell which line or character caused it. I suppose we
    // must serialize/deserialize again, in order to inject an indicator into the text?
    // We cannot recreate the original location AFAICT
}

mod key_casing {
    use serde;
    use serde_json;

    fn collect_keys(list: &mut Vec<String>, from: &serde_json::Value) {
        match *from {
            serde_json::Value::Object(ref map) => {
                for (k, v) in map {
                    list.push(k.to_owned());
                    collect_keys(list, v);
                }
            }
            serde_json::Value::Array(ref vec) => {
                for v in vec {
                    collect_keys(list, v);
                }
            }
            _ => {}
        }
    }

    pub fn collect_active_json_keys<'de, T>(value: &T) -> serde_json::error::Result<Vec<String>>
        where T: serde::Serialize,
              T: serde::Deserialize<'de>
    {
        let bytes = serde_json::to_vec(value)?;
        let generic: serde_json::Value = serde_json::from_slice(&bytes)?;
        let mut keys = Vec::new();
        collect_keys(&mut keys, &generic);
        Ok(keys)
    }

    #[allow(dead_code)]
    pub fn which_json_keys_cannot_roundtrip_casing<'de, T>(value: &T)
                                                      -> serde_json::error::Result<Vec<String>>
        where T: serde::Serialize,
              T: serde::Deserialize<'de>
    {
        let keys = collect_active_json_keys(value)?;

        Ok(keys.into_iter()
            .filter(|key| {
                let camelcase = style_id(key, Style::CamelCase);
                let snake_case = style_id(&camelcase, Style::CamelCase);
                camelcase != snake_case
            })
            .collect::<Vec<String>>())
    }

    /// Returns the number of roundtrip failures we printed
    pub fn print_keys_not_roundtrippable<'de, T>(value: &T) -> usize
        where T: serde::Serialize,
              T: serde::Deserialize<'de>
    {
        let keys = collect_active_json_keys(value)
            .expect("Value must be marked Serialize and Deserialize");

        let mut fail_count = 0;
        for key in keys {
            let camelcase = style_id(&key, Style::CamelCase);
            let snake_case = style_id(&camelcase, Style::Snake);

            if key != snake_case {
                println!("Cannot round-trip {} -> {} -> {}",
                         key,
                         camelcase,
                         snake_case);
                fail_count += 1;
            } else {
                // println!("Round-tripped {} -> {} -> {}", key, camelcase, snake_case);
            }
        }
        fail_count
    }

    use ::imageflow_helpers::identifier_styles::*;
}

#[test]
fn test_file_macro_for_this_build(){
    assert!(file!().starts_with(env!("CARGO_PKG_NAME")))
}


// mod try_nested_mut{
//
//    struct C<'a>{
//        v: &'a mut Vec<u8>
//    }
//    impl<'a> C<'a> {
//        fn b<'b>(&'b mut self) -> ::std::result::Result<(),()>{
//            Ok(())
//        }
//        fn a<'b>(&'b mut self) -> ::std::result::Result<(),()>{
//            {
//                self.b()?;
//            }
//            {
//                self.b()?;
//            }
//            {
//                self.b()
//            }
//        }
//    }
//    #[test]
//    fn test_c(){
//        let mut vec = Vec::new();
//        let mut c = C{v: &mut vec};
//        c.a().unwrap();
//    }
//
//
//    struct A<'d>{
//        v: &'d mut Vec<u8>,
//    }
//    struct B<'a>{
//        v: &'a mut Vec<u8>
//    }
// impl<'a> B<'a>{
//    fn ok(&mut self){
//        self.v.sort()
//    }
// }
//    impl<'d> A<'d>{
//        fn try(&mut self){ //&mut self is required to re-use self.v as a mutable reference.
//            let mut b = B{v: self.v};
//            b.ok();
//        }
//    }
//
//    #[test]
//    fn testit(){
//        let mut vec = Vec::new();
//        let mut a = A{v: &mut vec};
//        a.try();
//    }
//
// }

#[cfg(all(test, feature = "json-schema"))]
mod schema_tests {
    use super::*;
    use schemars::schema_for;

    #[test]
    fn generate_schemas() {
        let schema_build = schema_for!(Build001);
        let schema_response = schema_for!(Response001);
        let schema_execute = schema_for!(Execute001);

        // Basic validation: check if the schemas are generated without panic
        assert!(serde_json::to_string(&schema_build).is_ok());
        assert!(serde_json::to_string(&schema_response).is_ok());
        assert!(serde_json::to_string(&schema_execute).is_ok());

        // Optional: Print schemas (can be very large)
        // println!("Build001 Schema:\\n{}", serde_json::to_string_pretty(&schema_build).unwrap());
        // println!("Response001 Schema:\\n{}", serde_json::to_string_pretty(&schema_response).unwrap());
        // println!("Execute001 Schema:\\n{}", serde_json::to_string_pretty(&schema_execute).unwrap());
        println!("Successfully generated schemas for Build001, Response001, and Execute001.");
    }
}
