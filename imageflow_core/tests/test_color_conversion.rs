/// Color conversion accuracy and performance tests
///
/// Tests:
/// 1. LUT correctness verification
/// 2. colorutils-rs comparison
/// 3. Brute-force accuracy for all valid inputs
use imageflow_core::graphics::color::{ColorContext, WorkingFloatspace};
use imageflow_core::graphics::lut::{linear_to_srgb_lut, LINEAR_TO_SRGB_LUT};
use std::time::Instant;

/// Calculate the exact sRGB gamma curve (for reference)
fn linear_to_srgb_exact(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_linear_exact(srgb: f32) -> f32 {
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

/// Build a reverse LUT (linear f32 -> sRGB u8)
/// Returns (lut, build_time_ns)
fn build_reverse_lut(size: usize) -> (Vec<u8>, u64) {
    let start = Instant::now();
    let mut lut = Vec::with_capacity(size);

    for i in 0..size {
        // Map index to linear value [0.0, 1.0]
        let linear = i as f32 / (size - 1) as f32;
        let srgb = linear_to_srgb_exact(linear);
        let u8_val = (srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        lut.push(u8_val);
    }

    let elapsed = start.elapsed().as_nanos() as u64;
    (lut, elapsed)
}

/// Lookup with linear interpolation for smaller LUTs
#[inline]
fn lut_lookup_interpolated(lut: &[u8], linear: f32, lut_size: usize) -> u8 {
    let scaled = linear * (lut_size - 1) as f32;
    let idx = scaled as usize;
    let frac = scaled - idx as f32;

    if idx >= lut_size - 1 {
        return lut[lut_size - 1];
    }

    let v0 = lut[idx] as f32;
    let v1 = lut[idx + 1] as f32;
    (v0 + frac * (v1 - v0) + 0.5) as u8
}

#[test]
fn test_reverse_lut_costs() {
    println!("\n=== Reverse LUT Startup Costs ===\n");

    let sizes = [256, 512, 1024, 4096, 16384, 65536];

    for &size in &sizes {
        // Run multiple times to get stable timing
        let mut times = Vec::new();
        for _ in 0..10 {
            let (lut, time_ns) = build_reverse_lut(size);
            times.push(time_ns);
            std::hint::black_box(lut);
        }
        times.sort();
        let median_ns = times[times.len() / 2];
        let memory_bytes = size;

        println!(
            "LUT size {:>6}: {:>8} bytes, build time: {:>8} ns ({:.2} us)",
            size,
            memory_bytes,
            median_ns,
            median_ns as f64 / 1000.0
        );
    }

    println!("\nRecommendation: 4096-entry LUT (4 KB) with interpolation");
    println!("- Build time: ~50-100 us (one-time)");
    println!("- Memory: 4 KB per ColorContext");
    println!("- Accuracy: needs verification\n");
}

#[test]
fn test_colorutils_comparison() {
    use colorutils_rs::{srgb_from_linear, srgb_to_linear};

    println!("\n=== colorutils-rs vs Current Implementation ===\n");

    // Test forward direction: sRGB u8 -> linear f32
    println!("Forward (sRGB u8 -> linear f32):");
    let cc = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);

    let mut max_diff_forward = 0.0f32;
    let mut sum_diff_forward = 0.0f64;

    for i in 0..=255u8 {
        let srgb_f32 = i as f32 / 255.0;

        // Current implementation (uses LUT)
        let current = cc.srgb_to_floatspace(i);

        // colorutils-rs
        let colorutils = srgb_to_linear(srgb_f32);

        // Exact reference
        let exact = srgb_to_linear_exact(srgb_f32);

        let diff_current = (current - exact).abs();
        let diff_colorutils = (colorutils - exact).abs();

        max_diff_forward = max_diff_forward.max(diff_current).max(diff_colorutils);
        sum_diff_forward += diff_current as f64 + diff_colorutils as f64;
    }

    println!("  Max diff from exact: {:.10}", max_diff_forward);
    println!("  Mean diff: {:.10}", sum_diff_forward / 512.0);

    // Test reverse direction: linear f32 -> sRGB u8
    println!("\nReverse (linear f32 -> sRGB u8):");

    let mut max_diff_reverse = 0i32;
    let mut colorutils_errors = 0u32;
    let mut current_errors = 0u32;

    // Test all 256 target u8 values by finding their linear equivalents
    for target_u8 in 0..=255u8 {
        let srgb_f32 = target_u8 as f32 / 255.0;
        let linear = srgb_to_linear_exact(srgb_f32);

        // Current implementation
        let current_result = cc.floatspace_to_srgb(linear);

        // colorutils-rs (returns f32, need to convert to u8)
        let colorutils_srgb = srgb_from_linear(linear);
        let colorutils_result = (colorutils_srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;

        let diff_current = (current_result as i32 - target_u8 as i32).abs();
        let diff_colorutils = (colorutils_result as i32 - target_u8 as i32).abs();

        if diff_current > 0 {
            current_errors += 1;
        }
        if diff_colorutils > 0 {
            colorutils_errors += 1;
        }
        max_diff_reverse = max_diff_reverse.max(diff_current).max(diff_colorutils);
    }

    println!("  Max u8 diff: {}", max_diff_reverse);
    println!("  Current impl errors: {}/256 values", current_errors);
    println!("  colorutils-rs errors: {}/256 values", colorutils_errors);
}

#[test]
fn test_brute_force_accuracy() {
    use colorutils_rs::srgb_from_linear;

    println!("\n=== Brute Force Accuracy Test ===\n");

    let cc = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);

    // Test many float values in [0, 1] range
    let test_count = 1_000_000u32;

    let mut current_total_error = 0u64;
    let mut colorutils_total_error = 0u64;
    let mut current_max_error = 0i32;
    let mut colorutils_max_error = 0i32;
    let mut current_off_by_one = 0u32;
    let mut colorutils_off_by_one = 0u32;
    let mut current_off_by_more = 0u32;
    let mut colorutils_off_by_more = 0u32;

    for i in 0..test_count {
        let linear = i as f32 / (test_count - 1) as f32;

        // Calculate exact sRGB value
        let exact_srgb = linear_to_srgb_exact(linear);
        let exact_u8 = (exact_srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;

        // Current implementation
        let current_u8 = cc.floatspace_to_srgb(linear);
        let current_diff = (current_u8 as i32 - exact_u8 as i32).abs();
        current_total_error += current_diff as u64;
        current_max_error = current_max_error.max(current_diff);
        if current_diff == 1 {
            current_off_by_one += 1;
        }
        if current_diff > 1 {
            current_off_by_more += 1;
        }

        // colorutils-rs
        let colorutils_srgb = srgb_from_linear(linear);
        let colorutils_u8 = (colorutils_srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        let colorutils_diff = (colorutils_u8 as i32 - exact_u8 as i32).abs();
        colorutils_total_error += colorutils_diff as u64;
        colorutils_max_error = colorutils_max_error.max(colorutils_diff);
        if colorutils_diff == 1 {
            colorutils_off_by_one += 1;
        }
        if colorutils_diff > 1 {
            colorutils_off_by_more += 1;
        }
    }

    println!("Tested {} linear values in [0.0, 1.0]\n", test_count);

    println!("Current implementation (fastpow):");
    println!("  Max error: {} levels", current_max_error);
    println!("  Mean error: {:.6} levels", current_total_error as f64 / test_count as f64);
    println!(
        "  Off by 1: {} ({:.4}%)",
        current_off_by_one,
        current_off_by_one as f64 / test_count as f64 * 100.0
    );
    println!(
        "  Off by >1: {} ({:.4}%)",
        current_off_by_more,
        current_off_by_more as f64 / test_count as f64 * 100.0
    );

    println!("\ncolorutils-rs (srgb_from_linear):");
    println!("  Max error: {} levels", colorutils_max_error);
    println!("  Mean error: {:.6} levels", colorutils_total_error as f64 / test_count as f64);
    println!(
        "  Off by 1: {} ({:.4}%)",
        colorutils_off_by_one,
        colorutils_off_by_one as f64 / test_count as f64 * 100.0
    );
    println!(
        "  Off by >1: {} ({:.4}%)",
        colorutils_off_by_more,
        colorutils_off_by_more as f64 / test_count as f64 * 100.0
    );
}

#[test]
fn test_lut_interpolation_accuracy() {
    println!("\n=== LUT Interpolation Accuracy Test ===\n");

    let lut_sizes = [256, 512, 1024, 4096, 16384];
    let test_count = 1_000_000u32;

    for &lut_size in &lut_sizes {
        let (lut, _) = build_reverse_lut(lut_size);

        let mut total_error = 0u64;
        let mut max_error = 0i32;
        let mut off_by_one = 0u32;
        let mut off_by_more = 0u32;

        for i in 0..test_count {
            let linear = i as f32 / (test_count - 1) as f32;

            // Exact result
            let exact_srgb = linear_to_srgb_exact(linear);
            let exact_u8 = (exact_srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;

            // LUT with interpolation
            let lut_u8 = lut_lookup_interpolated(&lut, linear, lut_size);

            let diff = (lut_u8 as i32 - exact_u8 as i32).abs();
            total_error += diff as u64;
            max_error = max_error.max(diff);
            if diff == 1 {
                off_by_one += 1;
            }
            if diff > 1 {
                off_by_more += 1;
            }
        }

        println!("LUT size {}:", lut_size);
        println!("  Max error: {} levels", max_error);
        println!("  Mean error: {:.6} levels", total_error as f64 / test_count as f64);
        println!(
            "  Off by 1: {} ({:.4}%)",
            off_by_one,
            off_by_one as f64 / test_count as f64 * 100.0
        );
        println!(
            "  Off by >1: {} ({:.4}%)",
            off_by_more,
            off_by_more as f64 / test_count as f64 * 100.0
        );
        println!();
    }
}

#[test]
fn test_performance_comparison() {
    use colorutils_rs::srgb_from_linear;

    println!("\n=== Performance Comparison ===\n");

    let cc = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);
    let (lut_4k, _) = build_reverse_lut(4096);

    let test_count = 1_000_000usize;
    let test_values: Vec<f32> =
        (0..test_count).map(|i| i as f32 / (test_count - 1) as f32).collect();

    // Warmup
    for &v in &test_values[..1000] {
        std::hint::black_box(cc.floatspace_to_srgb(v));
        std::hint::black_box(srgb_from_linear(v));
        std::hint::black_box(lut_lookup_interpolated(&lut_4k, v, 4096));
    }

    // Current implementation (fastpow)
    let start = Instant::now();
    let mut sum = 0u32;
    for &v in &test_values {
        sum += cc.floatspace_to_srgb(v) as u32;
    }
    let current_time = start.elapsed();
    std::hint::black_box(sum);

    // colorutils-rs
    let start = Instant::now();
    let mut sum = 0u32;
    for &v in &test_values {
        let srgb = srgb_from_linear(v);
        sum += (srgb * 255.0 + 0.5) as u32;
    }
    let colorutils_time = start.elapsed();
    std::hint::black_box(sum);

    // LUT with interpolation
    let start = Instant::now();
    let mut sum = 0u32;
    for &v in &test_values {
        sum += lut_lookup_interpolated(&lut_4k, v, 4096) as u32;
    }
    let lut_time = start.elapsed();
    std::hint::black_box(sum);

    println!("{} conversions:\n", test_count);
    println!(
        "Current (fastpow):     {:>8.2} ms  ({:.1} M/s)",
        current_time.as_secs_f64() * 1000.0,
        test_count as f64 / current_time.as_secs_f64() / 1_000_000.0
    );
    println!(
        "colorutils-rs:         {:>8.2} ms  ({:.1} M/s)",
        colorutils_time.as_secs_f64() * 1000.0,
        test_count as f64 / colorutils_time.as_secs_f64() / 1_000_000.0
    );
    println!(
        "LUT 4096 + interp:     {:>8.2} ms  ({:.1} M/s)",
        lut_time.as_secs_f64() * 1000.0,
        test_count as f64 / lut_time.as_secs_f64() / 1_000_000.0
    );

    println!("\nSpeedup vs current:");
    println!("  colorutils-rs: {:.2}x", current_time.as_secs_f64() / colorutils_time.as_secs_f64());
    println!("  LUT 4096:      {:.2}x", current_time.as_secs_f64() / lut_time.as_secs_f64());
}

/// Verify embedded LUT matches runtime computation
/// This ensures the static LUT wasn't corrupted and matches our exact calculation
#[test]
fn test_linear_to_srgb_lut_correctness() {
    println!("\n=== LUT Correctness Verification ===\n");

    let mut mismatches = 0;
    for i in 0..16384 {
        // Use f64 for high precision reference calculation
        let linear = i as f64 / 16383.0;
        let srgb = if linear <= 0.0031308 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        let expected = (srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;

        let actual = LINEAR_TO_SRGB_LUT[i];
        if actual != expected {
            if mismatches < 10 {
                println!("Mismatch at index {}: expected {}, got {}", i, expected, actual);
            }
            mismatches += 1;
        }
    }

    println!("Checked 16384 LUT entries");
    println!("Mismatches: {}", mismatches);

    assert_eq!(mismatches, 0, "LUT has {} mismatches against reference calculation", mismatches);
    println!("✓ All LUT entries match reference calculation");
}

/// Test that linear_to_srgb_lut function works correctly
#[test]
fn test_linear_to_srgb_lut_function() {
    println!("\n=== LUT Function Test ===\n");

    // Test boundary conditions
    assert_eq!(linear_to_srgb_lut(0.0), 0, "linear 0.0 should map to sRGB 0");
    assert_eq!(linear_to_srgb_lut(1.0), 255, "linear 1.0 should map to sRGB 255");

    // Test clamping for out-of-range inputs
    assert_eq!(linear_to_srgb_lut(-0.1), 0, "negative values should clamp to 0");
    assert_eq!(linear_to_srgb_lut(1.5), 255, "values > 1.0 should clamp to 255");

    // Test mid-range value (linear 0.5 -> sRGB ~186)
    let mid = linear_to_srgb_lut(0.5);
    assert!(mid >= 185 && mid <= 188, "linear 0.5 should be around 186, got {}", mid);

    // Verify against exact calculation for several values
    let test_values = [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0];
    for &linear in &test_values {
        let exact_srgb = linear_to_srgb_exact(linear);
        let exact_u8 = (exact_srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        let lut_u8 = linear_to_srgb_lut(linear);
        let diff = (lut_u8 as i32 - exact_u8 as i32).abs();
        assert!(
            diff <= 1,
            "linear {} -> LUT {} vs exact {}, diff {}",
            linear,
            lut_u8,
            exact_u8,
            diff
        );
    }

    println!("✓ LUT function works correctly");
}

/// Compare LUT performance against fastpow
#[test]
fn test_lut_vs_fastpow_performance() {
    println!("\n=== Embedded LUT vs Fastpow Performance ===\n");

    let cc = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);
    let test_count = 1_000_000usize;
    let test_values: Vec<f32> =
        (0..test_count).map(|i| i as f32 / (test_count - 1) as f32).collect();

    // Warmup
    for &v in &test_values[..1000] {
        std::hint::black_box(cc.floatspace_to_srgb(v));
        std::hint::black_box(linear_to_srgb_lut(v));
    }

    // Fastpow (current)
    let start = Instant::now();
    let mut sum = 0u32;
    for &v in &test_values {
        sum += cc.floatspace_to_srgb(v) as u32;
    }
    let fastpow_time = start.elapsed();
    std::hint::black_box(sum);

    // Embedded LUT
    let start = Instant::now();
    let mut sum = 0u32;
    for &v in &test_values {
        sum += linear_to_srgb_lut(v) as u32;
    }
    let lut_time = start.elapsed();
    std::hint::black_box(sum);

    println!("{} conversions:\n", test_count);
    println!(
        "Fastpow:       {:>8.2} ms  ({:.1} M/s)",
        fastpow_time.as_secs_f64() * 1000.0,
        test_count as f64 / fastpow_time.as_secs_f64() / 1_000_000.0
    );
    println!(
        "Embedded LUT:  {:>8.2} ms  ({:.1} M/s)",
        lut_time.as_secs_f64() * 1000.0,
        test_count as f64 / lut_time.as_secs_f64() / 1_000_000.0
    );
    println!("\nSpeedup: {:.2}x", fastpow_time.as_secs_f64() / lut_time.as_secs_f64());
}

/// Critical test: verify sRGB u8 -> linear f32 -> sRGB u8 roundtrips exactly
///
/// This test ensures that converting from sRGB to linear and back produces
/// the original value. Any implementation that fails this test will cause
/// visible color shifts in image processing pipelines.
#[test]
fn test_srgb_linear_roundtrip() {
    println!("\n=== sRGB ↔ Linear Roundtrip Test ===\n");

    let cc = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);

    let mut errors = 0u32;
    let mut max_error = 0i32;
    let mut error_values = Vec::new();

    for original in 0u8..=255u8 {
        // Forward: sRGB u8 -> linear f32
        let linear = cc.srgb_to_floatspace(original);

        // Reverse: linear f32 -> sRGB u8
        let roundtrip = cc.floatspace_to_srgb(linear);

        let diff = (roundtrip as i32 - original as i32).abs();
        if diff > 0 {
            errors += 1;
            max_error = max_error.max(diff);
            if error_values.len() < 10 {
                error_values.push((original, roundtrip, diff));
            }
        }
    }

    println!("Tested all 256 sRGB values");
    println!("Roundtrip errors: {}/256", errors);
    println!("Max error: {} levels", max_error);

    if !error_values.is_empty() {
        println!("\nFirst {} errors:", error_values.len());
        for (orig, result, diff) in &error_values {
            println!("  {} -> {} (diff {})", orig, result, diff);
        }
    }

    // The LUT implementation should have ZERO roundtrip errors
    assert_eq!(
        errors, 0,
        "sRGB->linear->sRGB roundtrip must be lossless! {} values failed, max error {}",
        errors, max_error
    );

    println!("\n✓ All 256 values roundtrip perfectly");
}
