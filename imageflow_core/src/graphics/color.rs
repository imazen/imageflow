use crate::graphics::lut::linear_to_srgb_lut;
use crate::graphics::math::fastpow;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WorkingFloatspace {
    StandardRGB,
    LinearRGB,
    Gamma,
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

impl ColorContext {
    pub fn new(space: WorkingFloatspace, gamma: f32) -> ColorContext {
        let mut c = ColorContext {
            apply_gamma: space == WorkingFloatspace::Gamma,
            apply_srgb: space == WorkingFloatspace::LinearRGB,
            gamma,
            gamma_inverse: (1.0f64 / gamma as f64) as f32,
            byte_to_float: [0f32; 256],
        };
        for n in 0..256 {
            c.byte_to_float[n] = c.srgb_to_floatspace_uncached(n as u8);
        }
        c
    }

    #[inline]
    pub fn srgb_to_floatspace_uncached(&self, value: u8) -> f32 {
        let mut v: f32 = value as f32 * (1.0f32 / 255.0f32);
        if self.apply_srgb {
            v = srgb_to_linear(v)
        } else if self.apply_gamma {
            v = self.remove_gamma(v)
        }
        v
    }

    #[inline]
    pub fn remove_gamma(&self, value: f32) -> f32 {
        f32::powf(value, self.gamma)
    }
    #[inline]
    pub fn apply_gamma(&self, value: f32) -> f32 {
        f32::powf(value, self.gamma_inverse)
    }

    #[inline]
    pub fn srgb_to_floatspace(&self, value: u8) -> f32 {
        self.byte_to_float[value as usize]
    }
    #[inline]
    pub fn floatspace_to_srgb(&self, space_value: f32) -> u8 {
        let v: f32 = space_value;
        if self.apply_gamma {
            return uchar_clamp_ff(self.apply_gamma(v) * 255.0f32);
        }
        if self.apply_srgb {
            return linear_to_srgb_lut(v);
        }
        uchar_clamp_ff(255.0f32 * v)
    }
}

#[inline]
pub fn flow_colorcontext_floatspace_to_srgb(c: &ColorContext, space_value: f32) -> u8 {
    c.floatspace_to_srgb(space_value)
}

#[inline]
pub fn flow_colorcontext_srgb_to_floatspace(c: &ColorContext, value: u8) -> f32 {
    c.srgb_to_floatspace(value)
}

#[inline]
fn srgb_to_linear(s: f32) -> f32 {
    if s <= 0.04045f32 {
        s / 12.92f32
    } else {
        f32::powf((s + 0.055f32) / (1_f32 + 0.055f32), 2.4f32)
    }
}
#[inline]
fn linear_to_srgb(clr: f32) -> f32 {
    if clr <= 0.0031308f32 {
        12.92f32 * clr * 255.0f32
    } else {
        1.055f32 * 255.0f32 * fastpow(clr, 0.41666666f32) - 14.025f32
    }
}
#[inline]
pub(crate) fn uchar_clamp_ff(clr: f32) -> u8 {
    let mut result: u16;
    result = (clr as f64 + 0.5f64) as i16 as u16;
    if result as i32 > 255_i32 {
        result = if clr < 0_i32 as f32 { 0_i32 } else { 255_i32 } as u16
    }
    result as u8
}
