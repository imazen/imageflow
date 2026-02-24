use imageflow_core;
use imageflow_core::graphics::bitmaps::BitmapWindowMut;
use imageflow_types::PixelLayout;
use std::{self, panic};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BitmapDiffStats {
    pub pixels: i64,
    pub pixels_differing: i64,
    pub pixels_differing_by_more_than_1: i64,
    pub values: i64,
    pub values_differing: i64,
    pub values_differing_by_more_than_1: i64,
    pub raw_unmultiplied_difference: i64,
    pub values_abs_delta_sum: f64,
    /// Maximum absolute difference across any single channel byte
    pub max_channel_delta: i64,
}

// impl add for BitmapDiffStats
impl std::ops::Add for BitmapDiffStats {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            pixels: self.pixels + other.pixels,
            pixels_differing: self.pixels_differing + other.pixels_differing,
            pixels_differing_by_more_than_1: self.pixels_differing_by_more_than_1
                + other.pixels_differing_by_more_than_1,
            values: self.values + other.values,
            values_differing: self.values_differing + other.values_differing,
            values_differing_by_more_than_1: self.values_differing_by_more_than_1
                + other.values_differing_by_more_than_1,
            raw_unmultiplied_difference: self.raw_unmultiplied_difference
                + other.raw_unmultiplied_difference,
            values_abs_delta_sum: self.values_abs_delta_sum + other.values_abs_delta_sum,
            max_channel_delta: self.max_channel_delta.max(other.max_channel_delta),
        }
    }
}

impl BitmapDiffStats {
    pub fn no_changes(pixels: i64) -> Self {
        Self {
            pixels,
            pixels_differing: 0,
            pixels_differing_by_more_than_1: 0,
            values: pixels * 4,
            values_differing: 0,
            values_differing_by_more_than_1: 0,
            raw_unmultiplied_difference: 0,
            values_abs_delta_sum: 0.0,
            max_channel_delta: 0,
        }
    }

    fn diff_bytes(a: &[u8], b: &[u8]) -> BitmapDiffStats {
        let mut stats = BitmapDiffStats::default();
        stats.pixels = a.len() as i64;
        stats.values = stats.pixels * 4;

        let one = 1_f32 / 255.0;

        for (a_pixel, b_pixel) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            let a_alpha = a_pixel[3] as f32 / 255.0;
            let b_alpha = b_pixel[3] as f32 / 255.0;

            if a_pixel != b_pixel {
                stats.pixels_differing += 1;

                // Calculate absolute difference for all channels
                for i in 0..4 {
                    let abs_diff_ch = (i64::from(a_pixel[i]) - i64::from(b_pixel[i])).abs();
                    stats.raw_unmultiplied_difference += abs_diff_ch;
                    stats.max_channel_delta = stats.max_channel_delta.max(abs_diff_ch);
                    if abs_diff_ch > 1 {
                        stats.values_differing += 1;
                    }
                }

                let mut this_pixel_differing_by_more_than_1 = 0;

                // Calculate premultiplied delta for RGB channels
                for i in 0..3 {
                    let a_premultiplied = a_pixel[i] as f32 * a_alpha;
                    let b_premultiplied = b_pixel[i] as f32 * b_alpha;
                    let diff = (a_premultiplied - b_premultiplied).abs();
                    stats.values_abs_delta_sum += diff as f64;
                    if diff > one {
                        this_pixel_differing_by_more_than_1 += 1;
                    }
                }

                // Add alpha channel difference to premultiplied delta
                let alpha_diff = (i64::from(a_pixel[3]) - i64::from(b_pixel[3])).abs();
                stats.values_abs_delta_sum += alpha_diff as f64;
                if alpha_diff > 1 {
                    this_pixel_differing_by_more_than_1 += 1;
                }

                stats.values_differing_by_more_than_1 += this_pixel_differing_by_more_than_1;
                if this_pixel_differing_by_more_than_1 > 1 {
                    stats.pixels_differing_by_more_than_1 += 1;
                }
            }
        }
        stats
    }

    pub fn diff_bitmap_windows(
        a: &mut BitmapWindowMut<u8>,
        b: &mut BitmapWindowMut<u8>,
    ) -> BitmapDiffStats {
        if a.w() != b.w() || a.h() != b.h() || a.info().pixel_layout() != b.info().pixel_layout() {
            panic!("Bitmap dimensions differ. a:\n{:#?}\nb:\n{:#?}", a, b);
        }
        if a.info().pixel_layout() != PixelLayout::BGRA {
            panic!("Bitmap layout is not BGRA");
        }

        a.scanlines()
            .zip(b.scanlines())
            .map(|(a_scanline, b_scanline)| {
                if a_scanline.row() == b_scanline.row() {
                    BitmapDiffStats::no_changes(a_scanline.row().len() as i64 / 4)
                } else {
                    Self::diff_bytes(a_scanline.row(), b_scanline.row())
                }
            })
            .fold(BitmapDiffStats::no_changes(0), |a, b| a + b)
    }

    pub fn legacy_report(&self) -> String {
        if self.pixels_differing == 0 {
            return "no pixel differences".to_string();
        }
        let abs_degree =
            self.raw_unmultiplied_difference as f64 / (self.pixels_differing * 4) as f64;

        let pixels_that_differ_percent = self.pixels_differing as f64 * 100f64 / self.pixels as f64;

        format!("max channel delta {} | {} pixels differ ({:.3}% of {}) | avg abs err/channel {:.4} (total {})",
                            self.max_channel_delta,
                            self.pixels_differing,
                            pixels_that_differ_percent, self.pixels,
                            abs_degree, self.raw_unmultiplied_difference)
    }
}
