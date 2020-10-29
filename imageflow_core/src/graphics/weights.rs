/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */

use std::f64;
use std::i32;
use std::u32;

use serde::{Serialize,Deserialize};
use crate::graphics::aligned_buffer::AlignedBuffer;

/// Named interpolation function+configuration presets
#[repr(C)]
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub enum Filter {
    RobidouxFast = 1,
    Robidoux = 2,
    RobidouxSharp = 3,
    Ginseng = 4,
    GinsengSharp = 5,
    Lanczos = 6,
    LanczosSharp = 7,
    Lanczos2 = 8,
    Lanczos2Sharp = 9,
    CubicFast = 10,
    Cubic = 11,
    CubicSharp = 12,
    CatmullRom = 13,
    Mitchell = 14,
    CubicBSpline = 15,
    Hermite = 16,
    Jinc = 17,
    RawLanczos3 = 18,
    RawLanczos3Sharp = 19,
    RawLanczos2 = 20,
    RawLanczos2Sharp = 21,
    Triangle = 22,
    Linear = 23,
    Box = 24,
    CatmullRomFast = 25,
    CatmullRomFastSharp = 26,

    Fastest = 27,
    MitchellFast = 28,
    NCubic = 29,
    NCubicSharp = 30,
    LegacyIDCTFilter = 31
}

impl From<imageflow_types::Filter> for Filter {
    fn from(filter: imageflow_types::Filter) -> Self {
        match filter{
            imageflow_types::Filter::RobidouxFast => Filter::RobidouxFast,
            imageflow_types::Filter::Robidoux => Filter::Robidoux,
            imageflow_types::Filter::RobidouxSharp => Filter::RobidouxSharp,
            imageflow_types::Filter::Ginseng => Filter::Ginseng,
            imageflow_types::Filter::GinsengSharp => Filter::GinsengSharp,
            imageflow_types::Filter::Lanczos => Filter::Lanczos,
            imageflow_types::Filter::LanczosSharp => Filter::LanczosSharp,
            imageflow_types::Filter::Lanczos2 => Filter::Lanczos2,
            imageflow_types::Filter::Lanczos2Sharp => Filter::Lanczos2Sharp,
            imageflow_types::Filter::Cubic => Filter::Cubic,
            imageflow_types::Filter::CubicSharp => Filter::CubicSharp,
            imageflow_types::Filter::CatmullRom => Filter::CatmullRom,
            imageflow_types::Filter::Mitchell => Filter::Mitchell,
            imageflow_types::Filter::CubicBSpline => Filter::CubicBSpline,
            imageflow_types::Filter::Hermite => Filter::Hermite,
            imageflow_types::Filter::Jinc => Filter::Jinc,
            imageflow_types::Filter::Triangle => Filter::Triangle,
            imageflow_types::Filter::Linear => Filter::Linear,
            imageflow_types::Filter::Box => Filter::Box,
            imageflow_types::Filter::Fastest => Filter::Fastest,
            imageflow_types::Filter::NCubic => Filter::NCubic,
            imageflow_types::Filter::NCubicSharp => Filter::NCubicSharp,
        }

    }
}

pub struct InterpolationDetails {
    /// 1 is the default; near-zero overlapping between windows. 2 overlaps 50% on each side.
     window: f64,
    /// Coefficients for bicubic weighting
     p1: f64,
     p2: f64,
     p3: f64,
     q1: f64,
     q2: f64,
     q3: f64,
     q4: f64,
    /// Blurring factor when > 1, sharpening factor when < 1. Applied to weights.
    blur: f64,
    pub filter: fn(&InterpolationDetails,f64) -> f64,
    /// How much sharpening we are requesting
    sharpen_percent_goal: f32
}
impl Default for InterpolationDetails {
    fn default() -> InterpolationDetails {
        InterpolationDetails {
            window: 2f64,
            p1: 0.0,
            p2: 1f64,
            p3: 1f64,
            q1: 0.0,
            q2: 1f64,
            q3: 1f64,
            q4: 1f64,
            blur: 1f64,
            filter: filter_box,
            sharpen_percent_goal: 0.0,
        }
    }
}

