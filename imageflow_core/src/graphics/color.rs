use crate::graphics::math::{pow, fastpow};


#[derive(Copy, Clone, PartialEq, Eq)]
pub enum WorkingFloatspace {
    StandardRGB,
    LinearRGB,
    Gamma
}

// Gamma correction  http://www.4p8.com/eric.brasseur/gamma.html#formulas

#[derive(Copy, Clone)]
pub struct ColorContext {
    byte_to_float: [f32; 256],
    apply_srgb: bool,
    apply_gamma: bool,
    gamma: f32,
    gamma_inverse: f32,
}

impl ColorContext{

    pub fn new(space: WorkingFloatspace, gamma: f32) -> ColorContext{
        let mut c = ColorContext{
            apply_gamma: space == WorkingFloatspace::Gamma,
            apply_srgb: space == WorkingFloatspace::LinearRGB,
            gamma,
            gamma_inverse: (1.0f64 / gamma as f64) as f32,
            byte_to_float: [0f32;256]
        };
        for n in 0..256 {
            c.byte_to_float[n] =
                c.srgb_to_floatspace_uncached(n as u8);
        }
        c
    }


    #[inline]
    pub fn srgb_to_floatspace_uncached(&self, value: u8) -> f32{
        let mut v: f32 = value as f32 * (1.0f32 / 255.0f32);
        if self.apply_srgb {
            v = srgb_to_linear(v)
        } else if self.apply_gamma {
            v = self.remove_gamma(v)
        }
        return v;
    }

    #[inline]
    pub fn remove_gamma(&self, value: f32) -> f32{
        f32::powf(value, self.gamma)
    }
    #[inline]
    pub fn apply_gamma(&self, value: f32) -> f32{
        f32::powf(value, self.gamma_inverse)
    }

    #[inline]
    pub fn srgb_to_floatspace(&self, value: u8) -> f32{
        // Safe because array length is larger than u8 max value
        unsafe {
            *self.byte_to_float.get_unchecked(value as usize)
        }
    }
    #[inline]
    pub fn floatspace_to_srgb(&self, space_value: f32) -> u8{
        let v: f32 = space_value;
        if self.apply_gamma {
            return uchar_clamp_ff(self.apply_gamma(v) * 255.0f32);
        }
        if self.apply_srgb {
            return uchar_clamp_ff(linear_to_srgb(v));
        }
        return uchar_clamp_ff(255.0f32 * v);
    }

}

#[inline]
pub fn flow_colorcontext_floatspace_to_srgb(c: &ColorContext, space_value: f32) -> u8{
    c.floatspace_to_srgb(space_value)
}

#[inline]
pub fn flow_colorcontext_srgb_to_floatspace(c: &ColorContext, value: u8) -> f32{
    c.srgb_to_floatspace(value)
}


#[inline]
fn srgb_to_linear(s: f32) -> f32 {
    if s <= 0.04045f32 {
        s / 12.92f32
    } else {
        f32::powf(
            (s + 0.055f32) / (1 as i32 as f32 + 0.055f32),
            2.4f32,
        )
    }
}
#[inline]
fn linear_to_srgb(clr: f32) -> f32 {
    if clr <= 0.0031308f32 {
        12.92f32 * clr * 255.0f32
    }else {
        1.055f32 * 255.0f32 * unsafe{ fastpow(clr, 0.41666666f32)} - 14.025f32
    }
}
#[inline]
pub(crate) fn uchar_clamp_ff(clr: f32) -> u8 {
    let mut result: u16;
    result = (clr as f64 + 0.5f64) as i16 as u16;
    if result as i32 > 255 as i32 {
        result = if clr < 0 as i32 as f32 {
            0 as i32
        } else {
            255 as i32
        } as u16
    }
    return result as u8;
}


