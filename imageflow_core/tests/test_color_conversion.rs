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

// ========== Fastpow reimplementation for testing ==========
// Replicated from graphics/math.rs (pub(crate), not accessible from integration tests)

fn fastpow2(p: f32) -> f32 {
    let offset: f32 = if p < 0.0 { 1.0 } else { 0.0 };
    let clipp: f32 = if p < -126.0 { -126.0 } else { p };
    let _w: i32 = clipp as i32;
    let z: f32 = clipp - _w as f32 + offset;
    f32::from_bits(
        ((1_i32 << 23) as f32
            * (clipp + 121.274_055_f32 + 27.728_024_f32 / (4.842_525_5_f32 - z)
                - 1.490_129_1_f32 * z)) as u32,
    )
}

fn fastlog2(x: f32) -> f32 {
    let vx_i = x.to_bits();
    let mx_f = f32::from_bits(vx_i & 0x7fffff_u32 | 0x3f000000_u32);
    let mut y: f32 = vx_i as f32;
    y *= 1.192_092_9e-7_f32;
    y - 124.225_52_f32 - 1.498_030_3_f32 * mx_f - 1.725_88_f32 / (0.352_088_72_f32 + mx_f)
}

fn fastpow(x: f32, p: f32) -> f32 {
    fastpow2(p * fastlog2(x))
}

fn linear_to_srgb_fastpow(clr: f32) -> u8 {
    let v = if clr <= 0.0031308_f32 {
        12.92_f32 * clr * 255.0_f32
    } else {
        1.055_f32 * 255.0_f32 * fastpow(clr, 0.41666666_f32) - 14.025_f32
    };
    // Replicate uchar_clamp_ff
    let mut result = (v as f64 + 0.5) as i16 as u16;
    if result as i32 > 255 {
        result = if v < 0.0 { 0 } else { 255 } as u16;
    }
    result as u8
}

