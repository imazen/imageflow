use rgb::Bgra;

use crate::graphics::{bitmaps::BitmapWindowMut, histogram::populate_histogram_from_window};
use super::internal_prelude::*;
// TODO: someday look into better algorithms - see http://colorconstancy.com/ and http://ipg.fer.hr/ipg/resources/color_constancy
// http://localhost:39876/ir4/proxy_unsplash/photo-1496264057429-6a331647b69e?a.balancewhite=true&w=800
// http://localhost:39876/ir4/proxy_unsplash/photo-1496264057429-6a331647b69e?w=800


    pub static  WHITE_BALANCE_SRGB_MUTATE: WhiteBalanceSrgbMutDef = WhiteBalanceSrgbMutDef{};
    pub static  WHITE_BALANCE_SRGB: MutProtect<WhiteBalanceSrgbMutDef> = MutProtect{node: &WHITE_BALANCE_SRGB_MUTATE, fqn: "imazen.white_balance_srgb"};



fn area_threshold(histogram: &[u64], total_pixels: u64, low_threshold: f64, high_threshold: f64) -> (usize, usize){
    let mut low = 0;
    let mut high = histogram.len() - 1;
    let mut area = 0u64;
    let pixel_count = total_pixels as f64;
    for (ix, value) in histogram.iter().enumerate(){
        area += *value;
        if area as f64 / pixel_count > low_threshold {
            low = ix;
            break;
        }
    }
    area = 0u64;
    for (ix, value) in histogram.iter().enumerate().rev(){
        area += *value;
        if area as f64 / pixel_count > low_threshold {
            high = ix;
            break;
        }
    }
    // eprintln!("{},{}", low,high);
    (low, high)
}

fn create_byte_mapping(low: usize, high: usize) -> Vec<u8>{
    let scale = 255.0 / ((high - low) as f64);

    (0..256usize).map(|v| (v.saturating_sub(low) as f64 * scale).round().min(255f64).max(0f64) as u8).collect()
}


fn apply_mappings(bitmap: &mut BitmapWindowMut<Bgra<u8, u8>>, map_red: &[u8], map_green: &[u8], map_blue: &[u8]) -> Result<()>{


    if map_red.len() < 256 || map_green.len() < 256 || map_blue.len() < 256{
        return Err(nerror!(crate::ErrorKind::InvalidState));
    }
    for mut line in bitmap.scanlines(){
        for pixel in line.row_mut(){
            unsafe{
                pixel.r = *map_red.get_unchecked(pixel.r as usize);
                pixel.g = *map_green.get_unchecked(pixel.g as usize);
                pixel.b = *map_blue.get_unchecked(pixel.b as usize);
            }
        }
    }
    Ok(())
}

fn white_balance_srgb_mut(bitmap: &mut BitmapWindowMut<Bgra<u8, u8>>, histograms: &[[u64; 256];3], pixels_sampled: u64, low_threshold: Option<f32>, high_threshold: Option<f32>) -> Result<()>{
    let low_threshold = f64::from(low_threshold.unwrap_or(0.006));
    let high_threshold = f64::from(high_threshold.unwrap_or(0.006));

    let (red_low, red_high) = area_threshold(&histograms[0], pixels_sampled, low_threshold, high_threshold);
    let (green_low, green_high) = area_threshold(&histograms[1], pixels_sampled, low_threshold, high_threshold);
    let (blue_low, blue_high) = area_threshold(&histograms[2], pixels_sampled, low_threshold, high_threshold);

    let red_map = create_byte_mapping(red_low, red_high);
    let green_map = create_byte_mapping(green_low, green_high);
    let blue_map = create_byte_mapping(blue_low, blue_high);

    apply_mappings(bitmap, &red_map, &green_map, &blue_map)
}

#[derive(Debug, Clone)]
pub struct WhiteBalanceSrgbMutDef;
impl NodeDef for WhiteBalanceSrgbMutDef{
    fn as_one_mutate_bitmap(&self) -> Option<&dyn NodeDefMutateBitmap>{
        Some(self)
    }
}
impl NodeDefMutateBitmap for WhiteBalanceSrgbMutDef{
    fn fqn(&self) -> &'static str{
        "imazen.white_balance_srgb_mut"
    }
    fn mutate(&self, c: &Context, bitmap_key: BitmapKey,  p: &NodeParams) -> Result<()> {
        let bitmaps = c.borrow_bitmaps()
            .map_err(|e| e.at(here!()))?;
        let mut bitmap_bitmap = bitmaps.try_borrow_mut(bitmap_key)
            .map_err(|e| e.at(here!()))?;

        let mut window = bitmap_bitmap.get_window_bgra32().unwrap();


        let mut histograms: [[u64; 256]; 3] = [[0; 256]; 3];
        let pixels_sampled: u64 = window.w() as u64 * window.h() as u64;

        populate_histogram_from_window(&mut window, &mut histograms)
            .map_err(|e| e.at(here!()))?;

        if let NodeParams::Json(s::Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold }) = *p {
            white_balance_srgb_mut(&mut window, &histograms, pixels_sampled, threshold, threshold)
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ColorMatrixSrgb, got {:?}", p))
        }
    }
}
