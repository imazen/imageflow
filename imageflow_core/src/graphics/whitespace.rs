use crate::internal_prelude::works_everywhere::*;
use ::std::option::Option;
use crate::ffi::BitmapBgra;
use ::std::cmp;
use ::std::option::Option::*;
use num::Integer;

use imageflow_types::PixelFormat;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ScanEdge {
    Top,
    Right,
    Bottom,
    Left,
    NonDirectional,
}

#[derive(Copy, Clone, Debug)]
struct ScanRegion {
    edge: ScanEdge,
    x_1_percent: f32,
    y_1_percent: f32,
    x_2_percent: f32,
    y_2_percent: f32,
}

/// Thin horizontal and vertical search strips. Whitespace detection is essentially an ensured absence of sobel energy (or another kernel)
/// Finding energy in these tiny strips means we can report home quickly that there is nothing to trim.
const SCAN_QUICK_REGIONS: [ScanRegion; 12] = [
    // left half, middle, ->
    ScanRegion { edge: ScanEdge::Left, x_1_percent: 0f32, x_2_percent: 0.5f32, y_1_percent: 0.5f32, y_2_percent: 0.5f32 },
    // right half, middle, <-
    ScanRegion { edge: ScanEdge::Right, x_1_percent: 0.5f32, x_2_percent: 1f32, y_1_percent: 0.5f32, y_2_percent: 0.5f32 },

    // left half, bottom third ->
    ScanRegion { edge: ScanEdge::Left, x_1_percent: 0f32, x_2_percent: 0.5f32, y_1_percent: 0.677f32, y_2_percent: 0.677f32 },
    // right half, bottom third -<
    ScanRegion { edge: ScanEdge::Right, x_1_percent: 0.5f32, x_2_percent: 1f32, y_1_percent: 0.677f32, y_2_percent: 0.677f32 },
    // left half, top third ->
    ScanRegion { edge: ScanEdge::Left, x_1_percent: 0f32, x_2_percent: 0.5f32, y_1_percent: 0.333f32, y_2_percent: 0.333f32 },
    // right half, top third -<
    ScanRegion { edge: ScanEdge::Right, x_1_percent: 0.5f32, x_2_percent: 1f32, y_1_percent: 0.333f32, y_2_percent: 0.333f32 },

    // top half, center \/
    ScanRegion { edge: ScanEdge::Top, x_1_percent: 0.5f32, x_2_percent: 0.5f32, y_1_percent: 0f32, y_2_percent: 0.5f32 },
    // top half, right third
    ScanRegion { edge: ScanEdge::Top, x_1_percent: 0.677f32, x_2_percent: 0.677f32, y_1_percent: 0f32, y_2_percent: 0.5f32 },
    // top half, left third.
    ScanRegion { edge: ScanEdge::Top, x_1_percent: 0.333f32, x_2_percent: 0.333f32, y_1_percent: 0f32, y_2_percent: 0.5f32 },

    // bottom half, center \/
    ScanRegion { edge: ScanEdge::Bottom, x_1_percent: 0.5f32, x_2_percent: 0.5f32, y_1_percent: 0.5f32, y_2_percent: 1f32 },
    // bottom half, right third
    ScanRegion { edge: ScanEdge::Bottom, x_1_percent: 0.677f32, x_2_percent: 0.677f32, y_1_percent: 0.5f32, y_2_percent: 1f32 },
    // bottom half, left third.
    ScanRegion { edge: ScanEdge::Bottom, x_1_percent: 0.333f32, x_2_percent: 0.333f32, y_1_percent: 0.5f32, y_2_percent: 1f32 },
];

const SCAN_EVERYTHING_INWARD: [ScanRegion; 4] = [
    ScanRegion { edge: ScanEdge::Top, x_1_percent: 0f32, x_2_percent: 1f32, y_1_percent: 0f32, y_2_percent: 1f32 },
    ScanRegion { edge: ScanEdge::Right, x_1_percent: 0f32, x_2_percent: 1f32, y_1_percent: 0f32, y_2_percent: 1f32 },
    ScanRegion { edge: ScanEdge::Bottom, x_1_percent: 0f32, x_2_percent: 1f32, y_1_percent: 0f32, y_2_percent: 1f32 },
    ScanRegion { edge: ScanEdge::Left, x_1_percent: 0f32, x_2_percent: 1f32, y_1_percent: 0f32, y_2_percent: 1f32 },
];
const SCAN_FULL: ScanRegion = ScanRegion {
    edge: ScanEdge::NonDirectional,
    x_1_percent: 0f32,
    x_2_percent: 1f32,
    y_1_percent: 0f32,
    y_2_percent: 1f32,
};

