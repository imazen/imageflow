//! Quality Profile Perceptual Validation Tests
//!
//! Tests that the quality profile system (qp=low, qp=medium, qp=high) with DPR adjustment
//! (qp.dpr=) produces consistent perceptual quality across formats and DPR values.
//!
//! Querystring parameters used:
//! - `qp=<profile>` - Quality profile: lowest|low|mediumlow|medium|good|high|highest|lossless|<number>
//! - `qp.dpr=<number>` - Device pixel ratio hint for quality adjustment
//! - `format=<fmt>` - Output format: jpg|webp|png|gif|avif
//! - `width=<px>` - Target width in pixels
//! - `down.filter=<filter>` - Downscaling filter: lanczos|mitchell|etc.
//!
//! Theory:
//! - At higher DPR (e.g., 4x), images have more pixels per CSS pixel, so we can
//!   encode at lower quality without visible artifacts (they're subpixel-sized).
//! - At lower DPR (e.g., 1x), each encoded pixel maps to multiple screen pixels
//!   when upscaled by the browser, so we need higher quality.
//!
//! Methodology:
//! 1. Create a "perfect" reference image at visual comparison size (target_1x * VISUAL_COMPARISON_DPR)
//! 2. For each test variant (source, size, prescaling, qp, dpr, format):
//!    - Resize source to (target_size * dpr) pixels
//!    - Encode with qp= and qp.dpr= parameters
//!    - Decode the encoded bytes
//!    - Resize to visual comparison size (target_1x * VISUAL_COMPARISON_DPR) using Lanczos
//!    - DSSIM compare to reference at this consistent visual density
//!
//! Visual comparison density: Using a constant comparison size (e.g., 3x) ensures that
//! quality differences are measured at a resolution where they're perceptually significant.
//! Lower DPR images get upscaled, higher DPR images get downscaled to this common size.
//!
//! Expected outcome: For a given quality profile, the DSSIM should be relatively
//! consistent across DPR values, validating that the DPR adjustment algorithm
//! correctly compensates for pixel density.

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate imageflow_core;
extern crate imageflow_helpers as hlp;
extern crate imageflow_types as s;
extern crate serde_json;

pub mod common;

use crate::common::*;
use imageflow_core::{Context, FlowError};
use s::{CommandStringKind, Node, ResponsePayload};

//=============================================================================
// Test Configuration: Source Images
//=============================================================================

/// A source image to test against
#[derive(Debug, Clone)]
struct SourceImage {
    name: &'static str,
    url: &'static str,
}

// Ordered by resolution, smallest to largest
const SOURCE_IMAGES: &[SourceImage] = &[
    // ~480,000 px (800x600)
    SourceImage {
        name: "roof_test",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/roof_test_800x600.jpg",
    },
    // ~640,000 px (800x800)
    SourceImage {
        name: "mountain",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/mountain_800.png",
    },
    // ~1,044,484 px (1022x1022)
    SourceImage {
        name: "rings2",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/rings2.png",
    },
    // ~1,249,924 px (1118x1118)
    SourceImage {
        name: "frymire",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/frymire.png",
    },
    // ~4,194,304 px (2048x2048)
    SourceImage {
        name: "waterhouse",
        url: "https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/waterhouse.jpg",
    },
    // ~24,285,184 px (4928x4928)
    SourceImage {
        name: "turtleegglarge",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/turtleegglarge.jpg",
    },
    // Unknown resolution - larger images
    SourceImage {
        name: "wrenches",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/test_inputs/wrenches.jpg",
    },
    SourceImage {
        name: "nightshot",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/reference_image_originals/nightshot_iso_100.jpg",
    },
    SourceImage {
        name: "vgl_5674",
        url: "https://imageflow-resources.s3.us-west-2.amazonaws.com/reference_image_originals/vgl_5674_0098.jpg",
    },
];

//=============================================================================
// Test Configuration: Encoding Parameters
//=============================================================================

/// Target widths at 1x DPR to test (actual encoded width = target_width * dpr)
const TARGET_WIDTHS_1X: &[u32] = &[200, 400, 800];

/// Quality profiles to test (used with qp= parameter)
/// Named profiles: lowest|low|mediumlow|medium|good|high|highest|lossless
/// Numeric profiles: 0-100 (percentage)
const QUALITY_PROFILES: &[&str] = &[
    "lowest",    // ~15% perceptual quality
    "low",       // ~20%
    "mediumlow", // ~34%
    "medium",    // ~55%
    "good",      // ~73% (alias: mediumhigh)
    "high",      // ~91%
    "highest",   // ~96%
    "lossless",  // ~100%
    "25",        // Numeric: 25%
    "50",        // Numeric: 50%
    "75",        // Numeric: 75%
];

/// DPR values to test (used with qp.dpr= parameter)
/// These simulate different device pixel ratios
const DPR_VALUES: &[f32] = &[1.0, 1.5, 2.0, 3.0, 3.875, 4.0];

/// Visual comparison density multiplier
/// All DSSIM comparisons are performed at (target_width_1x * VISUAL_COMPARISON_DPR) resolution.
/// This simulates viewing the image at a consistent pixel density regardless of the encoded DPR.
/// Using 3x means we compare at "retina" resolution where quality differences are more visible.
const VISUAL_COMPARISON_DPR: f32 = 3.0;

/// Output formats to test (used with format= parameter)
/// Note: PNG excluded - it's lossless so qp.dpr has no effect
const FORMATS: &[&str] = &["jpg", "webp"];

//=============================================================================
// Test Configuration: Pre-scaling (resize filter and sharpening)
//=============================================================================

/// Pre-scaling configuration: commands applied during encoding
#[derive(Debug, Clone)]
struct PreScalingConfig {
    /// Name of this configuration
    name: &'static str,
    /// Commands applied during encoding (e.g., sharpening, filter selection)
    encode_commands: &'static str,
}

const PRESCALING_CONFIGS: &[PreScalingConfig] = &[
    PreScalingConfig { name: "baseline", encode_commands: "" },
    PreScalingConfig {
        name: "sharp-mitchell",
        encode_commands: "f.sharpen=15&f.sharpen_when=downscaling&down.filter=mitchell",
    },
];

//=============================================================================
// Test Variant: describes a single test combination
//=============================================================================

/// A single test variant describing all parameters for one encode-decode-compare cycle
#[derive(Debug, Clone)]
struct TestVariant<'a> {
    /// Source image reference
    source_image: &'a SourceImage,
    /// Target width at 1x DPR (actual encoded width = target_width_1x * dpr)
    target_width_1x: u32,
    /// Pre-scaling configuration
    prescaling: &'a PreScalingConfig,
    /// Quality profile name (qp= parameter value)
    quality_profile: &'a str,
    /// Device pixel ratio (qp.dpr= parameter value)
    dpr: f32,
    /// Output format (format= parameter value)
    format: &'a str,
}

impl<'a> TestVariant<'a> {
    /// Build the encode command string using individual querystring parameters
    /// Format: width=X&dpr=D&format=Y&qp=Z&qp.dpr=W[&extra_commands]
    /// Let imageflow compute the actual encoded width from width * dpr
    fn build_encode_command(&self) -> String {
        let mut cmd = format!(
            "width={}&dpr={}&format={}&qp={}&qp.dpr={}",
            self.target_width_1x, self.dpr, self.format, self.quality_profile, self.dpr
        );

        // Add prescaling encode commands if any
        if !self.prescaling.encode_commands.is_empty() {
            cmd.push('&');
            cmd.push_str(self.prescaling.encode_commands);
        }

        cmd
    }

    /// Get the expected encoded width in pixels
    fn encoded_width(&self) -> u32 {
        (self.target_width_1x as f32 * self.dpr) as u32
    }

    /// Create a descriptive string for error messages
    fn description(&self) -> String {
        format!(
            "image={}, width={}@{}x, prescaling={}, qp={}, format={}",
            self.source_image.name,
            self.target_width_1x,
            self.dpr,
            self.prescaling.name,
            self.quality_profile,
            self.format
        )
    }
}

//=============================================================================
// Helper Functions
//=============================================================================

/// Map format parameter to expected MIME type
fn expected_mime_type(format: &str) -> &'static str {
    match format {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "png" => "image/png",
        "gif" => "image/gif",
        "avif" => "image/avif",
        _ => "unknown",
    }
}

