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

use imgref::ImgRef;
use std::str::FromStr;
pub mod collections;



/// Memory layout for pixels.
/// sRGB w/ gamma encoding assumed for 8-bit channels.
#[repr(C)]
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
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
    Gray8(ImgRef<'a, rgb::alt::GRAY8>),
}

/// Named interpolation function+configuration presets
#[repr(C)]
#[derive(Copy, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Debug)]
pub enum Filter {
    RobidouxFast = 1,
    Robidoux = 2,
    RobidouxSharp = 3,
    Ginseng = 4,
    GinsengSharp = 5,
    Lanczos = 6,
    LanczosSharp = 7,
    Lanczos2 = 8,
    Lanczos2Sharp = 9,
    CubicFast = 10,
    Cubic = 11,
    CubicSharp = 12,
    CatmullRom = 13,
    Mitchell = 14,

    CubicBSpline = 15,
    Hermite = 16,
    Jinc = 17,
    RawLanczos3 = 18,
    RawLanczos3Sharp = 19,
    RawLanczos2 = 20,
    RawLanczos2Sharp = 21,
    Triangle = 22,
    Linear = 23,
    Box = 24,
    CatmullRomFast = 25,
    CatmullRomFastSharp = 26,

    Fastest = 27,

    MitchellFast = 28,
    NCubic = 29,
    NCubicSharp = 30,
}

impl FromStr for Filter {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &*s.to_ascii_lowercase() {
            "robidouxfast" => Ok(Filter::RobidouxFast),
            "robidoux" => Ok(Filter::Robidoux),
            "robidouxsharp" => Ok(Filter::RobidouxSharp),
            "ginseng" => Ok(Filter::Ginseng),
            "ginsengsharp" => Ok(Filter::GinsengSharp),
            "lanczos" => Ok(Filter::Lanczos),
            "lanczossharp" => Ok(Filter::LanczosSharp),
            "lanczos2" => Ok(Filter::Lanczos2),
            "lanczos2sharp" => Ok(Filter::Lanczos2Sharp),
            "cubicfast" => Ok(Filter::CubicFast),
            "cubic_0_1" => Ok(Filter::Cubic),
            "cubicsharp" => Ok(Filter::CubicSharp),
            "catmullrom" |
            "catrom" => Ok(Filter::CatmullRom),
            "mitchell" => Ok(Filter::Mitchell),
            "cubicbspline" |
            "bspline" => Ok(Filter::CubicBSpline),
            "hermite" => Ok(Filter::Hermite),
            "jinc" => Ok(Filter::Jinc),
            "rawlanczos3" => Ok(Filter::RawLanczos3),
            "rawlanczos3sharp" => Ok(Filter::RawLanczos3Sharp),
            "rawlanczos2" => Ok(Filter::RawLanczos2),
            "rawlanczos2sharp" => Ok(Filter::RawLanczos2Sharp),
            "triangle" => Ok(Filter::Triangle),
            "linear" => Ok(Filter::Linear),
            "box" => Ok(Filter::Box),
            "catmullromfast" => Ok(Filter::CatmullRomFast),
            "catmullromfastsharp" => Ok(Filter::CatmullRomFastSharp),
            "fastest" => Ok(Filter::Fastest),
            "mitchellfast" => Ok(Filter::MitchellFast),
            "ncubic" => Ok(Filter::NCubic),
            "ncubicsharp" => Ok(Filter::NCubicSharp),
            _ => Err("no match"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum PngBitDepth {
    #[serde(rename="png_32")]
    Png32,
    #[serde(rename="png_24")]
    Png24,
}

/// The color space to blend/combine pixels in. Downscaling is best done in linear light.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum ScalingFloatspace {
    #[serde(rename="srgb")]
    Srgb,
    #[serde(rename="linear")]
    Linear, // gamma = 2,
}

/// Encoder presets (each with optional configuration). These are exposed by the JSON API.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum EncoderPreset {
    LibjpegTurbo {
        quality: Option<i32>,
        progressive: Option<bool>,
        optimize_huffman_coding: Option<bool>
    },
    Libpng {
        depth: Option<PngBitDepth>,
        matte: Option<Color>,
        zlib_compression: Option<i32>,
    },
    Pngquant {
        quality: Option<(u8, u8)>,
        speed: Option<u8>,
        maximum_deflate: Option<bool>
    },
    Lodepng {
        maximum_deflate: Option<bool>
    },
    Mozjpeg {
        quality: Option<u8>,
        progressive: Option<bool>,
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
            progressive: None
        }
    }
    pub fn libjpeg_turbo_q(quality: Option<i32>) -> EncoderPreset {
        EncoderPreset::LibjpegTurbo {
            quality,
            optimize_huffman_coding: None,
            progressive: None
        }
    }
}