pub struct RectCorners {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Rect {
    fn to_corners_checked(&self) -> Option<RectCorners> {
        let x2 = self.x.checked_add(self.w);
        let y2 = self.y.checked_add(self.h);
        if let Some(x2) = self.x.checked_add(self.w) {
            if let Some(y2) = self.y.checked_add(self.h) {
                return Some(RectCorners {
                    x1: self.x,
                    y1: self.y,
                    x2,
                    y2,
                });
            }
        }
        return None;
    }
}

/// Grayscale buffer for running sobel edge detection filters

struct Buffer {
    pixels: [u8; 2048],
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Default for Buffer {
    fn default() -> Buffer {
        Buffer {
            pixels: [0u8; 2048],
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }
    }
}


struct WhitespaceSearch {
    w: u32,
    h: u32,
    threshold: u32,
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
}

impl WhitespaceSearch {
    fn get_search_rect(&self, region: &ScanRegion) -> Option<RectCorners> {
        let mut x1 = std::cmp::max(0u32, cmp::min(self.w, (region.x_1_percent * (self.w - 1) as f32).floor() as u32));
        let mut x2 = std::cmp::max(0u32, cmp::min(self.w, (region.x_2_percent * (self.w - 1) as f32).floor() as u32));
        let mut y1 = std::cmp::max(0u32, cmp::min(self.h, (region.y_1_percent * (self.h - 1) as f32).floor() as u32));
        let mut y2 = std::cmp::max(0u32, cmp::min(self.h, (region.y_2_percent * (self.h - 1) as f32).floor() as u32));

        // Snap the boundary depending on which side we're searching
        match region.edge {
            ScanEdge::Left => {
                x1 = 0;
                x2 = cmp::min(x2, self.min_x);
            }
            ScanEdge::Right => {
                x1 = cmp::max(x1, self.max_x);
                x2 = self.w;
            }
            ScanEdge::Top => {
                y1 = 0;
                y2 = cmp::min(y2, self.min_y);
            }
            ScanEdge::Bottom => {
                y1 = cmp::max(y1, self.max_y);
                y2 = self.h;
            }
            ScanEdge::NonDirectional => {
                // No need to do anything
            }
        }

        if x1 == x2 || y1 == y2 {
            return None; //Nothing left to search
        }

        // Let's make sure that we're searching at least 7 pixels in the perpendicular direction
        let min_region_width = if region.edge == ScanEdge::Right || region.edge == ScanEdge::Left { 3 } else { 7 };
        let min_region_height = if region.edge == ScanEdge::Top || region.edge == ScanEdge::Bottom { 3 } else { 7 };

        while y2 - y1 < min_region_height && (y1 > 0 || y2 < self.h) {
            y1 = if y1 > 0 { y1 - 1 } else { 0 };
            y2 = cmp::min(self.h, y2 + 1);
        }
        while x2 - x1 < min_region_width && (x1 > 0 || x2 < self.w) {
            x1 = if x1 > 0 { x1 - 1 } else { 0 };
            x2 = cmp::min(self.w, x2 + 1);
        }

        Some(RectCorners {
            x1,
            y1,
            x2,
            y2,
        })
    }
}

pub fn detect_content(b: &BitmapBgra, threshold: u32) -> Option<RectCorners> {
    if b.w > i32::max_value() as u32 || b.h > i32::max_value() as u32 {
        panic!("Bitmap dimension overflow")
    }
    if b.w < 3 || b.h < 3 {
        return Some(RectCorners { x1: 0, x2: b.w, y1: 0, y2: b.h });
    }

    let mut search = WhitespaceSearch {
        w: b.w,
        h: b.h,
        threshold,
        max_x: 0,
        max_y: 0,
        min_y: b.h,
        min_x: b.w,
    };
    let mut buf = Buffer { pixels: [0u8; 2048], x: 0, w: 0, h: 0, y: 0 };
    // Let's aim for a minimum dimension of 7px per window
    // We want to glean as much as possible from horizontal strips, as they are faster.
    for region in SCAN_QUICK_REGIONS.iter() {
        check_region(&mut search, &mut buf, b, region)
    }
    // We should now have a good idea of where boundaries lie. However... if it seems that more than 25% is whitespace,
    // we should do a different type of scan.
    let area_to_scan_separately: i64 = search.min_x as i64 * search.h as i64 + search.min_y as i64 * search.w as i64 + (search.w as i64 - search.max_x as i64) * search.h as i64
        + (search.h as i64 - search.max_y as i64) * search.h as i64;

    if area_to_scan_separately > (search.h as i64 * search.w as i64) {
        // Just scan it all at once, non-directionally
        check_region(&mut search, &mut buf, b, &SCAN_FULL)
    } else {

        // Finish by scanning everything that is left. Should be a smaller set.
        // Corners will overlap, and be scanned twice, if they are whitespace.
        for region in SCAN_EVERYTHING_INWARD.iter() {
            check_region(&mut search, &mut buf, b, region)
        }
    }
    // Consider the entire image as content if it is blank (or we were unable to detect any energy).

    if search.min_x == b.w && search.max_x == 0 && search.min_y == b.h && search.max_y == 0 {
        Some(RectCorners { x1: 0, y1: 0, x2: search.w, y2: search.h })
    } else {
        Some(RectCorners {
            x1: search.min_x,
            x2: search.max_x,
            y1: search.min_y,
            y2: search.max_y,
        })
    }
}

fn check_region(search: &mut WhitespaceSearch, buf: &mut Buffer, b: &BitmapBgra, region: &ScanRegion) {
    if let Some(RectCorners { x1, y1, x2, y2 }) = search.get_search_rect(region) {
        let w = x2 - x1;
        let h = y2 - y1;

        // Now we need to split this section into regions that fit in the buffer. Might as well do it vertically, so our
        // scans are minimal.

        let buf_size = buf.pixels.len() as u32;
        // If we are doing a full scan, make them wide along the X axis. Otherwise, make them square.
        let window_width = cmp::min(w, if region.edge == ScanEdge::NonDirectional {
            buf_size / 7u32
        } else {
            (buf_size as f32).sqrt().ceil() as u32
        });
        let window_height = cmp::min(h, buf_size / window_width);

        let vertical_windows = (h as f32 / (window_height - 2) as f32).ceil() as u32;
        let horizontal_windows = (w as f32 / (window_width - 2) as f32).ceil() as u32;

        for window_row in 0..vertical_windows {
            for window_column in 0..horizontal_windows {
                let mut buf = Buffer::default();


                // Set up default overlapping windows. These may be shrunk or shifted later
                buf.x = x1 + ((window_width - 2) * window_column);
                buf.y = y1 + ((window_height - 2) * window_row);
                buf.w = cmp::min(cmp::max(3, x2 - buf.x), window_width);
                buf.h = cmp::min(cmp::max(3, y2 - buf.y), window_height);
                let buf_x2 = buf.x + buf.w;
                let buf_y2 = buf.y + buf.h;

                let excluded_x = search.min_x < buf.x && search.max_x > buf_x2;
                let excluded_y = search.min_y < buf.y && search.max_y > buf_y2;

                if excluded_x && excluded_y {
                    // Entire window has already been excluded
                    continue;
                }
                if excluded_y && search.min_x < buf_x2 && buf_x2 < search.max_x {
                    buf.w = cmp::max(3, search.min_x - buf.x);
                } else if excluded_y && search.max_x > buf.x && buf.x > search.min_x {
                    buf.x = cmp::min(buf_x2 - 3, search.max_x);
                    buf.w = buf_x2 - buf.x;
                }
                if excluded_x && search.min_y < buf_y2 && buf_y2 < search.max_y {
                    buf.h = cmp::max(3, search.min_y - buf.y);
                } else if excluded_x && search.max_y > buf.y && buf.y > search.min_y {
                    buf.y = cmp::min(buf_y2 - 3, search.max_y);
                    buf.h = buf_y2 - buf.y;
                }

                // Shift window back within image bounds
                if buf.y + buf.h > search.h {
                    if buf.h <= search.h {
                        buf.y = search.h - buf.h;
                    } else {
                        // We tried to make the buffer wider than the image; reduce
                        buf.y = 0;
                        buf.h = search.h;
                    }
                }
                if buf.x + buf.w > search.w {
                    if buf.w <= search.w {
                        buf.x = search.w - buf.w;
                    } else {
                        // We tried to make the buffer wider than the image
                        buf.x = 0;
                        buf.w = search.w;
                    }
                }

                fill_grayscale_buffer_from_bitmap(&mut buf, b);

                sobel_scharr_detect(&buf, search);
            }
        }
    }
}

///
/// Computes a fast/approximated grayscale subset of the given bitmap
fn fill_grayscale_buffer_from_bitmap(buf: &mut Buffer, b: &BitmapBgra) {
    approximate_grayscale(&mut buf.pixels, buf.w as usize,buf.x, buf.y, buf.w, buf.h, b)
}
pub fn approximate_grayscale(grayscale: &mut [u8], grayscale_stride: usize, x: u32, y: u32, w: u32, h: u32, source_bitmap: &BitmapBgra) {
        /* Red: 0.299;
Green: 0.587;
Blue: 0.114;
*/
    let b = source_bitmap;

    if grayscale_stride < w as usize{
        panic!("Invalid grayscale_Stride")
    }

    let bytes_per_pixel = b.fmt.bytes();
    let first_pixel = b.stride as usize * y as usize + bytes_per_pixel * x as usize;
    let remnant: usize = b.stride as usize - (bytes_per_pixel * w as usize);
    let gray_remnant: usize = grayscale_stride - w as usize;
    unsafe {
        let bitmap_bytes_accessed = b.stride as usize * (y + h - 1) as usize + (bytes_per_pixel * (x + w) as usize);
        if bitmap_bytes_accessed > b.stride as usize * b.h as usize {
            panic!("Out of bounds bitmap access prevented");
        }

        let input_bitmap = b.pixels_slice().unwrap();
        let mut input_index = b.stride as usize * y as usize + bytes_per_pixel * x as usize;
        match b.fmt {
            PixelFormat::Bgra32 => {
                let mut buf_ix = 0usize;
                for y in 0..h {
                    for x in 0..w {
                        let bgra = &input_bitmap[input_index..input_index + 4];
                        let gray = (((233 * bgra[0] as u32 + 1197 * bgra[1] as u32 + 610 * bgra[2] as u32) * bgra[3] as u32 + 524288 - 1) / 524288) as u16;
                        grayscale[buf_ix] = if gray > 255 { 255 } else { gray as u8 };
                        input_index += 4;
                        buf_ix += 1;
                    }
                    buf_ix += gray_remnant;
                    input_index += remnant;
                }
            }
            PixelFormat::Bgr24 => {
                let mut buf_ix = 0usize;
                for y in 0..h {
                    for x in 0..w {
                        let bgra = &input_bitmap[input_index..input_index + 3];
                        grayscale[buf_ix] = ((233 * bgra[0] as u32 + 1197 * bgra[1] as u32 + 610 * bgra[2] as u32) / 2048) as u8;
                        input_index += 3;
                        buf_ix += 1;
                    }
                    buf_ix += gray_remnant;
                    input_index += remnant;
                }
            }
            PixelFormat::Bgr32 => {
                let mut buf_ix = 0usize;
                for y in 0..h {
                    for x in 0..w {
                        let bgra = &input_bitmap[input_index..input_index + 4];
                        grayscale[buf_ix] = ((233 * bgra[0] as u32 + 1197 * bgra[1] as u32 + 610 * bgra[2] as u32) / 2048) as u8;
                        input_index += 4;
                        buf_ix += 1;
                    }
                    buf_ix += gray_remnant;
                    input_index += remnant;
                }
            }
            PixelFormat::Gray8 => {
                let mut buf_ix = 0usize;
                for y in 0..h {
                    for x in 0..w {
                        let bgra = &input_bitmap[input_index..input_index + 1];
                        grayscale[buf_ix] = bgra[0];
                        input_index += 1;
                        buf_ix += 1;
                    }
                    buf_ix += gray_remnant;
                    input_index += remnant;
                }
            }
        }
    }
}

fn sobel_scharr_detect(buf: &Buffer, search: &mut WhitespaceSearch) {
    const COEFFA: u8 = 3;
    const COEFFB: u8 = 10;
    let w = buf.w as usize;
    let h = buf.h as usize;
    let y_end = h - 1;
    let x_end = w - 1;
    let threshold = search.threshold as i32;
    let mut buf_ix = w as usize + 1;
    for y in 1..y_end {
        for x in 1..x_end {
            let a11 = buf.pixels[buf_ix - w - 1];
            let a12 = buf.pixels[buf_ix - w];
            let a13 = buf.pixels[buf_ix - w + 1];
            let a21 = buf.pixels[buf_ix - 1];
            let a22 = buf.pixels[buf_ix];
            let a23 = buf.pixels[buf_ix + 1];
            let a31 = buf.pixels[buf_ix + w - 1];
            let a32 = buf.pixels[buf_ix + w];
            let a33 = buf.pixels[buf_ix + w + 1];

            // We have not implemented any aliasing operations. A checkerboard of pixels will produce zeroes.
            let gx: i32 = 3 * a11 as i32 + 10 * a21 as i32 + 3 * a31 as i32 + -3 * a13 as i32 + -10 * a23 as i32 + -3 * a33 as i32;

            let gy: i32 = 3 * a11 as i32 + 10 * a12 as i32 + 3 * a13 as i32 + -3 * a31 as i32 + -10 * a32 as i32 + -3 * a33 as i32;

            let scharr_value: i32 = gx.abs() + gy.abs();

            if scharr_value > threshold {
                // Now use differences to find the exact pixel
                let matrix = [a11, a12, a13, a21, a22, a23, a31, a32, a33];

                let mut local_min_x = 2u32;
                let mut local_min_y = 2u32;
                let mut local_max_x = 1u32;
                let mut local_max_y = 1u32;

                // Check horizontal and vertical differences.
                for my in 0..3usize {
                    let mut edge_found = false;
                    if (matrix[my * 3] as i32 - matrix[my * 3 + 1] as i32).abs() > threshold {
                        // vertical edge between x = 0,1
                        local_min_x = cmp::min(local_min_x, 1);
                        local_max_x = cmp::max(local_max_x, 1);
                        edge_found = true;
                    }
                    if (matrix[my * 3 + 1] as i32 - matrix[my * 3 + 2] as i32).abs() > threshold {
                        // vertical edge between x = 1,2
                        local_min_x = cmp::min(local_min_x, 2);
                        local_max_x = cmp::max(local_max_x, 2);
                        edge_found = true;
                    }
                    if edge_found {
                        local_min_y = cmp::min(local_min_y, my as u32);
                        local_max_y = cmp::max(local_max_y, my as u32 + 1);
                    }
                }
                for mx in 0..3usize {
                    let mut edge_found = false;
                    if (matrix[mx] as i32 - matrix[mx + 3] as i32).abs() > threshold {
                        // horizontal edge between y = 0,1
                        local_min_y = cmp::min(local_min_y, 1);
                        local_max_y = cmp::max(local_max_y, 1);
                        edge_found = true;
                    }
                    if (matrix[mx + 3] as i32 - matrix[mx + 6] as i32).abs() > threshold {
                        // horizontal edge between y = 1,2
                        local_min_y = cmp::min(local_min_y, 2);
                        local_max_y = cmp::max(local_max_y, 2);
                        edge_found = true;
                    }
                    if edge_found {
                        local_min_x = cmp::min(local_min_x, mx as u32);
                        local_max_x = cmp::max(local_max_x, mx as u32 + 1);
                    }
                }

                local_min_x += buf.x + x as u32 - 1;
                local_max_x += buf.x + x as u32 - 1;
                local_min_y += buf.y + y as u32 - 1;
                local_max_y += buf.y + y as u32 - 1;

                if local_min_x < search.min_x {
                    search.min_x = local_min_x;
                }
                if local_max_x > search.max_x {
                    search.max_x = local_max_x;
                }
                if local_min_y < search.min_y {
                    search.min_y = local_min_y;
                }
                if local_max_y > search.max_y {
                    search.max_y = local_max_y;
                }
            }
            buf_ix += 1;
        }
        buf_ix += 2;
    }
}