impl InterpolationDetails{
    pub fn set_sharpen_percent_goal(&mut self, goal: f32){
        self.sharpen_percent_goal = goal;
    }
    fn bicubic(window: f64, blur: f64, b: f64, c: f64) -> InterpolationDetails{
        let bx2 = b + b;
        InterpolationDetails{
            window, blur,
            filter: filter_flex_cubic,
            p1: 1.0 - (1.0 / 3.0) * b,
            p2: -3.0 + bx2 + c,
            p3: 2.0 - 1.5 * b - c,
            q1: (4.0 / 3.0) * b + 4.0 * c,
            q2: -8.0 * c - bx2,
            q3: b + 5.0 * c,
            q4: (-1.0 / 6.0) * b - c,
            sharpen_percent_goal: 0.0,
        }
    }

    pub fn create(filter: Filter) -> InterpolationDetails {
        match filter {
            Filter::Triangle | Filter::Linear
            => InterpolationDetails { window: 1f64, blur: 1f64, filter: filter_triangle, ..Default::default() },

            Filter::RawLanczos2 => InterpolationDetails { window: 2f64, blur: 1f64, filter: filter_sinc, ..Default::default() },
            Filter::RawLanczos3 => InterpolationDetails { window: 3f64, blur: 1f64, filter: filter_sinc, ..Default::default() },
            Filter::RawLanczos2Sharp => InterpolationDetails { window: 2f64, blur: 0.9549963639785485f64, filter: filter_sinc, ..Default::default() },
            Filter::RawLanczos3Sharp => InterpolationDetails { window: 3f64, blur: 0.9812505644269356f64, filter: filter_sinc, ..Default::default() },
            Filter::Lanczos2 => InterpolationDetails { window: 2f64, blur: 1f64, filter: filter_sinc_windowed, ..Default::default() },
            Filter::Lanczos => InterpolationDetails { window: 3f64, blur: 1f64, filter: filter_sinc_windowed, ..Default::default() },
            Filter::Lanczos2Sharp => InterpolationDetails { window: 2f64, blur: 0.9549963639785485f64, filter: filter_sinc_windowed, ..Default::default() },
            Filter::LanczosSharp => InterpolationDetails { window: 3f64, blur: 0.9812505644269356f64, filter: filter_sinc_windowed, ..Default::default() },
            Filter::CubicFast => InterpolationDetails { window: 2f64, blur: 1f64, filter: filter_bicubic_fast, ..Default::default() },
            Filter::Box => InterpolationDetails { window: 0.5f64, blur: 1f64, filter: filter_box, ..Default::default() },
            Filter::Ginseng => InterpolationDetails { window: 3f64, blur: 1f64, filter: filter_ginseng, ..Default::default() },
            Filter::GinsengSharp => InterpolationDetails { window: 3f64, blur: 0.9812505644269356f64, filter: filter_ginseng, ..Default::default() },
            Filter::Jinc => InterpolationDetails { window: 6f64, blur: 1f64, filter: filter_jinc, ..Default::default() },
            Filter::CubicBSpline => InterpolationDetails::bicubic(2f64, 1f64, 1f64, 0f64),
            Filter::Cubic => InterpolationDetails::bicubic(2f64, 1f64, 0f64, 1f64),
            Filter::CubicSharp => InterpolationDetails::bicubic(2f64, 0.9549963639785485f64, 0f64, 1f64),

            Filter::CatmullRom => InterpolationDetails::bicubic(2f64, 1f64, 0f64, 0.5f64),
            Filter::CatmullRomFast => InterpolationDetails::bicubic(1f64, 1f64, 0f64, 0.5f64),
            Filter::CatmullRomFastSharp => InterpolationDetails::bicubic(1f64, 13.0f64
               /
                16.0f64, 0f64, 0.5f64),

            Filter::Mitchell => InterpolationDetails::bicubic(2f64, 1f64, 1.0f64 / 3.0f64, 1.0f64 / 3.0f64),
            Filter::MitchellFast => InterpolationDetails::bicubic(1f64, 1f64, 1.0f64 / 3.0f64, 1.0f64 / 3.0f64),
            Filter::NCubic => InterpolationDetails::bicubic(2.5f64, 1.0f64
                /
                1.1685777620836933f64,
                                                            0.3782157550939987f64,
                                                            0.3108921224530007f64),

            Filter::NCubicSharp => InterpolationDetails::bicubic(2.5f64, 1.0f64
                /
                1.105822933719019f64,
                                                                 0.2620145123990142f64,
                                                                 0.3689927438004929f64),

            Filter::Robidoux => InterpolationDetails::bicubic(2f64, 1.0f64,
                                                              0.3782157550939987f64,
                                                              0.3108921224530007f64),

            Filter::LegacyIDCTFilter => InterpolationDetails::bicubic(2f64, 1. / 1.1685777620836932,
                                                                      0.3782157550939987f64,
                                                                      0.3108921224530007f64),


            Filter::Fastest => InterpolationDetails::bicubic(0.74f64,
                                                             0.74f64,
                                                             0.3782157550939987f64,
                                                             0.3108921224530007f64),
            Filter::RobidouxFast => InterpolationDetails::bicubic(
                1.05f64,
                1f64,
                0.3782157550939987f64,
                0.3108921224530007f64),

            Filter::RobidouxSharp => InterpolationDetails::bicubic(
                2f64,
                1f64,
                0.2620145123990142f64,
                0.3689927438004929f64),

            Filter::Hermite => InterpolationDetails::bicubic(
                1f64,
                1f64,
                0f64,
                0f64)

        }
    }

