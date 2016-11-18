#![feature(proc_macro)]
#![feature(conservative_impl_trait)]


#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

use std::ascii::AsciiExt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}


mod nodes {
    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct Decode {
        pub io_id: i32,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub enum Encoder {
        Png,
        Png24,
        Png8,
        Jpeg,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct Encode {
        pub io_id: i32,
        pub encoder: Option<Encoder>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub enum AnyNode {
        Decode(Decode),
        Encode(Encode),
    }

}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum MNode {
    Decode { io_id: i32 },
    Encode {
        io_id: i32,
        encoder: Option<nodes::Encoder>,
    },
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum PixelFormat {
    #[serde(rename="bgra32")]
    Bgra32,
    #[serde(rename="bgr24")]
    Bgr24,
    #[serde(rename="gray8")]
    Gray8,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Encoder {
    Png,
    Jpeg,
}



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
            "catmullrom" => Ok(Filter::CatmullRom),
            "catrom" => Ok(Filter::CatmullRom),
            "mitchell" => Ok(Filter::Mitchell),
            "cubicbspline" => Ok(Filter::CubicBSpline),
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum PngBitDepth{
    #[serde(rename="png32")]
    Png32,
    #[serde(rename="png24")]
    Png24,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum EncoderPreset {
    #[serde(rename="libjpegturbo")]
    LibjpegTurbo { quality: Option<i32> },
    #[serde(rename="libpng")]
    Libpng {  depth: Option<PngBitDepth>, matte: Option<Color>,
        #[serde(rename="zlibCompression")]
        zlib_compression: Option<i32>}
}

impl EncoderPreset{
    pub fn libpng32() -> EncoderPreset{
        EncoderPreset::Libpng{ depth: Some(PngBitDepth::Png32), matte: None, zlib_compression: None}
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ColorSrgb {
    #[serde(rename="hex")]
    Hex(String),
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Color {
    #[serde(rename="transparent")]
    Transparent,
    #[serde(rename="black")]
    Black,
    #[serde(rename="srgb")]
    Srgb(ColorSrgb),
}

impl Color {
    pub fn to_u32_bgra(self) -> std::result::Result<u32, std::num::ParseIntError> {
        self.to_u32_rgba().and_then(|u| Ok(u.swap_bytes().rotate_left(8)))
    }
    pub fn to_u32_rgba(self) -> std::result::Result<u32, std::num::ParseIntError> {
        match self {
            Color::Srgb(srgb) => {
                match srgb {
                    ColorSrgb::Hex(hex_srgb) => {
                        u32::from_str_radix(hex_srgb.as_str(), 16)
                            .and_then(|value| if hex_srgb.len() <= 6 {
                                Ok(value.checked_shl(8).unwrap() | 0xFF)
                            } else {
                                Ok(value)
                            })
                    }
                }
            },
            Color::Black => Ok(0x000000FF),
            Color::Transparent => Ok(0),
        }
    }
}


#[test]
fn test_color() {

    assert_eq!(Color::Srgb(ColorSrgb::Hex("FFAAEEDD".to_owned())).to_u32_rgba().unwrap(),
               0xFFAAEEDD);
    assert_eq!(Color::Srgb(ColorSrgb::Hex("FFAAEE".to_owned())).to_u32_rgba().unwrap(),
               0xFFAAEEFF);
}

#[test]
fn test_bgra() {

    assert_eq!(Color::Srgb(ColorSrgb::Hex("FFAAEEDD".to_owned())).to_u32_bgra().unwrap(),
               0xEEAAFFDD);
    assert_eq!(Color::Srgb(ColorSrgb::Hex("FFAAEE".to_owned())).to_u32_bgra().unwrap(),
               0xEEAAFFFF);
    assert_eq!(Color::Srgb(ColorSrgb::Hex("000000FF".to_owned())).to_u32_bgra().unwrap(),
               0x000000FF);


}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ResampleHints{
    #[serde(rename="sharpenPercent")]
    pub sharpen_percent: Option<f32>,

    #[serde(rename="prefer1dTwice")]
    pub prefer_1d_twice: Option<bool>
}

pub enum Constraint{

}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Node {
    #[serde(rename="flipV")]
    FlipV,
    #[serde(rename="flipH")]
    FlipH,
    #[serde(rename="crop")]
    Crop { x1: u32, y1: u32, x2: u32, y2: u32 },
    #[serde(rename="createCanvas")]
    CreateCanvas {
        format: PixelFormat,
        w: usize,
        h: usize,
        color: Color,
    },
    #[serde(rename="copyRectToCanvas")]
    CopyRectToCanvas {
        #[serde(rename="fromX")]
        from_x: u32,
        #[serde(rename="fromY")]
        from_y: u32,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
    },
    #[serde(rename="decode")]
    Decode {
        #[serde(rename="ioId")]
        io_id: i32,
    },
    #[serde(rename="encode")]
    Encode {
        #[serde(rename="ioId")]
        io_id: i32,
        preset: EncoderPreset
    },
    #[serde(rename="fillRect")]
    FillRect {
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        color: Color,
    },
    #[serde(rename="expandCanvas")]
    ExpandCanvas {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
        color: Color,
    },
    #[serde(rename="transpose")]
    Transpose,
    #[serde(rename="rotate90")]
    Rotate90,
    #[serde(rename="rotate180")]
    Rotate180,
    #[serde(rename="rotate270")]
    Rotate270,
    #[serde(rename="applyOrientation")]
    ApplyOrientation { flag: i32 },
    #[serde(rename="resample2d")]
    Resample2D {
        w: usize,
        h: usize,
        #[serde(rename="downFilter")]
        down_filter: Option<Filter>,
        #[serde(rename="upFilter")]
        up_filter: Option<Filter>,
        hints: Option<ResampleHints>
    },

    #[serde(rename="resample1d")]
    Resample1D {
        #[serde(rename="scaleToWidth")]
        scale_to_width: usize,
        #[serde(rename="transposeOnWrite")]
        transpose_on_write: bool,
        #[serde(rename="interpolationFilter")]
        interpolation_filter: Option<Filter>,
    },
    // TODO: Block use except from FFI/unit test use
    #[serde(rename="flowBitmapBgraPtr")]
    FlowBitmapBgraPtr {
        #[serde(rename="ptrToFlowBitmapBgraPtr")]
        ptr_to_flow_bitmap_bgra_ptr: usize,
    },
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum EdgeKind {
    #[serde(rename="input")]
    Input,
    #[serde(rename="canvas")]
    Canvas,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Edge {
    pub from: i32,
    pub to: i32,
    pub kind: EdgeKind,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Graph {
    pub nodes: std::collections::HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum TestEnum {
    A,
    B { c: i32 },
}


#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum IoDirection {
    #[serde(rename="output")]
    Output = 8,
    #[serde(rename="input")]
    Input = 4,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum IoEnum {
    #[serde(rename="bytesHex")]
    BytesHex(String),
    #[serde(rename="byteArray")]
    ByteArray(Vec<u8>),
    #[serde(rename="file")]
    Filename(String),
    #[serde(rename="url")]
    Url(String),
    #[serde(rename="outputBuffer")]
    OutputBuffer,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]

pub enum IoChecksum {
    #[serde(rename="djb2Hex")]
    Djb2Hex(String),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct IoObject {
    #[serde(rename="ioId")]
    pub io_id: i32,
    pub direction: IoDirection,
    pub io: IoEnum
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Framewise {
    #[serde(rename="graph")]
    Graph(Graph),
    #[serde(rename="steps")]
    Steps(Vec<Node>),
}

impl Framewise{
    pub fn clone_nodes<'a>(&'a self) -> Vec<&'a Node> {
        match *self {
            Framewise::Graph(ref graph) => {
                graph.nodes.values().collect::<Vec<&Node>>()
            }
            Framewise::Steps(ref nodes) => {
                nodes.iter().collect::<Vec<&Node>>()
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Build001GraphRecording {
    pub record_graph_versions: Option<bool>,
    pub record_frame_images: Option<bool>,
    pub render_last_graph: Option<bool>,
    pub render_graph_versions: Option<bool>,
    pub render_animated_graph: Option<bool>,
}

impl  Build001GraphRecording {
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
    #[serde(rename="enableJpegBlockScaling")]
    pub enable_jpeg_block_scaling: Option<bool>,
    #[serde(rename="processAllGifFrames")]
    pub process_all_gif_frames: Option<bool>,
    #[serde(rename="graphRecording")]
    pub graph_recording: Option<Build001GraphRecording>,
    #[serde(rename="noGammaCorrection")]
    pub no_gamma_correction: bool,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Build001 {
    #[serde(rename="builderConfig")]
    pub builder_config: Option<Build001Config>,
    pub io: Vec<IoObject>,
    pub framewise: Framewise,
}

impl IoEnum{
    pub fn example_byte_array() -> IoEnum{
        let tinypng = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00,
        0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82 ];
        IoEnum::ByteArray(tinypng)
    }
    pub fn example_byte_array_truncated() -> IoEnum{
        IoEnum::ByteArray(vec![0x89, 0x50, 0x4E, 0x47])
    }
    pub fn example_bytes_hex() -> IoEnum{
        IoEnum::BytesHex("89504E470D0A1A0A0000000D49484452000000010000000108060000001F15C4890000000A49444154789C63000100000500010D0A2DB40000000049454E44AE426082".to_owned())
    }
}

impl Build001 {
    pub fn example_with_steps() -> Build001 {
        Build001 {
            builder_config: None,
            io: vec![
            IoObject {

                direction: IoDirection::Input,
                io_id: 0,
                io: IoEnum::Url("http://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg".to_owned())
            },
            IoObject {

                direction: IoDirection::Input,
                io_id: 90,
                io: IoEnum::example_byte_array_truncated(),
            },
            IoObject {

                direction: IoDirection::Input,
                io_id: 91,
                io: IoEnum::example_bytes_hex(),
            },
            IoObject {
                io: IoEnum::Filename("output.png".to_owned()),
                io_id: 1,

                direction: IoDirection::Output
            },
            IoObject {
                io: IoEnum::OutputBuffer,
                io_id: 2,

                direction: IoDirection::Output
            }
            ],
            framewise: Framewise::example_graph()
        }
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Execute001 {
    #[serde(rename="noGammaCorrection")]
    pub no_gamma_correction: Option<bool>,
    #[serde(rename="graphRecording")]
    pub graph_recording: Option<Build001GraphRecording>,
    pub framewise: Framewise,
}

impl Framewise {
    pub fn example_steps() -> Framewise{
        Framewise::Steps(
            vec![
            Node::Decode { io_id: 0 },
            Node::ApplyOrientation { flag: 7 },
            Node::ExpandCanvas { left: 10, top: 10, right: 10, bottom: 10, color: Color::Srgb(ColorSrgb::Hex("FFEECCFF".to_owned())) },
            Node::Crop { x1: 10, y1: 10, x2: 650, y2: 490 },
            Node::FillRect { x1: 0, y1: 0, x2: 8, y2: 8, color: Color::Transparent },
            Node::FlipV,
            Node::FlipH,
            Node::Rotate90,
            Node::Rotate180,
            Node::Rotate270,
            Node::Transpose,
            Node::Resample2D {
                w: 100, h: 75,
                down_filter: Some(Filter::Robidoux),
                up_filter: Some(Filter::Ginseng),
                hints: Some(ResampleHints { sharpen_percent: Some(10f32), prefer_1d_twice: None })
            },
            Node::Resample2D {
                w: 200,
                h: 150,
                up_filter: None,
                down_filter: None,
                hints: None
            },
            Node::Encode { io_id: 1, preset: EncoderPreset::LibjpegTurbo { quality: Some(90) } }
            ]
        )
    }
    pub fn example_graph() -> Framewise{

        let mut nodes = std::collections::HashMap::new();
        nodes.insert("0".to_owned(), Node::Decode { io_id: 0});
        nodes.insert("1".to_owned(), Node::CreateCanvas { w: 200, h: 200, format: PixelFormat::Bgra32, color: Color::Transparent });
        nodes.insert("2".to_owned(), Node::CopyRectToCanvas { x: 0, y:0, from_x: 0, from_y: 0, width: 100, height: 100});
        nodes.insert("3".to_owned(), Node::Resample1D{ scale_to_width: 100, interpolation_filter: None, transpose_on_write: false});
        nodes.insert("4".to_owned(), Node::Encode{ io_id: 1, preset: EncoderPreset::Libpng{ matte: Some(Color::Srgb(ColorSrgb::Hex("999999".to_owned()))), zlib_compression: None,  depth: Some(PngBitDepth::Png24) }});
        nodes.insert("5".to_owned(), Node::Encode{ io_id: 2, preset: EncoderPreset::LibjpegTurbo { quality: Some(90) }});

        Framewise::Graph(Graph{
            edges: vec![
            Edge{
                from:0,
                to: 2,
                kind: EdgeKind::Input
            },
            Edge{
                from:1,
                to: 2,
                kind: EdgeKind::Canvas
            },
            Edge{
                from: 2,
                to: 3,
                kind: EdgeKind::Input
            },
            Edge{
                from:3,
                to: 4,
                kind: EdgeKind::Input
            },
            Edge{
                from:3,
                to: 5,
                kind: EdgeKind::Input
            }
            ],
            nodes: nodes
        })
    }
}
impl Execute001 {
    pub fn example_steps() -> Execute001{
        Execute001 {
            no_gamma_correction: None,
            graph_recording: None,
            framewise: Framewise::example_steps()
        }
    }
    pub fn example_graph() -> Execute001 {
        Execute001{
            no_gamma_correction: None,
            graph_recording: None,
            framewise: Framewise::example_graph()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct GetImageInfo001 {
    #[serde(rename="ioId")]
    pub io_id: i32,
}

impl GetImageInfo001 {
    pub fn example_get_image_info() -> GetImageInfo001{
        GetImageInfo001{
            io_id: 0
        }
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct JpegIDCTDownscaleHints {
    pub width: i64,
    pub height: i64,
    #[serde(rename="scaleLumaSpatially")]
    pub scale_luma_spatially: Option<bool>,
    #[serde(rename="gammaCorrectForSrgbDuringSpatialLumaScaling")]
    pub gamma_correct_for_srgb_during_spatial_luma_scaling: Option<bool>
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum TellDecoderWhat {
    JpegDownscaleHints(JpegIDCTDownscaleHints)
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct TellDecoder001 {
    #[serde(rename="ioId")]
    pub io_id: i32,
    pub command: TellDecoderWhat
}

impl TellDecoder001 {
    pub fn example_hints() -> TellDecoder001{
        TellDecoder001{
            io_id: 2,
            command: TellDecoderWhat::JpegDownscaleHints(JpegIDCTDownscaleHints{
                width: 1000,
                height: 1000,
                scale_luma_spatially: Some(true),
                gamma_correct_for_srgb_during_spatial_luma_scaling: Some(true)
            })
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ImageInfo {
    #[serde(rename="preferredMimeType")]
    pub preferred_mime_type: String,
    #[serde(rename="preferredExtension")]
    pub preferred_extension: String,
    //Warning, one cannot count frames in a GIF without scanning the whole thing.
    #[serde(rename="frameCount")]
    pub frame_count: usize,
    #[serde(rename="currentFrameIndex")]
    pub current_frame_index: i64,
    #[serde(rename="frame0Width")]
    pub frame0_width: i32,
    #[serde(rename="frame0Height")]
    pub frame0_height: i32,
    #[serde(rename="frame0PostDecodeFormat")]
    pub frame0_post_decode_format: PixelFormat,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct EncodeResult{
    #[serde(rename="preferredMimeType")]
    pub preferred_mime_type: String,
    #[serde(rename="preferredExtension")]
    pub preferred_extension: String,

    #[serde(rename="ioId")]
    pub io_id: i32,
    #[serde(rename="w")]
    pub w: i32,
    #[serde(rename="h")]
    pub h: i32
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct JobResult {
    pub encodes: Vec<EncodeResult>
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ResponsePayload {
    #[serde(rename="imageInfo")]
    ImageInfo(ImageInfo),
    #[serde(rename="jobResult")]
    JobResult(JobResult),
    None,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Response001 {
    pub code: i64,
    pub success: bool,
    pub message: Option<String>,
    pub data: ResponsePayload
}

impl Response001 {
    pub fn example_error() -> Response001 {
        Response001{
            code: 500,
            success: false,
            message: Some("Invalid internal state".to_owned()),
            data: ResponsePayload::None
        }
    }
    pub fn example_ok() -> Response001 {
        Response001{
            code: 200,
            success: true,
            message: None,
            data: ResponsePayload::None
        }
    }

    pub fn example_job_result_encoded(io_id: i32, w: i32, h: i32, mime: &'static str, ext: &'static str) -> Response001 {
        Response001{
            code: 200,
            success: true,
            message: None,
            data: ResponsePayload::JobResult(JobResult{
                encodes: vec![EncodeResult{io_id: io_id, w: w, h: h, preferred_mime_type: mime.to_owned(), preferred_extension: ext.to_owned()}]
            })
        }
    }


    pub fn example_image_info() -> Response001 {
        Response001{
            code: 200,
            success: true,
            message: None,
            data: ResponsePayload::ImageInfo(
                ImageInfo{
                    current_frame_index: 0,
                    frame_count: 1,
                    frame0_height: 480,
                    frame0_width: 640,
                    frame0_post_decode_format: PixelFormat::Bgr24,
                    preferred_mime_type: "image/png".to_owned(),
                    preferred_extension: "png".to_owned()
                }
            )
        }
    }
}



#[test]
fn test_roundtrip() {
    let point = Point { x: 1, y: 2 };

    let serialized = serde_json::to_string(&point).unwrap();
    assert_eq!(serialized, r#"{"x":1,"y":2}"#);

    let deserialized: Point = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized, Point { x: 1, y: 2 });
}


#[test]
fn test_decode_node() {
    let text = r#"{"Decode": { "io_id": 1 } }"#;

    let obj: nodes::AnyNode = serde_json::from_str(&text).unwrap();

    assert_eq!(obj, nodes::AnyNode::Decode(nodes::Decode { io_id: 1 }));
}


#[test]
fn test_decode_mnode() {
    let text = r#"[{"Decode": { "io_id": 1 } }, {"Encode": { "io_id": 2 } }]"#;

    let obj: Vec<MNode> = serde_json::from_str(&text).unwrap();

    assert_eq!(obj,
               vec![MNode::Decode { io_id: 1 },
                    MNode::Encode {
                        io_id: 2,
                        encoder: None,
                    }]);
}

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
            "0": {"decode": { "ioId": 1 } },
            "1": {"rotate90" : null}

        },
        "edges": [
            {"from": 0, "to": 1, "kind": "input"}
        ]
    }"#;

    let obj: Graph = serde_json::from_str(&text).unwrap();
    let expected = Graph {
        nodes: hashmap![ "0".to_owned() => Node::Decode{ io_id: 1 },
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

#[test]
fn error_from_string() {
    let text = r#"{ "B": { "c": "hi" } }"#;

    let val: Result<TestEnum, serde_json::Error> = serde_json::from_str(text);

    let (code, line, chr) = match val {
        Err(e) => {
            match e {
                serde_json::Error::Syntax(code, line, char) => (code, line, char),
                _ => {
                    assert!(false);
                    unreachable!()
                }
            }
        }
        _ => {
            assert!(false);
            unreachable!()
        }
    };

    assert_eq!(code,
               serde_json::ErrorCode::InvalidType(serde::de::Type::Str));
    assert_eq!(line, 1);
    assert_eq!(chr, 18);
}

#[test]
fn error_from_value() {

    let text = r#"{ "B": { "c": "hi" } }"#;

    let val: serde_json::Value = serde_json::from_str(text).unwrap();

    let x: Result<TestEnum, serde_json::Error> = serde_json::from_value(val);

    let (code, line, chr) = match x {
        Err(e) => {
            match e {
                serde_json::Error::Syntax(code, line, char) => (code, line, char),
                _ => {
                    assert!(false);
                    unreachable!()
                }
            }
        }
        _ => {
            assert!(false);
            unreachable!()
        }
    };

    assert_eq!(code,
               serde_json::ErrorCode::InvalidType(serde::de::Type::Str));
    assert_eq!(line, 0);
    assert_eq!(chr, 0);
    // When parsing from a value, we cannot tell which line or character caused it. I suppose we
    // must serialize/deserialize again, in order to inject an indicator into the text?
    // We cannot recreate the original location AFAICT

}
