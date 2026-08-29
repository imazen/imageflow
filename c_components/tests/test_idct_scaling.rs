// Force linkage of the native C library compiled by this crate's build.rs
extern crate imageflow_c_components;

/// A non-degenerate 8x8 source block.
///
/// **The shape of this input is load-bearing; do not "simplify" it.** These kernels
/// are separable weighted averages: the vertical pass reduces each input column to a
/// total, and the horizontal pass multiplies those totals by a per-column weight
/// vector. Any input that drives a column total to zero makes that column's weight
/// unobservable — the kernel could hold an arbitrary value there and every assertion
/// would still pass.
///
/// That is not hypothetical. `test_spatial_scaling` used to use only a checkerboard,
/// `input[x] = if x % 2 == 0 { 0 } else { 255 }`. Because the index is `row * 8 + col`
/// and `row * 8` is always even, index parity *is* column parity: every even column
/// was 0, so all four even-indexed horizontal weights were multiplied by zero.
/// Measured 2026-08-29: changing `weights_for_col_0[0]` from 47 to 4 in
/// `lib/codecs_jpeg_idct_fast.c` left that test passing with the same output, 188.
///
/// So this block is a two-axis gradient (distinct offset per row and per column, no
/// constant row or column) plus a high-frequency term on alternating cells, and is
/// asymmetric in both axes so the mirror-equivariance test below has something to
/// detect. Max value 3 + 13*7 + 17*7 + 24 = 237, so nothing wraps.
fn base_block() -> [u8; 64] {
    let mut b = [0u8; 64];
    for r in 0..8usize {
        for c in 0..8usize {
            b[r * 8 + c] = (3 + 13 * r + 17 * c + if (r + c) % 2 == 1 { 24 } else { 0 }) as u8;
        }
    }
    b
}

/// Run one kernel over a whole 8x8 block, returning the 8x8 output buffer.
/// Cells outside the kernel's NxN region keep `UNWRITTEN`.
fn run_kernel(func: blockscale_fn, input: &[u8; 64]) -> [u8; 64] {
    let mut input = *input;
    let mut output = [UNWRITTEN; 64];
    let mut rows: [*mut u8; 8] = unsafe {
        [
            output.as_mut_ptr(),
            output.as_mut_ptr().add(8),
            output.as_mut_ptr().add(16),
            output.as_mut_ptr().add(24),
            output.as_mut_ptr().add(32),
            output.as_mut_ptr().add(40),
            output.as_mut_ptr().add(48),
            output.as_mut_ptr().add(56),
        ]
    };
    unsafe { func(input.as_mut_ptr(), rows.as_mut_ptr(), 0) };
    output
}

/// Distinct from any plausible output, so an unwritten cell is detectable.
const UNWRITTEN: u8 = 0xAB;

fn mirror_h(b: &[u8; 64]) -> [u8; 64] {
    let mut o = [0u8; 64];
    for r in 0..8usize {
        for c in 0..8usize {
            o[r * 8 + c] = b[r * 8 + (7 - c)];
        }
    }
    o
}

fn mirror_v(b: &[u8; 64]) -> [u8; 64] {
    let mut o = [0u8; 64];
    for r in 0..8usize {
        for c in 0..8usize {
            o[r * 8 + c] = b[(7 - r) * 8 + c];
        }
    }
    o
}

/// Pins one exact end-to-end value through `flow_scale_spatial_srgb_1x1`, including
/// both sRGB lookup tables, and checks the range invariant on a non-degenerate block.
///
/// The checkerboard case is kept for the exact value — no property test replaces a
/// known-good constant, and it would catch a corrupted `lut_srgb_to_linear` /
/// `lut_linear_to_srgb`. But it cannot see half of the horizontal weight vector (see
/// `base_block`), so on its own it is not evidence the kernel is right. The weights
/// are covered by `test_every_row_and_column_affects_output` and
/// `test_mirror_equivariance`.
#[test]
fn test_spatial_scaling() {
    let mut input: [u8; 64] = [0; 64];
    for (x, v) in input.iter_mut().enumerate() {
        *v = if x % 2 == 0 { 0 } else { 255 };
    }

    let mut output: [u8; 1] = [0; 1];
    let mut output_rows: [*mut u8; 1] = [&mut output[0]];
    let output_col = 0;

    unsafe { flow_scale_spatial_srgb_1x1(&mut input[0], &mut output_rows[0], output_col) }

    assert_eq!(output[0], 188);

    // A weighted average with non-negative weights cannot leave the input's range,
    // and the sRGB transfer function is monotonic, so the bound survives the
    // round trip through linear light.
    let block = base_block();
    let got = run_kernel(flow_scale_spatial_srgb_1x1, &block)[0];
    let (lo, hi) = (*block.iter().min().unwrap(), *block.iter().max().unwrap());
    assert!(
        (lo..=hi).contains(&got),
        "1x1 downscale gave {got}, outside the input range {lo}..={hi}; a weighted \
         average of the block cannot fall outside it"
    );
}
//mod graphics;