    fn calculate_percent_negative_weight(&self) -> f64 {
        let samples: i32 = 50i32;
        let step: f64 =
            self.window / samples as f64;
        let mut last_height: f64 =
            (self.filter)(self, -step);
        let mut positive_area: f64 = 0i32 as f64;
        let mut negative_area: f64 = 0i32 as f64;
        for i in 0..(samples+3i32) {
            let height: f64 =
                (self.filter)(self, i as f64 * step);
            let area: f64 = (height + last_height) / 2.0f64 * step;
            last_height = height;
            if area > 0i32 as f64 {
                positive_area += area
            } else { negative_area -= area }
        }
        negative_area / positive_area
    }
}
 fn filter_flex_cubic(d: &InterpolationDetails, x: f64) -> f64{
    let t: f64 = x.abs() / d.blur;
    if t < 1.0 {
        return d.p1 + t * (t * (d.p2 + t * d.p3));
    }
    if t < 2.0 {
        return d.q1 + t * (d.q2 + t * (d.q3 + t * d.q4));
    }
    0.0
}

 fn filter_bicubic_fast(d: &InterpolationDetails,
                                          t: f64)
                                         -> f64 {
    let abs_t: f64 = t.abs() / d.blur;
    let abs_t_sq: f64 = abs_t * abs_t;
    if abs_t < 1f64 {
        1f64 - 2f64 * abs_t_sq +
            abs_t_sq * abs_t
    } else if abs_t < 2i32 as f64 {
        4f64 - 8f64 * abs_t +
            5f64 * abs_t_sq - abs_t_sq * abs_t
    } else {
        0f64
    }
}

 fn filter_sinc( d: &InterpolationDetails,t: f64) -> f64 {
    let abs_t: f64 = t.abs() / d.blur;
    if abs_t == 0f64 {
        // Avoid division by zero
        return 1f64
    } else if abs_t > d.window {
        return 0f64
    } else {
        let a: f64 = abs_t * f64::consts::PI;
        return a.sin() / a
    };
}
 fn filter_box(d: &InterpolationDetails, t: f64) -> f64 {
    let x: f64 = t / d.blur;
    if x >= -1f64 * d.window && x < d.window {
        1f64
    } else {
        0f64
    }
}
 fn filter_triangle(d: &InterpolationDetails, t: f64) -> f64 {
    let x: f64 = t.abs() / d.blur;
    if x < 1.0f64 { return 1.0f64 - x } else { return 0.0f64 };
}

 fn filter_sinc_windowed( d: &InterpolationDetails,
                                           t: f64)
                                          -> f64 {
    let x: f64 = t / d.blur;
    let abs_t: f64 = x.abs();
    if abs_t == 0i32 as f64 {
        // Avoid division by zero
        return 1f64
    } else if abs_t > d.window {
        return 0f64
    } else {
        return d.window * (f64::consts::PI * x / d.window).sin() *
            (x * f64::consts::PI).sin() /
            (f64::consts::PI * f64::consts::PI * x * x)
    };
}


 fn filter_jinc(d: &InterpolationDetails, t: f64) -> f64 {
    let x: f64 = t.abs() / d.blur;
    ////x crossing #1 1.2196698912665045
    if x == 0.0f64 {
        0.5f64 * f64::consts::PI
        //j1 is from libm
    } else {
        bessj1(f64::consts::PI * x) / x
    }
}


