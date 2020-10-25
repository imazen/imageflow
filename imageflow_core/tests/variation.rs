extern crate imageflow_core;

use imageflow_core::graphics::weights::*;

static REVERSE_LUT_SIZE_SHORT: u32 = 256 * 16;

// function to find integer near weight with common denominator. i8 bound check is very important.
fn find_integral_weights(divisor: &mut u32, contribs: &PixelWeightsSimple) -> Result<[i8; 8], String>
{
    let mut eight;

    let mut fail_reason = "".to_owned();
    for divisor_bits in (7..=10).rev() {
        *divisor = 1 << divisor_bits;
        let mut scalar: f32 = *divisor as f32;
        while scalar < *divisor as f32 + 10f32 {
            // println!("{:?}",divisor);
            let mut failed = false;
            eight = [0, 0, 0, 0, 0, 0, 0, 0];

            let mut sum = 0;

            for (index, &v) in contribs.weights.iter().enumerate() {
                let f = v * scalar;
                if (f as i32 > i8::MAX as i32) || ((f as i32) < i8::MIN as i32) {
                    // Out of bounds.
                    failed = true;
                    fail_reason.push_str(&format!("\nValue does not fit in i8: {}; scalar = {}", f, scalar));
                    break;
                }

                eight[(contribs.left as usize + index)] = f as i8;
                sum += eight[contribs.left as usize + index] as i32;
            }
            if !failed && sum as u32 != *divisor {
                fail_reason.push_str(&format!("\nSum {} does not equal divisor {}", sum, *divisor));
                failed = true;
            }

            //let original_divisor = *divisor;
            if !failed {
                for reduce in (1..=4).rev() {
                    let new_divisor: u32 = 1 << reduce;

                    let mut all_divisible = true;
                    for i in 0..8 {
                        if eight[i] as i32 % new_divisor as i32 != 0 {
                            all_divisible = false;
                        }
                    }
                    if all_divisible {
                        *divisor /= new_divisor as u32;
                        for i in 0usize..8usize {
                            eight[i] /= new_divisor as i8;
                        }
                        break;
                    }
                }
            }

            if !failed {
                //eprintln!("Divisor={}; Found with divisor={}, scalar = {}, {:?}", *divisor, original_divisor, scalar, eight);
                //print!("{:?}",eight);
                return Ok(eight);
            }
            scalar = *divisor as f32 + (if scalar > *divisor as f32 { -(scalar - *divisor as f32 + 0.125) } else { -(scalar - *divisor as f32 - 0.125) });
        }
    }
    return Err(format!("Failed to find integral weights: {}", fail_reason));
}


fn linear_to_srgb(clr: f32) -> f32
{
// Gamma correction
// http://www.4p8.com/eric.brasseur/gamma.html#formulas

    if clr <= 0.0031308f32 {
        return 12.92f32 * clr * 255.0f32;
    }


// a = 0.055; ret ((1+a) * s**(1/2.4) - a) * 255
    return 1.055f32 * 255.0f32 * (f32::powf(clr, 0.41666666f32)) - 14.025f32;
}

fn srgb_to_linear(s: f32) -> f32
{
    if s <= 0.04045f32 {
        return s / 12.92f32;
    }
    return f32::powf((s + 0.055f32) / (1f32 + 0.055f32), 2.4f32);
}


fn print_scale_header(scale_size: i32, linear: bool, end: &str) -> String {
    format!(
        "FLOW_EXPORT void flow_scale_spatial{}_{}x{}(uint8_t input[64], uint8_t ** output_rows, uint32_t output_col){}",
        if linear { "_srgb" } else { "" }, scale_size, scale_size, end)
}

