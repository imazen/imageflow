use crate::graphics::prelude::*;

pub unsafe fn flow_bitmap_bgra_clear_around_rounded_corners(
    b: &mut BitmapWindowMut<u8>,
    radius: u32,
    color: imageflow_types::Color
) -> Result<(), FlowError> {
    if  b.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(ErrorKind::InvalidArgument));
    }

    let rf = radius as f32;
    let r2f = rf * rf;

    let mut clear_widths = Vec::with_capacity(radius as usize);
    for y in (0..=radius).rev(){
        let yf = y as f32 - 0.5;
        let fwidth = f32::sqrt(r2f - yf * yf);
        let uwidth = (radius - fwidth.round() as u32) as usize;
        clear_widths.push(uwidth);
    }

    let bgcolor = color.to_color_32().unwrap().to_bgra8();

    let radius_usize = radius as usize;
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