/// Map format parameter to expected extension (unused but kept for reference)
#[allow(dead_code)]
fn expected_extension(format: &str) -> &'static str {
    match format {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "png" => "png",
        "gif" => "gif",
        "avif" => "avif",
        _ => "unknown",
    }
}

/// Get source image dimensions by decoding just enough to read the header
fn get_image_dimensions(source_bytes: &[u8]) -> Result<(u32, u32), FlowError> {
    let mut context = Context::create().map_err(|e| e.at(here!()))?;
    context.add_input_vector(0, source_bytes.to_vec()).map_err(|e| e.at(here!()))?;

    let response = context
        .execute_1(s::Execute001 {
            security: None,
            graph_recording: None,
            framewise: s::Framewise::Steps(vec![s::Node::Decode { io_id: 0, commands: None }]),
        })
        .map_err(|e| e.at(here!()))?;

    match response {
        ResponsePayload::JobResult(job) | ResponsePayload::BuildResult(job) => {
            if let Some(decode) = job.decodes.first() {
                Ok((decode.w as u32, decode.h as u32))
            } else {
                Err(nerror!(imageflow_core::ErrorKind::InternalError, "No decode result"))
            }
        }
        _ => Err(nerror!(imageflow_core::ErrorKind::InternalError, "Unexpected response type")),
    }
}

/// A single test result
#[derive(Debug, Clone)]
struct QualityTestResult {
    source_image: String,
    target_width_1x: u32,
    prescaling: String,
    quality_profile: String,
    dpr: f32,
    format: String,
    dssim: f64,
    file_size: usize,
    /// Expected encoded width (target_width_1x * dpr)
    expected_encoded_width: u32,
    /// Actual width of the encoded image (from encoder response)
    actual_encoded_width: u32,
    /// Actual height of the encoded image (from encoder response)
    actual_encoded_height: u32,
    /// True if actual dimensions match expected (within bounds)
    dimensions_match: bool,
    /// Validated: the encoder reported this MIME type
    encoded_mime_type: String,
    /// Validated: the decoder reported this MIME type when re-reading
    decoded_mime_type: String,
}

/// Create the reference bitmap at visual comparison size (target_width_1x * VISUAL_COMPARISON_DPR)
/// Returns (context, bitmap_key, width, height)
fn create_reference_context(
    source_bytes: &[u8],
    _prescaling: &PreScalingConfig,
    target_width_1x: u32,
) -> Result<(Box<Context>, imageflow_core::BitmapKey, u32, u32), FlowError> {
    let mut context = Context::create().map_err(|e| e.at(here!()))?;
    context.add_input_vector(0, source_bytes.to_vec()).map_err(|e| e.at(here!()))?;

    let mut result_bitmap = BitmapBgraContainer::empty();

    // Reference is created at visual comparison size (target_width * VISUAL_COMPARISON_DPR)
    let comparison_width = (target_width_1x as f32 * VISUAL_COMPARISON_DPR) as u32;

    // Use explicit Decode + Constrain with Lanczos for highest quality reference (no sharpening)
    context
        .execute_1(s::Execute001 {
            security: None,
            graph_recording: None,
            framewise: s::Framewise::Steps(vec![
                Node::Decode { io_id: 0, commands: None },
                Node::Constrain(s::Constraint {
                    mode: s::ConstraintMode::Within,
                    w: Some(comparison_width),
                    h: None,
                    hints: Some(s::ResampleHints {
                        sharpen_percent: Some(0.0),
                        down_filter: Some(s::Filter::Lanczos),
                        up_filter: Some(s::Filter::Lanczos),
                        scaling_colorspace: None,
                        background_color: None,
                        resample_when: None,
                        sharpen_when: None,
                    }),
                    gravity: None,
                    canvas_color: None,
                }),
                unsafe { result_bitmap.as_mut().get_node() },
            ]),
        })
        .map_err(|e| e.at(here!()))?;

    let key = unsafe { result_bitmap.bitmap_key(&context) }.ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "Failed to get reference bitmap")
    })?;

    // Get the actual dimensions of the reference bitmap
    let bitmaps = context.borrow_bitmaps().map_err(|e| e.at(here!()))?;
    let bitmap_ref = bitmaps.try_borrow_mut(key).map_err(|e| e.at(here!()))?;
    let ref_width = bitmap_ref.w();
    let ref_height = bitmap_ref.h();
    drop(bitmap_ref);
    drop(bitmaps);

    Ok((context, key, ref_width, ref_height))
}

/// Run a single encode-decode-compare cycle for a test variant
fn run_test_variant(
    source_bytes: &[u8],
    variant: &TestVariant,
    reference_context: &Context,
    reference_key: imageflow_core::BitmapKey,
    ref_width: u32,
    ref_height: u32,
) -> Result<QualityTestResult, FlowError> {
    // Step 1: Encode using individual querystring parameters (qp=, qp.dpr=, format=, width=)
    let mut encode_context = Context::create().map_err(|e| e.at(here!()))?;
    encode_context.add_input_vector(0, source_bytes.to_vec()).map_err(|e| e.at(here!()))?;
    encode_context.add_output_buffer(1).map_err(|e| e.at(here!()))?;

    let encode_command = variant.build_encode_command();

    let encode_response = encode_context
        .execute_1(s::Execute001 {
            security: None,
            graph_recording: None,
            framewise: s::Framewise::Steps(vec![Node::CommandString {
                kind: CommandStringKind::ImageResizer4,
                value: encode_command.clone(),
                decode: Some(0),
                encode: Some(1),
                watermarks: None,
            }]),
        })
        .map_err(|e| e.at(here!()))?;

    // Extract encoder results: MIME type and actual dimensions
    let (encoded_mime_type, actual_encoded_width, actual_encoded_height) = match &encode_response {
        ResponsePayload::JobResult(job) | ResponsePayload::BuildResult(job) => {
            if let Some(enc) = job.encodes.first() {
                (enc.preferred_mime_type.clone(), enc.w as u32, enc.h as u32)
            } else {
                ("no_encode_result".to_string(), 0, 0)
            }
        }
        _ => ("unexpected_response".to_string(), 0, 0),
    };

    let expected_mime = expected_mime_type(variant.format);
    if encoded_mime_type != expected_mime {
        return Err(nerror!(
            imageflow_core::ErrorKind::InvalidArgument,
            "Encode format mismatch: expected '{}' but encoder reported '{}' for command: {}",
            expected_mime,
            encoded_mime_type,
            encode_command
        ));
    }

    // Check if actual dimensions match expected
    let expected_encoded_width = variant.encoded_width();
    let dimensions_match = actual_encoded_width == expected_encoded_width;

    let encoded_bytes = encode_context.get_output_buffer_slice(1).map_err(|e| e.at(here!()))?;
    let file_size = encoded_bytes.len();
    let encoded_bytes_vec = encoded_bytes.to_vec();

    // Step 2: Decode and resize to visual comparison dimensions using Lanczos
    let mut decode_context = Context::create().map_err(|e| e.at(here!()))?;
    // Enable AVIF decoder when feature is available (bad-avif-decoder is for testing only)
    #[cfg(feature = "bad-avif-decoder")]
    decode_context.enabled_codecs.enable_bad_avif_decoder();
    decode_context.add_input_vector(0, encoded_bytes_vec).map_err(|e| e.at(here!()))?;

    let mut result_bitmap = BitmapBgraContainer::empty();

    // Use explicit Decode + Resample2D nodes for precise control over resize
    let decode_response = decode_context
        .execute_1(s::Execute001 {
            security: None,
            graph_recording: None,
            framewise: s::Framewise::Steps(vec![
                Node::Decode { io_id: 0, commands: None },
                Node::Resample2D {
                    w: ref_width,
                    h: ref_height,
                    hints: Some(s::ResampleHints {
                        sharpen_percent: Some(0.0),
                        down_filter: Some(s::Filter::Lanczos),
                        up_filter: Some(s::Filter::Lanczos),
                        scaling_colorspace: None,
                        background_color: None,
                        resample_when: None,
                        sharpen_when: None,
                    }),
                },
                unsafe { result_bitmap.as_mut().get_node() },
            ]),
        })
        .map_err(|e| e.at(here!()))?;

    // Validate decoder reported the correct format (should match what we encoded)
    let decoded_mime_type = match &decode_response {
        ResponsePayload::JobResult(job) | ResponsePayload::BuildResult(job) => job
            .decodes
            .first()
            .map(|d| d.preferred_mime_type.clone())
            .unwrap_or_else(|| "no_decode_result".to_string()),
        _ => "unexpected_response".to_string(),
    };

    if decoded_mime_type != expected_mime {
        return Err(nerror!(
            imageflow_core::ErrorKind::InvalidArgument,
            "Decode format mismatch: expected '{}' but decoder reported '{}' for format '{}'",
            expected_mime,
            decoded_mime_type,
            variant.format
        ));
    }

    let result_key = unsafe { result_bitmap.bitmap_key(&decode_context) }.ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "Failed to get result bitmap")
    })?;

    // Step 3: DSSIM compare using existing infrastructure
    let ctx = ChecksumCtx::visuals();

    // Get mutable windows for both bitmaps
    let result_bitmaps = decode_context.borrow_bitmaps().map_err(|e| e.at(here!()))?;
    let mut result_bitmap_ref =
        result_bitmaps.try_borrow_mut(result_key).map_err(|e| e.at(here!()))?;
    let mut result_window = result_bitmap_ref.get_window_u8().ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "Failed to get result window")
    })?;

    let ref_bitmaps = reference_context.borrow_bitmaps().map_err(|e| e.at(here!()))?;
    let mut ref_bitmap_ref =
        ref_bitmaps.try_borrow_mut(reference_key).map_err(|e| e.at(here!()))?;
    let mut ref_window = ref_bitmap_ref.get_window_u8().ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "Failed to get reference window")
    })?;

    // Use very loose bounds to always get the dssim value
    let compare_result = compare_bitmaps(
        &ctx,
        &mut result_window,
        &mut ref_window,
        Similarity::AllowDssimMatch(0.0, 10.0),
        false,
    );

    let dssim = compare_result.dssim.unwrap_or(0.0);

    Ok(QualityTestResult {
        source_image: variant.source_image.name.to_string(),
        target_width_1x: variant.target_width_1x,
        prescaling: variant.prescaling.name.to_string(),
        quality_profile: variant.quality_profile.to_string(),
        dpr: variant.dpr,
        format: variant.format.to_string(),
        dssim,
        file_size,
        expected_encoded_width,
        actual_encoded_width,
        actual_encoded_height,
        dimensions_match,
        encoded_mime_type,
        decoded_mime_type,
    })
}

