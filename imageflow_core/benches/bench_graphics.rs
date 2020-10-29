extern crate imageflow_core;

use imageflow_core::ffi::*;
use imageflow_types::*;
use imageflow_core::Context;
use criterion::{ criterion_group, criterion_main, Criterion};
use imageflow_core::graphics::bitmaps::*;
use imageflow_core::graphics::scaling::ScaleAndRenderParams;
use imageflow_core::graphics::color::WorkingFloatspace;
use std::time::Duration;
use itertools::Itertools;


fn benchmark_transpose(ctx: &mut Criterion) {
    for w in (1000u32..3000u32).step_by(1373) {
        for h in (1000u32..3000u32).step_by(1373) {
            let c = Context::create().unwrap();
            let mut a = Bitmap::create_u8(w, h, PixelLayout::BGRA, true, true, ColorSpace::StandardRGB,BitmapCompositing::ReplaceSelf).unwrap();
            let mut a = unsafe {
                let mut a_bgra = a.get_window_u8().unwrap().to_bitmap_bgra().unwrap();
                a_bgra.fill_rect( 0u32, 0u32, w, h, &Color::Srgb(ColorSrgb::Hex("FF0000FF".to_string()))).unwrap();
                a_bgra
            };
            let mut b = Bitmap::create_u8(h,w, PixelLayout::BGRA, true, true, ColorSpace::StandardRGB,BitmapCompositing::ReplaceSelf).unwrap();
            let mut b = unsafe {
                let a_bgra=b.get_window_u8().unwrap().to_bitmap_bgra().unwrap();
                a_bgra
            };

            let mut group = ctx.benchmark_group(&format!("transpose w={} && h={}", w, h));

            // Now we can perform benchmarks with this group
            group.bench_function("C", |bencher| bencher.iter(|| {
                unsafe {
                    assert_eq!(flow_bitmap_bgra_transpose(c.flow_c(), &mut a as *mut BitmapBgra, &mut b as *mut BitmapBgra), true)
                }
            } ));
            group.bench_function("Rust", |bencher| bencher.iter(|| {
                unsafe {
                    assert_eq!(imageflow_core::graphics::transpose::flow_bitmap_bgra_transpose(&mut a as *mut BitmapBgra, &mut b as *mut BitmapBgra), Ok(()))
                }
            }));

            group.finish();
        }
    }
}



fn benchmark_flip_v(ctx: &mut Criterion) {
    let fmts=[PixelLayout::BGRA,PixelLayout::BGR];

    for &fmt in fmts.iter(){
        for w in (500u32..3000u32).step_by(2373){
            for h in (500u32..3000u32).step_by(2373){
                let c = Context::create().unwrap();
                let mut bitmap_a = Bitmap::create_u8(w, h, fmt, true, true, ColorSpace::StandardRGB,BitmapCompositing::ReplaceSelf).unwrap();
                let mut a = unsafe {
                    let mut a_bgra = bitmap_a.get_window_u8().unwrap().to_bitmap_bgra().unwrap();
                    a_bgra.fill_rect( 0u32, 0u32, w, h, &Color::Srgb(ColorSrgb::Hex("FF0000FF".to_string()))).unwrap();
                    a_bgra
                };

                let mut group = ctx.benchmark_group(&format!("flip_v w={} && h={} fmt={:?}",w,h,fmt));

                // Now we can perform benchmarks with this group
                group.bench_function("C", |bencher| bencher.iter(|| {
                    unsafe { assert_eq!(flow_bitmap_bgra_flip_vertical(c.flow_c(), &mut a as *mut BitmapBgra),true) }
                } ));
                group.bench_function("Rust", |b| b.iter(|| {
                    unsafe { assert_eq!(imageflow_core::graphics::flip::flow_bitmap_bgra_flip_vertical(&mut a as *mut BitmapBgra), Ok(())) }
                }));

                group.finish();

            }
        }
    }

}


fn benchmark_flip_h(ctx: &mut Criterion) {
    let fmts=[PixelLayout::BGRA,PixelLayout::BGR];

    for &fmt in fmts.iter(){
        for w in (500u32..3000u32).step_by(2373){
            for h in (500u32..3000u32).step_by(2373){
                let c = Context::create().unwrap();
                let mut a = Bitmap::create_u8(w, h, fmt, true, true, ColorSpace::StandardRGB,BitmapCompositing::ReplaceSelf).unwrap();
                let mut a = unsafe {
                    let mut a_bgra = a.get_window_u8().unwrap().to_bitmap_bgra().unwrap();
                    a_bgra.fill_rect( 0u32, 0u32, w, h, &Color::Srgb(ColorSrgb::Hex("FF0000FF".to_string()))).unwrap();
                    a_bgra
                };

                let mut group = ctx.benchmark_group(&format!("flip_h w={} && h={} fmt={:?}",w,h,fmt));

                // Now we can perform benchmarks with this group
                group.bench_function("C", |bencher| bencher.iter(|| {
                    unsafe { assert_eq!(flow_bitmap_bgra_flip_horizontal(c.flow_c(), &mut a as *mut BitmapBgra),true) }
                } ));
                group.bench_function("Rust", |b| b.iter(|| {
                    unsafe { assert_eq!(imageflow_core::graphics::flip::flow_bitmap_bgra_flip_horizontal(&mut a as *mut BitmapBgra), Ok(())) }
                }));

                group.finish();
            }
        }
    }

}