// Define FFI types for the scaling functions
#[allow(non_camel_case_types)]
type blockscale_fn =
    unsafe extern "C" fn(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);

// Declare external C functions
unsafe extern "C" {
    pub fn flow_scale_spatial_srgb_7x7(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_srgb_6x6(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_srgb_5x5(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_srgb_4x4(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_srgb_3x3(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_srgb_2x2(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_srgb_1x1(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);

    pub fn flow_scale_spatial_7x7(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_6x6(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_5x5(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_4x4(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_3x3(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_2x2(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
    pub fn flow_scale_spatial_1x1(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);
}

// Create test module
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    // Each scaling function paired with the N of the NxN block it writes.
    // Every one of them writes to `output_rows[r] + output_col + c` for
    // `r, c < N` and touches nothing else.
    const BLOCKSCALE_FUNCTIONS_SIZED: &[(blockscale_fn, usize)] = &[
        (flow_scale_spatial_srgb_7x7, 7),
        (flow_scale_spatial_srgb_6x6, 6),
        (flow_scale_spatial_srgb_5x5, 5),
        (flow_scale_spatial_srgb_4x4, 4),
        (flow_scale_spatial_srgb_3x3, 3),
        (flow_scale_spatial_srgb_2x2, 2),
        (flow_scale_spatial_srgb_1x1, 1),
        (flow_scale_spatial_7x7, 7),
        (flow_scale_spatial_6x6, 6),
        (flow_scale_spatial_5x5, 5),
        (flow_scale_spatial_4x4, 4),
        (flow_scale_spatial_3x3, 3),
        (flow_scale_spatial_2x2, 2),
        (flow_scale_spatial_1x1, 1),
    ];

    // Define the list of scaling functions
    const BLOCKSCALE_FUNCTIONS: &[blockscale_fn] = &[
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

    /// Downscaling a flat 8x8 block must give back the same flat value, and must
    /// stay inside the NxN output region.
    ///
    /// Every kernel here is a weighted average whose weights sum to 256 per axis,
    /// so a constant input is an exact fixed point for the linear variants; the
    /// sRGB variants round-trip through linear light and can land one step off.
    /// That invariant holds for all fourteen functions without hardcoding
    /// per-function expected values, which is what makes it worth asserting.
    ///
    /// The previous version of this test fed an all-zero block to each function
    /// and asserted nothing whatsoever — it could only have caught a segfault,
    /// and an implementation that wrote nothing at all passed it.
    ///
    /// A flat block is still a degenerate input in one respect: it pins the *sum*
    /// of each weight vector (the fixed point breaks as soon as the sum moves off
    /// 256 per axis) but is blind to any rearrangement that preserves the sum,
    /// because every weight multiplies the same value. Permutations and asymmetric
    /// corruption are covered by `test_mirror_equivariance`, and a weight driven to
    /// zero by `test_every_row_and_column_affects_output`.
    #[test]
    fn test_block_downscaling() {
        for &(func, n) in BLOCKSCALE_FUNCTIONS_SIZED {
            for &level in &[0u8, 64, 128, 255] {
                let output = run_kernel(func, &[level; 64]);

                for r in 0..8 {
                    for c in 0..8 {
                        let got = output[r * 8 + c];
                        if r < n && c < n {
                            assert!(
                                got.abs_diff(level) <= 2,
                                "{n}x{n} downscale of a flat {level} block gave {got} at \
                                 ({r},{c}); a flat block must stay flat"
                            );
                        } else {
                            assert_eq!(
                                UNWRITTEN, got,
                                "{n}x{n} downscale wrote {got} at ({r},{c}), outside its \
                                 {n}x{n} output region"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Every input row and every input column must be able to move the output.
    ///
    /// A weight that has been driven to zero — or a kernel that simply ignores part
    /// of its input — makes one row or column invisible, and neither the flat-block
    /// test nor an exact-value pin on a degenerate input can see that. Forcing one
    /// row (or column) at a time to 255 from a block where nothing is already 255
    /// isolates each weight's contribution.
    #[test]
    fn test_every_row_and_column_affects_output() {
        let base = base_block();

        for (idx, &(func, n)) in BLOCKSCALE_FUNCTIONS_SIZED.iter().enumerate() {
            let reference = run_kernel(func, &base);

            for c in 0..8usize {
                let mut perturbed = base;
                for r in 0..8usize {
                    perturbed[r * 8 + c] = 255;
                }
                assert_ne!(
                    reference,
                    run_kernel(func, &perturbed),
                    "blockscale fn {idx} ({n}x{n}): saturating input column {c} did not \
                     change the output, so that column's weight is unobservable"
                );
            }

            for r in 0..8usize {
                let mut perturbed = base;
                for c in 0..8usize {
                    perturbed[r * 8 + c] = 255;
                }
                assert_ne!(
                    reference,
                    run_kernel(func, &perturbed),
                    "blockscale fn {idx} ({n}x{n}): saturating input row {r} did not change \
                     the output, so that row's weight is unobservable"
                );
            }
        }
    }

    /// Downscaling a mirrored block must equal mirroring the downscaled block.
    ///
    /// This is a property of the operation, not of this implementation: a resampler
    /// that is not mirror-equivariant is wrong. It is also the cheapest oracle for
    /// the weight vectors, because it fails on any corruption that breaks the
    /// symmetry of the 47/60/71/78/78/71/60/47 taps — including one that a flat
    /// block cannot see because the sum is unchanged. Verified exact (delta 0, not
    /// within a tolerance) for all fourteen kernels: the terms are the same integers
    /// summed in a different order, and integer addition is exact.
    #[test]
    fn test_mirror_equivariance() {
        let base = base_block();

        for (idx, &(func, n)) in BLOCKSCALE_FUNCTIONS_SIZED.iter().enumerate() {
            let reference = run_kernel(func, &base);
            let from_mirrored_h = run_kernel(func, &mirror_h(&base));
            let from_mirrored_v = run_kernel(func, &mirror_v(&base));

            for r in 0..n {
                for c in 0..n {
                    assert_eq!(
                        from_mirrored_h[r * 8 + c],
                        reference[r * 8 + (n - 1 - c)],
                        "blockscale fn {idx} ({n}x{n}) is not horizontally mirror-equivariant \
                         at ({r},{c}); the per-column weights are no longer symmetric"
                    );
                    assert_eq!(
                        from_mirrored_v[r * 8 + c],
                        reference[(n - 1 - r) * 8 + c],
                        "blockscale fn {idx} ({n}x{n}) is not vertically mirror-equivariant \
                         at ({r},{c}); the per-row weights are no longer symmetric"
                    );
                }
            }
        }
    }

    #[test]
    fn benchmark_block_downscaling() {
        // Allocate input and output buffers. An all-zero block used to be timed here;
        // it is neither representative of real coefficient data nor able to show that
        // the kernel did anything, so this times the same non-degenerate block the
        // correctness tests use.
        let mut input = base_block();
        let mut output = [0u8; 64];
        let mut rows: [*mut u8; 8] = unsafe {
            [
                output.as_mut_ptr(),
                output.as_mut_ptr().add(8),
                output.as_mut_ptr().add(16),
                output.as_mut_ptr().add(24),
                output.as_mut_ptr().add(32),
                output.as_mut_ptr().add(40),
                output.as_mut_ptr().add(48),
                output.as_mut_ptr().add(56),
            ]
        };

        // Set number of runs based on debug/release mode
        #[cfg(debug_assertions)]
        let max_runs = 1;
        #[cfg(not(debug_assertions))]
        let max_runs = 1000;

        let reps = std::cmp::min(max_runs, 900);

        // Benchmark each scaling function
        for (i, func) in BLOCKSCALE_FUNCTIONS.iter().enumerate() {
            // A timing loop over a function that does nothing measures nothing.
            // Mark the buffer and require the kernel to have written its top-left
            // output cell, which every size from 1x1 up covers.
            output[0] = 0xAB;

            let start = Instant::now();

            for _ in 0..reps {
                unsafe {
                    func(input.as_mut_ptr(), rows.as_mut_ptr(), 0);
                }
            }

            assert_ne!(
                0xAB, output[0],
                "blockscale fn {i} never wrote its output after {reps} calls"
            );

            let duration = start.elapsed();
            let ms = duration.as_secs_f64() * 1000.0;
            let megapixels = (reps as f64 * 64.0) / 1_000_000.0;

            println!(
                "Block downscaling fn {} took {:.5}ms for {} reps ({:.2} megapixels)",
                i, ms, reps, megapixels
            );
        }
    }
}