#[test]
fn test_quality_profiles_dpr_consistency() {
    let mut results: Vec<QualityTestResult> = Vec::new();
    let mut skipped_count = 0;

    // Test all source images
    for source_image in SOURCE_IMAGES {
        println!("\nLoading source image: {}", source_image.name);
        let source_bytes = match get_url_bytes_with_retry(source_image.url) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Failed to fetch {}: {:?}", source_image.name, e);
                continue;
            }
        };

        // Get source image dimensions to skip upscaling scenarios
        let (source_width, _source_height) = match get_image_dimensions(&source_bytes) {
            Ok(dims) => dims,
            Err(e) => {
                eprintln!("Failed to get dimensions for {}: {:?}", source_image.name, e);
                continue;
            }
        };
        println!("  Source dimensions: {}x...", source_width);

        // Test all target widths
        for &target_width_1x in TARGET_WIDTHS_1X {
            // Test all prescaling configs
            for prescaling in PRESCALING_CONFIGS {
                // Create reference for this image + prescaling + target size combination
                let (reference_context, reference_key, ref_width, ref_height) =
                    match create_reference_context(&source_bytes, prescaling, target_width_1x) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!(
                                "Failed to create reference for {} at {}px: {:?}",
                                source_image.name, target_width_1x, e
                            );
                            continue;
                        }
                    };

                for &qp in QUALITY_PROFILES {
                    for &dpr in DPR_VALUES {
                        let encoded_width = (target_width_1x as f32 * dpr) as u32;

                        // Skip if encoded width would exceed source width (not a downscale)
                        if encoded_width > source_width {
                            skipped_count += 1;
                            continue;
                        }

                        for &format in FORMATS {
                            let variant = TestVariant {
                                source_image,
                                target_width_1x,
                                prescaling,
                                quality_profile: qp,
                                dpr,
                                format,
                            };

                            match run_test_variant(
                                &source_bytes,
                                &variant,
                                &reference_context,
                                reference_key,
                                ref_width,
                                ref_height,
                            ) {
                                Ok(result) => {
                                    results.push(result);
                                }
                                Err(e) => {
                                    eprintln!("Failed for {}: {:?}", variant.description(), e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Separate results into in-bounds and out-of-bounds
    let in_bounds: Vec<_> = results.iter().filter(|r| r.dimensions_match).collect();
    let out_of_bounds: Vec<_> = results.iter().filter(|r| !r.dimensions_match).collect();

    println!("\nSkipped {} combinations (would require upscaling)", skipped_count);
    println!("In-bounds results: {}", in_bounds.len());
    println!("Out-of-bounds results: {} (dimensions didn't match expected)", out_of_bounds.len());

    // Print out-of-bounds results first (these need attention)
    if !out_of_bounds.is_empty() {
        println!("\n=== OUT OF BOUNDS RESULTS (dimensions mismatch) ===\n");
        println!(
            "{:<12} {:<6} {:<6} {:<8} {:<8} {:<10} {:<10}",
            "Image", "Width", "DPR", "Format", "Expected", "Actual", "DSSIM"
        );
        println!("{}", "-".repeat(75));

        for result in &out_of_bounds {
            println!(
                "{:<12} {:<6} {:<6.2} {:<8} {:<10} {:<10} {:<12.6}",
                result.source_image,
                result.target_width_1x,
                result.dpr,
                result.format,
                result.expected_encoded_width,
                result.actual_encoded_width,
                result.dssim
            );
        }
    }

    // Print in-bounds results grouped by source image, target width, and prescaling config
    println!("\n=== IN-BOUNDS RESULTS ===");
    for source_image in SOURCE_IMAGES {
        for &target_width_1x in TARGET_WIDTHS_1X {
            for prescaling in PRESCALING_CONFIGS {
                let filtered: Vec<_> = in_bounds
                    .iter()
                    .filter(|r| {
                        r.source_image == source_image.name
                            && r.target_width_1x == target_width_1x
                            && r.prescaling == prescaling.name
                    })
                    .collect();

                if filtered.is_empty() {
                    continue;
                }

                println!(
                    "\n=== {} @ {}px [prescaling: {}] ===\n",
                    source_image.name, target_width_1x, prescaling.name
                );
                println!(
                    "{:<10} {:<6} {:<8} {:<12} {:<12} {:<10}",
                    "QP", "DPR", "Format", "DSSIM", "File Size", "Enc Width"
                );
                println!("{}", "-".repeat(70));

                for result in &filtered {
                    println!(
                        "{:<10} {:<6.1} {:<8} {:<12.6} {:<12} {:<10}",
                        result.quality_profile,
                        result.dpr,
                        result.format,
                        result.dssim,
                        result.file_size,
                        result.actual_encoded_width
                    );
                }
            }
        }
    }

    // Analyze IN-BOUNDS results only: for each prescaling + quality profile, check DSSIM consistency
    println!("\n=== Analysis by Prescaling + Quality Profile (in-bounds only) ===\n");

    for prescaling in PRESCALING_CONFIGS {
        println!("--- Prescaling: {} ---", prescaling.name);
        for &qp in QUALITY_PROFILES {
            for &format in FORMATS {
                let qp_results: Vec<_> = in_bounds
                    .iter()
                    .filter(|r| {
                        r.prescaling == prescaling.name
                            && r.quality_profile == qp
                            && r.format == format
                    })
                    .collect();

                if qp_results.is_empty() {
                    continue;
                }

                let dssim_values: Vec<f64> = qp_results.iter().map(|r| r.dssim).collect();
                let min_dssim = dssim_values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_dssim = dssim_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let mean_dssim: f64 = dssim_values.iter().sum::<f64>() / dssim_values.len() as f64;
                let spread = max_dssim - min_dssim;
                let relative_spread =
                    if mean_dssim > 0.0 { spread / mean_dssim * 100.0 } else { 0.0 };

                println!(
                    "QP={:<8} Format={:<6}: mean={:.6}, spread={:.6} ({:.1}% relative)",
                    qp, format, mean_dssim, spread, relative_spread
                );
            }
        }
        println!();
    }

    // Compare prescaling configs for same quality profiles (in-bounds only)
    println!("\n=== Prescaling Comparison (same QP, format, in-bounds only) ===\n");
    println!(
        "{:<16} {:<10} {:<8} {:<12} {:<12}",
        "Prescaling", "QP", "Format", "Mean DSSIM", "Mean Size"
    );
    println!("{}", "-".repeat(60));

    for &qp in &["medium", "high"] {
        for &format in FORMATS {
            for prescaling in PRESCALING_CONFIGS {
                let filtered: Vec<_> = in_bounds
                    .iter()
                    .filter(|r| {
                        r.prescaling == prescaling.name
                            && r.quality_profile == qp
                            && r.format == format
                    })
                    .collect();

                if filtered.is_empty() {
                    continue;
                }

                let mean_dssim: f64 =
                    filtered.iter().map(|r| r.dssim).sum::<f64>() / filtered.len() as f64;
                let mean_size: f64 = filtered.iter().map(|r| r.file_size as f64).sum::<f64>()
                    / filtered.len() as f64;

                println!(
                    "{:<16} {:<10} {:<8} {:<12.6} {:<12.0}",
                    prescaling.name, qp, format, mean_dssim, mean_size
                );
            }
        }
    }

    // Verify that results were generated
    assert!(!results.is_empty(), "Should have generated test results");

    // Print summary statistics
    println!("\n=== Summary ===");
    println!("Total test combinations: {}", results.len());
    println!("  In-bounds: {}", in_bounds.len());
    println!("  Out-of-bounds: {}", out_of_bounds.len());

    // Check that higher quality profiles have lower DSSIM (closer to reference) - in-bounds only
    let low_count = in_bounds.iter().filter(|r| r.quality_profile == "low").count();
    let high_count = in_bounds.iter().filter(|r| r.quality_profile == "high").count();

    if low_count > 0 && high_count > 0 {
        let low_mean: f64 =
            in_bounds.iter().filter(|r| r.quality_profile == "low").map(|r| r.dssim).sum::<f64>()
                / low_count as f64;

        let high_mean: f64 =
            in_bounds.iter().filter(|r| r.quality_profile == "high").map(|r| r.dssim).sum::<f64>()
                / high_count as f64;

        println!("Mean DSSIM for 'low' quality (in-bounds): {:.6}", low_mean);
        println!("Mean DSSIM for 'high' quality (in-bounds): {:.6}", high_mean);

        // Sanity check: high quality should have lower DSSIM than low quality
        assert!(
            high_mean < low_mean,
            "High quality profile should have lower DSSIM (closer to reference) than low quality. High={:.6}, Low={:.6}",
            high_mean,
            low_mean
        );
    } else {
        println!("Not enough in-bounds results to compare quality profiles");
    }
}

/// More focused test on DPR adjustment effectiveness
/// This test compares the DSSIM spread WITH vs WITHOUT qp.dpr= adjustment
#[test]
fn test_dpr_adjustment_reduces_dssim_variance() {
    // Use the first source image (waterhouse) for this focused test
    let source_image = &SOURCE_IMAGES[0];
    let target_width_1x = 400;

    let source_bytes =
        get_url_bytes_with_retry(source_image.url).expect("Failed to fetch source image");

    // Get native source dimensions
    let (source_width, source_height) =
        get_image_dimensions(&source_bytes).expect("Failed to get source dimensions");

    let baseline = &PRESCALING_CONFIGS[0]; // Use baseline prescaling
    let (reference_context, reference_key, ref_width, ref_height) =
        create_reference_context(&source_bytes, baseline, target_width_1x)
            .expect("Failed to create reference bitmap");

    // Report image info
    println!("\n=== Image Info ===");
    println!("Source: {} (native: {}x{})", source_image.name, source_width, source_height);
    println!(
        "1x target: {}px, visual comparison at {}x = {}x{}",
        target_width_1x, VISUAL_COMPARISON_DPR, ref_width, ref_height
    );

    // Use the full constants for testing
    let test_qps = QUALITY_PROFILES;
    let test_formats = FORMATS;
    let test_dprs = DPR_VALUES;

    // Helper to calculate bpp
    fn calc_bpp(file_size: usize, width: u32, height: u32) -> f64 {
        if width == 0 || height == 0 {
            return 0.0;
        }
        (file_size as f64 * 8.0) / (width as f64 * height as f64)
    }

    println!("\n=== Quality Profile Results (with qp.dpr) ===\n");
    println!(
        "{:<10} {:<8} {:<12} {:<12} {:<10}",
        "QP", "Format", "DSSIM Spread", "Mean DSSIM", "Mean BPP"
    );
    println!("{}", "-".repeat(60));

    for &qp in test_qps {
        for &format in test_formats {
            let mut results: Vec<(f32, f64, usize, u32, u32, bool)> = Vec::new();

            for &dpr in test_dprs {
                let variant = TestVariant {
                    source_image,
                    target_width_1x,
                    prescaling: baseline,
                    quality_profile: qp,
                    dpr,
                    format,
                };

                if let Ok(result) = run_test_variant(
                    &source_bytes,
                    &variant,
                    &reference_context,
                    reference_key,
                    ref_width,
                    ref_height,
                ) {
                    results.push((
                        dpr,
                        result.dssim,
                        result.file_size,
                        result.actual_encoded_width,
                        result.actual_encoded_height,
                        result.dimensions_match,
                    ));
                }
            }

            // Calculate stats
            let in_bounds: Vec<_> = results.iter().filter(|(_, _, _, _, _, m)| *m).collect();
            if in_bounds.is_empty() {
                continue;
            }
            let dssims: Vec<f64> = in_bounds.iter().map(|(_, d, _, _, _, _)| *d).collect();
            let min_dssim = dssims.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_dssim = dssims.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let mean_dssim: f64 = dssims.iter().sum::<f64>() / dssims.len() as f64;
            let spread = (max_dssim - min_dssim) / mean_dssim * 100.0;

            let mean_bpp: f64 =
                in_bounds.iter().map(|(_, _, size, w, h, _)| calc_bpp(*size, *w, *h)).sum::<f64>()
                    / in_bounds.len() as f64;

            println!(
                "{:<10} {:<8} {:<12.1}% {:<12.6} {:<10.2}",
                qp, format, spread, mean_dssim, mean_bpp
            );
        }
    }

    // Detailed output for all qp/format combinations
    for &qp in test_qps {
        for &format in test_formats {
            let mut results: Vec<(f32, f64, usize, u32, u32, bool)> = Vec::new();

            for &dpr in test_dprs {
                let variant = TestVariant {
                    source_image,
                    target_width_1x,
                    prescaling: baseline,
                    quality_profile: qp,
                    dpr,
                    format,
                };

                if let Ok(result) = run_test_variant(
                    &source_bytes,
                    &variant,
                    &reference_context,
                    reference_key,
                    ref_width,
                    ref_height,
                ) {
                    results.push((
                        dpr,
                        result.dssim,
                        result.file_size,
                        result.actual_encoded_width,
                        result.actual_encoded_height,
                        result.dimensions_match,
                    ));
                }
            }

            println!("\n=== {} / {} ===\n", qp, format);
            println!(
                "{:<6} {:<12} {:<10} {:<8} {:<10} {:<10}",
                "DPR", "DSSIM", "Size", "BPP", "Dims", "In-Bounds"
            );
            for (dpr, dssim, size, width, height, matched) in &results {
                let bpp = calc_bpp(*size, *width, *height);
                let dims = format!("{}x{}", width, height);
                let bounds = if *matched { "yes" } else { "NO" };
                println!(
                    "{:<6.2} {:<12.6} {:<10} {:<8.2} {:<10} {:<10}",
                    dpr, dssim, size, bpp, dims, bounds
                );
            }
        }
    }
}

/// Full AVIF calibration test - find optimal QP adjustments for each DPR
/// Requires `--features bad-avif-decoder` to enable AVIF round-trip testing
#[test]
#[ignore] // AVIF encoding is slow, run explicitly with --ignored
#[cfg(feature = "bad-avif-decoder")]
fn test_quality_profiles_avif() {
    // Use the first source image (waterhouse) for this focused test
    let source_image = &SOURCE_IMAGES[0];
    let target_width_1x = 400;

    let source_bytes =
        get_url_bytes_with_retry(source_image.url).expect("Failed to fetch source image");
    let baseline = &PRESCALING_CONFIGS[0]; // Use baseline prescaling
    let (reference_context, reference_key, ref_width, ref_height) =
        create_reference_context(&source_bytes, baseline, target_width_1x)
            .expect("Failed to create reference bitmap");

    println!("\n=== AVIF Quality Profile Test ===\n");
    println!("{:<10} {:<6} {:<12} {:<12}", "QP", "DPR", "DSSIM", "File Size");
    println!("{}", "-".repeat(50));

    for &qp in &["medium", "high"] {
        for &dpr in &[1.0_f32, 2.0, 4.0] {
            let variant = TestVariant {
                source_image,
                target_width_1x,
                prescaling: baseline,
                quality_profile: qp,
                dpr,
                format: "avif",
            };

            match run_test_variant(
                &source_bytes,
                &variant,
                &reference_context,
                reference_key,
                ref_width,
                ref_height,
            ) {
                Ok(result) => {
                    println!(
                        "{:<10} {:<6.1} {:<12.6} {:<12}",
                        result.quality_profile, result.dpr, result.dssim, result.file_size
                    );
                }
                Err(e) => {
                    eprintln!("Failed for qp={}, dpr={}, format=avif: {:?}", qp, dpr, e);
                }
            }
        }
    }
}

/// Helper function to print QP mapping and adjustment factor tables
fn print_qp_tables(
    title: &str,
    dssim_cache: &std::collections::HashMap<(u32, usize), f64>,
    dpr_values: &[f32],
    reference_qps: &[u32],
    qp_values: &[u32],
    ref_dpr_idx: usize,
) {
    println!("\n--- {} ---", title);
    println!("\nOptimal QP Mapping (QP at DPR -> adjusted QP to match DPR={} quality)\n",
        dpr_values[ref_dpr_idx]);

    print!("{:<6}", "QP\\DPR");
    for &dpr in dpr_values {
        print!(" {:>5.1}", dpr);
    }
    println!();
    println!("{}", "-".repeat(6 + dpr_values.len() * 6));

    let mut best_qps: std::collections::HashMap<(u32, usize), u32> = std::collections::HashMap::new();

    for &ref_qp in reference_qps {
        let target_dssim = match dssim_cache.get(&(ref_qp, ref_dpr_idx)) {
            Some(&d) => d,
            None => {
                println!("{:<6} (skipped - no data)", ref_qp);
                continue;
            }
        };

        print!("{:<6}", ref_qp);

        for (dpr_idx, &_dpr) in dpr_values.iter().enumerate() {
            if dpr_idx == ref_dpr_idx {
                print!(" {:>5}", ref_qp);
                best_qps.insert((ref_qp, dpr_idx), ref_qp);
                continue;
            }

            // Check if we have any data for this DPR
            let has_data = qp_values.iter().any(|&qp| dssim_cache.contains_key(&(qp, dpr_idx)));
            if !has_data {
                print!(" {:>5}", "-");
                continue;
            }

            // Find qp with closest DSSIM to target
            let mut best_qp = ref_qp;
            let mut best_diff = f64::INFINITY;

            for &qp in qp_values {
                if let Some(&dssim) = dssim_cache.get(&(qp, dpr_idx)) {
                    let diff = (dssim - target_dssim).abs();
                    if diff < best_diff {
                        best_diff = diff;
                        best_qp = qp;
                    }
                }
            }

            print!(" {:>5}", best_qp);
            best_qps.insert((ref_qp, dpr_idx), best_qp);
        }
        println!();
    }

    // Print adjustment factors
    println!("\nAdjustment Factors (adjusted_qp / original_qp)\n");
    print!("{:<6}", "QP\\DPR");
    for &dpr in dpr_values {
        print!(" {:>5.1}", dpr);
    }
    println!();
    println!("{}", "-".repeat(6 + dpr_values.len() * 6));

    for &ref_qp in reference_qps {
        if !dssim_cache.contains_key(&(ref_qp, ref_dpr_idx)) {
            continue;
        }

        print!("{:<6}", ref_qp);

        for (dpr_idx, _) in dpr_values.iter().enumerate() {
            if let Some(&best_qp) = best_qps.get(&(ref_qp, dpr_idx)) {
                let factor = best_qp as f64 / ref_qp as f64;
                print!(" {:>5.2}", factor);
            } else {
                print!(" {:>5}", "-");
            }
        }
        println!();
    }
}

/// Result of encoding with a specific configuration
#[derive(Debug, Clone)]
struct EncodeResult {
    dssim: f64,
    file_size: usize,
}

/// Encode an image and measure DSSIM against a reference bitmap
fn encode_and_measure(
    source_bytes: &[u8],
    encode_command: &str,
    ref_width: u32,
    ref_height: u32,
) -> Option<EncodeResult> {
    use imageflow_core::Context;

    let mut encode_context = Context::create().ok()?;
    encode_context.add_input_vector(0, source_bytes.to_vec()).ok()?;
    encode_context.add_output_buffer(1).ok()?;

    encode_context.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![Node::CommandString {
            kind: CommandStringKind::ImageResizer4,
            value: encode_command.to_string(),
            decode: Some(0),
            encode: Some(1),
            watermarks: None,
        }]),
    }).ok()?;

    let encoded_bytes = encode_context.get_output_buffer_slice(1).ok()?.to_vec();
    let file_size = encoded_bytes.len();

    // Decode and resize to comparison size
    let mut decode_context = Context::create().ok()?;
    #[cfg(feature = "bad-avif-decoder")]
    decode_context.enabled_codecs.enable_bad_avif_decoder();
    decode_context.add_input_vector(0, encoded_bytes).ok()?;

    let mut result_bitmap = BitmapBgraContainer::empty();

    decode_context.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: ref_width,
                h: ref_height,
                hints: Some(s::ResampleHints {
                    sharpen_percent: Some(0.0),
                    down_filter: Some(s::Filter::Lanczos),
                    up_filter: Some(s::Filter::Lanczos),
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            },
            unsafe { result_bitmap.as_mut().get_node() },
        ]),
    }).ok()?;

    // Create reference from original source
    let mut ref_context = Context::create().ok()?;
    ref_context.add_input_vector(0, source_bytes.to_vec()).ok()?;
    let mut ref_bitmap = BitmapBgraContainer::empty();
    ref_context.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w: ref_width,
                h: ref_height,
                hints: Some(s::ResampleHints {
                    sharpen_percent: Some(0.0),
                    down_filter: Some(s::Filter::Lanczos),
                    up_filter: Some(s::Filter::Lanczos),
                    scaling_colorspace: None,
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            },
            unsafe { ref_bitmap.as_mut().get_node() },
        ]),
    }).ok()?;

    let result_key = unsafe { result_bitmap.bitmap_key(&decode_context) }?;
    let ref_key = unsafe { ref_bitmap.bitmap_key(&ref_context) }?;

    let ctx = ChecksumCtx::visuals();
    let result_bitmaps = decode_context.borrow_bitmaps().ok()?;
    let mut result_bitmap_ref = result_bitmaps.try_borrow_mut(result_key).ok()?;
    let mut result_window = result_bitmap_ref.get_window_u8()?;

    let ref_bitmaps = ref_context.borrow_bitmaps().ok()?;
    let mut ref_bitmap_ref = ref_bitmaps.try_borrow_mut(ref_key).ok()?;
    let mut ref_window = ref_bitmap_ref.get_window_u8()?;

    let compare_result = compare_bitmaps(
        &ctx,
        &mut result_window,
        &mut ref_window,
        Similarity::AllowDssimMatch(0.0, 10.0),
        false,
    );

    Some(EncodeResult {
        dssim: compare_result.dssim?,
        file_size,
    })
}

