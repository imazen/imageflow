use imageflow_core::ffi::*;
use imageflow_core::graphics::color::{linear_to_luv, luv_to_linear};


fn linear_to_srgb(clr: f32) -> f32
{
// Gamma correction
// http://www.4p8.com/eric.brasseur/gamma.html#formulas

    if clr <= 0.0031308f32 {
        return 12.92f32 * clr * 255.0f32;
    }


// a = 0.055; ret ((1+a) * s**(1/2.4) - a) * 255
    return 1.055f32 * 255.0f32 * (f32::powf(clr, 0.41666666f32)) - 14.025f32;
}

fn srgb_to_linear(s: f32) -> f32
{
    if s <= 0.04045f32 {
        return s / 12.92f32;
    }
    return f32::powf((s + 0.055f32) / (1f32 + 0.055f32), 2.4f32);
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
    assert_eq!(bgra[0],0.2f32);
    assert_eq!(bgra[1],0.2f32);
    assert_eq!(bgra[2],0.2f32);
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



