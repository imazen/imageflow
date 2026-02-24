// Force linkage of the native C library compiled by this crate's build.rs
extern crate imageflow_c_components;

#[test]
fn test_spatial_scaling() {
    let mut input: [u8; 64] = [0; 64];
    for x in 0..64 {
        input[x] = if x % 2 == 0 { 0 } else { 255 };
    }

    let mut output: [u8; 1] = [0; 1];
    let mut output_rows: [*mut u8; 1] = [&mut output[0]];
    let output_col = 0;

    unsafe { flow_scale_spatial_srgb_1x1(&mut input[0], &mut output_rows[0], output_col) }

    assert_eq!(output[0], 188);
}
//mod graphics;

// Define FFI types for the scaling functions
#[allow(non_camel_case_types)]
type blockscale_fn =
    unsafe extern "C" fn(input: *mut u8, output_rows: *mut *mut u8, output_col: u32);

// Declare external C functions
extern "C" {
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

    #[test]
    fn test_block_downscaling() {
        // Allocate input and output buffers
        let mut input = [0u8; 64];
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

        // Test each scaling function
        for func in BLOCKSCALE_FUNCTIONS {
            unsafe {
                func(input.as_mut_ptr(), rows.as_mut_ptr(), 0);
            }
        }
    }

    #[test]
    fn benchmark_block_downscaling() {
        // Allocate input and output buffers
        let mut input = [0u8; 64];
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
            let start = Instant::now();

            for _ in 0..reps {
                unsafe {
                    func(input.as_mut_ptr(), rows.as_mut_ptr(), 0);
                }
            }

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
