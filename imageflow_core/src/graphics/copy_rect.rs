use crate::internal_prelude::works_everywhere::*;
use ::std::option::Option;
use crate::ffi::BitmapBgra;
use ::std::cmp;
use ::std::option::Option::*;
use num::Integer;

use imageflow_types::PixelFormat;

pub fn copy_rect(input: &mut BitmapBgra, canvas: &mut BitmapBgra, from_x: u32, from_y: u32, to_x: u32, to_y: u32, w: u32, h: u32) -> Result<()>{

    if input.w <= from_x || input.h <= from_y ||
        input.w < from_x + w ||
        input.h < from_y + h ||
        canvas.w < to_x + w ||
        canvas.h < to_y + h {
        return Err(nerror!(crate::ErrorKind::InvalidArgument, "Invalid argument to copy_rect. Canvas is {}x{}, Input is {}x{}, Arguments provided: {:?}",
                         canvas.w,
                         canvas.h,
                         input.w,
                         input.h,
                         (from_x,from_y,to_x,to_y,w,h)));
    }


    canvas.compositing_mode = crate::ffi::BitmapCompositingMode::BlendWithSelf;

    if canvas.fmt == input.fmt ||
        (canvas.fmt == PixelFormat::Bgra32 && input.fmt == PixelFormat::Bgr32){

        if input.fmt == PixelFormat::Bgr32 && canvas.fmt == PixelFormat::Bgra32{
            input.normalize_alpha()?;
        }

        let bytes_pp = input.fmt.bytes() as u32;
        if from_x == 0 && to_x == 0 && w == input.w && w == canvas.w &&
            input.stride == canvas.stride && input.stride == input.w * input.fmt.bytes() as u32{
            //This optimization has the side effect of copying irrelevant data, so we don't want to do it if windowed, only
            // if padded or permanently cropped.
            unsafe {
                let from_offset = input.stride * from_y;
                let from_ptr = input.pixels.offset(from_offset as isize);
                let to_offset = canvas.stride * to_y;
                let to_ptr = canvas.pixels.offset(to_offset as isize);
                ptr::copy_nonoverlapping(from_ptr, to_ptr, (input.stride * h) as usize);
            }
        } else {
            for row in 0..h {
                unsafe {
                    let from_offset = input.stride * (from_y + row) + bytes_pp * from_x;
                    let from_ptr = input.pixels.offset(from_offset as isize);
                    let to_offset = canvas.stride * (to_y + row) + bytes_pp * to_x;
                    let to_ptr = canvas.pixels.offset(to_offset as isize);

                    ptr::copy_nonoverlapping(from_ptr, to_ptr, (w * bytes_pp) as usize);
                }
            }
        }
        Ok(())
    } else if input.fmt == PixelFormat::Bgr24 && canvas.fmt.bytes() == 4{
        for row in 0..h {
            unsafe {
                let from_offset = input.stride * (from_y + row) + input.fmt.bytes() as u32 * from_x;
                let from_ptr = input.pixels.offset(from_offset as isize);
                let from_width = w as usize * input.fmt.bytes();
                let to_offset = canvas.stride * (to_y + row) + canvas.fmt.bytes() as u32 * to_x;
                let to_ptr = canvas.pixels.offset(to_offset as isize);
                let to_width = w as usize * canvas.fmt.bytes();

                let from_slice = slice::from_raw_parts_mut(from_ptr, from_width);
                let to_slice = slice::from_raw_parts_mut(to_ptr, to_width);

                for (from,to) in from_slice.chunks(3).zip(to_slice.chunks_mut(4)) {
                    to[0] = from[0];
                    to[1] = from[1];
                    to[2] = from[2];
                    to[3] = 0xff;
                }
            }
        }
        Ok(())
    } else {
        Err(nerror!(crate::ErrorKind::InvalidOperation, "Cannot copy bytes from format {:?} to {:?}", input.fmt, canvas.fmt))
    }


}