/// Get the dpr_quality_range bounds for a given QP value
/// Returns (floor_p, ceiling_p) based on the quality profile tiers
fn get_qp_bounds(qp: u32) -> (u32, u32) {
    // These match the dpr_quality_range values from QUALITY_HINTS in auto.rs
    match qp {
        0..=17 => (10, 55),    // Lowest: floor~10, ceiling=Medium
        18..=27 => (15, 73),   // Low: floor=Lowest, ceiling=Good
        28..=44 => (15, 91),   // MediumLow: floor=Lowest, ceiling=High
        45..=64 => (20, 91),   // Medium: floor=Low, ceiling=High
        65..=82 => (34, 91),   // Good: floor=MediumLow, ceiling=High
        83..=93 => (55, 96),   // High: floor=Medium, ceiling=Highest
        94..=99 => (73, 96),   // Highest: floor=Good, ceiling=Highest
        _ => (100, 100),       // Lossless: no adjustment
    }
}

/// Compare qp.dpr implementation against calibration-found optimal QP (within clamped bounds)
fn compare_qp_dpr_implementation(
    source_bytes: &[u8],
    source_name: &str,
    format: &str,
    target_width_1x: u32,
    dpr_values: &[f32],
    reference_qps: &[u32],
    ref_dpr_idx: usize,
    dssim_cache: &std::collections::HashMap<(u32, usize), f64>,
    qp_values: &[u32],
) {
    let reference_dpr = dpr_values[ref_dpr_idx];
    let ref_width = (target_width_1x as f32 * reference_dpr) as u32;
    // Approximate height based on typical aspect ratio
    let ref_height = (ref_width as f32 * 0.67) as u32;

    println!("\n=== qp.dpr Implementation Comparison: {} - {} ===", format, source_name);
    println!("Comparing: reference (DPR={}), qp.dpr adjustment, and clamped-optimal QP\n", reference_dpr);

    // Header
    println!("{:<6} {:>5} {:>5}-{:<5} | {:>10} {:>6} | {:>10} {:>6} {:>5} | {:>10} {:>6} {:>5}",
        "QP", "DPR", "floor", "ceil",
        "Ref DSSIM", "KB",
        "qp.dpr", "KB", "Δ%",
        "ClampOpt", "KB", "OptQP");
    println!("{}", "-".repeat(95));

    for &ref_qp in reference_qps {
        let (floor_qp, ceiling_qp) = get_qp_bounds(ref_qp);

        // Get reference DSSIM at DPR=3
        let ref_dssim = match dssim_cache.get(&(ref_qp, ref_dpr_idx)) {
            Some(&d) => d,
            None => continue,
        };

        // Get reference file size (encode at DPR=3 with ref_qp)
        let ref_encoded_width = (target_width_1x as f32 * reference_dpr) as u32;
        let ref_cmd = format!("width={}&format={}&qp={}", ref_encoded_width, format, ref_qp);
        let ref_result = match encode_and_measure(source_bytes, &ref_cmd, ref_width, ref_height) {
            Some(r) => r,
            None => continue,
        };

        for (dpr_idx, &dpr) in dpr_values.iter().enumerate() {
            let encoded_width = (target_width_1x as f32 * dpr) as u32;

            if dpr_idx == ref_dpr_idx {
                // Reference DPR row - show baseline (no adjustment needed)
                println!("{:<6} {:>5.1} {:>5}-{:<5} | {:>10.6} {:>6.1} | {:>10.6} {:>6.1} {:>5} | {:>10.6} {:>6.1} {:>5}  (reference)",
                    ref_qp, dpr, floor_qp, ceiling_qp,
                    ref_dssim, ref_result.file_size as f64 / 1024.0,
                    ref_dssim, ref_result.file_size as f64 / 1024.0, "  0%",
                    ref_dssim, ref_result.file_size as f64 / 1024.0, ref_qp);
                continue;
            }

            // 1. Encode with qp.dpr (the actual implementation - already clamps internally)
            let qp_dpr_cmd = format!("width={}&format={}&qp={}&qp.dpr={}",
                encoded_width, format, ref_qp, dpr);
            let qp_dpr_result = match encode_and_measure(source_bytes, &qp_dpr_cmd, ref_width, ref_height) {
                Some(r) => r,
                None => continue,
            };

            // 2. Find optimal QP from calibration WITHIN THE CLAMPED BOUNDS
            let mut best_qp = ref_qp;
            let mut best_diff = f64::INFINITY;
            for &qp in qp_values {
                // Only consider QP values within the allowed range
                if qp < floor_qp || qp > ceiling_qp {
                    continue;
                }
                if let Some(&d) = dssim_cache.get(&(qp, dpr_idx)) {
                    let diff = (d - ref_dssim).abs();
                    if diff < best_diff {
                        best_diff = diff;
                        best_qp = qp;
                    }
                }
            }

            // 3. Encode with clamped-optimal QP
            let opt_cmd = format!("width={}&format={}&qp={}", encoded_width, format, best_qp);
            let opt_result = match encode_and_measure(source_bytes, &opt_cmd, ref_width, ref_height) {
                Some(r) => r,
                None => continue,
            };

            // Calculate size difference percentage (qp.dpr vs clamped optimal)
            let size_diff_pct = ((qp_dpr_result.file_size as f64 / opt_result.file_size as f64) - 1.0) * 100.0;

            println!("{:<6} {:>5.1} {:>5}-{:<5} | {:>10.6} {:>6.1} | {:>10.6} {:>6.1} {:>+4.0}% | {:>10.6} {:>6.1} {:>5}",
                ref_qp, dpr, floor_qp, ceiling_qp,
                ref_dssim, ref_result.file_size as f64 / 1024.0,
                qp_dpr_result.dssim, qp_dpr_result.file_size as f64 / 1024.0, size_diff_pct,
                opt_result.dssim, opt_result.file_size as f64 / 1024.0, best_qp);
        }
    }
}

