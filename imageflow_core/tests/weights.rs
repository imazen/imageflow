extern crate imageflow_core;
extern crate imageflow_helpers as hlp;
extern crate imageflow_types as s;

use imageflow_core::graphics::weights::InterpolationDetails;

// extern "C" {
//     fn flow_interpolation_line_contributions_create(context:&ImageflowContext,to_width:i32,from_width:i32,details:);
// }

fn function_bounded(details: &InterpolationDetails,
                    input_start_value: f64, stop_at_abs: f64, input_step: f64, result_low_threshold: f64,
                    result_high_threshold: f64) -> bool {
    let input_value = input_start_value;

    if input_value.abs() >= stop_at_abs.abs() {
        return true;
    }
    let result_value = (details.filter)(details, input_value);

    if result_value < result_low_threshold {
        return false;
    } else if result_value > result_high_threshold {
        return false;
    };
    function_bounded(details, input_value + input_step, stop_at_abs, input_step,
                     result_low_threshold, result_high_threshold)
}

// struct InterpolationLineContributions {
//     ContribRow: *mut InterpolationPixelContribution,
//     /* Row (or column) of contribution weights */
//     WindowSize: u32,
//     /* Filter window size (of affecting source pixels) */
//     LineLength: u32,
//     /* Length of line (no. or rows / cols) */
//     percent_negative: f64,
//     /* Estimates the sharpening effect actually applied*/
// };
//
// struct InterpolationPixelContribution {
//     Weights: *mut f64,
//     /* Normalized weights of neighboring pixels */
//     Left: i32,
//     /* Bounds of source pixels window */
//     Right: i32,
// }

macro_rules! function_bounded_bi {
    ($details:expr,$input_start_value:expr, $stop_at_abs:expr, $input_step:expr, $result_low_threshold:expr,$result_high_threshold:expr) => {
       {
            function_bounded( $details, $input_start_value, $stop_at_abs, $input_step, $result_low_threshold,$result_high_threshold)&&
             function_bounded($details, $input_start_value * -1.0f64, $stop_at_abs, $input_step * -1.0f64,$result_low_threshold, $result_high_threshold)
       }
    }
}

fn test_filter(filter: imageflow_core::graphics::weights::Filter, expected_first_crossing: f64, expected_second_crossing: f64, expected_near0: f64, near0_threshold: f64, expected_end: f64) {
    let details = imageflow_core::graphics::weights::InterpolationDetails::create(filter);
    let top = (details.filter)(&details, 0.0);

    assert_eq!(function_bounded_bi!(&details, 0.0, expected_end, 0.05, -500.0, top), true);


    assert_eq!(function_bounded_bi!(&details,
         expected_near0,
         if expected_second_crossing > 0.0 {expected_second_crossing}  else{ expected_end},
          0.05,
           -500.0,
           near0_threshold), true);

    assert_eq!(function_bounded_bi!(&details,expected_end, expected_end + 1.0, 0.05, -0.0001f64, 0.0001f64), true);


    if expected_first_crossing != 0.0 && expected_second_crossing != 0.0 {
        assert_eq!(function_bounded_bi!(&details, expected_first_crossing + 0.05, expected_second_crossing - 0.05,
                                 0.05, -500.0, -0.0001f64), true);
        if expected_end > expected_second_crossing + 0.1 {
            assert_eq!(function_bounded_bi!(&details, expected_second_crossing + 0.05, expected_end - 0.02, 0.02,
                                     0.0, 500.0), true);
        }
    } else {
        assert_eq!(function_bounded_bi!( &details, expected_near0, expected_end, 0.05, -0.0001, 500.0), true);
    }

}

#[test]
fn test_interpolation_filter() {
    test_filter(imageflow_core::graphics::weights::Filter::Cubic, 1f64, 2f64, 1f64, 0.08f64, 2f64);
    test_filter(imageflow_core::graphics::weights::Filter::Hermite, 0f64, 0f64, 0.99f64, 0.08f64, 1f64);
    test_filter(imageflow_core::graphics::weights::Filter::Triangle, 0f64, 0f64, 0.99f64, 0.08f64, 1f64);
    test_filter(imageflow_core::graphics::weights::Filter::Box, 0f64, 0f64, 0.51f64, 0.001f64, 0.51f64);
    test_filter(imageflow_core::graphics::weights::Filter::CatmullRom, 1f64, 2f64, 1f64, 0.08f64, 2f64);
    test_filter(imageflow_core::graphics::weights::Filter::CubicBSpline, 0f64, 0f64, 1.75f64, 0.08f64, 2f64);
    test_filter(imageflow_core::graphics::weights::Filter::Mitchell, 8.0 / 7.0, 2.0, 1f64, 0.08, 2.0);
    test_filter(imageflow_core::graphics::weights::Filter::Robidoux, 1.1685777620836932, 2f64, 1f64, 0.08, 2f64);
    test_filter(imageflow_core::graphics::weights::Filter::RobidouxSharp, 1.105822933719019, 2f64, 1f64, 0.08, 2f64);
    test_filter(imageflow_core::graphics::weights::Filter::Lanczos2, 1f64, 2f64, 1f64, 0.08, 2f64);
    test_filter(imageflow_core::graphics::weights::Filter::Lanczos2Sharp, 0.954, 1.86, 1f64, 0.08, 2f64);
    test_filter(imageflow_core::graphics::weights::Filter::Lanczos, 1f64, 2f64, 1f64, 0.1, 3f64);
    test_filter(imageflow_core::graphics::weights::Filter::Lanczos2Sharp, 0.98, 1.9625, 1f64, 0.1, 2.943)
}