fn print_short_luts() -> String {
    let mut output = String::from("");
    let mut reverse_lut = vec![0; REVERSE_LUT_SIZE_SHORT as usize];
    output.push_str(&format!("FLOW_ALIGN_16 static const uint8_t lut_linear_to_srgb[{}] = {{\n", REVERSE_LUT_SIZE_SHORT));
    for a in 0..REVERSE_LUT_SIZE_SHORT / 16 {
        output.push_str("    ");
        for b in 0..16 {
            let index = (a * 16 + b) as usize;
            let v
                = f32::min(255f32, f32::max(0f32, 0.5f32 + linear_to_srgb((index as f32 + 0.1875f32)
                / (REVERSE_LUT_SIZE_SHORT - 1) as f32)));
            reverse_lut[index] = v as u32;
            output.push_str(&format!("{}, ", v as u32));
        }
        output.push_str("\n");
    }
    output.push_str("};\n\n");

    let mut lut = [0f32; 256];

    output.push_str("FLOW_ALIGN_16 static const uint16_t lut_srgb_to_linear[256] = {\n");
    for a in 0..16 {
        output.push_str("    ");
        for b in 0..16 {
            let index = a * 16 + b;

            let f = srgb_to_linear(index as f32 / 255.0f32);
            lut[index] = 0.5f32 + f * (REVERSE_LUT_SIZE_SHORT as f32 - 1f32);

            let reverse_lut_index = lut[index];
            let roundtrip = reverse_lut[reverse_lut_index as usize];
            assert_eq!(roundtrip, index as u32);
            output.push_str(&format!("{}, ", lut[index] as u32));
        }
        output.push_str("\n");
    }
    output.push_str("};\n\n");
    output
}

fn get_max_window_size(matrix: &[i8; 64], rows: i32) -> i32
{
    let mut max_window_size: i32 = 0;
    for i in 0usize..rows as usize {
        let mut first_sampled = 8;
        let mut last_sampled = 0;
        for j in 0usize..8 {
            let value = matrix[i * 8 + j];
            if value != 0 {
                if j < first_sampled {
                    first_sampled = j;
                }

                if j > last_sampled {
                    last_sampled = j;
                }
            }
        }
        let window = last_sampled as i32 - first_sampled as i32 + 1;
        if window > max_window_size {
            max_window_size = window;
        }
    }
    max_window_size as i32
}

fn index_of_first_nonzero(arr: &[i8]) -> i32
{
    for (i, &value) in arr.into_iter().enumerate() {
        if value != 0 {
            return i as i32;
        }
    }
    return -1;
}

fn index_of_last_nonzero(arr: &[i8]) -> i32
{
    for (i, &value) in arr.into_iter().rev().enumerate() {
        if value != 0 {
            return (arr.len() as i32 - i as i32 - 1) as i32;
        }
    }
    return -1;
}