/// Calibration test: find the optimal qp adjustment for each DPR to match reference quality at DPR=3
///
/// This test builds a mapping table: for each (reference_qp, dpr) -> best_adjusted_qp
/// The reference point is DPR=3.0 (our visual comparison density).
///
/// Optimizations:
/// - Uses multiple source images and averages results
/// - Tests odd QP values only (1, 3, 5, ..., 99) for 2x speedup
/// - Uses 0.5 DPR intervals instead of 0.25 for 2x speedup
/// - Parallelizes across DPR values using threads
#[test]
fn test_calibrate_qp_dpr_mapping() {
    use std::collections::HashMap;
    use std::io::Write;
    use std::fs::File;
    use std::sync::{Arc, Mutex};
    use std::thread;

    let target_width_1x = 400;

    // Use multiple images for more robust calibration
    // We'll filter by width after fetching since SourceImage doesn't store dimensions
    let test_images: Vec<_> = SOURCE_IMAGES.iter()
        .take(6) // Test up to 6 images, filter by dimension after fetch
        .collect();

    println!("\n=== QP-DPR Calibration Test (Multi-threaded) ===");
    println!("Testing {} images: {}", test_images.len(),
        test_images.iter().map(|i| i.name).collect::<Vec<_>>().join(", "));

    // Reference quality values (corresponding to named profiles)
    let reference_qps: &[u32] = &[15, 21, 35, 55, 73, 91, 95]; // Use odd values close to profiles

    // DPR range: 0.5 to 5.0 in 0.5 increments
    let dpr_values: Vec<f32> = (1..=10).map(|i| i as f32 * 0.5).collect();

    // QP values to test: odd numbers only for speed
    let qp_values: Vec<u32> = (1..=99).step_by(2).collect();

    // Reference DPR (the baseline we're calibrating against)
    let reference_dpr: f32 = 3.0;
    let ref_dpr_idx = dpr_values.iter().position(|&d| (d - reference_dpr).abs() < 0.01)
        .expect("Reference DPR not in list");

    // Open log file in current directory
    let log_file = Arc::new(Mutex::new(
        File::create("qp_calibration_log.csv").expect("Failed to create log file")
    ));
    {
        let mut f = log_file.lock().unwrap();
        writeln!(f, "image,format,dpr,encoded_width,qp,dssim,file_size_bytes").ok();
    }

    println!("DPR values: {:?}", dpr_values);
    println!("Testing {} QP values (odd only)", qp_values.len());
    println!("Reference DPR: {}", reference_dpr);

    // Test each format
    for format in FORMATS {
        println!("\n=== Format: {} ===", format);

        // Track successfully processed images
        let mut images_processed = 0u32;

        // Store the last (largest) successful image for comparison
        // Images are ordered smallest to largest, so last = largest resolution
        // Tuple: (source_bytes, image_name, per_image_dssim_cache)
        let mut largest_image_for_comparison: Option<(Vec<u8>, String, HashMap<(u32, usize), f64>)> = None;

        // Process each image
        for (img_idx, source_image) in test_images.iter().enumerate() {
            println!("Processing image {}/{}: {}", img_idx + 1, test_images.len(), source_image.name);

            let source_bytes = match get_url_bytes_with_retry(source_image.url) {
                Ok(b) => b,
                Err(e) => {
                    println!("  Skipping (failed to fetch): {}", e);
                    continue;
                }
            };

            let (source_width, source_height) = match get_image_dimensions(&source_bytes) {
                Ok(dims) => dims,
                Err(e) => {
                    println!("  Skipping (failed to get dimensions): {:?}", e);
                    continue;
                }
            };

            // Adjust target_width_1x for this image based on source size
            // We want the max DPR (5.0) to still fit within source_width
            let max_dpr = dpr_values.iter().cloned().fold(0.0_f32, f32::max);
            let effective_target_1x = (source_width as f32 / max_dpr).floor() as u32;
            let effective_target_1x = effective_target_1x.min(target_width_1x).max(100); // At least 100px

            println!("  Source: {}x{}, effective 1x target: {}px",
                source_width, source_height, effective_target_1x);

            let baseline = &PRESCALING_CONFIGS[0];
            let ref_result = create_reference_context(&source_bytes, baseline, effective_target_1x);
            let (_reference_context, _reference_key, ref_width, ref_height) = match ref_result {
                Ok(r) => r,
                Err(e) => {
                    println!("  Skipping (failed to create reference): {:?}", e);
                    continue;
                }
            };

            // Build work items for parallel processing
            // Skip DPR values that would exceed source width
            let work_items: Vec<(usize, f32, u32)> = dpr_values.iter().enumerate()
                .filter(|(_, &dpr)| {
                    let encoded_width = (effective_target_1x as f32 * dpr) as u32;
                    encoded_width <= source_width
                })
                .flat_map(|(dpr_idx, &dpr)| {
                    qp_values.iter().map(move |&qp| (dpr_idx, dpr, qp))
                })
                .collect();

            let total_items = work_items.len();
            let completed = Arc::new(Mutex::new(0usize));

            // Process in parallel using thread pool
            let num_threads = std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4)
                .min(8); // Cap at 8 threads

            let chunk_size = (work_items.len() + num_threads - 1) / num_threads;
            let work_chunks: Vec<_> = work_items.chunks(chunk_size).map(|c| c.to_vec()).collect();

            // Results: (qp, dpr_idx, dssim, file_size)
            let results: Arc<Mutex<Vec<(u32, usize, f64, usize)>>> = Arc::new(Mutex::new(Vec::new()));

            let handles: Vec<_> = work_chunks.into_iter().map(|chunk| {
                let source_bytes = source_bytes.clone();
                let format = format.to_string();
                let results = Arc::clone(&results);
                let completed = Arc::clone(&completed);
                let log_file = Arc::clone(&log_file);
                let image_name = source_image.name.to_string();
                let eff_target_1x = effective_target_1x;

                thread::spawn(move || {
                    for (dpr_idx, dpr, qp) in chunk {
                        let encoded_width = (eff_target_1x as f32 * dpr) as u32;
                        let encode_command = format!(
                            "width={}&format={}&qp={}",
                            encoded_width, format, qp
                        );

                        let result = (|| -> Option<(f64, usize)> {
                            let mut encode_context = Context::create().ok()?;
                            encode_context.add_input_vector(0, source_bytes.clone()).ok()?;
                            encode_context.add_output_buffer(1).ok()?;

                            encode_context.execute_1(s::Execute001 {
                                security: None,
                                graph_recording: None,
                                framewise: s::Framewise::Steps(vec![Node::CommandString {
                                    kind: CommandStringKind::ImageResizer4,
                                    value: encode_command,
                                    decode: Some(0),
                                    encode: Some(1),
                                    watermarks: None,
                                }]),
                            }).ok()?;

                            let encoded_bytes = encode_context.get_output_buffer_slice(1).ok()?.to_vec();
                            let file_size = encoded_bytes.len();

                            // Decode and resize to comparison size
                            let mut decode_context = Context::create().ok()?;
                            decode_context.add_input_vector(0, encoded_bytes).ok()?;

                            let mut result_bitmap = BitmapBgraContainer::empty();

                            decode_context.execute_1(s::Execute001 {
                                security: None,
                                graph_recording: None,
                                framewise: s::Framewise::Steps(vec![
                                    Node::Decode { io_id: 0, commands: None },
                                    Node::Resample2D {
                                        w: ref_width,
                                        h: ref_height,
                                        hints: Some(s::ResampleHints {
                                            sharpen_percent: Some(0.0),
                                            down_filter: Some(s::Filter::Lanczos),
                                            up_filter: Some(s::Filter::Lanczos),
                                            scaling_colorspace: None,
                                            background_color: None,
                                            resample_when: None,
                                            sharpen_when: None,
                                        }),
                                    },
                                    unsafe { result_bitmap.as_mut().get_node() },
                                ]),
                            }).ok()?;

                            // Create reference for this thread
                            let mut ref_context = Context::create().ok()?;
                            ref_context.add_input_vector(0, source_bytes.clone()).ok()?;
                            let mut ref_bitmap = BitmapBgraContainer::empty();
                            ref_context.execute_1(s::Execute001 {
                                security: None,
                                graph_recording: None,
                                framewise: s::Framewise::Steps(vec![
                                    Node::Decode { io_id: 0, commands: None },
                                    Node::Resample2D {
                                        w: ref_width,
                                        h: ref_height,
                                        hints: Some(s::ResampleHints {
                                            sharpen_percent: Some(0.0),
                                            down_filter: Some(s::Filter::Lanczos),
                                            up_filter: Some(s::Filter::Lanczos),
                                            scaling_colorspace: None,
                                            background_color: None,
                                            resample_when: None,
                                            sharpen_when: None,
                                        }),
                                    },
                                    unsafe { ref_bitmap.as_mut().get_node() },
                                ]),
                            }).ok()?;

                            let result_key = unsafe { result_bitmap.bitmap_key(&decode_context) }?;
                            let ref_key = unsafe { ref_bitmap.bitmap_key(&ref_context) }?;

                            let ctx = ChecksumCtx::visuals();
                            let result_bitmaps = decode_context.borrow_bitmaps().ok()?;
                            let mut result_bitmap_ref = result_bitmaps.try_borrow_mut(result_key).ok()?;
                            let mut result_window = result_bitmap_ref.get_window_u8()?;

                            let ref_bitmaps = ref_context.borrow_bitmaps().ok()?;
                            let mut ref_bitmap_ref = ref_bitmaps.try_borrow_mut(ref_key).ok()?;
                            let mut ref_window = ref_bitmap_ref.get_window_u8()?;

                            let compare_result = compare_bitmaps(
                                &ctx,
                                &mut result_window,
                                &mut ref_window,
                                Similarity::AllowDssimMatch(0.0, 10.0),
                                false,
                            );

                            Some((compare_result.dssim?, file_size))
                        })();

                        if let Some((dssim, file_size)) = result {
                            results.lock().unwrap().push((qp, dpr_idx, dssim, file_size));

                            // Log to file
                            if let Ok(mut f) = log_file.lock() {
                                writeln!(f, "{},{},{},{},{},{:.8},{}",
                                    image_name, format, dpr, encoded_width, qp, dssim, file_size).ok();
                            }
                        }

                        let mut c = completed.lock().unwrap();
                        *c += 1;
                        if *c % 50 == 0 {
                            print!(".");
                            std::io::stdout().flush().ok();
                        }
                    }
                })
            }).collect();

            // Wait for all threads
            for handle in handles {
                handle.join().ok();
            }
            println!(" done ({} items)", total_items);

            // Build per-image dssim cache and print table
            let results = results.lock().unwrap();
            let mut per_image_cache: HashMap<(u32, usize), f64> = HashMap::new();
            let mut per_image_size_cache: HashMap<(u32, usize), usize> = HashMap::new();
            for &(qp, dpr_idx, dssim, file_size) in results.iter() {
                per_image_cache.insert((qp, dpr_idx), dssim);
                per_image_size_cache.insert((qp, dpr_idx), file_size);
            }

            // Print per-image table
            let title = format!("{} - {} ({}x{})",
                format, source_image.name, source_width, source_height);
            print_qp_tables(
                &title,
                &per_image_cache,
                &dpr_values,
                reference_qps,
                &qp_values,
                ref_dpr_idx,
            );

            // Store this image for comparison (last one wins = largest resolution)
            // DSSIM values are not comparable across images, so we use only the largest image for comparisons
            largest_image_for_comparison = Some((
                source_bytes.clone(),
                source_image.name.to_string(),
                per_image_cache.clone(),
            ));

            images_processed += 1;
        }

        println!("\nProcessed {} images for format {}", images_processed, format);

        // Compare qp.dpr implementation to calibration results using largest image's DSSIM cache
        // (DSSIM values are not comparable across images, so we don't average)
        if let Some((ref comparison_bytes, ref comparison_name, ref per_image_dssim)) = largest_image_for_comparison {
            compare_qp_dpr_implementation(
                comparison_bytes,
                comparison_name,
                format,
                target_width_1x,
                &dpr_values,
                reference_qps,
                ref_dpr_idx,
                per_image_dssim,
                &qp_values,
            );

            // Compare formula approaches using the same image's DSSIM cache
            compare_formula_approaches(
                format,
                &dpr_values,
                reference_qps,
                ref_dpr_idx,
                per_image_dssim,
                &qp_values,
            );
        }
    }
}