/// Brute-force comparison of fastpow vs LUT vs exact sRGB for all inputs.
/// This tests the raw conversion delta, not lossy encoding amplification.
#[test]
fn test_fastpow_vs_lut_brute_force() {
    println!("\n=== Fastpow vs LUT Brute Force Comparison ===\n");

    let test_count = 10_000_000u32;
    let mut max_delta_fastpow_lut: i32 = 0;
    let mut max_delta_fastpow_exact: i32 = 0;
    let mut max_delta_lut_exact: i32 = 0;
    let mut fastpow_lut_disagree = 0u64;
    let mut fastpow_wrong_vs_exact = 0u64;
    let mut lut_wrong_vs_exact = 0u64;
    let mut fastpow_lut_delta_sum = 0u64;

    // Track worst cases
    let mut worst_fastpow_lut: Vec<(f32, u8, u8, u8, i32)> = Vec::new(); // (linear, fastpow, lut, exact, delta)

    for i in 0..=test_count {
        let linear = i as f32 / test_count as f32;

        let fp = linear_to_srgb_fastpow(linear);
        let lut = linear_to_srgb_lut(linear);

        // Exact reference (f64 precision)
        let exact_f = if (linear as f64) <= 0.0031308 {
            12.92 * linear as f64
        } else {
            1.055 * (linear as f64).powf(1.0 / 2.4) - 0.055
        };
        let exact = (exact_f * 255.0 + 0.5).clamp(0.0, 255.0) as u8;

        let delta_fp_lut = (fp as i32 - lut as i32).abs();
        let delta_fp_exact = (fp as i32 - exact as i32).abs();
        let delta_lut_exact = (lut as i32 - exact as i32).abs();

        fastpow_lut_delta_sum += delta_fp_lut as u64;

        if delta_fp_lut > max_delta_fastpow_lut {
            max_delta_fastpow_lut = delta_fp_lut;
            worst_fastpow_lut.clear();
        }
        if delta_fp_lut == max_delta_fastpow_lut && delta_fp_lut > 0 && worst_fastpow_lut.len() < 20
        {
            worst_fastpow_lut.push((linear, fp, lut, exact, fp as i32 - lut as i32));
        }

        max_delta_fastpow_exact = max_delta_fastpow_exact.max(delta_fp_exact);
        max_delta_lut_exact = max_delta_lut_exact.max(delta_lut_exact);

        if delta_fp_lut > 0 {
            fastpow_lut_disagree += 1;
        }
        if delta_fp_exact > 0 {
            fastpow_wrong_vs_exact += 1;
        }
        if delta_lut_exact > 0 {
            lut_wrong_vs_exact += 1;
        }
    }

    let total = test_count as u64 + 1;
    println!("Tested {} linear values in [0.0, 1.0]\n", total);

    println!("Max delta (fastpow vs LUT):   {}", max_delta_fastpow_lut);
    println!("Max delta (fastpow vs exact):  {}", max_delta_fastpow_exact);
    println!("Max delta (LUT vs exact):      {}", max_delta_lut_exact);
    println!();
    println!(
        "Fastpow disagrees with LUT:    {} ({:.4}%)",
        fastpow_lut_disagree,
        fastpow_lut_disagree as f64 / total as f64 * 100.0
    );
    println!(
        "Fastpow wrong vs exact:        {} ({:.4}%)",
        fastpow_wrong_vs_exact,
        fastpow_wrong_vs_exact as f64 / total as f64 * 100.0
    );
    println!(
        "LUT wrong vs exact:            {} ({:.4}%)",
        lut_wrong_vs_exact,
        lut_wrong_vs_exact as f64 / total as f64 * 100.0
    );
    println!("Mean |fastpow - LUT|:          {:.6}", fastpow_lut_delta_sum as f64 / total as f64);

    if !worst_fastpow_lut.is_empty() {
        println!("\nWorst fastpow vs LUT disagreements (max delta = {}):", max_delta_fastpow_lut);
        println!(
            "  {:>12}  {:>8}  {:>8}  {:>8}  {:>8}",
            "linear", "fastpow", "LUT", "exact", "fp-lut"
        );
        for (linear, fp, lut, exact, delta) in &worst_fastpow_lut {
            println!("  {:>12.10}  {:>8}  {:>8}  {:>8}  {:>+8}", linear, fp, lut, exact, delta);
        }
    }

    // Distribution of deltas
    println!("\nDelta distribution (fastpow vs exact):");
    let mut fp_delta_counts = [0u64; 4];
    let mut lut_delta_counts = [0u64; 4];
    for i in 0..=test_count {
        let linear = i as f32 / test_count as f32;
        let fp = linear_to_srgb_fastpow(linear);
        let lut = linear_to_srgb_lut(linear);
        let exact_f = if (linear as f64) <= 0.0031308 {
            12.92 * linear as f64
        } else {
            1.055 * (linear as f64).powf(1.0 / 2.4) - 0.055
        };
        let exact = (exact_f * 255.0 + 0.5).clamp(0.0, 255.0) as u8;

        let d_fp = (fp as i32 - exact as i32).abs().min(3) as usize;
        let d_lut = (lut as i32 - exact as i32).abs().min(3) as usize;
        fp_delta_counts[d_fp] += 1;
        lut_delta_counts[d_lut] += 1;
    }
    println!("  delta=0: fastpow={}, LUT={}", fp_delta_counts[0], lut_delta_counts[0]);
    println!("  delta=1: fastpow={}, LUT={}", fp_delta_counts[1], lut_delta_counts[1]);
    println!("  delta=2: fastpow={}, LUT={}", fp_delta_counts[2], lut_delta_counts[2]);
    println!("  delta≥3: fastpow={}, LUT={}", fp_delta_counts[3], lut_delta_counts[3]);

    // Now check: when fastpow and LUT disagree, who is closer to exact?
    println!("\nWhen fastpow and LUT disagree, who is closer to exact?");
    let mut fp_closer = 0u64;
    let mut lut_closer = 0u64;
    let mut both_same_dist = 0u64;
    for i in 0..=test_count {
        let linear = i as f32 / test_count as f32;
        let fp = linear_to_srgb_fastpow(linear);
        let lut = linear_to_srgb_lut(linear);
        if fp != lut {
            let exact_f = if (linear as f64) <= 0.0031308 {
                12.92 * linear as f64
            } else {
                1.055 * (linear as f64).powf(1.0 / 2.4) - 0.055
            };
            let exact = (exact_f * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            let d_fp = (fp as i32 - exact as i32).abs();
            let d_lut = (lut as i32 - exact as i32).abs();
            if d_fp < d_lut {
                fp_closer += 1;
            } else if d_lut < d_fp {
                lut_closer += 1;
            } else {
                both_same_dist += 1;
            }
        }
    }
    println!("  Fastpow closer: {}", fp_closer);
    println!("  LUT closer:     {}", lut_closer);
    println!("  Same distance:  {}", both_same_dist);

    assert!(
        max_delta_fastpow_lut <= 1,
        "fastpow vs LUT should differ by at most 1, got {}",
        max_delta_fastpow_lut
    );
    assert!(
        max_delta_lut_exact <= 1,
        "LUT vs exact should differ by at most 1, got {}",
        max_delta_lut_exact
    );
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

/// Build a minimal valid PNG file with RGBA pixels.
/// Uses flate2 for deflate compression of the raw scanlines.
fn build_rgba_png(w: u32, h: u32, pixels: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    assert_eq!(pixels.len(), (w * h * 4) as usize);

    let mut buf = Vec::new();
    // PNG signature
    buf.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // Helper to write a PNG chunk
    fn write_chunk(buf: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
        buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
        buf.extend_from_slice(chunk_type);
        buf.extend_from_slice(data);
        let mut crc_data = Vec::with_capacity(4 + data.len());
        crc_data.extend_from_slice(chunk_type);
        crc_data.extend_from_slice(data);
        let crc = png_crc32(&crc_data);
        buf.extend_from_slice(&crc.to_be_bytes());
    }

    fn png_crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc ^ 0xFFFFFFFF
    }

    // IHDR: width, height, bit_depth=8, color_type=6 (RGBA), compression=0, filter=0, interlace=0
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_chunk(&mut buf, b"IHDR", &ihdr);

    // IDAT: filtered scanlines (filter byte 0 = None for each row)
    let mut raw_scanlines = Vec::new();
    for y in 0..h as usize {
        raw_scanlines.push(0u8); // filter byte: None
        raw_scanlines.extend_from_slice(&pixels[y * w as usize * 4..(y + 1) * w as usize * 4]);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw_scanlines).unwrap();
    let compressed = encoder.finish().unwrap();
    write_chunk(&mut buf, b"IDAT", &compressed);

    // IEND
    write_chunk(&mut buf, b"IEND", &[]);

    buf
}

/// Test that matte compositing during scaling produces correct RGB values.
///
/// This exercises the blend_matte() path in scaling.rs by:
/// 1. Creating a PNG with semi-transparent red pixels
/// 2. Scaling it down with a white background color (triggers BlendWithMatte)
/// 3. Verifying the output RGB values match the correct compositing formula
///
/// The bug: blend_matte() divides by final_alpha (producing straight alpha),
/// then demultiply_alpha() divides by alpha again, making semi-transparent
/// pixels too bright. The fix skips demultiply after blend_matte.
#[test]
fn test_matte_compositing_no_double_division() {
    use imageflow_core::graphics::color::{ColorContext, WorkingFloatspace};
    use imageflow_core::Context;

    // Create a 10x10 RGBA image: solid red with alpha=128
    let w = 10u32;
    let h = 10u32;
    let alpha = 128u8;
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        pixels.extend_from_slice(&[255, 0, 0, alpha]); // RGBA: red, alpha=128
    }
    let png_bytes = build_rgba_png(w, h, &pixels);

    // Resize to 5x5 with white matte — this triggers blend_matte in scaling.rs
    let steps = vec![
        imageflow_types::Node::Decode { io_id: 0, commands: None },
        imageflow_types::Node::Resample2D {
            w: 5,
            h: 5,
            hints: Some(imageflow_types::ResampleHints {
                background_color: Some(imageflow_types::Color::Srgb(
                    imageflow_types::ColorSrgb::Hex("FFFFFFFF".to_owned()),
                )),
                ..imageflow_types::ResampleHints::new()
            }),
        },
        imageflow_types::Node::Encode {
            io_id: 1,
            preset: imageflow_types::EncoderPreset::Lodepng { maximum_deflate: None },
        },
    ];

    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = imageflow_types::Execute001 {
        graph_recording: None,
        security: None,
        framewise: imageflow_types::Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();

    // Decode the output PNG and check pixel values
    let mut ctx2 = Context::create().unwrap();
    ctx2.add_input_vector(0, output_bytes.clone()).unwrap();
    ctx2.add_output_buffer(1).unwrap();
    let decode_steps = vec![
        imageflow_types::Node::Decode { io_id: 0, commands: None },
        imageflow_types::Node::Encode {
            io_id: 1,
            preset: imageflow_types::EncoderPreset::Lodepng { maximum_deflate: None },
        },
    ];
    let execute2 = imageflow_types::Execute001 {
        graph_recording: None,
        security: None,
        framewise: imageflow_types::Framewise::Steps(decode_steps),
    };
    ctx2.execute_1(execute2).unwrap();

    // Use lodepng to decode raw pixels from the output
    let result = lodepng::decode32(output_bytes.as_slice()).unwrap();
    let out_w = result.width;
    let out_h = result.height;
    let out_pixels = result.buffer;

    println!("\n=== Matte Compositing Test ({}x{} -> {}x{}) ===\n", w, h, out_w, out_h);

    // Compute expected value:
    // Input: premultiplied red at alpha=128/255 ≈ 0.502
    // sRGB red=255 -> linear ≈ 1.0
    // In premultiplied: pixel_R_linear = alpha_f * 1.0
    // Matte: white -> linear = 1.0
    // blend_matte formula (on premultiplied data):
    //   pixel_a = alpha_f (from premul conversion)
    //   a = (1 - pixel_a) * matte_a = (1 - 0.502) * 1.0 = 0.498
    //   final_alpha = pixel_a + a = 0.502 + 0.498 = 1.0
    //   result_R = (premul_R + linear_matte_R * a) / final_alpha
    //            = (0.502 * 1.0 + 1.0 * 0.498) / 1.0 = 1.0  ... for opaque matte
    //
    // But that's only true if alpha_f is exactly 128/255. The key test is that
    // the RGB values are correct, not washed out by double-division.
    //
    // With a white matte and any non-zero alpha, the result for a pure red pixel should be:
    //   result_R_linear = (linear_red * alpha_f + 1.0 * (1-alpha_f)) / 1.0
    //                   = alpha_f * linear_red + (1 - alpha_f)
    //   result_G_linear = (0 * alpha_f + 1.0 * (1-alpha_f)) / 1.0 = (1 - alpha_f)
    //   result_B_linear = same as G
    //
    // alpha_f = 128/255 ≈ 0.50196
    // result_R_linear ≈ 0.50196 + 0.49804 = 1.0 → sRGB 255
    // result_G_linear ≈ 0.49804 → sRGB ~186
    // result_B_linear ≈ 0.49804 → sRGB ~186
    //
    // With the double-division bug, the values would be much brighter/whiter.

    let cc = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);
    let alpha_f = alpha as f32 / 255.0;
    // Red channel: linear_red=1.0 (sRGB 255 -> linear 1.0)
    let linear_red = cc.srgb_to_floatspace(255);
    let expected_r_linear = alpha_f * linear_red + (1.0 - alpha_f) * 1.0; // matte is white=1.0
    let expected_g_linear = alpha_f * 0.0 + (1.0 - alpha_f) * 1.0;
    let expected_b_linear = expected_g_linear;

    let expected_r = cc.floatspace_to_srgb(expected_r_linear);
    let expected_g = cc.floatspace_to_srgb(expected_g_linear);
    let expected_b = cc.floatspace_to_srgb(expected_b_linear);

    println!(
        "Expected (from correct compositing): R={}, G={}, B={}",
        expected_r, expected_g, expected_b
    );

    let mut max_r_diff = 0i32;
    let mut max_g_diff = 0i32;
    let mut max_b_diff = 0i32;

    for y in 0..out_h {
        for x in 0..out_w {
            let px = &out_pixels[y * out_w + x];
            let r_diff = (px.r as i32 - expected_r as i32).abs();
            let g_diff = (px.g as i32 - expected_g as i32).abs();
            let b_diff = (px.b as i32 - expected_b as i32).abs();
            max_r_diff = max_r_diff.max(r_diff);
            max_g_diff = max_g_diff.max(g_diff);
            max_b_diff = max_b_diff.max(b_diff);

            if r_diff > 2 || g_diff > 2 || b_diff > 2 {
                println!(
                    "  Pixel ({},{}): R={} G={} B={} A={} (expected R={} G={} B={}, diff R={} G={} B={})",
                    x, y, px.r, px.g, px.b, px.a,
                    expected_r, expected_g, expected_b,
                    r_diff, g_diff, b_diff
                );
            }
        }
    }

    println!("Max diffs: R={}, G={}, B={}", max_r_diff, max_g_diff, max_b_diff);

    // Allow ±2 tolerance for rounding through the pipeline
    // (sRGB->linear->premul->scale->blend_matte->sRGB has multiple rounding steps)
    assert!(
        max_r_diff <= 2 && max_g_diff <= 2 && max_b_diff <= 2,
        "Matte compositing produced wrong RGB values! Max diffs: R={}, G={}, B={}. \
         This likely indicates double-division by alpha in blend_matte + demultiply_alpha.",
        max_r_diff,
        max_g_diff,
        max_b_diff
    );

    println!("\n✓ Matte compositing produces correct RGB values");
}

