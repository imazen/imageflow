use crate::preludes::from_std::*;
use std;


fn parse_rgba_slices(r: &str, g: &str, b: &str, a :&str) -> Result<Color32,std::num::ParseIntError>{
   [a, r,g,b].iter().map(|s|{
        match s.len() {
            0 => Ok(255),
            1 => u8::from_str_radix(s, 16).map(|v| (v << 4) | v),
            2 => u8::from_str_radix(s, 16),
            _ => panic!("segments may be zero to two characters, but no more"),
        }.map( u32::from)
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
    }).map(Color32)
}


///
/// Parses #RRGGBBAA #RRGGBB #RGB #RGBA - with and without leading #, case insensitive
pub fn parse_color_hex(value: &str) -> std::result::Result<Color32, ParseColorError> {
    let value = match &value[0..1] {
        "#" => &value[1..],
        _ => value
    };
    let u32_result = u32::from_str_radix(value, 16);
    if u32_result.is_ok() {
        let why = "Any substring of a valid hexadecimal string should also be a valid hexadecimal string";
        match value.len() {
            3 => Ok(parse_rgba_slices(&value[0..1], &value[1..2], &value[2..3], "").expect(why)),
            4 => Ok(parse_rgba_slices(&value[0..1], &value[1..2], &value[2..3], &value[3..4]).expect(why)),
            6 => Ok(parse_rgba_slices(&value[0..2], &value[2..4], &value[4..6], "").expect(why)),
            8 => Ok(parse_rgba_slices(&value[0..2], &value[2..4], &value[4..6], &value[6..8]).expect(why)),
            _ => Err(ParseColorError::FormatIncorrect("CSS hexadecimal colors must be in the form [#]RGB, [#]RGBA, [#]RRGGBBAA, or [#]RRGGBB. "))
        }
    } else {
        Err(ParseColorError::NotHexadecimal{desc: "Only hexadecimal colors are permitted here", parse_error: u32_result.unwrap_err()})
    }
}


/// Parses named CSS3 colors, plus #RRGGBBAA #RRGGBB #RGB #RGBA - with and without leading #, case insensitive
/// Returns in 0xRRGGBBAA format, or abgr byte order
pub fn parse_color_hex_or_named(value: &str) -> std::result::Result<Color32, ParseColorError> {
    match parse_color_hex(value){
        Err(ParseColorError::NotHexadecimal{parse_error, ..}) => {
            match COLORS.get(value.to_lowercase().as_str()) {
                Some(v) => Ok(Color32(*v)),
                None => Err(ParseColorError::ColorNotRecognized(parse_error))
            }
        }
        other => other
    }
}

/// Native storage format is 0xAARRGGBB
/// Can only represent `sRGB`, non-linear values with 8 bit precision per channel.
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
pub struct Color32(pub u32);

impl Color32{
    pub fn to_rrggbbaa_string(&self) -> String{
        format!("{:08X}", self.to_abgr_le())
    }
    pub fn to_aarrggbb_string(&self) -> String{
        format!("{:08X}", self.to_bgra_le())
    }
    pub fn to_bgra_le(&self) -> u32{
        self.0
    }
    pub fn to_abgr_le(&self) -> u32{
        self.0.rotate_left(8)
    }
    pub fn transparent_black() -> Color32{
        Color32(0)
    }

    pub fn black() -> Color32{
        Color32(0xFF_00_00_00)
    }

    pub fn is_transparent(&self) -> bool{
        (self.0 & 0xFF_00_00_00) == 0
    }
    pub fn is_opaque(&self) -> bool{
        (self.0 & 0xFF_00_00_00) == 0xFF_00_00_00
    }
}

#[test]
fn test_color32(){
    assert_eq!(Color32(0xFFEEDDCC).to_aarrggbb_string(), "FFEEDDCC");
    assert_eq!(Color32(0xFFEEDDCC).to_rrggbbaa_string(), "EEDDCCFF");

    fn t(value: &str, expected: Color32) {
        let actual = parse_color_hex_or_named(value).unwrap();

        if actual != expected {
            let _ = write!(::std::io::stderr(), "Expected {}, actual={}\n", expected.to_aarrggbb_string(), actual.to_aarrggbb_string());
        }
        assert_eq!(actual, expected);
    }

    t("red", Color32(0xffff0000));
    t("f00", Color32(0xffff0000));
    t("ff00", Color32(0x00ffff00));
    t("ff0000", Color32(0xffff0000));
    t("0000ffff", Color32(0xff0000ff));

    t("darkseagreen", Color32(0xff8fbc8b));
    t("8fbc8b", Color32(0xff8fbc8b));
    t("8fbc8bff", Color32(0xff8fbc8b));

    t("lightslategray", Color32(0xff778899));
    t("789", Color32(0xff778899));
    t("789f", Color32(0xff778899));
    t("778899", Color32(0xff778899));
    t("77889953", Color32(0x53778899));

    t("white", Color32(0xffffffff));
    t("fff", Color32(0xffffffff));
    t("ffff", Color32(0xffffffff));
    t("ffffff", Color32(0xffffffff));
    t("ffffffff", Color32(0xffffffff));

}

#[derive(Debug,Clone,PartialEq)]
pub enum ParseColorError{
    ColorNotRecognized(std::num::ParseIntError),
    NotHexadecimal{ desc: &'static str, parse_error: std::num::ParseIntError},
    FormatIncorrect(&'static str)
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
// BGRA form u32
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
        "transparent" => 0x0,
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
