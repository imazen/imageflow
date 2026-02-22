use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use imageflow_focus::{analyze_saliency, AnalysisConfig};

/// Create a synthetic BGRA test image with a bright region and skin-like region.
fn make_test_image(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;

            // Background: neutral gray
            let mut r = 128u8;
            let mut g = 128u8;
            let mut b = 128u8;

            // Upper-left quadrant: bright red (edge + saturation signal)
            if x < width / 4 && y < height / 4 {
                r = 220;
                g = 30;
                b = 30;
            }

            // Center: skin-like color (skin signal)
            let cx = width / 2;
            let cy = height / 2;
            let dx = (x as i32 - cx as i32).unsigned_abs();
            let dy = (y as i32 - cy as i32).unsigned_abs();
            if dx < width / 8 && dy < height / 8 {
                r = 198;
                g = 155;
                b = 119;
            }

            pixels[idx] = b; // B
            pixels[idx + 1] = g; // G
            pixels[idx + 2] = r; // R
            pixels[idx + 3] = 255; // A
        }
    }

    pixels
}

fn bench_analyze_pipeline(c: &mut Criterion) {
    let config = AnalysisConfig::default();
    let mut group = c.benchmark_group("saliency_pipeline");

    for size in [256, 512, 1024] {
        let pixels = make_test_image(size, size);
        group.bench_with_input(BenchmarkId::new("analyze", size), &size, |b, &size| {
            b.iter(|| analyze_saliency(&pixels, size, size, &config));
        });
    }

    group.finish();
}

fn bench_analyze_no_wb(c: &mut Criterion) {
    let config = AnalysisConfig { white_balance_compensate: false, ..AnalysisConfig::default() };
    let mut group = c.benchmark_group("saliency_no_wb");

    for size in [256, 512] {
        let pixels = make_test_image(size, size);
        group.bench_with_input(BenchmarkId::new("analyze_no_wb", size), &size, |b, &size| {
            b.iter(|| analyze_saliency(&pixels, size, size, &config));
        });
    }

    group.finish();
}

fn bench_large_source(c: &mut Criterion) {
    // Benchmark with a large source image that requires downsampling
    let config = AnalysisConfig::default();
    let pixels = make_test_image(2048, 1536);

    c.bench_function("analyze_2048x1536_downsample", |b| {
        b.iter(|| analyze_saliency(&pixels, 2048, 1536, &config));
    });
}

criterion_group!(benches, bench_analyze_pipeline, bench_analyze_no_wb, bench_large_source);
criterion_main!(benches);
