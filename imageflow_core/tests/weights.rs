extern crate imageflow_core;
extern crate imageflow_helpers as hlp;
extern crate imageflow_types as s;

use itertools::Itertools;

use imageflow_core::ffi::ImageflowContext;
use imageflow_core::imaging::weights::InterpolationDetails;

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

fn test_filter(filter: s::Filter, expected_first_crossing: f64, expected_second_crossing: f64, expected_near0: f64, near0_threshold: f64, expected_end: f64) {
    let details = imageflow_core::imaging::weights::InterpolationDetails::create(filter);
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
    test_filter(s::Filter::Cubic, 1f64, 2f64, 1f64, 0.08f64, 2f64);
    test_filter(s::Filter::Hermite, 0f64, 0f64, 0.99f64, 0.08f64, 1f64);
    test_filter(s::Filter::Triangle, 0f64, 0f64, 0.99f64, 0.08f64, 1f64);
    test_filter(s::Filter::Box, 0f64, 0f64, 0.51f64, 0.001f64, 0.51f64);
    test_filter(s::Filter::CatmullRom, 1f64, 2f64, 1f64, 0.08f64, 2f64);
    test_filter(s::Filter::CubicBSpline, 0f64, 0f64, 1.75f64, 0.08f64, 2f64);
    test_filter(s::Filter::Mitchell, 8.0 / 7.0, 2.0, 1f64, 0.08, 2.0);
    test_filter(s::Filter::Robidoux, 1.1685777620836932, 2f64, 1f64, 0.08, 2f64);
    test_filter(s::Filter::RobidouxSharp, 1.105822933719019, 2f64, 1f64, 0.08, 2f64);
    test_filter(s::Filter::Lanczos2, 1f64, 2f64, 1f64, 0.08, 2f64);
    test_filter(s::Filter::Lanczos2Sharp, 0.954, 1.86, 1f64, 0.08, 2f64);
    test_filter(s::Filter::Lanczos, 1f64, 2f64, 1f64, 0.1, 3f64);
    test_filter(s::Filter::Lanczos2Sharp, 0.98, 1.9625, 1f64, 0.1, 2.943)
}

#[test]
fn test_output_weight() {
    use s::Filter::*;
    let scalings: [u32; 44] = [/*downscale to 1px*/ 1, 1, 2, 1, 3, 1, 4, 1, 5, 1, 6, 1, 7, 1, 17, 1,
        /*upscale from 2px*/ 2, 3, 2, 4, 2, 5, 2, 17,
        /*other*/ 11, 7, 7, 3,
        /* IDCT kernel sizes */ 8, 8, 8, 7, 8, 6, 8, 5, 8, 4, 8, 3, 8, 2, 8, 1];
    let filters = [RobidouxFast, RobidouxFast, RobidouxSharp, Ginseng, GinsengSharp, Lanczos, LanczosSharp, Lanczos2, Lanczos2Sharp, Cubic, CubicSharp, CatmullRom, Mitchell, CubicBSpline, Hermite, Jinc, RawLanczos3, RawLanczos3Sharp, RawLanczos2, RawLanczos2Sharp, Triangle, Linear, Box, CatmullRomFast, CatmullRomFastSharp, Fastest, MitchellFast, NCubic, NCubicSharp
    ];
    // let filters=[RobidouxFast,RobidouxFast];

    for &filter in filters.iter() {
        let details = InterpolationDetails::create(filter);
        for i in (0..scalings.len()).step_by(2) {
            let mut w = imageflow_core::imaging::weights::PixelRowWeights {
                contrib_row: vec![],
                window_size: 0,
                line_length: 0,
                percent_negative: 0.0,
            };
            println!("{:?} {} {}",filter,scalings[i+1], scalings[i]);
            assert_eq!(imageflow_core::imaging::weights::populate_weights(&mut w, scalings[i+1], scalings[i], &details),true);
            for output_pixel in w.contrib_row{
                for current in output_pixel.right..=output_pixel.left{

                }
            }

        }
    }
}