/// Formula approach 1: Continuous p-dependent k values
fn formula_continuous(p: f32, dpr: f32, floor_p: f32, ceiling_p: f32) -> f32 {
    let reference_dpr = 3.0_f32;
    let ratio = reference_dpr / dpr;
    let p_normalized = p / 100.0;

    let adjusted = if dpr < reference_dpr {
        // Low DPR: boost quality, but less for already-high quality
        // k_up decreases as p increases (less headroom to increase)
        let k_up = 2.0 * (1.0 - p_normalized).powf(1.2);
        p * ratio.powf(k_up)
    } else {
        // High DPR: reduce quality, more aggressively for high quality
        // k_down increases as p increases (more room to decrease)
        let k_down = 0.8 + 1.2 * p_normalized;
        p * ratio.powf(k_down)
    };

    adjusted.clamp(floor_p, ceiling_p)
}

/// Formula approach 2: Discrete tier-based k values
fn formula_tiered(p: f32, dpr: f32, floor_p: f32, ceiling_p: f32) -> f32 {
    let reference_dpr = 3.0_f32;
    let ratio = reference_dpr / dpr;

    let (k_up, k_down) = match p as u32 {
        0..=30   => (1.8, 1.0),   // Low quality: big swings up, modest down
        31..=60  => (1.2, 1.3),   // Medium: moderate both ways
        61..=85  => (0.6, 1.6),   // Good: small up, larger down
        _        => (0.2, 1.8),   // High: minimal up, big down
    };

    let adjusted = if dpr < reference_dpr {
        p * ratio.powf(k_up)
    } else {
        p * ratio.powf(k_down)
    };

    adjusted.clamp(floor_p, ceiling_p)
}

