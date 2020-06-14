

use super::internal_prelude::*;


pub static COLOR_MATRIX_SRGB: MutProtect<ColorMatrixSrgbMutDef> = MutProtect{ node: &COLOR_MATRIX_SRGB_MUTATE, fqn: "imazen.color_matrix_srgb"};
pub static COLOR_FILTER_SRGB: ColorFilterSrgb = ColorFilterSrgb{};
pub static COLOR_MATRIX_SRGB_MUTATE: ColorMatrixSrgbMutDef = ColorMatrixSrgbMutDef{};



#[derive(Debug, Clone)]
pub struct ColorMatrixSrgbMutDef;
impl NodeDef for ColorMatrixSrgbMutDef{
    fn as_one_mutate_bitmap(&self) -> Option<&dyn NodeDefMutateBitmap>{
        Some(self)
    }
}
impl NodeDefMutateBitmap for ColorMatrixSrgbMutDef{
    fn fqn(&self) -> &'static str{
        "imazen.color_matrix_srgb_mut"
    }
    fn mutate(&self, c: &Context, bitmap: &mut BitmapBgra,  p: &NodeParams) -> Result<()> {
        if let NodeParams::Json(s::Node::ColorMatrixSrgb { ref matrix }) = *p {
            unsafe {
                let color_matrix_ptrs = matrix.iter().map(|row| row as *const f32).collect::<Vec<*const f32>>();

                if !crate::ffi::flow_bitmap_bgra_apply_color_matrix(c.flow_c(), bitmap, 0, (*bitmap).h, color_matrix_ptrs.as_ptr()) {
                    return Err(cerror!(c, "Failed to apply color matrix"))
                }

                let _ = color_matrix_ptrs;

            }
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ColorMatrixSrgb, got {:?}", p))
        }
    }
}


#[derive(Debug,Clone)]
pub struct ColorFilterSrgb;
impl NodeDef for ColorFilterSrgb{
    fn as_one_input_expand(&self) -> Option<&dyn NodeDefOneInputExpand>{
        Some(self)
    }
}
impl NodeDefOneInputExpand for ColorFilterSrgb{
    fn fqn(&self) -> &'static str{
        "imazen.color_filter_srgb"
    }
    fn expand(&self, ctx: &mut OpCtxMut, ix: NodeIndex, p: NodeParams, parent: FrameInfo) -> Result<()> {
        if let NodeParams::Json(s::Node::ColorFilterSrgb(filter))= p {

            let matrix = match filter as s::ColorFilterSrgb {
                s::ColorFilterSrgb::Sepia => sepia(),
                s::ColorFilterSrgb::GrayscaleNtsc => grayscale_ntsc(),
                s::ColorFilterSrgb::GrayscaleRy => grayscale_ry(),
                s::ColorFilterSrgb::GrayscaleFlat => grayscale_flat(),
                s::ColorFilterSrgb::GrayscaleBt709 => grayscale_bt709(),
                s::ColorFilterSrgb::Invert => invert(),
                s::ColorFilterSrgb::Alpha(a) => alpha(a),
                s::ColorFilterSrgb::Contrast(a) => contrast(a),
                s::ColorFilterSrgb::Saturation(a) => saturation(a),
                s::ColorFilterSrgb::Brightness(a) => brightness(a),
            };
            let mut nodes = Vec::new();
            if let  imageflow_types::ColorFilterSrgb::Alpha(_) = filter{
                nodes.push(Node::n(&ENABLE_TRANSPARENCY, NodeParams::None));

            }
            nodes.push(Node::n(&COLOR_MATRIX_SRGB_MUTATE,
                               NodeParams::Json(s::Node::ColorMatrixSrgb { matrix })));

            ctx.replace_node(ix, nodes);
            Ok(())
        }else{
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ColorFilterSrgb, got {:?}", p))
        }
    }
}


fn sepia() -> [[f32;5];5] {
    [
        [0.393f32, 0.349f32, 0.272f32, 0f32, 0f32],
        [0.769f32, 0.686f32, 0.534f32, 0f32, 0f32],
        [0.189f32, 0.168f32, 0.131f32, 0f32, 0f32],
        [0f32, 0f32, 0f32, 1f32, 0f32],
        [0f32, 0f32, 0f32, 0f32, 0f32]
    ]
}
fn grayscale(r: f32, g:f32, b: f32) -> [[f32;5];5] {
    [
        [r, r, r, 0f32, 0f32],
        [g, g, g, 0f32, 0f32],
        [b, b, b, 0f32, 0f32],
        [0f32, 0f32, 0f32, 1f32, 0f32],
        [0f32, 0f32, 0f32, 0f32, 1f32],
    ]
}
fn grayscale_flat()-> [[f32;5];5] {
    grayscale(0.5f32, 0.5f32, 0.5f32)
}

