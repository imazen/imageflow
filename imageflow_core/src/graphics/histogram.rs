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
            r[pixel.r as usize] += 1;
            g[pixel.g as usize] += 1;
            b[pixel.b as usize] += 1;
        }
    }
    Ok(())
}
