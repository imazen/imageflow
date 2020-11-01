use imageflow_core::ffi::*;
use imageflow_core::graphics::color::{linear_to_luv, luv_to_linear};
use imageflow_core::graphics::convolve::*;
use imageflow_core::graphics::bitmaps::*;

#[inline]
fn margin_comparison(lhs:f32, rhs:f32,margin:f32)->bool {
    return (lhs + margin >= rhs) && (rhs + margin >= lhs);
}


// Returns average delta per channel per pixel. returns (double)INT32_MAX if dimension or channel mismatch
fn flow_bitmap_float_compare(a: &BitmapFloat, b:&BitmapFloat,pixels_a:&[f32],pixels_b:&[f32])->f32
{
    assert_eq!(a.w,b.w);
    assert_eq!(a.h,b.h);
    assert_eq!(a.channels,b.channels);
    assert_eq!(a.float_count,b.float_count);
    assert_eq!(a.float_stride,b.float_stride);

    let mut difference_total = 0f32;
    let mut max_delta = 0f32;
    for y in 0..a.h {
        let mut row_delta = 0f32;
        for x in 0..a.w {
            let pixel = y * a.float_stride + x * a.channels;
            for cx in 0..a.channels {
                let delta = f32::abs(pixels_a[(pixel+cx) as usize] - pixels_b[(pixel+cx) as usize]);
                if delta > max_delta {
                    max_delta = delta;
                }
                row_delta += delta;
            }
        }
        difference_total = row_delta / ((a.w * a.channels) as f32);
    }
    assert!(max_delta<0.12);
    assert!(difference_total / (a.h as f32) < 0.03);
    return difference_total / (a.h as f32);
}


#[inline]
fn uchar_clamp_ff(clr:f32)->u8 {
    if clr + 0.5f32 < u8::MIN as f32 {
        u8::MIN
    } else if clr +0.5f32 > u8::MAX as f32 {
        u8::MAX
    } else {
        (clr + 0.5) as u8
    }
}


#[test]
fn test_rgb_000_to_luv(){
    let mut bgra=[0f32;4];
    unsafe { linear_to_luv(bgra.as_mut_ptr()); }
    assert_eq!(bgra[0],0f32);
    assert_eq!(bgra[1],100f32);
    assert_eq!(bgra[2],100f32);
}

#[test]
fn test_roundtrip_rgb_luv2(){
    let mut bgra=[0.2f32,0.2f32,0.2f32,1.0f32];

    unsafe {
        linear_to_luv(bgra.as_mut_ptr());
        luv_to_linear(bgra.as_mut_ptr());
    }
    assert!(margin_comparison(bgra[0],0.2f32,0.00001));
    assert!(margin_comparison(bgra[1],0.2f32,0.00001));
    assert!(margin_comparison(bgra[2],0.2f32,0.00001));
}


#[test]
fn test_roundstrip_srgb_linear_rgb_luv(){
    for x in 0..=255{
        assert_eq!(x as u8,uchar_clamp_ff(linear_to_srgb(srgb_to_linear((x as f32)/255f32))));
    }
}


#[test]
fn test_roundtrip_rgb_luv0(){
    let mut bgra=[0f32;4];
    unsafe {
        linear_to_luv(bgra.as_mut_ptr());
        luv_to_linear(bgra.as_mut_ptr());
    };
    assert_eq!(bgra[0],0f32);
    assert_eq!(bgra[1],0f32);
    assert_eq!(bgra[2],0f32);
}


#[test]
fn test_gaussian_blur_approximation() {
    let sigma = 2.0f64;

    // We figure this out just for the actual gaussian function
    let kernel_radius
        = i32::max(1, f64::ceil(sigma * 3.11513411073090629014797467185716068837128426554157826035269f64 - 0.5f64) as i32); // Should provide at least 7 bits of precision, and almost always 8.

    let bitmap_width = 300;
   //  unsafe { flow_bitmap_float_approx_gaussian_calculate_d(sigma as f32, bitmap_width) };

    let buffer_elements = unsafe { flow_bitmap_float_approx_gaussian_buffer_element_count_required(sigma as f32, bitmap_width) };

    let mut buffer: Vec<f32> = Vec::with_capacity(buffer_elements as usize);

    // Preferably test premultiplication

    let mut bitmap = Bitmap::create_float(bitmap_width, 1, PixelLayout::BGRA, true, true, ColorSpace::LinearRGB).unwrap();
    let mut window = bitmap.get_window_f32().unwrap();
    let mut image = unsafe { window.to_bitmap_float().unwrap() };
    let pixels = window.slice();
    for i in 0..image.w * 4 {
        pixels[i as usize] = ((if i % 8 == 0 { 0.5 } else { 0f32 }) + (if i % 12 == 0 { 0.4 } else { 0.1 })) as f32;
    }
    let mut bitmap_b = Bitmap::create_float(bitmap_width, 1, PixelLayout::BGRA, true, true, ColorSpace::LinearRGB).unwrap();
    let mut b_window=bitmap_b.get_window_f32().unwrap();
    let mut image_b = unsafe { b_window.to_bitmap_float().unwrap() };
    let pixels_b = b_window.slice();

    for i in 0..image.float_stride {
        pixels_b[i as usize] = pixels[i as usize];
    }

    unsafe { flow_bitmap_float_approx_gaussian_blur_rows(&mut image as *mut BitmapFloat, sigma as f32, buffer.as_mut_ptr(), buffer_elements as usize, 0, 1).unwrap(); }

    let mut gaussian
        = unsafe { flow_convolution_kernel_create_gaussian_normalized(sigma, kernel_radius as u32).unwrap() };
    unsafe { flow_bitmap_float_convolve_rows(&mut image_b as *mut BitmapFloat, &mut gaussian, 4, 0, 1).unwrap() };
    // Compare image_a and image_b
   flow_bitmap_float_compare(&image, &image_b, pixels, pixels_b);
}



