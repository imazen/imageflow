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

#[derive(Copy, Clone)]
#[repr(C)]
pub union UnionU32F32 {
    pub i: u32,
    pub f: f32,
}

#[inline]
fn fastpow2(p: f32) -> f32 {
    let offset: f32 = if p < 0_i32 as f32 { 1.0f32 } else { 0.0f32 };
    let clipp: f32 = if p < -126_f32 { -126.0f32 } else { p };
    let w: i32 = clipp as i32;
    let z: f32 = clipp - w as f32 + offset;
    /* Was i: (((1 as i32) << 23 as i32) as f32
     * (clipp + 121.2740575f32 + 27.7280233f32 / (4.84252568f32 - z) - 1.49012907f32 * z)) */
    let v: UnionU32F32 = UnionU32F32 {
        i: ((1_i32 << 23_i32) as f32
            * (clipp + 121.274_055_f32 + 27.728_024_f32 / (4.842_525_5_f32 - z)
                - 1.490_129_1_f32 * z)) as u32,
    };
    unsafe { v.f }
}
#[inline]
fn fastlog2(x: f32) -> f32 {
    unsafe {
        let vx: UnionU32F32 = UnionU32F32 { f: x };
        let mx: UnionU32F32 = UnionU32F32 { i: vx.i & 0x7fffff_i32 as u32 | 0x3f000000_i32 as u32 };
        let mut y: f32 = vx.i as f32;
        /* Was y *= 1.1920928955078125e-7f32;
        y - 124.22551499f32 - 1.498030302f32 * mx.f - 1.72587999f32 / (0.3520887068f32 + mx.f); */
        y *= 1.192_092_9e-7_f32;
        y - 124.225_52_f32 - 1.498_030_3_f32 * mx.f - 1.725_88_f32 / (0.352_088_72_f32 + mx.f)
    }
}
#[inline]
pub(crate) fn fastpow(x: f32, p: f32) -> f32 {
    fastpow2(p * fastlog2(x))
}
