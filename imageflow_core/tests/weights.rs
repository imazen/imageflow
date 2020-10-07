extern crate imageflow_core;
extern crate imageflow_helpers as hlp;
extern crate imageflow_types as s;

use imageflow_core::imaging::weights::InterpolationDetails;

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
