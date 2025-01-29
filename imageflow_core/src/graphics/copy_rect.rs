use crate::internal_prelude::works_everywhere::*;
use ::std::option::Option;

use ::std::cmp;
use ::std::option::Option::*;
use num::Integer;

use imageflow_types::{CompositingMode, PixelFormat, PixelLayout};

use super::bitmaps::Bitmap;

pub fn copy_rectangle(input: &mut Bitmap, canvas: &mut Bitmap, from_x: u32, from_y: u32, to_x: u32, to_y: u32, w: u32, h: u32) -> Result<()>{

    if input.w() <= from_x || input.h() <= from_y ||
        input.w() < from_x + w ||
        input.h() < from_y + h ||
        canvas.w() < to_x + w ||
        canvas.h() < to_y + h {
        return Err(nerror!(crate::ErrorKind::InvalidArgument, "Invalid argument to copy_rect. Canvas is {}x{}, Input is {}x{}, Arguments provided: {:?}",
                         canvas.w(),
                         canvas.h(),
                         input.w(),
                         input.h(),
                         (from_x,from_y,to_x,to_y,w,h)));
    }

    canvas.set_compositing(super::bitmaps::BitmapCompositing::BlendWithSelf);

    let (canvas_layout, canvas_alpha_used) = (canvas.info().pixel_layout(), canvas.info().alpha_meaningful());
    let (input_layout, input_alpha_used) = (input.info().pixel_layout(), input.info().alpha_meaningful());
    let (canvas_fmt, input_fmt) =
    (canvas.info().calculate_pixel_format().map_err(|e| e.at(here!()))?,
                    input.info().calculate_pixel_format().map_err(|e| e.at(here!()))?);
    let canvas_stride = canvas.info().item_stride();
    let input_stride = input.info().item_stride();
    let (canvas_w, canvas_h) = canvas.size();
    let (input_w, input_h) = input.size();
    let mut cropping_happened = input.is_cropped() || canvas.is_cropped();

    let (mut canvas_window, mut input_window) = (canvas.get_window_u8().unwrap(), input.get_window_u8().unwrap());

    if input_fmt == PixelFormat::Bgr32 && canvas_fmt == PixelFormat::Bgra32{
        input_window.normalize_unused_alpha()?;
    }

    let mut from_window = input_window.window(from_x, from_y, w + from_x, h + from_y)
        .ok_or_else(|| Some(nerror!(crate::ErrorKind::InvalidArgument, "Input window is not valid")))
        .unwrap();
    let mut to_window = canvas_window.window(to_x, to_y, w + to_x, h + to_y)
        .ok_or_else(|| Some(nerror!(crate::ErrorKind::InvalidArgument, "Canvas window is not valid")))
        .unwrap();
    cropping_happened = cropping_happened || from_window.is_cropped() || to_window.is_cropped();


    if canvas_fmt == input_fmt ||
        (canvas_fmt == PixelFormat::Bgra32 && input_fmt == PixelFormat::Bgr32){

        let bytes_pp = input_fmt.bytes() as u32;
        //This optimization has the side effect of copying irrelevant data, so we don't want to do it if windowed, only
            // if padded or permanently cropped.
        if !cropping_happened && input_stride == canvas_stride{
            let from_slice = from_window.slice_mut();
            let to_slice = to_window.slice_mut();
            to_slice.copy_from_slice(from_slice);
        } else {
            for mut row in from_window.scanlines().zip(to_window.scanlines()) {
                row.1.row_mut().copy_from_slice(row.0.row());
            }
        }
        Ok(())
        // The next branch is for 24->32 (of any type)
    } else if input_fmt == PixelFormat::Bgr24 && canvas_layout == PixelLayout::BGRA{
        for mut row in from_window.scanlines_bgra().unwrap().zip(to_window.scanlines_bgra().unwrap()) {
            for (from,to) in row.0.row().into_iter().zip(row.1.row_mut()) {
                to.a = 0xff;
                to.r = from.r;
                to.g = from.g;
                to.b = from.b;
            }
        }
        Ok(())
    } else {
        Err(nerror!(crate::ErrorKind::InvalidOperation, "Cannot copy bytes from format {:?} to {:?}", input_fmt, canvas_fmt))
    }


}
