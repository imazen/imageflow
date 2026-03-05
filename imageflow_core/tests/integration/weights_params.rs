use imageflow_core::graphics::weights::{Filter, InterpolationDetails, PixelRowWeights};
use std::fmt;

/// Set to `true` to overwrite weights_params.txt with current output.
/// Must be set back to `false` before committing.
const UPDATE_REFERENCE: bool = false;

/// A parameter variation to apply to a filter.
#[derive(Clone, Copy)]
enum ParamVariation {
    /// Default filter parameters (kernel_scale=1.0, sharpen=0%)
    Default,
    /// Scale the kernel width. >1.0 widens (blurs), <1.0 narrows (sharpens).
    KernelScale(f64),
    /// Set sharpen percent goal (amplifies negative lobes).
    Sharpen(f32),
}

impl fmt::Display for ParamVariation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamVariation::Default => write!(f, "default"),
            ParamVariation::KernelScale(s) => write!(f, "kernel_scale={:.2}", s),
            ParamVariation::Sharpen(s) => write!(f, "sharpen={:.1}", s),
        }
    }
}

impl ParamVariation {
    fn apply(&self, details: &mut InterpolationDetails) {
        match self {
            ParamVariation::Default => {}
            ParamVariation::KernelScale(factor) => details.set_kernel_width_scale(*factor),
            ParamVariation::Sharpen(pct) => details.set_sharpen_percent_goal(*pct),
        }
    }
}

/// Generate weight output for various blur/sharpen parameter combinations.
///
/// Format matches weights.txt style for easy diffing:
///   `FilterName param (Wpx to Hpx): x=0 from (w0 w1 ...), ...`
fn generate_param_weights() -> String {
    // Representative filters covering each filter function type
    let filters = [
        Filter::Robidoux,      // bicubic (default)
        Filter::RobidouxSharp, // bicubic (sharp variant)
        Filter::Lanczos,       // sinc_windowed, window=3
        Filter::Lanczos2,      // sinc_windowed, window=2
        Filter::Lanczos2Sharp, // sinc_windowed, blur<1
        Filter::CatmullRom,    // bicubic (B=0, C=0.5)
        Filter::Mitchell,      // bicubic (B=1/3, C=1/3)
        Filter::Ginseng,       // ginseng (jinc-windowed sinc)
        Filter::CubicFast,     // bicubic_fast
        Filter::Hermite,       // bicubic (B=0, C=0)
        Filter::Triangle,      // triangle
        Filter::Box,           // box
    ];

    // Scaling combos: downscale, upscale, same-size, IDCT-like
    let scalings: [(u32, u32); 10] = [
        (1, 1),
        (4, 1),   // 4x downscale
        (7, 3),   // fractional downscale
        (11, 7),  // fractional downscale
        (2, 5),   // 2.5x upscale
        (2, 9),   // 4.5x upscale
        (8, 8),   // same size
        (8, 5),   // IDCT-like downscale
        (8, 3),   // IDCT-like downscale
        (17, 11), // moderate downscale with enough edge/center variation
    ];

    let variations = [
        ParamVariation::Default,
        ParamVariation::KernelScale(0.8),
        ParamVariation::KernelScale(0.9),
        ParamVariation::KernelScale(1.1),
        ParamVariation::KernelScale(1.2),
        ParamVariation::Sharpen(5.0),
        ParamVariation::Sharpen(15.0),
        ParamVariation::Sharpen(50.0),
    ];

    let mut output = String::from("filter, param, from_width, to_width, weights");

    for &filter in &filters {
        for &variation in &variations {
            let mut details = InterpolationDetails::create(filter);
            variation.apply(&mut details);

            for &(from_w, to_w) in &scalings {
                let mut w = PixelRowWeights::new();
                let result = imageflow_core::graphics::weights::populate_weights(
                    &mut w, to_w, from_w, &details,
                );
                if result.is_err() {
                    output.push_str(&format!(
                        "\n{:?} {} ({: >3}px to {: >2}px): ERROR",
                        filter, variation, from_w, to_w
                    ));
                    continue;
                }

                output.push_str(&format!(
                    "\n{:?} {} ({: >3}px to {: >2}px):",
                    filter, variation, from_w, to_w
                ));
                for (o_index, output_pixel) in w.contrib_row().iter().enumerate() {
                    output.push_str(&format!(" x={} from ", o_index));
                    for (w_index, &weight) in w.weights()
                        [output_pixel.left_weight as usize..=output_pixel.right_weight as usize]
                        .iter()
                        .enumerate()
                    {
                        output.push_str(if w_index == 0 { "(" } else { " " });
                        output.push_str(&format!("{:.6}", weight));
                    }
                    output.push_str("),");
                }
            }
        }
    }
    output
}

#[test]
fn test_param_weights() {
    let output = generate_param_weights();
    let reference_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("integration")
        .join("weights_params.txt");

    if UPDATE_REFERENCE {
        std::fs::write(&reference_path, &output).expect("Failed to write reference file");
        panic!(
            "UPDATE_REFERENCE is true — wrote {}. Set it back to false before committing.",
            reference_path.display()
        );
    }

    let reference = std::fs::read_to_string(&reference_path)
        .unwrap_or_else(|e| {
            panic!(
                "Failed to read {}: {}. \
                 Set UPDATE_REFERENCE = true in weights_params.rs to generate it.",
                reference_path.display(),
                e
            )
        })
        .replace("\r\n", "\n");

    assert_eq!(
        output.trim(),
        reference.trim(),
        "Generated param weights differ from reference file {}. \
         Set UPDATE_REFERENCE = true in weights_params.rs, run the test, then set it back to false.",
        reference_path.display()
    );
}

/// Guard: UPDATE_REFERENCE must not be committed as true.
#[test]
fn test_update_reference_is_false() {
    assert!(
        !UPDATE_REFERENCE,
        "UPDATE_REFERENCE must be false in committed code"
    );
}
