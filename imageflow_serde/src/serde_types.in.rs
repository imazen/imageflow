#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}


mod nodes {
    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct Decode {
        pub io_id: i32
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub enum Encoder{
        Png,
        Png24,
        Png8,
        Jpeg

    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct Encode {
        pub io_id: i32,
        pub encoder: Option<Encoder>
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub enum AnyNode {
        Decode(Decode),
        Encode(Encode),
    }

}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum MNode {
    Decode{io_id: i32},
    Encode{io_id: i32, encoder: Option<nodes::Encoder>},
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum PixelFormat{
    Bgra32, Bgr24, Gray8
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Encoder{
    Png,
    Jpeg
}



#[repr(C)]
#[derive(Serialize, Deserialize, Clone, PartialEq, PartialOrd, Debug)]
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
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum EncoderHints{
    Jpeg{quality: Option<i32>},
    Png{disable_alpha: Option<bool>}
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ColorSrgb{
    Hex(String)
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Color{
    Srgb(ColorSrgb)
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Node{
    FlipV,
    FlipH,
    Crop{ x1: u32, y1: u32, x2: u32, y2: u32},
    CreateCanvas{ format: PixelFormat, w: usize, h: usize, color: Color},
    CopyRectToCanvas { from_x: u32, from_y: u32, width: u32, height: u32, x: u32, y: u32},
    Decode{io_id: i32},
    Encode{io_id: i32, encoder: Option<Encoder>, encoder_id: Option<i64>, hints: Option<EncoderHints> },
    FillRect {x1: u32, y1: u32, x2: u32, y2: u32, color: Color},
    ExpandCanvas {left: u32, top: u32, right: u32, bottom: u32, color: Color},
    Transpose,
    Rotate90,
    Rotae180,
    Rotate270,
    Scale{ w: usize, h: usize, down_filter: Option<Filter>, up_filter: Option<Filter>, sharpen_percent: Option<f32>, flags: Option<usize>}
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum EdgeKind{
    Input,
    Canvas
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Edge{
    from: i32,
    to: i32,
    kind: EdgeKind
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Graph{
    nodes: std::collections::HashMap<u32, Node>,
    edges: Vec<Edge>
}