#[inline]
#[allow(non_snake_case)]
pub unsafe fn linear_to_luv(bgr: *mut f32) {
    let xn: f32 = 0.312713f32;
    let yn: f32 = 0.329016f32;
    let Yn: f32 = 1.0f32;
    let un: f32 =
        4 as i32 as f32 * xn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let vn: f32 =
        9 as i32 as f32 * yn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let y_split: f32 = 0.00885645f32;
    let y_adjust: f32 = 903.3f32;
    let R: f32 = *bgr.offset(2);
    let G: f32 = *bgr.offset(1);
    let B: f32 = *bgr.offset(0);
    if R == 0 as i32 as f32 && G == 0 as i32 as f32 && B == 0 as i32 as f32 {
        *bgr.offset(0) = 0 as i32 as f32;
        let ref mut fresh0 = *bgr.offset(2);
        *fresh0 = 100 as i32 as f32;
        *bgr.offset(1) = *fresh0;
        return;
    }
    let X: f32 = 0.412453f32 * R + 0.35758f32 * G + 0.180423f32 * B;
    let Y: f32 = 0.212671f32 * R + 0.71516f32 * G + 0.072169f32 * B;
    let Z: f32 = 0.019334f32 * R + 0.119193f32 * G + 0.950227f32 * B;
    let Yd: f32 = Y / Yn;
    let u: f32 = 4 as i32 as f32 * X / (X + 15 as i32 as f32 * Y + 3 as i32 as f32 * Z);
    let v: f32 = 9 as i32 as f32 * Y / (X + 15 as i32 as f32 * Y + 3 as i32 as f32 * Z);
    let ref mut fresh1 = *bgr.offset(0);
    *fresh1 = if Yd > y_split {
        (116 as i32 as f32 * pow(Yd as f64, (1.0f32 / 3.0f32) as f64) as f32) - 16 as i32 as f32
    } else {
        (y_adjust) * Yd
    };
    let L: f32 = *fresh1;
    *bgr.offset(1) = 13 as i32 as f32 * L * (u - un) + 100 as i32 as f32;
    *bgr.offset(2) = 13 as i32 as f32 * L * (v - vn) + 100 as i32 as f32;
}
#[inline]
#[allow(non_snake_case)]
pub unsafe fn luv_to_linear(luv: *mut f32) {
    let L: f32 = *luv.offset(0);
    let U: f32 = *luv.offset(1) - 100.0f32;
    let V: f32 = *luv.offset(2) - 100.0f32;
    if L == 0 as i32 as f32 {
        let ref mut fresh2 = *luv.offset(2);
        *fresh2 = 0 as i32 as f32;
        let ref mut fresh3 = *luv.offset(1);
        *fresh3 = *fresh2;
        *luv.offset(0) = *fresh3;
        return;
    }
    let xn: f32 = 0.312713f32;
    let yn: f32 = 0.329016f32;
    let Yn: f32 = 1.0f32;
    let un: f32 =
        4 as i32 as f32 * xn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let vn: f32 =
        9 as i32 as f32 * yn / (-(2 as i32) as f32 * xn + 12 as i32 as f32 * yn + 3 as i32 as f32);
    let y_adjust_2: f32 = 0.00110705645f32;
    let u: f32 = U / (13 as i32 as f32 * L) + un;
    let v: f32 = V / (13 as i32 as f32 * L) + vn;
    let Y: f32 = if L > 8 as i32 as f32 {
        (Yn) * pow(
            ((L + 16 as i32 as f32) / 116 as i32 as f32) as f64,
            3 as i32 as f64,
        ) as f32
    } else {
        (Yn * L) * y_adjust_2
    };
    let X: f32 = 9 as i32 as f32 / 4.0f32 * Y * u / v;
    let Z: f32 = (9 as i32 as f32 * Y - 15 as i32 as f32 * v * Y - v * X) / (3 as i32 as f32 * v);
    let r: f32 = 3.240479f32 * X - 1.53715f32 * Y - 0.498535f32 * Z;
    let g: f32 = -0.969256f32 * X + 1.875991f32 * Y + 0.041556f32 * Z;
    let b: f32 = 0.055648f32 * X - 0.204043f32 * Y + 1.057311f32 * Z;
    *luv.offset(0) = b;
    *luv.offset(1) = g;
    *luv.offset(2) = r;
}