/*

static inline double window_jinc (double x) {
    double x_a = x * 1.2196698912665045;
    if (x == 0.0)
        return 1;
    return (BesselOrderOne (IR_PI*x_a) / (x_a * IR_PI * 0.5));
    ////x crossing #1 1.2196698912665045
}

static double filter_window_jinc (const struct flow_interpolation_details * d, double t) {
    return window_jinc (t / (d->blur * d->window));
}
*/

 fn filter_ginseng(d: &InterpolationDetails, t: f64) -> f64 {
    // Sinc windowed by jinc
    let abs_t: f64 = t.abs() / d.blur;
    let t_pi: f64 = abs_t * f64::consts::PI;
    if abs_t == 0f64 {
        // Avoid division by zero
         1f64
    } else if abs_t > 3f64 {
         0f64
    } else {
        let jinc_input: f64 =
            1.2196698912665046f64 * t_pi / d.window;
        let jinc_output: f64 =
            bessj1(jinc_input) / (jinc_input * 0.5f64);
         jinc_output * t_pi.sin() / t_pi
    }
}


 fn bessj1(x :f64) -> f64 {
    // For improvement consider https://www.cl.cam.ac.uk/~jrh13/papers/bessel.pdf
    // TODO: test jinc filters against C impl
    let ax = x.abs();
    let ans = if ax < 8f64 {
        let y = x * x;
        let ans1 = x * (72362614232.0 + y * (-7895059235.0 + y * (242396853.1
            + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
        let ans2 = 144725228442.0 + y * (2300535178.0 + y * (18583304.74
            + y * (99447.43394 + y * (376.9991397 + y * 1.0))));
        ans1 / ans2
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 2.356194491;
        let ans1 = 1.0 + y * (0.183105e-2 + y * (-0.3516396496e-4
            + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
        let ans2 = 0.04687499995 + y * (-0.2002690873e-3
            + y * (0.8449199096e-5 + y * (-0.88228987e-6
            + y * 0.105787412e-6)));
        (0.636619772 / ax).sqrt() * ((xx).cos() * ans1 - z * (xx).sin() * ans2)
    };
    if x < 0f64 {
        -ans
    } else {
        ans
    }
}


#[derive(Debug,  Clone, PartialEq, Eq)]
pub enum WeightsError{
    AllocationFailed,
    NoPixelInputs,
    ContribDataTooSmall,
    PixelWeightsTooSmall,
    SourcePixelCountTooLarge,
    TotalWeightZero,
    TryReserveCalledRepeatedly,
    TryReserveNotCalled
}



 pub trait WeightContainer{
     /// Must be called before add_output_pixel, and must only be called once
    fn try_reserve(&mut self, total_output_pixels: u32, inputs_per_outputs: u32) -> Result<(),WeightsError>;
    fn add_output_pixel(&mut self, left_input_pixel: u32, right_input_pixel: u32, weights: &[f32]) -> Result<(),WeightsError>;

}

pub struct PixelRowWeights {
    contrib_row: Option<AlignedBuffer<PixelWeightIndexes>>,
    contrib_row_length: usize,
    weights: Option<AlignedBuffer<f32>>,
    weights_length: usize
}

impl PixelRowWeights{
    pub fn new() -> PixelRowWeights{
        PixelRowWeights{
            contrib_row: None,
            contrib_row_length: 0,
            weights: None,
            weights_length: 0
        }
    }
    pub fn contrib_row(&self) -> &[PixelWeightIndexes]{
        self.contrib_row
            .as_ref()
            .map(|r| &r.as_slice()[..self.contrib_row_length])
            .unwrap_or(&[])
    }
    pub fn weights(&self) -> &[f32]{
        self.weights
            .as_ref()
            .map(|r| &r.as_slice()[..self.weights_length])
            .unwrap_or(&[])
    }

    pub fn create_for(details: &InterpolationDetails,
                      output_line_size :u32,
                      input_line_size :u32) -> Result<PixelRowWeights,WeightsError>{
        let mut weights = PixelRowWeights::new();
        populate_weights(&mut weights, output_line_size, input_line_size, details)?;
        Ok(weights)
    }

}
#[derive(Clone, Copy)]
#[repr(C)]
pub struct PixelWeightIndexes {
    /// index of weight for first input pixel
    pub left_weight: u32,
    /// index of weight for last input pixel
    pub right_weight: u32,
    /// index of first input pixel
    pub left_pixel: u32,
    /// index of last input pixel
    pub right_pixel: u32,
}

unsafe impl rgb::Zeroable for PixelWeightIndexes{}

impl WeightContainer for PixelRowWeights {
    fn try_reserve(&mut self, total_output_pixels: u32, inputs_per_outputs: u32) -> Result<(),WeightsError> {
        if self.contrib_row.is_some() ||
            self.weights.is_some(){
            return Err(WeightsError::TryReserveCalledRepeatedly);
        }
        self.contrib_row = Some(AlignedBuffer::new(total_output_pixels as usize, 16)
            .map_err(|e| WeightsError::AllocationFailed )?);

        self.weights = Some(AlignedBuffer::new(inputs_per_outputs as usize * total_output_pixels as usize, 64)
            .map_err(|e| WeightsError::AllocationFailed )?);

        Ok(()) //TODO: use fallible allocators in nightly mode
    }

    fn add_output_pixel(&mut self, left_input_pixel: u32, right_input_pixel: u32, weights_to_add: &[f32]) -> Result<(),WeightsError> {
        if self.contrib_row.is_none() ||
            self.weights.is_none(){
            return Err(WeightsError::TryReserveNotCalled)
        }
        let contrib_row = self.contrib_row.as_mut().unwrap().as_slice_mut();
        let weights = self.weights.as_mut().unwrap().as_slice_mut();


        if self.contrib_row_length >= contrib_row.len() {
            return Err(WeightsError::ContribDataTooSmall);
        }
        if self.weights_length + weights_to_add.len() > weights.len() {
            return Err(WeightsError::PixelWeightsTooSmall);
        }
        if weights_to_add.len() == 0 {
            return Err(WeightsError::NoPixelInputs);
        }


        let left_weight = self.weights_length;
        let right_weight = self.weights_length + weights_to_add.len() - 1;

        weights[left_weight..=right_weight].copy_from_slice(weights_to_add);
        self.weights_length = right_weight + 1;

        contrib_row[self.contrib_row_length] = PixelWeightIndexes {
            left_weight: left_weight as u32,
            right_weight: right_weight as u32,
            left_pixel: left_input_pixel,
            right_pixel: right_input_pixel
        };
        self.contrib_row_length += 1;
        Ok(())
    }
}


#[derive(Clone)]
#[repr(C)]
pub struct PixelRowWeightsSimple {
    pub contrib_row: Vec<PixelWeightsSimple>,
}
#[derive(Clone)]
#[repr(C)]
pub struct PixelWeightsSimple {
    /// weights for input pixels
    pub weights: Vec<f32>,
    /// index of first input pixel
    pub left: i32,
    /// index of last input pixel
    pub right: i32,
}

impl WeightContainer for PixelRowWeightsSimple {
    fn try_reserve(&mut self, total_output_pixels: u32, inputs_per_outputs: u32) -> Result<(),WeightsError> {
        let space_needed = total_output_pixels as usize - self.contrib_row.len();
        if space_needed > 0{
            self.contrib_row.reserve_exact(space_needed)
        }
        Ok(()) //TODO: use fallible allocators in nightly mode
    }

    fn add_output_pixel(&mut self, left_input_pixel: u32, right_input_pixel: u32, weights: &[f32]) -> Result<(),WeightsError> {
        if self.contrib_row.len() < self.contrib_row.capacity() {
            self.contrib_row.push(PixelWeightsSimple { weights: weights.to_vec(), left: left_input_pixel as i32, right: right_input_pixel as i32});
            Ok(())
        }else{
            Err(WeightsError::ContribDataTooSmall)
        }

    }
}
pub  fn populate_weights<T:WeightContainer>(container: &mut T, output_line_size :u32,
                         input_line_size :u32,details: &InterpolationDetails) -> Result<(),WeightsError> {
    let sharpen_ratio: f64 = details.calculate_percent_negative_weight();
    let desired_sharpen_ratio: f64 =
        1.0f64.min(
             sharpen_ratio.max(
                  details.sharpen_percent_goal as f64 /
                      100.0f64));
    let scale_factor: f64 =
        output_line_size as f64 /
            input_line_size as f64;
    let downscale_factor: f64 = 1.0f64.min(scale_factor);
    let half_source_window: f64 =
        (details.window + 0.5f64) / downscale_factor;
    let allocated_window_size: u32 =
        ((2i32 as f64 * (half_source_window - 0.00001f64)).ceil() as
            i32 + 1i32) as u32;

    container.try_reserve(output_line_size, allocated_window_size)?;



    let filter_func = details.filter;


    let mut negative_area: f64 = 0f64;
    let mut positive_area: f64 = 0f64;

    let mut weights: Vec<f32> = Vec::with_capacity(allocated_window_size as usize); //Allocation!
    for u in 0..output_line_size {
        weights.clear();
        let center_src_pixel: f64 =
            (u as f64 + 0.5f64) / scale_factor - 0.5f64;
        let left_edge: i32 = (center_src_pixel - details.window / downscale_factor - 0.0001)
            .ceil() as i32;
        let right_edge: i32 = (center_src_pixel + details.window / downscale_factor + 0.0001)
            .floor() as i32;
        let left_src_pixel: u32 =
            0i32.max(left_edge) as u32;
        let right_src_pixel: u32 =
            right_edge.min(input_line_size as i32 - 1i32) as
                u32;
        // Net weight
        let mut total_weight: f64 = 0.0f64;
        // Sum of negative and positive weights
        let mut total_negative_weight: f64 = 0.0f64;
        let mut total_positive_weight: f64 = 0.0f64;
        let source_pixel_count: u32 =
            right_src_pixel.wrapping_sub(left_src_pixel).wrapping_add(1i32
                as
                u32);
        if source_pixel_count > allocated_window_size {
            //flow_status_Invalid_internal_state,
            return Err(WeightsError::SourcePixelCountTooLarge);
        }

        for ix in left_src_pixel..=right_src_pixel {
            let tx = ix - left_src_pixel;
            let mut add: f64 =
                filter_func(details,
                            downscale_factor
                                *
                                (ix
                                    as
                                    f64
                                    -
                                    center_src_pixel));
            if add.abs() <= 2e-8f64 { add = 0.0f64 }
            // Weights below a certain threshold make consistent x-plat
            // integration test results impossible. pos/neg zero, etc.
            // They should be rounded down to zero at the threshold at which results are consistent.
            weights.push(add as f32);
            total_weight += add;
            total_negative_weight += add.min(0f64);
            total_positive_weight += add.max(0f64);
        }
        let mut neg_factor = (1.0f64 / total_weight) as f32;
        let mut pos_factor = neg_factor;
        //printf("cur= %f cur+= %f cur-= %f desired_sharpen_ratio=%f sharpen_ratio-=%f\n", total_weight, total_positive_weight, total_negative_weight, desired_sharpen_ratio, sharpen_ratio);

        if total_weight <= 0.0f64 ||  desired_sharpen_ratio > sharpen_ratio {
            if total_negative_weight < 0.0f64 {
                if desired_sharpen_ratio < 1.0f64 {
                    let target_positive_weight: f64 =
                        1.0f64 / (1.0f64 - desired_sharpen_ratio);
                    let target_negative_weight: f64 =
                        desired_sharpen_ratio * -target_positive_weight;
                    pos_factor =
                        (target_positive_weight /
                            total_positive_weight) as f32;
                    neg_factor =
                        (target_negative_weight /
                            total_negative_weight) as f32;
                    if total_negative_weight == 0f64
                        {
                            neg_factor = 1.0f32
                        }
                }
            } else if total_weight == 0f64{
                // In this situation we have a problem to report
                return Err(WeightsError::TotalWeightZero);
            }
        }
        //printf("\n");
        for v in weights.iter_mut() {
            if *v < 0f32 {
                *v *= neg_factor;
                negative_area -= *v as f64;
            } else {
                *v *= pos_factor;
                positive_area += *v as f64;
            }
        }

        // Shrink to improve perf & result consistency
        // Shrink region from the right
        let mut shrunk_right_src_pixel = right_src_pixel;
        while weights.ends_with(&[0f32]) {
            shrunk_right_src_pixel -= 1;
            weights.truncate(weights.len() - 1);
        }
        let mut shrunk_left_src_pixel = left_src_pixel;
        while weights.starts_with(&[0f32]) {
            shrunk_left_src_pixel += 1;
            weights.remove(0);
        }

        container.add_output_pixel(shrunk_left_src_pixel, shrunk_right_src_pixel, &weights)?;
    }
    //(*res).percent_negative = negative_area / positive_area;
    Ok(())
}