/// Representations of an sRGB color value.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ColorSrgb {
    /// Hex in RRGGBBAA (css) form or variant thereof
    #[serde(rename="hex")]
    Hex(String),
}

/// Represents arbitrary colors (not color space specific)
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Color {
    #[serde(rename="transparent")]
    Transparent,
    #[serde(rename="black")]
    Black,
    #[serde(rename="srgb")]
    Srgb(ColorSrgb),
}

use imageflow_helpers::colors::*;
impl Color {

    pub fn to_u32_bgra(&self) -> std::result::Result<u32, ParseColorError> {
        self.to_color_32().map(|c| c.to_bgra_le() )
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
pub enum ResampleWhen{
    #[serde(rename="size_differs")]
    SizeDiffers,
    #[serde(rename="size_differs_or_sharpening_requested")]
    SizeDiffersOrSharpeningRequested,
    #[serde(rename="always")]
    Always
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum CommandStringKind{
    #[serde(rename="ir4")]
    ImageResizer4
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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



#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ConstraintGravity {
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "percentage")]
    Percentage{x: f32, y: f32}
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Constraint {
    pub mode: ConstraintMode,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub hints: Option<ResampleHints>,
    pub gravity: Option<ConstraintGravity>,
    pub canvas_color: Option<Color>
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum WatermarkConstraintBox{
    #[serde(rename = "image_percentage")]
    ImagePercentage{ x1: f32, y1: f32, x2: f32, y2: f32},
    #[serde(rename = "image_margins")]
    ImageMargins{ left: u32, top: u32, right: u32, bottom: u32},
}


#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Watermark{
    pub io_id: i32,
    pub gravity: Option<ConstraintGravity>,
    pub fit_box: Option<WatermarkConstraintBox>,
    pub fit_mode: Option<WatermarkConstraintMode>,
    pub opacity: Option<f32>,
    pub hints: Option<ResampleHints>
}

/// Blend pixels (if transparent) or replace?
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum CompositingMode {
    #[serde(rename="compose")]
    Compose,
    #[serde(rename="overwrite")]
    Overwrite
}

/// Represents a image operation. Currently used both externally (for JSON API) and internally.
/// The most important data type
#[allow(unreachable_patterns)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
        encode: Option<i32>
    },
    #[serde(rename="constrain")]
    Constrain(Constraint),
    #[serde(rename="copy_rect_to_canvas")]
    CopyRectToCanvas {
        from_x: u32,
        from_y: u32,
        w: u32, //TODO: inconsistent with w/h or x2/y2 elsewhere
        h: u32,
        x: u32,
        y: u32,
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
    #[serde(rename="flow_bitmap_bgra_ptr")]
    FlowBitmapBgraPtr {
        ptr_to_flow_bitmap_bgra_ptr: usize,
    },
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
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
pub enum EdgeKind {
    #[serde(rename="input")]
    Input,
    #[serde(rename="canvas")]
    Canvas,
}

/// Operation nodes are connected by edges. JSON only.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Edge {
    pub from: i32,
    pub to: i32,
    pub kind: EdgeKind,
}


/// An operation graph; should be directed and acyclic. JSON only.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Graph {
    pub nodes: std::collections::HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

/// We must mark IO objects as data sources or data destinations.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[repr(C)]
pub enum IoDirection {
    #[serde(rename="out")]
    Out = 8,
    #[serde(rename="in")]
    In = 4,
}

/// Describes (or contains) a data source or destination
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum IoEnum {
    #[serde(rename="bytes_hex")]
    BytesHex(String),
    #[serde(rename="base_64")]
    Base64(String),
    #[serde(rename="byte_array")]
    ByteArray(Vec<u8>),
    #[serde(rename="file")]
    Filename(String),
    #[serde(rename="url")]
    Url(String),
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
pub struct IoObject {
    pub io_id: i32,
    pub direction: IoDirection,
    pub io: IoEnum,
}

/// Represents an operation graph or series (series is simpler to think about and suitable for most tasks).
/// Operation graphs *may* be applied to each frame in the source data - thus 'Framewise'.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
pub struct Build001Config {
    // pub process_all_gif_frames: Option<bool>,
    pub graph_recording: Option<Build001GraphRecording>,

}

/// Represents a complete build job, combining IO objects with a framewise operation graph.
/// TODO: cleanup builder_config.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
            IoObject {

                direction: IoDirection::In,
                io_id: 0,
                io: IoEnum::Url("http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())
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
pub struct Execute001 {
    pub graph_recording: Option<Build001GraphRecording>,
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
                                  preset: EncoderPreset::LibjpegTurbo { quality: Some(90), optimize_huffman_coding: Some(true), progressive: Some(true)},
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
                         preset: EncoderPreset::LibjpegTurbo { quality: Some(90), optimize_huffman_coding: Some(true), progressive: Some(true) },
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
            framewise: Framewise::example_steps(),
        }
    }
    pub fn example_graph() -> Execute001 {
        Execute001 {
            graph_recording: None,
            framewise: Framewise::example_graph(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct GetImageInfo001 {
    pub io_id: i32,
}

impl GetImageInfo001 {
    pub fn example_get_image_info() -> GetImageInfo001 {
        GetImageInfo001 { io_id: 0 }
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct JpegIDCTDownscaleHints {
    pub width: i64,
    pub height: i64,
    pub scale_luma_spatially: Option<bool>,
    pub gamma_correct_for_srgb_during_spatial_luma_scaling: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct WebPDecoderHints {
    pub width: i32,
    pub height: i32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum DecoderCommand {
    #[serde(rename="jpeg_downscale_hints")]
    JpegDownscaleHints(JpegIDCTDownscaleHints),
    #[serde(rename="webp_decoder_hints")]
    WebPDecoderHints(WebPDecoderHints),
    #[serde(rename="discard_color_profile")]
    DiscardColorProfile
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
pub struct ImageInfo {
    pub preferred_mime_type: String,
    pub preferred_extension: String,
    // Warning, one cannot count frames in a GIF without scanning the whole thing.
//    pub frame_count: usize,
//    pub current_frame_index: i64,
    pub image_width: i32,
    pub image_height: i32,
    pub frame_decodes_into: PixelFormat
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
pub struct EncodeResult {
    pub preferred_mime_type: String,
    pub preferred_extension: String,

    pub io_id: i32,
    pub w: i32,
    pub h: i32,

    pub bytes: ResultBytes,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct NodePerf{
    pub wall_microseconds: u64,
    pub name: String
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct BuildPerformance{
    pub frames: Vec<FramePerformance>,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
pub struct JobResult {
    pub encodes: Vec<EncodeResult>,
    pub performance: Option<BuildPerformance>
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ResponsePayload {
    #[serde(rename="image_info")]
    ImageInfo(ImageInfo),
    #[serde(rename="job_result")]
    JobResult(JobResult),
    #[serde(rename="build_result")]
    BuildResult(JobResult),
    #[serde(rename="none")]
    None,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
//                current_frame_index: 0,
//                frame_count: 1,
                image_height: 480,
                image_width: 640,
                frame_decodes_into: PixelFormat::Bgr24,
                preferred_mime_type: "image/png".to_owned(),
                preferred_extension: "png".to_owned(),

            }),
        }
    }
}
#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
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
