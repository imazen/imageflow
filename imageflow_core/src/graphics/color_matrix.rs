use rgb::alt::BGRA8;

use crate::graphics::prelude::*;

pub fn window_bgra32_apply_color_matrix(window: &mut BitmapWindowMut<BGRA8>, m: &[[f32;5];5]) -> Result<(), FlowError>{

    let m40: f32 = m[4][0] * 255.0f32;
    let m41: f32 = m[4][1] * 255.0f32;
    let m42: f32 = m[4][2] * 255.0f32;
    let m43: f32 = m[4][3] * 255.0f32;

    for mut line in window.scanlines(){
        for pixel in line.row_mut(){
            let r = pixel.r as f32;
            let g = pixel.g as f32;
            let b = pixel.b as f32;
            let a = pixel.a as f32;

            pixel.r = uchar_clamp_ff(m[0][0] * r + m[1][0] * g + m[2][0] * b + m[3][0] * a + m40);
            pixel.g = uchar_clamp_ff(m[0][1] * r + m[1][1] * g + m[2][1] * b + m[3][1] * a + m41);
            pixel.b = uchar_clamp_ff(m[0][2] * r + m[1][2] * g + m[2][2] * b + m[3][2] * a + m42);
            pixel.a = uchar_clamp_ff(m[0][3] * r + m[1][3] * g + m[2][3] * b + m[3][3] * a + m43);
        }
    }
    Ok(())
}
