use imageflow_types::{Color, RoundCornersMode};
use crate::graphics::prelude::*;



fn get_radius_pixels(radius: RoundCornersMode, w: u32, h: u32) -> Result<f32, FlowError>{
    match radius{
        RoundCornersMode::Percentage(p) =>  Ok(w.min(h) as f32 * p / 200f32),
        RoundCornersMode::Pixels(p) => Ok(p),
        RoundCornersMode::Circle =>  Err(unimpl!("RoundCornersMode::Circle is not implemented")),
        RoundCornersMode::PercentageCustom {.. } => Err(unimpl!("RoundCornersMode::PercentageCustom is not implemented")),
        RoundCornersMode::PixelsCustom {.. } => Err(unimpl!("RoundCornersMode::PixelsCustom is not implemented"))
    }
}

pub unsafe fn flow_bitmap_bgra_clear_around_rounded_corners(
    b: &mut BitmapWindowMut<u8>,
    radius_mode: RoundCornersMode,
    color: imageflow_types::Color
) -> Result<(), FlowError> {
    if  b.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::InvalidArgument));
    }

    let radius = get_radius_pixels(radius_mode, b.w(), b.h())?;
    let radius_ceil = radius.ceil() as usize;

    let rf = radius as f32;
    let r2f = rf * rf;

    let mut clear_widths = Vec::with_capacity(radius_ceil);
    for y in (0..=radius_ceil).rev(){
        let yf = y as f32 - 0.5;
        clear_widths.push(radius_ceil - f32::sqrt(r2f - yf * yf).round() as usize);
    }

    let bgcolor = color.to_color_32().unwrap().to_bgra8();

    let radius_usize = radius_ceil;
    let width = b.w() as usize;
    let height = b.h() as usize;

    //eprintln!("color {},{},{},{:?}", bgcolor.r, bgcolor.g, bgcolor.b, bgcolor.a);

    for y in 0..height{
        if y <= radius_usize || y >= height - radius_usize {
            let mut row = b.row_window(y as u32).unwrap();

            let row_width = row.w();
            let slice = row.slice_of_pixels_first_row().unwrap();

            let pixels_from_bottom = height - y - 1;

            let nearest_line_index = y.min(pixels_from_bottom);

            let mut clear_width = if nearest_line_index < clear_widths.len() {
                clear_widths[nearest_line_index]
            } else {
                0
            };

            //eprintln!("row width {}, slice width {}, bitmap width {}", row_width, slice.len(), width);
            if slice.len() != width { panic!("Width mismatch bug"); }

            clear_width = clear_width.min(width);

            if clear_width > 0 {
                //eprintln!("clear {}", clear_width);
                slice[0..clear_width].fill(bgcolor.clone());
                slice[width-clear_width..width].fill(bgcolor.clone());
            }

        }
    }


    Ok(())
}