#[test]
fn test_output_weight() {
    use imageflow_core::graphics::weights::Filter::*;
    let scalings: [u32; 44] = [/*downscale to 1px*/ 1, 1, 2, 1, 3, 1, 4, 1, 5, 1, 6, 1, 7, 1, 17, 1,
        /*upscale from 2px*/ 2, 3, 2, 4, 2, 5, 2, 17,
        /*other*/ 11, 7, 7, 3,
        /* IDCT kernel sizes */ 8, 8, 8, 7, 8, 6, 8, 5, 8, 4, 8, 3, 8, 2, 8, 1];
    let filters = [RobidouxFast, Robidoux, RobidouxSharp, Ginseng, GinsengSharp, Lanczos, LanczosSharp, Lanczos2, Lanczos2Sharp, CubicFast,Cubic, CubicSharp, CatmullRom, Mitchell, CubicBSpline, Hermite, Jinc, RawLanczos3, RawLanczos3Sharp, RawLanczos2, RawLanczos2Sharp, Triangle, Linear, Box, CatmullRomFast, CatmullRomFastSharp, Fastest, MitchellFast, NCubic, NCubicSharp
    ];
    let mut output=String::from("filter, from_width, to_width, weights");
    for (index,&filter) in filters.iter().enumerate() {
        let details = InterpolationDetails::create(filter);

        for i in (0..scalings.len()).step_by(2) {
            let mut w = imageflow_core::graphics::weights::PixelRowWeights::new();
            output.push_str(&format!("\r\nfilter_{:0>2} ({: >2}px to {: >2}px):",index+1,scalings[i],scalings[i+1]));
            assert_eq!(imageflow_core::graphics::weights::populate_weights(&mut w, scalings[i+1], scalings[i], &details), Ok(()));
            for (o_index,output_pixel) in w.contrib_row().iter().enumerate(){
                output.push_str(&format!(" x={} from ",o_index));
                for (w_index,&weight) in w.weights()[output_pixel.left_weight as usize..=output_pixel.right_weight as usize].iter().enumerate(){
                    output.push_str(if w_index==0 {"(" } else {" "});
                    output.push_str(&format!("{:.6}",weight));
                }
                output.push_str("),");
            }
        }
    }
    assert_eq!(output.trim(),include_str!("visuals/weights.txt").to_string().trim());
}

#[test]
fn test_output_weight_symmetric() {
    use imageflow_core::graphics::weights::Filter::*;
    let scalings: [u32; 44] = [/*downscale to 1px*/ 1, 1, 2, 1, 3, 1, 4, 1, 5, 1, 6, 1, 7, 1, 17, 1,
        /*upscale from 2px*/ 2, 3, 2, 4, 2, 5, 2, 17,
        /*other*/ 11, 7, 7, 3,
        /* IDCT kernel sizes */ 8, 8, 8, 7, 8, 6, 8, 5, 8, 4, 8, 3, 8, 2, 8, 1];
    let filters = [RobidouxFast, Robidoux, RobidouxSharp, Ginseng, GinsengSharp, Lanczos, LanczosSharp, Lanczos2, Lanczos2Sharp, CubicFast,Cubic, CubicSharp, CatmullRom, Mitchell, CubicBSpline, Hermite, Jinc, RawLanczos3, RawLanczos3Sharp, RawLanczos2, RawLanczos2Sharp, Triangle, Linear, Box, CatmullRomFast, CatmullRomFastSharp, Fastest, MitchellFast, NCubic, NCubicSharp, LegacyIDCTFilter
    ];
    for &filter in filters.iter(){
        let details = InterpolationDetails::create(filter);
        for i in (0..scalings.len()).step_by(2) {
            let mut w = imageflow_core::graphics::weights::PixelRowWeights::new();
            assert_eq!(imageflow_core::graphics::weights::populate_weights(&mut w, scalings[i+1], scalings[i], &details), Ok(()));
            for o_index in 0..w.contrib_row().len()/2 {
                let output_pixel = &w.contrib_row()[o_index];
                let opposite_output_pixel = &w.contrib_row()[w.contrib_row().len() - 1 - o_index];
                assert_eq!((scalings[i] as i32) - 1 - opposite_output_pixel.right_pixel as i32, output_pixel.left_pixel as i32);
                assert_eq!((scalings[i] as i32) - 1 - output_pixel.right_pixel as i32, opposite_output_pixel.left_pixel as i32);
                for (w_index, &weight) in w.weights()[output_pixel.left_weight as usize..=output_pixel.right_weight as usize].iter().enumerate() {
                    let opposite_weights = &w.weights()[opposite_output_pixel.left_weight as usize..=opposite_output_pixel.right_weight as usize];
                    assert_eq!((weight - opposite_weights[opposite_weights.len() - 1 - w_index]).abs() < 1e-5, true);
                    assert_eq!(weight < 5f32, true)
                }
            }

        }
    }

}