fn grayscale_bt709()-> [[f32;5];5] {
    grayscale(0.2125f32, 0.7154f32, 0.0721f32)
}
fn grayscale_ry()-> [[f32;5];5] {
    grayscale(0.5f32, 0.419f32, 0.081f32)
}
fn grayscale_y()-> [[f32;5];5] {
    grayscale(0.229f32, 0.587f32, 0.114f32)
}
fn grayscale_ntsc()-> [[f32;5];5] {
    grayscale_y()
}


//Warming Filter (85) #EC8A00
//Warming Filter (LBA) #FA9600
//Warming Filter (81) #EBB113
//Cooling Filter (80) #006DFF
//Cooling Filter (LBB) #005DFF
//Cooling Filter (82) #00B5FF
//Red #EA1A1A
//Orange #F38417
//Yellow #F9E31C
//Green #19C919
//Cyan #1DCBEA
//Blue #1D35EA
//Violet #9B1DEA
//Magenta #E318E3
//Sepia #AC7A33
//Deep Red #FF0000
//Deep Blue #0022CD
//Deep Emerald #008C00
//Deep Yellow #FFD500
//Underwater #00C1B1
struct Color{
    b: u8,
    g: u8,
    r: u8,
    a: u8
}

fn color_shift(c: &Color) -> [[f32;5];5]{
    let percent = f32::from(c.a) / 255.0f32;
    [
        [1f32 - percent, 0f32, 0f32, 0f32, 0f32],
        [0f32, 1f32 - percent, 0f32, 0f32, 0f32],
        [0f32, 0f32, 1f32 - percent, 0f32, 0f32],
        [0f32, 0f32, 0f32, 1f32, 0f32],
        [(f32::from(c.r) - 128f32) / 128f32 * percent, (f32::from(c.g) - 128f32) / 128f32 * percent, (f32::from(c.b) - 128f32) / 128f32 * percent, 0f32, 1f32]
    ]
}
fn invert() -> [[f32;5];5] {
    [
        [-1f32, 0f32, 0f32, 0f32, 0f32],
        [0f32, -1f32, 0f32, 0f32, 0f32],
        [0f32, 0f32, -1f32, 0f32, 0f32],
        [0f32, 0f32, 0f32, 1f32, 0f32],
        [1f32, 1f32, 1f32, 0f32, 1f32],
    ]
}


fn alpha(alpha: f32) -> [[f32;5];5] {
    //http://www.codeproject.com/KB/GDI-plus/CsTranspTutorial2.aspx
    [
        [1f32, 0f32, 0f32, 0f32, 0f32],
        [0f32, 1f32, 0f32, 0f32, 0f32],
        [0f32, 0f32, 1f32, 0f32, 0f32],
        [0f32, 0f32, 0f32, alpha, 0f32],
        [0f32, 0f32, 0f32, 0f32, 1f32],
    ]
}

fn contrast(c: f32) -> [[f32;5];5] {
    let c = c + 1f32; //Stop at -1

    let factor_t = 0.5f32 * (1.0f32 - c);
    [
        [c, 0f32, 0f32, 0f32, 0f32],
        [0f32, c, 0f32, 0f32, 0f32],
        [0f32, 0f32, c, 0f32, 0f32],
        [0f32, 0f32, 0f32, 1f32, 0f32],
        [factor_t, factor_t, factor_t, 0f32, 1f32],
    ]
}


fn brightness(factor: f32) -> [[f32;5];5] {
    [
        [1f32, 0f32, 0f32, 0f32, 0f32],
        [0f32, 1f32, 0f32, 0f32, 0f32],
        [0f32, 0f32, 1f32, 0f32, 0f32],
        [0f32, 0f32, 0f32, 1f32, 0f32],
        [factor, factor, factor, 0f32, 1f32],
    ]
}

// Saturation is between -1 and infinity

fn saturation(saturation: f32) -> [[f32;5];5] {
    //http://www.bobpowell.net/imagesaturation.htm
    let saturation = (saturation + 1f32).max(0f32); //Stop at -1

    let complement = 1.0f32 - saturation;
    let complement_r = 0.3086f32 * complement;
    let complement_g = 0.6094f32 * complement;
    let complement_b = 0.0820f32 * complement;
    [
        [complement_r + saturation, complement_r, complement_r, 0.0f32, 0.0f32],
        [complement_g, complement_g + saturation, complement_g, 0.0f32, 0.0f32],
        [complement_b, complement_b, complement_b + saturation, 0.0f32, 0.0f32],
        [0.0f32, 0.0f32, 0.0f32, 1.0f32, 0.0f32],
        [0.0f32, 0.0f32, 0.0f32, 0.0f32, 1.0f32],
    ]
}
