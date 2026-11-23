use crate::internal_prelude::works_everywhere::*;
use ::std::option::Option;

use ::std::cmp;
use ::std::option::Option::*;

use imageflow_types::{CompositingMode, PixelFormat, PixelLayout};

use super::bitmaps::Bitmap;

#[allow(clippy::too_many_arguments)]
pub fn copy_rectangle(
    input: &mut Bitmap,
    canvas: &mut Bitmap,
    from_x: u32,
    from_y: u32,
    to_x: u32,
    to_y: u32,
    w: u32,
    h: u32,
) -> Result<()> {
    if input.w() <= from_x
        || input.h() <= from_y
        || input.w() < from_x + w
        || input.h() < from_y + h
        || canvas.w() < to_x + w
        || canvas.h() < to_y + h
    {
        return Err(nerror!(crate::ErrorKind::InvalidArgument, "Invalid argument to copy_rect. Canvas is {}x{}, Input is {}x{}, Arguments provided: {:?}",
                         canvas.w(),
                         canvas.h(),
                         input.w(),
                         input.h(),
                         (from_x,from_y,to_x,to_y,w,h)));
    }

    canvas.set_compositing(super::bitmaps::BitmapCompositing::BlendWithSelf);

    let (canvas_layout, canvas_alpha_used) =
        (canvas.info().pixel_layout(), canvas.info().alpha_meaningful());
    let (input_layout, input_alpha_used) =
        (input.info().pixel_layout(), input.info().alpha_meaningful());
    let (mut canvas_fmt, input_fmt) = (
        canvas.info().calculate_pixel_format().map_err(|e| e.at(here!()))?,
        input.info().calculate_pixel_format().map_err(|e| e.at(here!()))?,
    );
    if canvas_fmt == PixelFormat::Bgr32 && input_fmt == PixelFormat::Bgra32 {
        // Normalize alpha, make alpha meaningful
        let mut window = canvas.get_window_u8().unwrap();
        window.normalize_unused_alpha().map_err(|e| e.at(here!()))?;
        canvas.set_alpha_meaningful(true);
        // Then update the canvas fmt
        canvas_fmt = canvas.info().calculate_pixel_format().map_err(|e| e.at(here!()))?;
    }
    let canvas_stride = canvas.info().t_stride();
    let input_stride = input.info().t_stride();
    let (canvas_w, canvas_h) = canvas.size();
    let (input_w, input_h) = input.size();
    let mut cropping_happened = input.is_cropped() || canvas.is_cropped();

    let (mut canvas_window, mut input_window) =
        (canvas.get_window_u8().unwrap(), input.get_window_u8().unwrap());

    if input_fmt == PixelFormat::Bgr32 && canvas_fmt == PixelFormat::Bgra32 {
        input_window.normalize_unused_alpha()?;
    }

    let mut from_window = input_window
        .window(from_x, from_y, w + from_x, h + from_y)
        .ok_or_else(|| {
            Some(nerror!(crate::ErrorKind::InvalidArgument, "Input window is not valid"))
        })
        .unwrap();
    let mut to_window = canvas_window
        .window(to_x, to_y, w + to_x, h + to_y)
        .ok_or_else(|| {
            Some(nerror!(crate::ErrorKind::InvalidArgument, "Canvas window is not valid"))
        })
        .unwrap();
    cropping_happened = cropping_happened || from_window.is_cropped() || to_window.is_cropped();

    if canvas_fmt == input_fmt
        || (canvas_fmt == PixelFormat::Bgra32 && input_fmt == PixelFormat::Bgr32)
    {
        let bytes_pp = input_fmt.bytes() as u32;
        //This optimization has the side effect of copying irrelevant data, so we don't want to do it if windowed, only
        // if padded or permanently cropped.
        if !cropping_happened && input_stride == canvas_stride {
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
    } else if input_fmt == PixelFormat::Bgr24 && canvas_layout == PixelLayout::BGRA {
        for mut row in from_window.scanlines().zip(to_window.scanlines()) {
            // Bgr24 [B, G, R] -> Bgra32 [B, G, R, 255] uses the exact same 3->4 append-0xff layout as rgb_to_rgba.
            garb::bytes::rgb_to_rgba(row.0.row(), row.1.row_mut()).map_err(|e| {
                nerror!(crate::ErrorKind::InvalidOperation, "Bgr24 -> Bgra32 copy failed: {:?}", e)
            })?;
        }
        Ok(())
    } else {
        Err(nerror!(
            crate::ErrorKind::InvalidOperation,
            "Cannot copy bytes from format {:?} to {:?}",
            input_fmt,
            canvas_fmt
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};

    #[test]
    fn copy_rectangle_bgr24_to_bgra32_preserves_bgr_and_sets_alpha_ff() {
        let mut src = Bitmap::create_u8(
            4,
            2,
            PixelLayout::BGR,
            false,
            false,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();
        {
            let mut win = src.get_window_u8().unwrap();
            let bgr_rows = [
                [10u8, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120],
                [15u8, 25, 35, 45, 55, 65, 75, 85, 95, 105, 115, 125],
            ];
            for (y, mut row) in win.scanlines().enumerate() {
                row.row_mut().copy_from_slice(&bgr_rows[y]);
            }
        }
        let mut dst = Bitmap::create_u8(
            4,
            2,
            PixelLayout::BGRA,
            true,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();

        copy_rectangle(&mut src, &mut dst, 0, 0, 0, 0, 4, 2).unwrap();

        let mut win = dst.get_window_u8().unwrap();
        let rows: Vec<Vec<u8>> = win.scanlines().map(|r| r.row().to_vec()).collect();
        assert_eq!(
            rows[0],
            vec![10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255]
        );
        assert_eq!(
            rows[1],
            vec![15, 25, 35, 255, 45, 55, 65, 255, 75, 85, 95, 255, 105, 115, 125, 255]
        );
    }
}