/// Test that fully-transparent pixels become the matte color after compositing.
///
/// When alpha=0, the pixel is fully transparent — the matte should show through
/// completely. The current blend_matte() has `if alpha > 0` which skips these
/// pixels entirely, leaving them as black/transparent instead of the matte color.
#[test]
fn test_matte_compositing_fully_transparent_pixels() {
    use imageflow_core::Context;

    // Create a 4x4 RGBA image: fully transparent (alpha=0)
    let w = 4u32;
    let h = 4u32;
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        pixels.extend_from_slice(&[0, 0, 0, 0]); // RGBA: transparent black
    }
    let png_bytes = build_rgba_png(w, h, &pixels);

    // Resize to 2x2 with RED matte — transparent pixels should become red
    let steps = vec![
        imageflow_types::Node::Decode { io_id: 0, commands: None },
        imageflow_types::Node::Resample2D {
            w: 2,
            h: 2,
            hints: Some(imageflow_types::ResampleHints {
                background_color: Some(imageflow_types::Color::Srgb(
                    imageflow_types::ColorSrgb::Hex("FF0000FF".to_owned()),
                )),
                ..imageflow_types::ResampleHints::new()
            }),
        },
        imageflow_types::Node::Encode {
            io_id: 1,
            preset: imageflow_types::EncoderPreset::Lodepng { maximum_deflate: None },
        },
    ];

    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = imageflow_types::Execute001 {
        graph_recording: None,
        security: None,
        framewise: imageflow_types::Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();

    let result = lodepng::decode32(output_bytes.as_slice()).unwrap();
    let out_w = result.width;
    let out_h = result.height;
    let out_pixels = result.buffer;

    println!("\n=== Fully Transparent Matte Test ({}x{} -> {}x{}) ===\n", w, h, out_w, out_h);

    // With a fully-transparent input and an opaque red matte:
    // The result should be opaque red (255, 0, 0, 255)
    let mut max_r_diff = 0i32;
    let mut max_g_diff = 0i32;
    let mut max_b_diff = 0i32;
    let mut max_a_diff = 0i32;

    for y in 0..out_h {
        for x in 0..out_w {
            let px = &out_pixels[y * out_w + x];
            let r_diff = (px.r as i32 - 255).abs();
            let g_diff = (px.g as i32 - 0).abs();
            let b_diff = (px.b as i32 - 0).abs();
            let a_diff = (px.a as i32 - 255).abs();
            max_r_diff = max_r_diff.max(r_diff);
            max_g_diff = max_g_diff.max(g_diff);
            max_b_diff = max_b_diff.max(b_diff);
            max_a_diff = max_a_diff.max(a_diff);

            println!(
                "  Pixel ({},{}): R={} G={} B={} A={} (expected R=255 G=0 B=0 A=255, diff R={} G={} B={} A={})",
                x, y, px.r, px.g, px.b, px.a, r_diff, g_diff, b_diff, a_diff
            );
        }
    }

    println!("Max diffs: R={}, G={}, B={}, A={}", max_r_diff, max_g_diff, max_b_diff, max_a_diff);

    assert!(
        max_r_diff <= 2 && max_g_diff <= 2 && max_b_diff <= 2 && max_a_diff <= 2,
        "Fully-transparent pixels should become the matte color! \
         Max diffs: R={}, G={}, B={}, A={}. \
         If alpha/RGB are near 0, blend_matte is skipping transparent pixels.",
        max_r_diff,
        max_g_diff,
        max_b_diff,
        max_a_diff
    );

    println!("\n✓ Fully transparent pixels correctly become the matte color");
}