fn benchmark_scale_2d(ctx: &mut Criterion) {
    let fmts=[PixelLayout::BGRA];
    let float_spaces=[Floatspace::Srgb,Floatspace::Linear];
    for &float_space in float_spaces.iter(){
        for &fmt in fmts.iter(){
            for w in (500u32..4000u32).step_by(2400){
                for h in (500u32..4000u32).step_by(2400){
                    let c = Context::create().unwrap();
                    let mut bitmap_a = Bitmap::create_u8(w, h, PixelLayout::BGRA, true, true, ColorSpace::LinearRGB,BitmapCompositing::ReplaceSelf).unwrap();
                    let mut a = unsafe {
                        let mut a_bgra = bitmap_a.get_window_u8().unwrap().to_bitmap_bgra().unwrap();
                        a_bgra.fill_rect( 0u32, 0u32, w, h, &Color::Srgb(ColorSrgb::Hex("FF0000FF".to_string()))).unwrap();
                        a_bgra
                    };
                    let mut bitmap_b = Bitmap::create_u8(800u32,800u32, PixelLayout::BGRA, true, true, ColorSpace::LinearRGB,BitmapCompositing::ReplaceSelf).unwrap();
                    let mut b = unsafe {
                        let a_bgra=bitmap_b.get_window_u8().unwrap().to_bitmap_bgra().unwrap();
                        a_bgra
                    };
                    let scale_c =Scale2dRenderToCanvas1d{
                        x: 0u32,
                        y: 0u32,
                        w:800u32,
                        h:800u32,
                        sharpen_percent_goal: 0.0,
                        interpolation_filter: Filter::Robidoux,
                        scale_in_colorspace: float_space
                    };

                    let scale_rust = ScaleAndRenderParams{
                        x: 0u32,
                        y: 0u32,
                        w:800u32,
                        h:800u32,
                        sharpen_percent_goal: 0.0,
                        interpolation_filter: imageflow_core::graphics::weights::Filter::Robidoux,
                        scale_in_colorspace: match float_space{
                            Floatspace::Srgb => WorkingFloatspace::StandardRGB,
                            Floatspace::Linear => WorkingFloatspace::LinearRGB
                        }
                    };

                    let mut group = ctx.benchmark_group(&format!("scale_2d w={} && h={} fmt={:?} float_space={:?}",w,h,fmt,float_space));

                    group.measurement_time(Duration::from_secs(10));

                    // Now we can perform benchmarks with this group
                    group.bench_function("C", |bencher| bencher.iter(|| {
                        unsafe { assert_eq!(flow_node_execute_scale2d_render1d(c.flow_c(),
                                                                               &mut a as *mut BitmapBgra,
                                                                               &mut b as *mut BitmapBgra,
                                                                               &scale_c), true) }
                    } ));
                    group.bench_function("Rust", |b| b.iter(|| {
                        unsafe { assert_eq!(imageflow_core::graphics::scaling::flow_node_execute_scale2d_render1d(
                            bitmap_a.get_window_u8().unwrap(),bitmap_b.get_window_u8().unwrap(),&scale_rust), Ok(())) }
                    }));

                    group.finish();
                }
            }
        }
    }
}
//
extern "C" {
    pub fn flow_scale_spatial_srgb_7x7(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_srgb_6x6(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_srgb_5x5(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_srgb_4x4(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_srgb_3x3(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_srgb_2x2(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_srgb_1x1(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_7x7(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_6x6(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_5x5(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_4x4(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_3x3(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_2x2(input:*const u8, output_rows:*const*mut u8, output_col:u32);
    pub fn flow_scale_spatial_1x1(input:*const u8, output_rows:*const*mut u8, output_col:u32);
}

fn benchmark_downscaling(ctx: &mut Criterion) {
    let mut output = [[0u8; 8]; 8];
    let input = [0u8; 64];
    let input_ptr = input.as_ptr();
    let mut row = output.iter_mut().map(|ele| ele.as_mut_ptr()).collect_vec();
    let funs = [flow_scale_spatial_srgb_7x7,
        flow_scale_spatial_srgb_6x6,
        flow_scale_spatial_srgb_5x5,
        flow_scale_spatial_srgb_4x4,
        flow_scale_spatial_srgb_3x3, flow_scale_spatial_srgb_2x2,
        flow_scale_spatial_srgb_1x1, flow_scale_spatial_7x7, flow_scale_spatial_6x6,
        flow_scale_spatial_5x5, flow_scale_spatial_4x4, flow_scale_spatial_3x3,
        flow_scale_spatial_2x2, flow_scale_spatial_1x1
    ];


    for (i, &fun) in funs.iter().enumerate() {
        ctx.bench_function(&format!("downscale function={}", i), |bn| bn.iter(|| {
            unsafe { fun(input_ptr, row.as_mut_ptr(), 0) }
        }));
    }
}
criterion_group!(benches ,benchmark_scale_2d,benchmark_transpose,benchmark_downscaling,benchmark_flip_h,benchmark_flip_v);
criterion_main!(benches);

