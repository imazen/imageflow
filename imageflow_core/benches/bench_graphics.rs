extern crate imageflow_core;

use criterion::{criterion_group, criterion_main, Criterion};
use imageflow_core::graphics::bitmaps::*;
use imageflow_core::graphics::color::WorkingFloatspace;
use imageflow_core::graphics::scaling::ScaleAndRenderParams;
use imageflow_types::*;
use itertools::Itertools;
use std::time::Duration;

fn benchmark_transpose(ctx: &mut Criterion) {
    let sizes: &[(u32, u32)] = &[
        (1000, 1000),
        (1000, 2373),
        (2373, 1000),
        (2373, 2373),
        (3840, 2160), // 4K
        (7680, 4320), // 8K
    ];

    for &(w, h) in sizes {
        let mut a = Bitmap::create_u8(
            w,
            h,
            PixelLayout::BGRA,
            true,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();
        let mut b = Bitmap::create_u8(
            h,
            w,
            PixelLayout::BGRA,
            true,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();
        let mut a_window = a.get_window_u8().unwrap();
        let mut b_window = b.get_window_u8().unwrap();

        a_window
            .fill_rect(0, 0, w, h, &Color::Srgb(ColorSrgb::Hex("FF0000FF".to_string())))
            .unwrap();

        let mut group = ctx.benchmark_group(&format!("transpose w={} && h={}", w, h));
        group.measurement_time(Duration::from_secs(3));

        group.bench_function("Rust", |bencher| {
            bencher.iter(|| {
                imageflow_core::graphics::transpose::bitmap_window_transpose(
                    &mut a_window,
                    &mut b_window,
                )
                .unwrap();
            })
        });

        group.finish();
    }
}

/// Benchmark transpose with different cache tile block sizes to find the optimum.
fn benchmark_transpose_block_sizes(_ctx: &mut Criterion) {
    // Requires the archmage-based transpose code (transpose_u32_slices_with_block_size).
    // Skipped when benchmarking original code.
    {
        use imageflow_core::graphics::transpose::transpose_u32_slices_with_block_size;

        let sizes: &[(usize, usize)] = &[
            (2373, 2373),
            (3840, 2160), // 4K
            (7680, 4320), // 8K
        ];
        let block_sizes: &[usize] = &[8, 16, 24, 32, 48, 64, 96, 128, 192, 256];

        for &(w, h) in sizes {
            let from: Vec<u32> = (0..(w * h) as u32).collect();
            let mut to = vec![0u32; w * h];

            let mut group = _ctx.benchmark_group(&format!("transpose_blk {w}x{h}"));
            group.measurement_time(Duration::from_secs(3));

            for &bs in block_sizes {
                group.bench_function(&format!("bs={bs}"), |bencher| {
                    bencher.iter(|| {
                        transpose_u32_slices_with_block_size(&from, &mut to, w, h, w, h, bs)
                            .unwrap();
                    })
                });
            }

            group.finish();
        }
    }
}

// cargo bench -- benchmark_flip_v
fn benchmark_flip_v(ctx: &mut Criterion) {
    let fmts = [PixelLayout::BGRA];

    for &fmt in fmts.iter() {
        for w in (500u32..3000u32).step_by(2373) {
            for h in (500u32..3000u32).step_by(2373) {
                let mut bitmap_a = Bitmap::create_u8(
                    w,
                    h,
                    fmt,
                    true,
                    true,
                    ColorSpace::StandardRGB,
                    BitmapCompositing::ReplaceSelf,
                )
                .unwrap();
                bitmap_a
                    .get_window_u8()
                    .unwrap()
                    .fill_rect(0, 0, w, h, &Color::Srgb(ColorSrgb::Hex("FF0000FF".to_string())))
                    .unwrap();

                let mut group =
                    ctx.benchmark_group(&format!("flip_v w={} && h={} fmt={:?}", w, h, fmt));

                group.bench_function("Rust", |b| {
                    b.iter(|| {
                        imageflow_core::graphics::flip::flow_bitmap_bgra_flip_vertical_safe(
                            &mut bitmap_a,
                        )
                        .unwrap();
                    })
                });

                group.finish();
            }
        }
    }
}

// cargo bench --bench bench_graphics  -- flip_h
fn benchmark_flip_h(ctx: &mut Criterion) {
    let fmts = [PixelLayout::BGRA];

    for &fmt in fmts.iter() {
        for w in (500u32..3000u32).step_by(2373) {
            for h in (500u32..3000u32).step_by(2373) {
                let mut a = Bitmap::create_u8(
                    w,
                    h,
                    fmt,
                    true,
                    true,
                    ColorSpace::StandardRGB,
                    BitmapCompositing::ReplaceSelf,
                )
                .unwrap();
                let mut a_window = a.get_window_u8().unwrap();
                a_window
                    .fill_rect(0, 0, w, h, &Color::Srgb(ColorSrgb::Hex("FF0000FF".to_string())))
                    .unwrap();

                let mut group =
                    ctx.benchmark_group(&format!("flip_h w={} && h={} fmt={:?}", w, h, fmt));

                group.bench_function("Rust", |b| {
                    b.iter(|| {
                        imageflow_core::graphics::flip::flow_bitmap_bgra_flip_horizontal_safe(
                            &mut a,
                        )
                        .unwrap();
                    })
                });

                group.finish();
            }
        }
    }
}

fn benchmark_scale_2d(ctx: &mut Criterion) {
    let fmts = [PixelLayout::BGRA];
    let float_spaces = [WorkingFloatspace::LinearRGB, WorkingFloatspace::StandardRGB];
    for &float_space in float_spaces.iter() {
        for &fmt in fmts.iter() {
            for w in (500u32..4000u32).step_by(2400) {
                for h in (500u32..4000u32).step_by(2400) {
                    let mut bitmap_a = Bitmap::create_u8(
                        w,
                        h,
                        PixelLayout::BGRA,
                        true,
                        true,
                        ColorSpace::LinearRGB,
                        BitmapCompositing::ReplaceSelf,
                    )
                    .unwrap();

                    let mut bitmap_b = Bitmap::create_u8(
                        800u32,
                        800u32,
                        PixelLayout::BGRA,
                        true,
                        true,
                        ColorSpace::LinearRGB,
                        BitmapCompositing::ReplaceSelf,
                    )
                    .unwrap();

                    let scale_rust = ScaleAndRenderParams {
                        x: 0u32,
                        y: 0u32,
                        w: 800u32,
                        h: 800u32,
                        sharpen_percent_goal: 0.0,
                        interpolation_filter: imageflow_core::graphics::weights::Filter::Robidoux,
                        scale_in_colorspace: float_space,
                    };

                    let mut group = ctx.benchmark_group(&format!(
                        "scale_2d w={} && h={} fmt={:?} float_space={:?}",
                        w, h, fmt, float_space
                    ));

                    group.measurement_time(Duration::from_secs(5));

                    group.bench_function("SafeRust", |b| {
                        b.iter(|| {
                            assert_eq!(
                                imageflow_core::graphics::scaling::scale_and_render(
                                    bitmap_a.get_window_u8().unwrap(),
                                    bitmap_b.get_window_u8().unwrap(),
                                    &scale_rust
                                ),
                                Ok(())
                            )
                        })
                    });

                    group.finish();
                }
            }
        }
    }
}
//
extern "C" {
    pub fn flow_scale_spatial_srgb_7x7(
        input: *const u8,
        output_rows: *const *mut u8,
        output_col: u32,
    );
    pub fn flow_scale_spatial_srgb_6x6(
        input: *const u8,
        output_rows: *const *mut u8,
        output_col: u32,
    );
    pub fn flow_scale_spatial_srgb_5x5(
        input: *const u8,
        output_rows: *const *mut u8,
        output_col: u32,
    );
    pub fn flow_scale_spatial_srgb_4x4(
        input: *const u8,
        output_rows: *const *mut u8,
        output_col: u32,
    );
    pub fn flow_scale_spatial_srgb_3x3(
        input: *const u8,
        output_rows: *const *mut u8,
        output_col: u32,
    );
    pub fn flow_scale_spatial_srgb_2x2(
        input: *const u8,
        output_rows: *const *mut u8,
        output_col: u32,
    );
    pub fn flow_scale_spatial_srgb_1x1(
        input: *const u8,
        output_rows: *const *mut u8,
        output_col: u32,
    );
    pub fn flow_scale_spatial_7x7(input: *const u8, output_rows: *const *mut u8, output_col: u32);
    pub fn flow_scale_spatial_6x6(input: *const u8, output_rows: *const *mut u8, output_col: u32);
    pub fn flow_scale_spatial_5x5(input: *const u8, output_rows: *const *mut u8, output_col: u32);
    pub fn flow_scale_spatial_4x4(input: *const u8, output_rows: *const *mut u8, output_col: u32);
    pub fn flow_scale_spatial_3x3(input: *const u8, output_rows: *const *mut u8, output_col: u32);
    pub fn flow_scale_spatial_2x2(input: *const u8, output_rows: *const *mut u8, output_col: u32);
    pub fn flow_scale_spatial_1x1(input: *const u8, output_rows: *const *mut u8, output_col: u32);
}

fn benchmark_downscaling(ctx: &mut Criterion) {
    let mut output = [[0u8; 8]; 8];
    let input = [0u8; 64];
    let input_ptr = input.as_ptr();
    let mut row = output.iter_mut().map(|ele| ele.as_mut_ptr()).collect_vec();
    let funs = [
        flow_scale_spatial_srgb_7x7,
        flow_scale_spatial_srgb_6x6,
        flow_scale_spatial_srgb_5x5,
        flow_scale_spatial_srgb_4x4,
        flow_scale_spatial_srgb_3x3,
        flow_scale_spatial_srgb_2x2,
        flow_scale_spatial_srgb_1x1,
        flow_scale_spatial_7x7,
        flow_scale_spatial_6x6,
        flow_scale_spatial_5x5,
        flow_scale_spatial_4x4,
        flow_scale_spatial_3x3,
        flow_scale_spatial_2x2,
        flow_scale_spatial_1x1,
    ];

    for (i, &fun) in funs.iter().enumerate() {
        ctx.bench_function(&format!("downscale function={}", i), |bn| {
            bn.iter(|| unsafe { fun(input_ptr, row.as_mut_ptr(), 0) })
        });
    }
}
// Micro-benchmarks for optimization targets
fn benchmark_color_conversion(ctx: &mut Criterion) {
    use imageflow_core::graphics::color::{ColorContext, WorkingFloatspace};

    let cc_linear = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);
    let cc_srgb = ColorContext::new(WorkingFloatspace::StandardRGB, 0.0);

    // Test data: 1000x1000 pixels worth of color values
    let pixel_count = 1000 * 1000;
    let u8_values: Vec<u8> = (0..pixel_count).map(|i| (i % 256) as u8).collect();
    let f32_values: Vec<f32> = (0..pixel_count).map(|i| (i % 256) as f32 / 255.0).collect();

    let mut group = ctx.benchmark_group("color_conversion");
    group.throughput(criterion::Throughput::Elements(pixel_count as u64));

    // sRGB u8 -> linear f32 (uses LUT)
    group.bench_function("srgb_to_floatspace_linear", |b| {
        b.iter(|| {
            let mut sum = 0.0f32;
            for &v in &u8_values {
                sum += cc_linear.srgb_to_floatspace(v);
            }
            criterion::black_box(sum)
        })
    });

    // sRGB u8 -> sRGB f32 (no conversion, uses LUT)
    group.bench_function("srgb_to_floatspace_srgb", |b| {
        b.iter(|| {
            let mut sum = 0.0f32;
            for &v in &u8_values {
                sum += cc_srgb.srgb_to_floatspace(v);
            }
            criterion::black_box(sum)
        })
    });

    // linear f32 -> sRGB u8 (uses fastpow - HOT PATH)
    group.bench_function("floatspace_to_srgb_linear", |b| {
        b.iter(|| {
            let mut sum = 0u32;
            for &v in &f32_values {
                sum += cc_linear.floatspace_to_srgb(v) as u32;
            }
            criterion::black_box(sum)
        })
    });

    // sRGB f32 -> sRGB u8 (no gamma, just scale)
    group.bench_function("floatspace_to_srgb_srgb", |b| {
        b.iter(|| {
            let mut sum = 0u32;
            for &v in &f32_values {
                sum += cc_srgb.floatspace_to_srgb(v) as u32;
            }
            criterion::black_box(sum)
        })
    });

    group.finish();
}

fn benchmark_row_operations(ctx: &mut Criterion) {
    // Test multiply_and_add_row (already has multiversion)
    let width = 2000usize;
    let channels = 4usize;
    let len = width * channels;

    let input_row: Vec<f32> = (0..len).map(|i| (i % 256) as f32 / 255.0).collect();
    let mut output_row: Vec<f32> = vec![0.0; len];

    let mut group = ctx.benchmark_group("row_operations");
    group.throughput(criterion::Throughput::Elements(len as u64));

    group.bench_function("multiply_and_add_row", |b| {
        b.iter(|| {
            output_row.fill(0.0);
            // Simulate typical vertical scaling: ~5-10 weighted row additions
            for weight in [0.05f32, 0.15, 0.30, 0.30, 0.15, 0.05] {
                imageflow_core::graphics::scaling::multiply_and_add_row(
                    &mut output_row,
                    &input_row,
                    weight,
                );
            }
            criterion::black_box(output_row[0])
        })
    });

    group.finish();
}

fn benchmark_horizontal_scale(ctx: &mut Criterion) {
    use imageflow_core::graphics::weights::{Filter, InterpolationDetails, PixelRowWeights};

    // Simulate 2000px -> 800px horizontal downscale
    let src_width = 2000usize;
    let dst_width = 800usize;
    let channels = 4usize;

    let source: Vec<f32> = (0..src_width * channels).map(|i| (i % 256) as f32 / 255.0).collect();
    let mut target: Vec<f32> = vec![0.0; dst_width * channels];

    let details = InterpolationDetails::create(Filter::Robidoux);
    let weights =
        PixelRowWeights::create_for(&details, dst_width as u32, src_width as u32).unwrap();

    let mut group = ctx.benchmark_group("horizontal_scale");
    group.throughput(criterion::Throughput::Elements((dst_width * channels) as u64));

    group.bench_function("scale_row_bgra_f32_2000_to_800", |b| {
        b.iter(|| {
            imageflow_core::graphics::scaling::scale_row_bgra_f32(
                &source,
                src_width,
                &mut target,
                dst_width,
                &weights,
                0,
            );
            criterion::black_box(target[0])
        })
    });

    group.finish();
}

fn benchmark_full_scale_pipeline(ctx: &mut Criterion) {
    // End-to-end benchmark with different sizes
    let test_cases = [
        (800, 600, 400, 300, "800x600_to_400x300"),
        (1920, 1080, 640, 360, "1920x1080_to_640x360"),
        (4000, 3000, 800, 600, "4000x3000_to_800x600"),
    ];

    for (src_w, src_h, dst_w, dst_h, name) in test_cases {
        let mut src_bitmap = Bitmap::create_u8(
            src_w,
            src_h,
            PixelLayout::BGRA,
            true,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();

        // Fill with gradient data
        {
            let mut window = src_bitmap.get_window_u8().unwrap();
            for y in 0..src_h {
                if let Some(row) = window.row_mut(y as usize) {
                    for x in 0..src_w {
                        let idx = (x * 4) as usize;
                        row[idx] = (x % 256) as u8;
                        row[idx + 1] = (y % 256) as u8;
                        row[idx + 2] = ((x + y) % 256) as u8;
                        row[idx + 3] = 255;
                    }
                }
            }
        }

        let mut dst_bitmap = Bitmap::create_u8(
            dst_w,
            dst_h,
            PixelLayout::BGRA,
            true,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();

        let params = ScaleAndRenderParams {
            x: 0,
            y: 0,
            w: dst_w,
            h: dst_h,
            sharpen_percent_goal: 0.0,
            interpolation_filter: imageflow_core::graphics::weights::Filter::Robidoux,
            scale_in_colorspace: WorkingFloatspace::LinearRGB,
        };

        let mut group = ctx.benchmark_group("full_scale_pipeline");
        group.throughput(criterion::Throughput::Elements((dst_w * dst_h) as u64));
        group.measurement_time(Duration::from_secs(5));

        group.bench_function(name, |b| {
            b.iter(|| {
                imageflow_core::graphics::scaling::scale_and_render(
                    src_bitmap.get_window_u8().unwrap(),
                    dst_bitmap.get_window_u8().unwrap(),
                    &params,
                )
                .unwrap();
            })
        });

        group.finish();
    }
}

criterion_group!(
    benches,
    benchmark_color_conversion,
    benchmark_row_operations,
    benchmark_horizontal_scale,
    benchmark_full_scale_pipeline,
    benchmark_scale_2d,
    benchmark_transpose,
    benchmark_transpose_block_sizes,
    benchmark_downscaling,
    benchmark_flip_h,
    benchmark_flip_v
);
criterion_main!(benches);
