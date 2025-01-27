use crate::graphics::prelude::*;
// existing code...

// Step 1: Add import for Bitmap to leverage safe windowing/scanline operations, similar to copy_rect
use crate::graphics::bitmaps::Bitmap;
use imageflow_types::{PixelFormat, PixelLayout};

// Step 2: Port flow_bitmap_bgra_flip_vertical to a safe, window-based approach
/// Safely flip a bitmap vertically by swapping rows top-to-bottom.
pub fn flow_bitmap_bgra_flip_vertical_safe(b: &mut Bitmap) -> Result<(), FlowError> {
    // Step 2a: Determine image size and retrieve a safe window into the bitmap
    let (width, height) = b.size();
    let mut top = b
        .get_window_u8()
        .ok_or_else(|| nerror!(ErrorKind::InvalidArgument, "No valid window"))?;
    let mut bottom_half = top.split_off((height / 2) as u32).unwrap();

    for (mut top_row, mut bottom_row) in top.scanlines()
                        .zip(bottom_half.scanlines_reverse()) {
        top_row.row_mut().swap_with_slice(&mut bottom_row.row_mut());
    }
    Ok(())
}

// Step 3: Port flow_bitmap_bgra_flip_horizontal to a safe, window-based approach
/// Safely flip a bitmap horizontally by reversing each row in place.
pub fn flow_bitmap_bgra_flip_horizontal_safe(b: &mut Bitmap) -> Result<(), FlowError> {
    // Step 3a: Determine image size and retrieve a safe window
    let (width, height) = b.size();
    let pixel_format = b.info().calculate_pixel_format().map_err(|e| e.at(here!()))?;
    let bytes_per_pixel = pixel_format.bytes() as usize;

    let mut window = b
        .get_window_u8()
        .ok_or_else(|| nerror!(ErrorKind::InvalidArgument, "No valid window"))?;

    // Step 3b: Reverse each row, pixel pair by pixel pair
    let half_width = width / 2;
    let offset = width % 2;
    // Iterate over each row
    for mut scanline in window.scanlines_bgra().unwrap() {
        // Grab a mutable slice for this row

        let (left_slice, mut right_slice) =
                    scanline.row_mut().split_at_mut(half_width);

        right_slice = &mut right_slice[offset..];

        for (a, b) in
        left_slice.iter_mut().zip(right_slice.iter_mut().rev()) {
            std::mem::swap(a, b);
        }
    }
    Ok(())
}

// existing code...