/// Test matte compositing with mixed alpha values in a single image.
///
/// Creates a 40x10 image with 4 vertical bands at alpha = [0, 85, 170, 255],
/// all with green foreground, scales to 20x5 with a blue matte, and verifies
/// that the center of each band matches the expected blend result.
#[test]
fn test_matte_compositing_mixed_alpha() {
    use imageflow_core::graphics::color::{ColorContext, WorkingFloatspace};
    use imageflow_core::Context;

    // 40x10 image with 4 bands of 10px each, green pixels at varying alpha
    let w = 40u32;
    let h = 10u32;
    let alphas: [u8; 4] = [0, 85, 170, 255];
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..h {
        for x in 0..w {
            let band = (x / 10) as usize;
            let a = alphas[band];
            pixels.extend_from_slice(&[0, 255, 0, a]); // RGBA: green at varying alpha
        }
    }
    let png_bytes = build_rgba_png(w, h, &pixels);

    // Scale to 20x5 with blue matte — triggers blend_matte in scaling.rs
    let steps = vec![
        imageflow_types::Node::Decode { io_id: 0, commands: None },
        imageflow_types::Node::Resample2D {
            w: 20,
            h: 5,
            hints: Some(imageflow_types::ResampleHints {
                background_color: Some(imageflow_types::Color::Srgb(
                    imageflow_types::ColorSrgb::Hex("0000FFFF".to_owned()),
                )),
                ..imageflow_types::ResampleHints::new()
            }),
        },
        imageflow_types::Node::Encode {
            io_id: 1,
            preset: imageflow_types::EncoderPreset::Lodepng { maximum_deflate: None },
        },
    ];

    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let execute = imageflow_types::Execute001 {
        graph_recording: None,
        security: None,
        framewise: imageflow_types::Framewise::Steps(steps),
    };
    ctx.execute_1(execute).unwrap();
    let output_bytes = ctx.get_output_buffer_slice(1).unwrap().to_vec();

    let result = lodepng::decode32(output_bytes.as_slice()).unwrap();
    let out_w = result.width;
    let out_pixels = result.buffer;

    println!("\n=== Mixed Alpha Matte Test ({}x{} -> {}x{}) ===\n", w, h, out_w, result.height);

    let cc = ColorContext::new(WorkingFloatspace::LinearRGB, 0.0);
    let matte_b_linear = cc.srgb_to_floatspace(255u8); // blue matte: B=255
    let fg_g_linear = cc.srgb_to_floatspace(255u8); // green fg: G=255

    let mut any_failed = false;

    // Sample the center pixel of each band on the middle row
    let mid_row = result.height / 2;
    for (band, &alpha) in alphas.iter().enumerate() {
        let center_x = band * 5 + 2; // center of each 5px band in the 20px output
        let px = &out_pixels[mid_row * out_w + center_x];
        let alpha_f = alpha as f32 / 255.0;

        // Expected: straight-alpha composite over opaque matte
        // result = fg * alpha_f + matte * (1 - alpha_f), final_alpha = 1.0
        let exp_r = cc.floatspace_to_srgb(0.0); // both fg and matte have R=0
        let exp_g = cc.floatspace_to_srgb(fg_g_linear * alpha_f);
        let exp_b = cc.floatspace_to_srgb(matte_b_linear * (1.0 - alpha_f));

        let r_diff = (px.r as i32 - exp_r as i32).abs();
        let g_diff = (px.g as i32 - exp_g as i32).abs();
        let b_diff = (px.b as i32 - exp_b as i32).abs();

        let ok = r_diff <= 2 && g_diff <= 2 && b_diff <= 2 && px.a >= 253;
        let marker = if ok { "ok" } else { "FAIL" };

        println!(
            "  [{}] alpha={:3}: got R={:3} G={:3} B={:3} A={:3}, expected R={:3} G={:3} B={:3} A=255 (diff R={} G={} B={})",
            marker, alpha, px.r, px.g, px.b, px.a, exp_r, exp_g, exp_b, r_diff, g_diff, b_diff
        );

        if !ok {
            any_failed = true;
        }
    }

    assert!(
        !any_failed,
        "Matte compositing produced wrong values for some alpha levels. \
         If alpha=0 pixels are black/transparent, blend_matte is skipping them."
    );

    println!("\n✓ All alpha levels composited correctly over matte");
}
