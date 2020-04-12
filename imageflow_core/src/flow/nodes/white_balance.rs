use super::internal_prelude::*;
// TODO: someday look into better algorithms - see http://colorconstancy.com/ and http://ipg.fer.hr/ipg/resources/color_constancy
// http://localhost:39876/ir4/proxy_unsplash/photo-1496264057429-6a331647b69e?a.balancewhite=true&w=800
// http://localhost:39876/ir4/proxy_unsplash/photo-1496264057429-6a331647b69e?w=800


    pub static  WHITE_BALANCE_SRGB: WhiteBalanceSrgbMutDef = WhiteBalanceSrgbMutDef{};
    pub static  WHITE_BALANCE_SRGB_MUTATE: MutProtect<WhiteBalanceSrgbMutDef> = MutProtect{node: &WHITE_BALANCE_SRGB, fqn: "imazen.white_balance_srgb"};



fn area_threshold(histogram: &[u64], total_pixels: u64, low_threshold: f64, high_threshold: f64) -> (usize, usize){
    let mut low = 0;
    let mut high = histogram.len() - 1;
    let mut area = 0u64;
    let pixel_count = total_pixels as f64;
    for (ix, value) in histogram.iter().enumerate(){
        area += u64::from(*value);
        if area as f64 / f64::from(pixel_count) > low_threshold {
            low = ix;
            break;
        }
    }
    area = 0u64;
    for (ix, value) in histogram.iter().enumerate().rev(){
        area += u64::from(*value);
        if area as f64 / f64::from(pixel_count) > low_threshold {
            high = ix;
            break;
        }
    }
    eprintln!("{},{}", low,high);
    (low, high)
}

fn create_byte_mapping(low: usize, high: usize) -> Vec<u8>{
    let scale = 255.0 / ((high - low) as f64);

    (0..256usize).map(|v| (v.saturating_sub(low) as f64 * scale).round().min(255f64).max(0f64) as u8).collect()
}


fn apply_mappings(bitmap: *mut BitmapBgra, map_red: &[u8], map_green: &[u8], map_blue: &[u8]) -> Result<()>{

    let input: &BitmapBgra = unsafe{ &*bitmap };
    let bytes: &mut [u8] = unsafe { slice::from_raw_parts_mut::<u8>(input.pixels, (input.stride * input.h) as usize) };

    if map_red.len() < 256 || map_green.len() < 256 || map_blue.len() < 256{
        return Err(nerror!(crate::ErrorKind::InvalidState));
    }

    match input.fmt {
        PixelFormat::Gray8 =>
            Err(unimpl!())
        ,
        PixelFormat::Bgra32 | PixelFormat::Bgr32=> {
            for row in bytes.chunks_mut(input.stride as usize){
                for pixel in row.chunks_mut(4).take(input.w as usize){
                    //pixel[0] = map_blue[pixel[0]];
                    //pixel[1] = map_green[pixel[1]];
                    //pixel[2] = map_red[pixel[2]];
                    unsafe {
                        *pixel.get_unchecked_mut(0) = *map_blue.get_unchecked(*pixel.get_unchecked(0) as usize);
                        *pixel.get_unchecked_mut(1) = *map_green.get_unchecked(*pixel.get_unchecked(1) as usize);
                        *pixel.get_unchecked_mut(2) = *map_red.get_unchecked(*pixel.get_unchecked(2) as usize);
                    }
                }
            }
            Ok(())
        },
        PixelFormat::Bgr24 => {
            for row in bytes.chunks_mut(input.stride as usize){
                for pixel in row.chunks_mut(3).take(input.w as usize){
                    //pixel[0] = *map_blue[pixel[0]];
                    //pixel[1] = *map_green[pixel[1]];
                    //pixel[2] = *map_red[pixel[2]];
                    unsafe {
                        *pixel.get_unchecked_mut(0) = *map_blue.get_unchecked(*pixel.get_unchecked(0) as usize);
                        *pixel.get_unchecked_mut(1) = *map_green.get_unchecked(*pixel.get_unchecked(1) as usize);
                        *pixel.get_unchecked_mut(2) = *map_red.get_unchecked(*pixel.get_unchecked(2) as usize);
                    }
                }
            }
            Ok(())
        },
    }
}

fn white_balance_srgb_mut(bitmap: *mut BitmapBgra, histograms: &[u64;768], pixels_sampled: u64, low_threshold: Option<f32>, high_threshold: Option<f32>) -> Result<()>{
    let low_threshold = f64::from(low_threshold.unwrap_or(0.006));
    let high_threshold = f64::from(high_threshold.unwrap_or(0.006));

    let (red_low, red_high) = area_threshold(&histograms[0..256], pixels_sampled, low_threshold, high_threshold);
    let (green_low, green_high) = area_threshold(&histograms[256..512], pixels_sampled, low_threshold, high_threshold);
    let (blue_low, blue_high) = area_threshold(&histograms[512..768], pixels_sampled, low_threshold, high_threshold);

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
    fn mutate(&self, c: &Context, bitmap: &mut BitmapBgra,  p: &NodeParams) -> Result<()> {

        unsafe {
            let mut histograms: [u64; 768] = [0; 768];
            let mut pixels_sampled: u64 = 0;
            if !crate::ffi::flow_bitmap_bgra_populate_histogram(c.flow_c(), bitmap as *mut BitmapBgra, histograms.as_mut_ptr(), 256, 3, &mut pixels_sampled as *mut u64) {
                return Err(cerror!(c, "Failed to populate histogram"))
            }
            if let NodeParams::Json(s::Node::WhiteBalanceHistogramAreaThresholdSrgb { threshold }) = *p {
                white_balance_srgb_mut(bitmap, &histograms, pixels_sampled, threshold, threshold)
            } else {
                Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need ColorMatrixSrgb, got {:?}", p))
            }
        }

    }
}