fn print_function(scale_size: i32, contribs: PixelRowWeightsSimple, linear: bool) -> String
{
    let mut output = String::from("");
    let mut matrix = [0; 64];
    let mut divisors = [0u32; 8];


    for (i, row) in contribs.contrib_row.iter().enumerate().take(scale_size as usize) {
        for (j, &pixel) in find_integral_weights(&mut divisors[i], row).unwrap().iter().enumerate() {
            matrix[i * 8 + j] = pixel;
        }
    }
    let max_window_size = get_max_window_size(&matrix, scale_size);
    output.push_str("    int32_t i, sum, j;\n");
    if linear {
        output.push_str("    FLOW_ALIGN_16_VAR(int32_t linearized[64]);\n");
        output.push_str("    for (i = 0; i < 64; i++)\n");
        output.push_str("        linearized[i] = lut_srgb_to_linear[input[i]];\n\n");
    }
    output.push_str(&format!("    FLOW_ALIGN_16_VAR(int32_t temp[{}]);\n", 8 * (max_window_size + 2)));
    let mut matrix_counts = [0u32; 8];
    for (index, col) in matrix.chunks(8usize).enumerate().take(scale_size as usize) {
        let left = index_of_first_nonzero(col);
        let right = index_of_last_nonzero(col);
        let mut col_inputs = 0;
// Write down weights for 1 output pixel
        output.push_str(&format!("    FLOW_ALIGN_16_VAR(int32_t weights_for_col_{}[]) = {{", index));
        if left == -1 || right == -1 {
            continue;
        }
        for input_col in left..=right {
            let weight = col[input_col as usize];
            output.push_str(&format!("{}, ", weight));
            col_inputs += 1;
        }
        matrix_counts[index] = col_inputs;
        output.push_str("};\n");
    }

// Scale vertically, then horizontally
    for (row, ele) in matrix.chunks(8usize).enumerate().take(scale_size as usize) {
        output.push_str(&format!("\n    // Begin work for output row {}\n", row));

        let input_starts_at = index_of_first_nonzero(ele);
        let input_ends_at = index_of_last_nonzero(ele);

        if input_starts_at == -1 || input_ends_at == -1 {
            continue;
        }
        let mut input_row_count = 0;
// Multiply rows
        for input_row in input_starts_at as u32..=input_ends_at as u32 {
            let weight = ele[input_row as usize];

            let relative_row = input_row - (input_starts_at as u32);
            output.push_str(&format!("    for (i = 0; i < 8; i++) temp[i + {}] = {} * {}[i + {}];\n", relative_row * 8,
                                     weight, if linear {
                    "linearized"
                } else {
                    "input"
                }
                                     , input_row * 8));
            input_row_count += 1;
        }

        let temp_row_index_a = 8 * max_window_size;

        let temp_row_index_b = 8 * (max_window_size + 1);

        output.push_str(&format!("    for (i = 0; i < 8; i++){{
        sum = 0;
        for (j = 0; j < {}; j++)
              sum += temp[j * 8 + i];
        temp[{} + i] = sum;
    }}", input_row_count, temp_row_index_a));

// Scale horizontally now
        for (col, ele) in matrix.chunks(8usize).enumerate().take(scale_size as usize) {
            output.push_str(&format!("\n    // Begin work for output pixel {},{}\n", col, row));
            let left = index_of_first_nonzero(ele);
            let col_inputs = matrix_counts[col];
// Multiply weights
            output.push_str(&format!("    for (i = 0; i < {}; i++) temp[{} + i] = temp[{} + i] * weights_for_col_{}[i];\n",
                                     col_inputs, temp_row_index_b, temp_row_index_a + left, col));

// Sum values

            let divisor_sum = divisors[row as usize] * divisors[col as usize]; // Add the rounding offset first
            output.push_str(&format!("    sum = {};\n", divisor_sum / 2));
            output.push_str(&format!("    for (i = 0; i < {}; i++) sum += temp[{} + i];\n", col_inputs, temp_row_index_b));

            //println!("{} {}",divisors[row as usize],divisors[col as usize]);
            let upper_bound = REVERSE_LUT_SIZE_SHORT as u64 * divisor_sum as u64;

// Add and shift (divides with rounding), then perform lookup
            output.push_str(&format!("    *(output_rows[{}] + output_col + {}) = sum < 0 ? (uint8_t)0 : (sum >= {} ? (uint8_t)255 : (uint8_t){}(sum >> {}){});\n",
                                     row, col, upper_bound, if linear {
                    "lut_linear_to_srgb["
                } else { "" }, (divisor_sum as f64).log2(),
                                     if linear {
                                         "]"
                                     } else { "" }));

            output.push_str(&format!("    // Pixel {},{} complete\n", col, row));
        }
    }
    output
}


fn print_header(scale_size: i32, linear: bool, end: &str) -> String
{
    format!(
        "void jpeg_idct_spatial{}_{}x{}(j_decompress_ptr cinfo, jpeg_component_info * compptr, JCOEFPTR coef_block,
                                JSAMPARRAY output_buf, JDIMENSION output_col){}",
        if linear { "_srgb" } else { "" }, scale_size, scale_size, end)
}


