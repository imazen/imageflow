pub const IR_PI: f64 = std::f64::consts::PI;
#[inline]
fn int_max(a: i32, b: i32) -> i32 {
    if a >= b {
        a
    } else {
        b
    }
}
#[inline]
fn int_min(a: i32, b: i32) -> i32 {
    if a <= b {
        a
    } else {
        b
    }
}

#[inline]
fn fastpow2(p: f32) -> f32 {
    let offset: f32 = if p < 0_i32 as f32 { 1.0f32 } else { 0.0f32 };
    let clipp: f32 = if p < -126_f32 { -126.0f32 } else { p };
    let w: i32 = clipp as i32;
    let z: f32 = clipp - w as f32 + offset;
    f32::from_bits(
        ((1_i32 << 23_i32) as f32
            * (clipp + 121.274_055_f32 + 27.728_024_f32 / (4.842_525_5_f32 - z)
                - 1.490_129_1_f32 * z)) as u32,
    )
}
#[inline]
fn fastlog2(x: f32) -> f32 {
    let vx = x.to_bits();
    let mx = f32::from_bits(vx & 0x7fffff_u32 | 0x3f000000_u32);
    let mut y: f32 = vx as f32;
    y *= 1.192_092_9e-7_f32;
    y - 124.225_52_f32 - 1.498_030_3_f32 * mx - 1.725_88_f32 / (0.352_088_72_f32 + mx)
}
#[inline]
pub(crate) fn fastpow(x: f32, p: f32) -> f32 {
    fastpow2(p * fastlog2(x))
}