/// Current implementation formula (for comparison)
fn formula_current(p: f32, dpr: f32, floor_p: f32, ceiling_p: f32, format: &str) -> f32 {
    let reference_dpr = 3.0_f32;
    let ratio = reference_dpr / dpr;

    // Current per-format k values from auto.rs
    let (k_up, k_down) = match format {
        "jpg" => (2.3, 1.6),
        "webp" => (3.0, 2.5),
        // AVIF needs aggressive adjustment due to visible compression artifacts when upscaled
        "avif" => (3.5, 3.5),
        _ => (2.3, 1.6),
    };

    let adjusted = if dpr < reference_dpr {
        p * ratio.powf(k_up)
    } else {
        p * ratio.powf(k_down)
    };

    adjusted.clamp(floor_p, ceiling_p)
}

/// Compare formula approaches against calibration-optimal
fn compare_formula_approaches(
    format: &str,
    dpr_values: &[f32],
    reference_qps: &[u32],
    ref_dpr_idx: usize,
    dssim_cache: &std::collections::HashMap<(u32, usize), f64>,
    qp_values: &[u32],
) {
    println!("\n=== Formula Comparison: {} ===", format);
    println!("Comparing: Current impl, Continuous p-dependent, Tiered k-values vs Calibration Optimal\n");

    // Header
    println!("{:<6} {:>5} {:>5}-{:<5} | {:>6} {:>6} {:>6} | {:>6} | {:>5} {:>5} {:>5}",
        "QP", "DPR", "floor", "ceil",
        "Curr", "Cont", "Tier",
        "Optim",
        "ΔCurr", "ΔCont", "ΔTier");
    println!("{}", "-".repeat(85));

    let mut total_error_current = 0.0_f32;
    let mut total_error_continuous = 0.0_f32;
    let mut total_error_tiered = 0.0_f32;
    let mut count = 0;

    for &ref_qp in reference_qps {
        let (floor_qp, ceiling_qp) = get_qp_bounds(ref_qp);
        let floor_p = floor_qp as f32;
        let ceiling_p = ceiling_qp as f32;
        let p = ref_qp as f32;

        // Get reference DSSIM at DPR=3
        let ref_dssim = match dssim_cache.get(&(ref_qp, ref_dpr_idx)) {
            Some(&d) => d,
            None => continue,
        };

        for (dpr_idx, &dpr) in dpr_values.iter().enumerate() {
            if dpr_idx == ref_dpr_idx {
                // Reference DPR row - all formulas should produce ref_qp
                println!("{:<6} {:>5.1} {:>5}-{:<5} | {:>6} {:>6} {:>6} | {:>6} | {:>5} {:>5} {:>5}  (reference)",
                    ref_qp, dpr, floor_qp, ceiling_qp,
                    ref_qp, ref_qp, ref_qp,
                    ref_qp,
                    "  =  ", "  =  ", "  =  ");
                continue;
            }

            // Find calibration-optimal QP within clamped bounds
            let mut optimal_qp = ref_qp;
            let mut best_diff = f64::INFINITY;
            for &qp in qp_values {
                if qp < floor_qp || qp > ceiling_qp {
                    continue;
                }
                if let Some(&d) = dssim_cache.get(&(qp, dpr_idx)) {
                    let diff = (d - ref_dssim).abs();
                    if diff < best_diff {
                        best_diff = diff;
                        optimal_qp = qp;
                    }
                }
            }

            // Calculate what each formula produces
            let current_qp = formula_current(p, dpr, floor_p, ceiling_p, format).round() as u32;
            let continuous_qp = formula_continuous(p, dpr, floor_p, ceiling_p).round() as u32;
            let tiered_qp = formula_tiered(p, dpr, floor_p, ceiling_p).round() as u32;

            // Calculate errors (difference from optimal)
            let err_current = (current_qp as i32 - optimal_qp as i32).abs();
            let err_continuous = (continuous_qp as i32 - optimal_qp as i32).abs();
            let err_tiered = (tiered_qp as i32 - optimal_qp as i32).abs();

            total_error_current += err_current as f32;
            total_error_continuous += err_continuous as f32;
            total_error_tiered += err_tiered as f32;
            count += 1;

            // Format error display
            let err_str = |_e: i32, qp: u32, opt: u32| -> String {
                if qp == opt {
                    "  =  ".to_string()
                } else {
                    format!("{:>+4}", qp as i32 - opt as i32)
                }
            };

            println!("{:<6} {:>5.1} {:>5}-{:<5} | {:>6} {:>6} {:>6} | {:>6} | {:>5} {:>5} {:>5}",
                ref_qp, dpr, floor_qp, ceiling_qp,
                current_qp, continuous_qp, tiered_qp,
                optimal_qp,
                err_str(err_current, current_qp, optimal_qp),
                err_str(err_continuous, continuous_qp, optimal_qp),
                err_str(err_tiered, tiered_qp, optimal_qp));
        }
    }

    // Summary statistics
    println!("\n--- Summary ({} comparisons) ---", count);
    println!("Mean Absolute Error:");
    println!("  Current:    {:>6.2} QP units", total_error_current / count as f32);
    println!("  Continuous: {:>6.2} QP units", total_error_continuous / count as f32);
    println!("  Tiered:     {:>6.2} QP units", total_error_tiered / count as f32);

    // Show k values for each formula at key points
    println!("\n--- Effective k values ---");
    println!("{:<6} | {:>12} | {:>12} | {:>12}", "QP", "Current k", "Continuous k", "Tiered k");
    println!("{}", "-".repeat(50));

    for &qp in &[15u32, 35, 55, 73, 91] {
        let p = qp as f32;
        let p_norm = p / 100.0;

        let (curr_up, curr_down) = match format {
            "jpg" => (2.3, 1.6),
            "webp" => (3.0, 2.5),
            _ => (2.3, 1.6),
        };

        let cont_up = 2.0 * (1.0 - p_norm).powf(1.2);
        let cont_down = 0.8 + 1.2 * p_norm;

        let (tier_up, tier_down) = match qp {
            0..=30   => (1.8, 1.0),
            31..=60  => (1.2, 1.3),
            61..=85  => (0.6, 1.6),
            _        => (0.2, 1.8),
        };

        println!("{:<6} | {:>5.2}/{:<5.2} | {:>5.2}/{:<5.2} | {:>5.2}/{:<5.2}",
            qp,
            curr_up, curr_down,
            cont_up, cont_down,
            tier_up, tier_down);
    }
}
