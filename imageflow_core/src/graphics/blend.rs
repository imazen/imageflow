use crate::graphics::prelude::*;
use itertools::Itertools;


//noinspection RsBorrowChecker,RsBorrowChecker
pub fn apply_matte(b: &mut BitmapWindowMut<u8>, matte_color: imageflow_types::Color) -> Result<(), FlowError>{
    // There's nothing to do unless it's BGRA
    if b.info().channels() != 4 || !b.info().alpha_meaningful() {
        return Ok(())
    }

    let colorcontext = ColorContext::new(WorkingFloatspace::LinearRGB,0f32);

    let matte = matte_color.to_color_32().map_err(|e| FlowError::from(e).at(here!()))?.to_bgra8();

    let alpha_to_float = (1.0f32) / 255.0f32;

    let matte_a = matte.a as f32 * alpha_to_float;
    let matte_b = colorcontext.srgb_to_floatspace(matte.b);
    let matte_g = colorcontext.srgb_to_floatspace(matte.g);
    let matte_r = colorcontext.srgb_to_floatspace(matte.r);

    for y in 0..b.h(){
        let mut row = b.row_window(y).unwrap();

        for mut pixel in row.slice_of_pixels().unwrap().iter_mut(){
            let pixel_a = (*pixel).a;
            let pixel_a_f32 = pixel_a as i32 as f32 * alpha_to_float;
            // if pixel_a == 0{
            //     *pixel = matte;
            // }else if pixel_a != 255{
                let matte_a = (1.0f32 - pixel_a_f32) * matte_a;
                let final_a: f32 = matte_a + pixel_a_f32;
                (*pixel).b = colorcontext.floatspace_to_srgb(
                    (colorcontext.srgb_to_floatspace(pixel.b) * pixel_a_f32 + matte_b * matte_a) / final_a);
                (*pixel).g = colorcontext.floatspace_to_srgb(
                    (colorcontext.srgb_to_floatspace(pixel.g) * pixel_a_f32 + matte_g * matte_a) / final_a);
                (*pixel).r = colorcontext.floatspace_to_srgb(
                    (colorcontext.srgb_to_floatspace(pixel.r) * pixel_a_f32 + matte_r * matte_a) / final_a);
                (*pixel).a =    uchar_clamp_ff(255f32 * final_a);
            //}
        }
    }

    Ok(())
}