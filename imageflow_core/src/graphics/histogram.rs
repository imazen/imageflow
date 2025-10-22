use rgb::Bgra;

use crate::graphics::prelude::*;

/// Populates a histogram from a window of a bitmap.
/// histogram order is RGB
pub fn populate_histogram_from_window(
    window: &mut BitmapWindowMut<Bgra<u8, u8>>,
    histograms: &mut [[u64; 256]; 3],
) -> Result<(), FlowError> {
    let [r, g, b] = histograms;
    for line in window.scanlines() {
        for pixel in line.row() {
            // Safe because histogram size is 256, which u8 cannot overflow
            unsafe {
                *r.get_unchecked_mut(pixel.r as usize) += 1;
                *g.get_unchecked_mut(pixel.g as usize) += 1;
                *b.get_unchecked_mut(pixel.b as usize) += 1;
            }
        }
    }
    Ok(())
}