fn print_all_idct_functions() -> String
{
    let idct_function_begin = "    JSAMPLE input[64];
        JSAMPROW rows[8]
            = { &input[0],     &input[8],     &input[8 * 2], &input[8 * 3],
                &input[8 * 4], &input[8 * 5], &input[8 * 6], &input[8 * 7] };
        jpeg_idct_islow(cinfo, compptr, coef_block, &rows[0], 0);\n";
    let details = InterpolationDetails::create(Filter::Robidoux);
    let mut output = String::from("");
    for &linear in [false, true].iter() {
        for size in (1u32..8u32).rev() {
            output.push_str(&print_scale_header(size as i32, linear, "{\n"));
            let mut contrib = PixelRowWeightsSimple {
                contrib_row: vec![],
            };
            assert_eq!(populate_weights(&mut contrib, size, 8, &details), Ok(()));

            output.push_str(&print_function(size as i32, contrib, linear));
            output.push_str("}\n");

            output.push_str("\n#ifndef FLOW_GCC_IDCT\n");
            output.push_str(&print_header(size as i32, linear, "{\n"));
            output.push_str(idct_function_begin);
            output.push_str(&format!("    flow_scale_spatial{}_{}x{}(input, output_buf, output_col);\n", if linear {
                "_srgb"
            } else {
                ""
            }, size, size));
            output.push_str("}\n");
            output.push_str("#endif\n\n");
        }
    }
    output
}


fn print_c_intro() -> String
{
    "// This file is autogenerated by test_variations. Do not edit; regenerate\n
#include <stdint.h>

#ifndef FLOW_GCC_IDCT
#define JPEG_INTERNALS
#include <stdio.h>
#include \"jpeglib.h\"
#include \"jdct.h\" /* Private declarations for DCT subsystem */
#endif

#if defined(__GNUC__) && !defined(__clang__)
#define HOT
    __attribute__((hot)) __attribute__((optimize(\"-funsafe-math-optimizations\",\"-ftree-vectorize\")))
#else
#if defined(__GNUC__)
#define HOT __attribute__((hot))
#else
#define HOT
#endif
#endif
#ifdef _MSC_VER
#define FLOW_EXPORT __declspec(dllexport)
#define FLOW_ALIGN_16 __declspec(align(16))
#define FLOW_ALIGN_16_VAR(X) __declspec(align(16)) X
#else
#define FLOW_EXPORT
#define FLOW_ALIGN_16 __attribute__((aligned(16)))
#define FLOW_ALIGN_16_VAR(X) X __attribute__((aligned(16)))
#endif\n".to_owned()
}


#[test]
fn test_generate_code_to_disk() {
    let mut output = print_c_intro();
    for size in (1..=7).rev() {
        output.push_str(&print_scale_header(size, true, " HOT;\n\n"));
    }
    for size in (1..=7).rev() {
        output.push_str(&print_scale_header(size, false, " HOT;\n\n"));
    }
    output.push_str("\n#ifndef FLOW_GCC_IDCT\n");
    for size in (1..=7).rev() {
        output.push_str(&print_header(size, true, " HOT;\n\n"));
    }
    for size in (1..=7).rev() {
        output.push_str(&print_header(size, false, " HOT;\n\n"));
    }
    output.push_str("#endif\n\n");
    output.push_str(&print_short_luts());
    output.push_str(&print_all_idct_functions());

    output.push_str("#ifdef FLOW_GCC_IDCT\n void main(void){}\n#endif\n");
    std::fs::write("generated_idct.c", &output).expect("error creating file");
}

#[test]
fn test_variation() {
    let details
        = InterpolationDetails::create(Filter::Robidoux);
    for size in 1..8 {
        let mut contrib = PixelRowWeightsSimple {
            contrib_row: vec![],
        };
        assert_eq!(populate_weights(&mut contrib, size, 8, &details), Ok(()));

        for pixel in contrib.contrib_row {
            //  println!("{:?} ",pixel.weights);
            if let Err(e) = find_integral_weights(&mut 0u32, &pixel) {
                eprintln!("{}", e);
                assert!(false);
            }
        }
    }
}

