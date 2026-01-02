//! Codec Quality Calibration Pipeline
//!
//! Builds a mapping database that translates JPEG quality values to equivalent
//! WebP and AVIF quality values across different viewing conditions.
//!
//! # Implementation Details
//!
//! ## Aspect-Ratio Preservation
//! Uses `Constrain` node with `ConstraintMode::Within` to preserve aspect ratio
//! and only downsample (never upsample) reference images.
//!
//! ## Bitmap-Based Processing
//! All browser simulation and viewing transforms are done in-memory using
//! `decode_with_view_sim()` which outputs directly to DSSIM-ready bitmaps.
//! This avoids intermediate JPEG q100 encoding which would introduce quality loss.
//!
//! ## Reference Caching
//! Transformed reference images are cached per (image, ratio, ppd) combination
//! to avoid redundant decode+resize when testing multiple codec/quality settings.
//!
//! ## Timing Instrumentation
//! Pipeline stages are timed and summarized per codec:
//! - Encoding time
//! - Decode+simulation time
//! - DSSIM computation time
//!
//! # Remaining TODOs
//!
//! ## TODO: Per-category analysis
//! The CSV output contains `subdir` (category) information. Should add code to:
//! 1. Analyze results per category (products, clothing, marketing, etc.)
//! 2. Produce category-specific mapping tables
//! 3. Identify outliers and category-specific quality requirements
//! 4. Statistical validation (confidence intervals, variance analysis)
//!
//! ## TODO: Test configuration
//! These calibration tests are `#[ignore]` but should also:
//! - Not compile by default (feature-gate behind `calibration` feature)
//! - Have clear documentation on how to run
//! - Support partial runs (e.g., just AVIF, just one image)
//!
//! ## Analysis Tools Recommendations
//! For working with the CSV results:
//! - **Python + pandas + matplotlib**: Best for exploratory analysis
//! - **Jupyter notebooks**: Interactive visualization, shareable
//! - **Observable (observablehq.com)**: Web-based, good for sharing
//! - **DuckDB**: SQL queries on CSV, fast aggregations
//! - **Vega-Lite / Altair**: Declarative visualization, good for faceted plots
//!
//! Key visualizations needed:
//! - Quality vs DSSIM curves per codec (at each ratio/ppd)
//! - File size vs DSSIM scatter plots (efficiency frontier)
//! - Heatmaps: quality equivalence across codecs
//! - Per-category box plots showing variance
//!
//! ## Theory: Viewing-Condition-Aware Quality
//!
//! Human visual acuity is approximately 1 arcminute (1/60 degree). This means:
//! - Details smaller than 1 arcminute cannot be resolved by the eye
//! - At high DPI with normal viewing distance, compression artifacts become invisible
//! - Quality that looks acceptable on a phone may look terrible on a desktop monitor
//!
//! ### Pixels Per Degree (PPD)
//!
//! PPD measures how many pixels fit within one degree of visual angle. It combines:
//! - Display pixel density (PPI)
//! - Viewing distance
//!
//! ```text
//! PPD ≈ (PPI × viewing_distance_inches × π) / 180
//!
//! Example calculations:
//!   Desktop 1080p 24" @ 24": 92 PPI × 24" × π / 180 ≈ 38 PPD
//!   Laptop 15" retina @ 18": 220 PPI × 18" × π / 180 ≈ 69 PPD
//!   Phone 6" 3x @ 12":       440 PPI × 12" × π / 180 ≈ 92 PPD
//! ```
//!
//! Higher PPD = more pixels per degree = smaller angular size per pixel = artifacts less visible.
//!
//! ### How We Simulate Viewing Conditions
//!
//! We use PPD=40 as the baseline (worst case - desktop, artifacts most visible).
//! For higher PPD values, we downsample both reference and test images:
//!
//! ```text
//! view_scale = BASELINE_PPD / target_ppd
//!
//! PPD=40 (desktop):  scale=1.0  → full resolution comparison
//! PPD=70 (laptop):   scale=0.57 → downsample to 57%, simulating reduced acuity
//! PPD=95 (phone):    scale=0.42 → downsample to 42%, artifacts blend together
//! ```
//!
//! This downsampling simulates the eye's spatial integration - at high PPD, multiple
//! pixels fall within one "visual pixel" (1 arcminute), so we blur them together.
//!
//! ### Practical Implications
//!
//! 1. **Same quality number looks better on phones** - PPD=95 DSSIM is ~70% lower than PPD=40
//! 2. **Can use lower quality for mobile** - Saves bandwidth with no perceived loss
//! 3. **Codec mapping is PPD-independent** - The relative quality between codecs stays constant
//!
//! ## srcset / DPR Simulation
//!
//! The `ratio` parameter simulates browser srcset behavior:
//!
//! ```text
//! ratio = intrinsic_size / display_size
//!
//! ratio=0.5 (2x srcset): Encode at 50% size, browser upsamples 2x with Mitchell
//! ratio=1.0 (1x srcset): Encode at display size, no browser scaling
//! ratio=2.0 (0.5x):      Encode at full size, browser downsamples with Catmull-Rom
//! ```
//!
//! Browser upsampling (ratio < 1) makes artifacts MORE visible because the browser
//! interpolates between compressed pixels, spreading the damage.
//!
//! ## Architecture
//!
//! The pipeline caches at two levels:
//! 1. **Encoded files**: In `encoded/` with version in filename (e.g., `mozjpeg_v1_q80.jpg`)
//! 2. **Measurements**: In `measurements/` with versions in filename (e.g., `img_mozjpeg_p2c1_q80_r100_p40.json`)
//!
//! Browser simulation and viewing transforms run in-memory (fast, no caching needed).
//!
//! ## Cache-Breaking
//!
//! Per-codec version constants allow invalidating one codec without recomputing others:
//! - `MOZJPEG_VERSION`: Bump when mozjpeg encoder settings change
//! - `WEBP_VERSION`: Bump when WebP encoder settings change
//! - `AVIF_VERSION`: Bump when AVIF encoder settings change (e.g., speed param)
//! - `PIPELINE_VERSION`: Bump when viewing model changes (affects ALL codecs)
//!
//! ## Running
//!
//! ```bash
//! cargo test --release --package imageflow_core --test codec_calibration -- --nocapture --ignored
//! ```

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate imageflow_core;

use imageflow_core::{Context, FlowError};
use imageflow_types as s;
use s::{AvifEncoderHints, EncoderHints, Node, OutputImageFormat, ResponsePayload};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

pub mod common;
use common::BitmapBgraContainer;

//=============================================================================
// Version Stamps for Cache Invalidation
//=============================================================================

/// Bump when viewing model changes (browser kernels, PPD values, DSSIM method)
/// This invalidates ALL measurements across all codecs.
const PIPELINE_VERSION: u32 = 2;

/// Per-codec encoder versions - bump to invalidate only that codec's cache
/// This allows re-running calibration for one codec without redoing others.
const MOZJPEG_VERSION: u32 = 1;
const WEBP_VERSION: u32 = 1;
const AVIF_VERSION: u32 = 1;
// Future: const JPEGLI_VERSION: u32 = 1;

//=============================================================================
// Test Matrix Configuration
//=============================================================================

/// Quality values to test (profile anchor points + interpolation)
/// Includes:
/// - Low quality points (1, 5, 10) for proper interpolation at the low end
///   Note: AVIF encoder requires quality >= 1, so we start at 1 instead of 0
/// - Common values (25, 50, 75)
/// - Fine-grained points (34, 57, 73, etc.) for accurate curve fitting
/// - Popular encoder defaults (JPEG 75, WebP 75, AVIF 65)
const QUALITY_VALUES: &[u32] =
    &[1, 5, 10, 15, 20, 25, 30, 34, 40, 45, 50, 55, 57, 60, 65, 70, 73, 76, 80, 85, 89, 90, 91, 95, 96, 100];

/// Intrinsic/device pixel ratios for srcset simulation
///
/// ```text
/// ratio = encoded_size / display_size
///
/// 0.25 → 4x srcset (ultra-high DPI, e.g., 400px image at 1600px CSS width)
/// 0.33 → 3x srcset (e.g., 600px image displayed at 1800px CSS width)
/// 0.50 → 2x srcset (retina, most common high-DPI case)
/// 0.67 → 1.5x srcset
/// 1.00 → 1x srcset (image matches display size exactly)
/// 1.50 → undersized (image is 1.5x larger than needed)
/// 2.00 → undersized (image is 2x larger than needed, browser downsamples)
/// 4.00 → very undersized (image is 4x larger than needed)
/// ```
const RATIOS: &[f32] = &[0.25, 0.33, 0.5, 0.67, 1.0, 1.5, 2.0, 4.0];

/// Pixels per degree (viewing conditions)
///
/// Represents different device + viewing distance combinations:
///
/// ```text
/// PPD=40: Desktop 1080p 24" at 24" distance (worst case, artifacts most visible)
/// PPD=55: Desktop 1440p/4K 27" at 20" distance
/// PPD=70: Laptop 15" retina at 18" distance
/// PPD=95: Phone 6" 3x retina at 12" distance (best case, artifacts least visible)
/// ```
///
/// Formula: PPD ≈ (PPI × viewing_distance_inches × π) / 180
const PPD_VALUES: &[u32] = &[40, 55, 70, 95];

/// Baseline PPD for viewing transform
///
/// We use desktop (PPD=40) as baseline because it's the worst case - artifacts are
/// most visible here. For higher PPD targets, we downsample to simulate reduced acuity.
const BASELINE_PPD: u32 = 40;

/// Maximum reference dimension (long edge)
/// Images larger than this are downsampled to this size for the reference.
const MAX_REFERENCE_DIM: u32 = 1600;

/// Minimum reference dimension (short edge)
/// References smaller than this may have insufficient detail for meaningful DSSIM.
const MIN_REFERENCE_DIM: u32 = 200;

//=============================================================================
// Codec Configuration
//=============================================================================

/// Codecs under calibration
///
/// Note on quality scales:
/// - Each codec has its own quality scale (0-100) with different meanings
/// - Our "reference quality" in tables currently uses mozjpeg's scale as the baseline
/// - Future codecs (jpegli) will have their own quality scales that don't match mozjpeg
/// - Cross-codec tables map between these different scales based on perceptual equivalence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Codec {
    /// Mozilla's optimized JPEG encoder
    /// Quality scale: 0-100 (mozjpeg native)
    Mozjpeg,
    /// WebP lossy encoder (libwebp)
    /// Quality scale: 0-100 (libwebp native, not directly comparable to mozjpeg)
    WebP,
    /// AVIF encoder (ravif/rav1e)
    /// Quality scale: 0-100 (ravif native, not directly comparable to mozjpeg)
    /// Speed parameter affects encode time vs quality tradeoff
    Avif { speed: u8 },
    // Future: Jpegli - Google's improved JPEG encoder with different quality scale
}

impl Codec {
    fn extension(&self) -> &'static str {
        match self {
            Codec::Mozjpeg => "jpg",
            Codec::WebP => "webp",
            Codec::Avif { .. } => "avif",
        }
    }

    fn name(&self) -> String {
        match self {
            Codec::Mozjpeg => "mozjpeg".to_string(),
            Codec::WebP => "webp".to_string(),
            Codec::Avif { speed } => format!("avif_s{}", speed),
        }
    }

    /// Per-codec version for cache invalidation
    /// Bump the corresponding constant to invalidate only this codec's cache
    fn version(&self) -> u32 {
        match self {
            Codec::Mozjpeg => MOZJPEG_VERSION,
            Codec::WebP => WEBP_VERSION,
            Codec::Avif { .. } => AVIF_VERSION,
        }
    }

    /// All codecs to test
    fn all() -> Vec<Codec> {
        vec![Codec::Mozjpeg, Codec::WebP, Codec::Avif { speed: 6 }]
    }

    /// Get encoder preset for this codec
    fn encoder_preset(&self, quality: u32) -> s::EncoderPreset {
        match self {
            Codec::Mozjpeg => s::EncoderPreset::Mozjpeg {
                quality: Some(quality as u8),
                progressive: Some(true),
                matte: None,
            },
            Codec::WebP => s::EncoderPreset::WebPLossy { quality: quality as f32 },
            Codec::Avif { speed } => s::EncoderPreset::Format {
                format: OutputImageFormat::Avif,
                quality_profile: None,
                quality_profile_dpr: None,
                matte: None,
                lossless: None,
                allow: None,
                encoder_hints: Some(EncoderHints {
                    jpeg: None,
                    png: None,
                    webp: None,
                    avif: Some(AvifEncoderHints {
                        quality: Some(quality as f32),
                        speed: Some(*speed),
                        alpha_quality: None,
                    }),
                    gif: None,
                }),
            },
        }
    }
}

//=============================================================================
// Data Structures
//=============================================================================

/// Metadata about a reference image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceMetadata {
    pub source_path: String,
    pub source_width: u32,
    pub source_height: u32,
    pub reference_width: u32,
    pub reference_height: u32,
    pub downsample_factor: f32,
}

/// A single measurement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    /// Image stem (filename without extension)
    pub image: String,
    /// Corpus subdirectory
    pub subdir: String,
    /// Codec name (jpeg, webp, avif_s6)
    pub codec: String,
    /// AVIF speed parameter (None for JPEG/WebP)
    pub codec_speed: Option<u8>,
    /// Quality value (0-100)
    pub quality: u32,
    /// Intrinsic/device ratio
    pub ratio: f32,
    /// Pixels per degree (viewing condition)
    pub ppd: u32,
    /// DSSIM value (0 = identical)
    pub dssim: f64,
    /// Encoded file size in bytes
    pub file_size: usize,
    /// Reference image width
    pub reference_width: u32,
    /// Reference image height
    pub reference_height: u32,
    /// Encoded image width (intrinsic size)
    pub encoded_width: u32,
    /// Encoded image height (intrinsic size)
    pub encoded_height: u32,
    /// Path to encoded file (relative to cache_dir)
    pub encoded_path: String,
}

/// Cache key for encoded files
/// Includes codec version for per-codec cache invalidation
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EncodedKey {
    image: String,
    codec: String,
    codec_version: u32,
    quality: u32,
    ratio_x100: u32,
}

impl EncodedKey {
    fn new(image: &str, codec: &Codec, quality: u32, ratio: f32) -> Self {
        Self {
            image: image.to_string(),
            codec: codec.name(),
            codec_version: codec.version(),
            quality,
            ratio_x100: (ratio * 100.0).round() as u32,
        }
    }

    fn filename(&self, ext: &str) -> String {
        // Include codec version in filename for cache invalidation
        // e.g., "mozjpeg_v1_q80.jpg" or "avif_s6_v1_q60_r50.avif"
        if self.ratio_x100 == 100 {
            format!("{}_v{}_q{}.{}", self.codec, self.codec_version, self.quality, ext)
        } else {
            format!("{}_v{}_q{}_r{}.{}", self.codec, self.codec_version, self.quality, self.ratio_x100, ext)
        }
    }
}

/// Cache key for measurements
/// Includes both pipeline version (viewing model) and codec version (encoder settings)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct MeasurementKey {
    image: String,
    codec: String,
    codec_version: u32,
    pipeline_version: u32,
    quality: u32,
    ratio_x100: u32,
    ppd: u32,
}

impl MeasurementKey {
    fn new(image: &str, codec: &Codec, pipeline_version: u32, quality: u32, ratio: f32, ppd: u32) -> Self {
        Self {
            image: image.to_string(),
            codec: codec.name(),
            codec_version: codec.version(),
            pipeline_version,
            quality,
            ratio_x100: (ratio * 100.0).round() as u32,
            ppd,
        }
    }

    fn filename(&self) -> String {
        // Include both versions for cache invalidation
        // e.g., "image_mozjpeg_p2c1_q80_r100_p40.json"
        format!(
            "{}_{}_p{}c{}_q{}_r{}_p{}.json",
            self.image, self.codec, self.pipeline_version, self.codec_version,
            self.quality, self.ratio_x100, self.ppd
        )
    }
}

/// Pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub corpus_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub quality_values: Vec<u32>,
    pub ratios: Vec<f32>,
    pub ppd_values: Vec<u32>,
    pub codecs: Vec<Codec>,
    pub max_reference_dim: u32,
    pub pipeline_version: u32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            corpus_dir: PathBuf::from("/mnt/v/work/corpus"),
            cache_dir: PathBuf::from("/mnt/v/work/corpus_cache/v2"),
            quality_values: QUALITY_VALUES.to_vec(),
            ratios: RATIOS.to_vec(),
            ppd_values: PPD_VALUES.to_vec(),
            codecs: Codec::all(),
            max_reference_dim: MAX_REFERENCE_DIM,
            pipeline_version: PIPELINE_VERSION,
        }
    }
}

impl PipelineConfig {
    fn references_dir(&self) -> PathBuf {
        self.cache_dir.join("references")
    }

    fn encoded_dir(&self) -> PathBuf {
        self.cache_dir.join("encoded")
    }

    fn measurements_dir(&self) -> PathBuf {
        self.cache_dir.join("measurements")
    }

    fn results_dir(&self) -> PathBuf {
        self.cache_dir.join("results")
    }
}

//=============================================================================
// Imageflow Operations
//=============================================================================

fn load_file(path: &Path) -> Result<Vec<u8>, FlowError> {
    fs::read(path).map_err(|e| {
        nerror!(
            imageflow_core::ErrorKind::InternalError,
            "Failed to read {}: {}",
            path.display(),
            e
        )
    })
}

/// Get dimensions by decoding and encoding to a dummy output
/// This works around a bug where decode metadata returns incorrect dimensions
fn get_dimensions(bytes: &[u8]) -> Result<(u32, u32), FlowError> {
    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    ctx.add_input_vector(0, bytes.to_vec()).map_err(|e| e.at(here!()))?;
    ctx.add_output_buffer(1).map_err(|e| e.at(here!()))?;

    let resp = ctx
        .execute_1(s::Execute001 {
            security: None,
            graph_recording: None,
            framewise: s::Framewise::Steps(vec![
                Node::Decode { io_id: 0, commands: None },
                // Encode to get correct dimensions from encode metadata
                Node::Encode {
                    io_id: 1,
                    preset: s::EncoderPreset::Mozjpeg {
                        quality: Some(1), // Lowest quality for speed
                        progressive: Some(false),
                        matte: None,
                    },
                },
            ]),
        })
        .map_err(|e| e.at(here!()))?;

    match resp {
        ResponsePayload::JobResult(job) | ResponsePayload::BuildResult(job) => {
            // Read from encode metadata (more reliable than decode metadata)
            if let Some(e) = job.encodes.first() {
                Ok((e.w as u32, e.h as u32))
            } else {
                Err(nerror!(imageflow_core::ErrorKind::InternalError, "No encode result"))
            }
        }
        _ => Err(nerror!(imageflow_core::ErrorKind::InternalError, "Unexpected response")),
    }
}

/// Constrain image dimensions (aspect-ratio preserving) and save as JPEG q100
///
/// Uses `Constrain` node with `ConstraintMode::Within` to:
/// - Preserve aspect ratio
/// - Only downsample, never upsample
/// - Handle non-square images correctly
fn constrain_to_jpeg100(
    src: &[u8],
    max_w: u32,
    max_h: u32,
    filter: s::Filter,
) -> Result<Vec<u8>, FlowError> {
    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    ctx.add_input_vector(0, src.to_vec()).map_err(|e| e.at(here!()))?;
    ctx.add_output_buffer(1).map_err(|e| e.at(here!()))?;

    ctx.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Constrain(s::Constraint {
                mode: s::ConstraintMode::Within, // Only downsample, preserve aspect ratio
                w: Some(max_w),
                h: Some(max_h),
                hints: Some(s::ResampleHints {
                    sharpen_percent: Some(0.0),
                    down_filter: Some(filter),
                    up_filter: Some(filter),
                    scaling_colorspace: Some(s::ScalingFloatspace::Linear),
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
                gravity: None,
                canvas_color: None,
            }),
            Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Mozjpeg {
                    quality: Some(100),
                    progressive: Some(false),
                    matte: None,
                },
            },
        ]),
    })
    .map_err(|e| e.at(here!()))?;

    Ok(ctx.get_output_buffer_slice(1).map_err(|e| e.at(here!()))?.to_vec())
}

/// Resample to exact dimensions using Resample2D
///
/// Used when exact dimensions are required (browser simulation of upsampling).
/// Kept for potential debugging/testing use.
#[allow(dead_code)]
fn resample_to_jpeg100(
    src: &[u8],
    w: u32,
    h: u32,
    filter: s::Filter,
) -> Result<Vec<u8>, FlowError> {
    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    ctx.add_input_vector(0, src.to_vec()).map_err(|e| e.at(here!()))?;
    ctx.add_output_buffer(1).map_err(|e| e.at(here!()))?;

    ctx.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Resample2D {
                w,
                h,
                hints: Some(s::ResampleHints {
                    sharpen_percent: Some(0.0),
                    down_filter: Some(filter),
                    up_filter: Some(filter),
                    scaling_colorspace: Some(s::ScalingFloatspace::Linear),
                    background_color: None,
                    resample_when: None,
                    sharpen_when: None,
                }),
            },
            Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Mozjpeg {
                    quality: Some(100),
                    progressive: Some(false),
                    matte: None,
                },
            },
        ]),
    })
    .map_err(|e| e.at(here!()))?;

    Ok(ctx.get_output_buffer_slice(1).map_err(|e| e.at(here!()))?.to_vec())
}

/// Decode to JPEG q100 (for consistent intermediate storage)
#[allow(dead_code)]
fn decode_to_jpeg100(src: &[u8]) -> Result<Vec<u8>, FlowError> {
    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    #[cfg(feature = "bad-avif-decoder")]
    ctx.enabled_codecs.enable_bad_avif_decoder();
    ctx.add_input_vector(0, src.to_vec()).map_err(|e| e.at(here!()))?;
    ctx.add_output_buffer(1).map_err(|e| e.at(here!()))?;

    ctx.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![
            Node::Decode { io_id: 0, commands: None },
            Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Mozjpeg {
                    quality: Some(100),
                    progressive: Some(false),
                    matte: None,
                },
            },
        ]),
    })
    .map_err(|e| e.at(here!()))?;

    Ok(ctx.get_output_buffer_slice(1).map_err(|e| e.at(here!()))?.to_vec())
}

/// Encode with specific codec/quality, optionally resizing to exact dimensions first
///
/// Uses `Resample2D` for exact sizing since we need precise intrinsic dimensions.
/// The target dimensions are calculated to preserve aspect ratio (intrinsic = ref * ratio),
/// so we're not actually distorting - just ensuring exact pixel dimensions.
fn encode_image(
    src: &[u8],
    codec: Codec,
    quality: u32,
    resize_to: Option<(u32, u32)>,
) -> Result<Vec<u8>, FlowError> {
    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    ctx.add_input_vector(0, src.to_vec()).map_err(|e| e.at(here!()))?;
    ctx.add_output_buffer(1).map_err(|e| e.at(here!()))?;

    let mut steps: Vec<Node> = vec![Node::Decode { io_id: 0, commands: None }];

    if let Some((w, h)) = resize_to {
        // Use Resample2D for exact dimensions - important for consistent DSSIM comparison
        steps.push(Node::Resample2D {
            w,
            h,
            hints: Some(s::ResampleHints {
                sharpen_percent: Some(0.0),
                down_filter: Some(s::Filter::Lanczos),
                up_filter: Some(s::Filter::Lanczos),
                scaling_colorspace: Some(s::ScalingFloatspace::Linear),
                background_color: None,
                resample_when: None,
                sharpen_when: None,
            }),
        });
    }

    steps.push(Node::Encode { io_id: 1, preset: codec.encoder_preset(quality) });

    ctx.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(steps),
    })
    .map_err(|e| e.at(here!()))?;

    Ok(ctx.get_output_buffer_slice(1).map_err(|e| e.at(here!()))?.to_vec())
}

//=============================================================================
// DSSIM Computation
//=============================================================================

/// Compute DSSIM directly from DSSIM-ready images
fn compute_dssim_from_images(
    reference: &imgref::ImgVec<rgb::Rgba<f32>>,
    test: &imgref::ImgVec<rgb::Rgba<f32>>,
) -> Result<f64, FlowError> {
    let d = dssim::new();
    let ref_d = d.create_image(reference).ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "DSSIM failed to create reference image")
    })?;
    let test_d = d.create_image(test).ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "DSSIM failed to create test image")
    })?;

    let (val, _) = d.compare(&ref_d, test_d);
    Ok(val.into())
}

/// Parameters for browser and viewing simulation
#[derive(Debug, Clone)]
struct ViewSimParams {
    /// Browser simulation: None = no resize, Some((w, h, filter)) = resize to w x h
    browser_resize: Option<(u32, u32, s::Filter)>,
    /// Viewing transform: None = no resize, Some((w, h)) = resize using Lanczos
    view_resize: Option<(u32, u32)>,
}

/// Decode image and apply browser/viewing simulation, outputting directly to bitmap
///
/// This avoids intermediate JPEG q100 encoding by doing all operations in a single graph:
/// decode → browser_resize → view_resize → bitmap
fn decode_with_view_sim(
    bytes: &[u8],
    params: &ViewSimParams,
) -> Result<imgref::ImgVec<rgb::Rgba<f32>>, FlowError> {
    use dssim::*;
    use s::PixelLayout;

    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    #[cfg(feature = "bad-avif-decoder")]
    ctx.enabled_codecs.enable_bad_avif_decoder();
    ctx.add_input_vector(0, bytes.to_vec()).map_err(|e| e.at(here!()))?;

    let mut steps: Vec<Node> = vec![Node::Decode { io_id: 0, commands: None }];

    // Browser simulation resize
    if let Some((w, h, filter)) = params.browser_resize {
        steps.push(Node::Resample2D {
            w,
            h,
            hints: Some(s::ResampleHints {
                sharpen_percent: Some(0.0),
                down_filter: Some(filter),
                up_filter: Some(filter),
                scaling_colorspace: Some(s::ScalingFloatspace::Linear),
                background_color: None,
                resample_when: None,
                sharpen_when: None,
            }),
        });
    }

    // Viewing transform resize
    if let Some((w, h)) = params.view_resize {
        steps.push(Node::Resample2D {
            w,
            h,
            hints: Some(s::ResampleHints {
                sharpen_percent: Some(0.0),
                down_filter: Some(s::Filter::Lanczos),
                up_filter: Some(s::Filter::Lanczos),
                scaling_colorspace: Some(s::ScalingFloatspace::Linear),
                background_color: None,
                resample_when: None,
                sharpen_when: None,
            }),
        });
    }

    let mut bmp = BitmapBgraContainer::empty();
    steps.push(unsafe { bmp.as_mut().get_node() });

    ctx.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(steps),
    })
    .map_err(|e| e.at(here!()))?;

    let key = unsafe { bmp.bitmap_key(&ctx) }
        .ok_or_else(|| nerror!(imageflow_core::ErrorKind::InternalError, "No bitmap"))?;

    let bitmaps = ctx.borrow_bitmaps().map_err(|e| e.at(here!()))?;
    let mut br = bitmaps.try_borrow_mut(key).map_err(|e| e.at(here!()))?;

    let mut window = br
        .get_window_u8()
        .ok_or_else(|| nerror!(imageflow_core::ErrorKind::InternalError, "No window"))?;

    window.normalize_unused_alpha().map_err(|e| {
        nerror!(imageflow_core::ErrorKind::InternalError, "Normalize alpha: {:?}", e)
    })?;

    if window.info().pixel_layout() != PixelLayout::BGRA {
        return Err(nerror!(imageflow_core::ErrorKind::InternalError, "Pixel layout is not BGRA"));
    }

    let w = window.w() as usize;
    let h = window.h() as usize;
    let slice = window.get_slice();
    let new_stride = window.info().t_stride() as usize / 4;

    let cast_to_bgra8 = bytemuck::cast_slice::<u8, rgb::alt::BGRA8>(slice);

    Ok(imgref::Img::new_stride(cast_to_bgra8.to_rgbaplu(), w, h, new_stride))
}

/// Legacy function for simple decode to DSSIM format
/// Kept for potential debugging/testing use.
#[allow(dead_code)]
fn load_for_dssim(bytes: &[u8]) -> Result<imgref::ImgVec<rgb::Rgba<f32>>, FlowError> {
    decode_with_view_sim(bytes, &ViewSimParams {
        browser_resize: None,
        view_resize: None,
    })
}

//=============================================================================
// Pipeline Stages
//=============================================================================

/// Stage 1: Prepare reference images
///
/// Uses `Constrain` with `Within` mode to:
/// - Only downsample (never upsample)
/// - Preserve aspect ratio
/// - Limit to MAX_REFERENCE_DIM on the long edge
fn prepare_references(
    config: &PipelineConfig,
) -> Result<Vec<(PathBuf, String, String, ReferenceMetadata)>, FlowError> {
    let refs_dir = config.references_dir();
    fs::create_dir_all(&refs_dir).ok();

    let mut results = Vec::new();

    for entry in
        walkdir::WalkDir::new(&config.corpus_dir).into_iter().filter_map(|e| e.ok()).filter(|e| {
            e.path()
                .extension()
                .map(|x| {
                    x.eq_ignore_ascii_case("jpg")
                        || x.eq_ignore_ascii_case("jpeg")
                        || x.eq_ignore_ascii_case("png")
                })
                .unwrap_or(false)
        })
    {
        let src = entry.path();
        let rel = src.strip_prefix(&config.corpus_dir).unwrap_or(src);
        let subdir = rel.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();

        let out_dir = refs_dir.join(&subdir);
        fs::create_dir_all(&out_dir).ok();

        let ref_path = out_dir.join(format!("{}.jpg", stem));
        let meta_path = out_dir.join(format!("{}.json", stem));

        // Check if already done
        if ref_path.exists() && meta_path.exists() {
            if let Ok(s) = fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<ReferenceMetadata>(&s) {
                    results.push((ref_path, subdir, stem, meta));
                    continue;
                }
            }
        }

        println!("  Preparing reference: {}/{}", subdir, stem);

        let src_bytes = load_file(src)?;

        // Always use Constrain::Within to handle any source dimensions
        // This avoids relying on potentially incorrect decode metadata
        let max_dim = config.max_reference_dim;
        let ref_bytes = constrain_to_jpeg100(&src_bytes, max_dim, max_dim, s::Filter::Lanczos)?;

        // Get ACTUAL dimensions from the output
        let (actual_rw, actual_rh) = get_dimensions(&ref_bytes)?;

        // Sanity check: reference dimensions must be at least MIN_REFERENCE_DIM
        let min_dim = actual_rw.min(actual_rh);
        if min_dim < MIN_REFERENCE_DIM {
            return Err(nerror!(
                imageflow_core::ErrorKind::InvalidArgument,
                "Reference too small: {}x{} (min edge {} < {}) for {}/{}",
                actual_rw, actual_rh, min_dim, MIN_REFERENCE_DIM, subdir, stem
            ));
        }

        println!("    Reference: {}x{}", actual_rw, actual_rh);

        // Atomic write: temp file + rename
        let temp_path = out_dir.join(format!("{}.tmp", stem));
        fs::write(&temp_path, &ref_bytes)
            .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Write ref: {}", e))?;
        fs::rename(&temp_path, &ref_path)
            .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Rename ref: {}", e))?;

        // Metadata uses actual output dimensions
        // Note: source dimensions not reliably available from decode metadata
        let meta = ReferenceMetadata {
            source_path: src.to_string_lossy().to_string(),
            source_width: 0,  // Not reliably available
            source_height: 0, // Not reliably available
            reference_width: actual_rw,
            reference_height: actual_rh,
            downsample_factor: 1.0, // Not calculated
        };

        fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).ok();
        results.push((ref_path, subdir, stem, meta));
    }

    Ok(results)
}

/// Encoded file result with dimensions
struct EncodedResult {
    bytes: Vec<u8>,
    path: PathBuf,
    width: u32,
    height: u32,
}

/// Stage 2: Encode image with caching
fn get_or_create_encoded(
    config: &PipelineConfig,
    ref_bytes: &[u8],
    ref_w: u32,
    ref_h: u32,
    subdir: &str,
    image: &str,
    codec: Codec,
    quality: u32,
    ratio: f32,
) -> Result<EncodedResult, FlowError> {
    let key = EncodedKey::new(image, &codec, quality, ratio);
    let out_dir = config.encoded_dir().join(subdir).join(image);
    fs::create_dir_all(&out_dir).ok();

    let filename = key.filename(codec.extension());
    let out_path = out_dir.join(&filename);

    // Calculate intrinsic size for ratio < 1.0
    let (intrinsic_w, intrinsic_h) = if ratio < 1.0 {
        ((ref_w as f32 * ratio).round() as u32, (ref_h as f32 * ratio).round() as u32)
    } else {
        (ref_w, ref_h)
    };

    // Sanity check: intrinsic dimensions must be positive
    if intrinsic_w == 0 || intrinsic_h == 0 {
        return Err(nerror!(
            imageflow_core::ErrorKind::InvalidArgument,
            "Invalid intrinsic dimensions {}x{} (ref {}x{}, ratio {}) for {}/{}",
            intrinsic_w, intrinsic_h, ref_w, ref_h, ratio, subdir, image
        ));
    }

    // Sanity check: intrinsic dimensions should match expected ratio
    if ratio < 1.0 {
        let expected_w = (ref_w as f32 * ratio).round() as u32;
        let expected_h = (ref_h as f32 * ratio).round() as u32;
        if intrinsic_w != expected_w || intrinsic_h != expected_h {
            return Err(nerror!(
                imageflow_core::ErrorKind::InternalError,
                "Dimension mismatch: got {}x{}, expected {}x{} for {}/{}",
                intrinsic_w, intrinsic_h, expected_w, expected_h, subdir, image
            ));
        }
    }

    // Check cache
    if out_path.exists() {
        let bytes = load_file(&out_path)?;
        return Ok(EncodedResult {
            bytes,
            path: out_path,
            width: intrinsic_w,
            height: intrinsic_h,
        });
    }

    // Encode (with resize for ratio < 1)
    let encoded = if ratio < 1.0 {
        encode_image(ref_bytes, codec, quality, Some((intrinsic_w, intrinsic_h)))?
    } else {
        encode_image(ref_bytes, codec, quality, None)?
    };

    // Atomic write
    let temp_path = out_dir.join(format!("{}.tmp", filename));
    fs::write(&temp_path, &encoded)
        .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Write encoded: {}", e))?;
    fs::rename(&temp_path, &out_path)
        .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Rename encoded: {}", e))?;

    Ok(EncodedResult {
        bytes: encoded,
        path: out_path,
        width: intrinsic_w,
        height: intrinsic_h,
    })
}

/// Check if measurement is cached
fn get_cached_measurement(config: &PipelineConfig, key: &MeasurementKey) -> Option<Measurement> {
    let path = config.measurements_dir().join(key.filename());
    if path.exists() {
        if let Ok(s) = fs::read_to_string(&path) {
            if let Ok(m) = serde_json::from_str(&s) {
                return Some(m);
            }
        }
    }
    None
}

/// Cache a measurement
fn cache_measurement(config: &PipelineConfig, key: &MeasurementKey, m: &Measurement) {
    let dir = config.measurements_dir();
    fs::create_dir_all(&dir).ok();
    let path = dir.join(key.filename());
    if let Ok(json) = serde_json::to_string(m) {
        // Atomic write
        let temp_path = dir.join(format!("{}.tmp", key.filename()));
        if fs::write(&temp_path, &json).is_ok() {
            fs::rename(&temp_path, &path).ok();
        }
    }
}

/// Stages 3-6: Browser sim + viewing transform + DSSIM (bitmap-based) with timing
///
/// Returns (Measurement, encode_ms, decode_ms, dssim_ms)
fn process_measurement_with_ref_timed(
    config: &PipelineConfig,
    ref_bytes: &[u8],
    ref_w: u32,
    ref_h: u32,
    subdir: &str,
    image: &str,
    codec: Codec,
    quality: u32,
    ratio: f32,
    ppd: u32,
    cached_ref: Option<&imgref::ImgVec<rgb::Rgba<f32>>>,
) -> Result<(Measurement, f64, f64, f64), FlowError> {
    let key = MeasurementKey::new(image, &codec, config.pipeline_version, quality, ratio, ppd);

    // Check measurement cache (return 0 timing for cached results)
    if let Some(m) = get_cached_measurement(config, &key) {
        return Ok((m, 0.0, 0.0, 0.0));
    }

    // Get or create encoded file (with timing)
    let encode_start = Instant::now();
    let enc_result = get_or_create_encoded(
        config, ref_bytes, ref_w, ref_h, subdir, image, codec, quality, ratio,
    )?;
    let encode_ms = encode_start.elapsed().as_secs_f64() * 1000.0;

    let file_size = enc_result.bytes.len();
    let encoded_width = enc_result.width;
    let encoded_height = enc_result.height;
    let encoded = enc_result.bytes;
    let encoded_path = enc_result.path;

    //=========================================================================
    // Calculate browser and viewing simulation dimensions
    //=========================================================================
    //
    // Browser Scaling Simulation:
    // ratio < 1.0: Image is HIGHER resolution than display (e.g., 2x srcset)
    //   → Browser UPSAMPLES the decoded image to display size
    //   → Uses Mitchell (CubicBSpline) filter, which spreads artifacts
    //
    // ratio > 1.0: Image is LOWER resolution than display (undersized)
    //   → Browser DOWNSAMPLES both image and reference
    //   → Uses Catmull-Rom filter
    //
    // ratio = 1.0: Image matches display size exactly, no scaling needed
    //
    // Viewing Condition Transform:
    // view_scale = BASELINE_PPD / target_ppd
    // PPD=40 (desktop): scale=1.0, no change (baseline, artifacts most visible)
    // PPD=70 (laptop):  scale=0.57, downsample to 57%
    // PPD=95 (phone):   scale=0.42, downsample to 42% (artifacts blend together)
    //=========================================================================

    let view_scale = BASELINE_PPD as f32 / ppd as f32;

    // Calculate final dimensions for test image after browser + view transforms
    let (test_browser_dim, test_browser_filter) = if ratio < 1.0 {
        // Upsample to display size (ref dimensions)
        (Some((ref_w, ref_h)), Some(s::Filter::CubicBSpline))
    } else if ratio > 1.0 {
        // Downsample to display size
        let dw = (ref_w as f32 / ratio).round() as u32;
        let dh = (ref_h as f32 / ratio).round() as u32;
        // Sanity check: browser sim dimensions must be positive
        if dw == 0 || dh == 0 {
            return Err(nerror!(
                imageflow_core::ErrorKind::InvalidArgument,
                "Browser sim would produce 0-size image: {}x{} / {} = {}x{} for {}/{}",
                ref_w, ref_h, ratio, dw, dh, subdir, image
            ));
        }
        (Some((dw, dh)), Some(s::Filter::CatmullRom))
    } else {
        (None, None)
    };

    // Calculate viewing transform dimensions
    let test_view_dim = if view_scale < 1.0 {
        let base_w = test_browser_dim.map(|(w, _)| w).unwrap_or(encoded_width);
        let base_h = test_browser_dim.map(|(_, h)| h).unwrap_or(encoded_height);
        let vw = (base_w as f32 * view_scale).round() as u32;
        let vh = (base_h as f32 * view_scale).round() as u32;
        // Sanity check: viewing transform dimensions must be positive
        if vw == 0 || vh == 0 {
            return Err(nerror!(
                imageflow_core::ErrorKind::InvalidArgument,
                "View transform would produce 0-size: {}x{} * {} = {}x{} for {}/{}",
                base_w, base_h, view_scale, vw, vh, subdir, image
            ));
        }
        Some((vw, vh))
    } else {
        None
    };

    // Build test image simulation params
    let test_params = ViewSimParams {
        browser_resize: test_browser_dim.zip(test_browser_filter).map(|((w, h), f)| (w, h, f)),
        view_resize: test_view_dim,
    };

    // Calculate reference image transforms
    let (ref_browser_dim, ref_browser_filter) = if ratio > 1.0 {
        // Reference also downsampled for undersized images
        let dw = (ref_w as f32 / ratio).round() as u32;
        let dh = (ref_h as f32 / ratio).round() as u32;
        // Sanity check: dimensions must be positive
        if dw == 0 || dh == 0 {
            return Err(nerror!(
                imageflow_core::ErrorKind::InvalidArgument,
                "Ref browser sim would produce 0-size: {}x{} / {} = {}x{} for {}/{}",
                ref_w, ref_h, ratio, dw, dh, subdir, image
            ));
        }
        (Some((dw, dh)), Some(s::Filter::CatmullRom))
    } else {
        (None, None)
    };

    let ref_view_dim = if view_scale < 1.0 {
        let base_w = ref_browser_dim.map(|(w, _)| w).unwrap_or(ref_w);
        let base_h = ref_browser_dim.map(|(_, h)| h).unwrap_or(ref_h);
        let vw = (base_w as f32 * view_scale).round() as u32;
        let vh = (base_h as f32 * view_scale).round() as u32;
        // Sanity check: viewing transform dimensions must be positive
        if vw == 0 || vh == 0 {
            return Err(nerror!(
                imageflow_core::ErrorKind::InvalidArgument,
                "Ref view transform would produce 0-size: {}x{} * {} = {}x{} for {}/{}",
                base_w, base_h, view_scale, vw, vh, subdir, image
            ));
        }
        Some((vw, vh))
    } else {
        None
    };

    let ref_params = ViewSimParams {
        browser_resize: ref_browser_dim.zip(ref_browser_filter).map(|((w, h), f)| (w, h, f)),
        view_resize: ref_view_dim,
    };

    //=========================================================================
    // Stage 3-5: Decode + Browser sim + Viewing transform + DSSIM
    //=========================================================================
    // All done in-memory without intermediate JPEG encoding
    //=========================================================================

    let decode_start = Instant::now();
    let test_bitmap = decode_with_view_sim(&encoded, &test_params)?;

    // Use cached reference if available, otherwise compute it
    let ref_bitmap_owned;
    let ref_bitmap: &imgref::ImgVec<rgb::Rgba<f32>> = match cached_ref {
        Some(r) => r,
        None => {
            ref_bitmap_owned = decode_with_view_sim(ref_bytes, &ref_params)?;
            &ref_bitmap_owned
        }
    };
    let decode_ms = decode_start.elapsed().as_secs_f64() * 1000.0;

    //=========================================================================
    // Stage 5: DSSIM Measurement
    //=========================================================================
    // DSSIM (Structural Dissimilarity) measures perceptual difference.
    // 0 = identical, higher = more different
    //
    // Typical DSSIM ranges:
    //   < 0.0002:  Visually lossless (impossible to see difference)
    //   0.0002-0.001: Near-lossless (subtle differences under scrutiny)
    //   0.001-0.005:  High quality (minor artifacts on close inspection)
    //   0.005-0.01:   Good quality (visible artifacts in detailed areas)
    //   0.01-0.02:    Fair quality (noticeable compression artifacts)
    //   > 0.02:       Low quality (obvious compression)
    //=========================================================================
    // Sanity check: reference and test must have same dimensions for DSSIM
    if ref_bitmap.width() != test_bitmap.width() || ref_bitmap.height() != test_bitmap.height() {
        return Err(nerror!(
            imageflow_core::ErrorKind::InternalError,
            "Dimension mismatch for DSSIM: ref {}x{} vs test {}x{} for {}/{} {:?} q{} r{} ppd{}",
            ref_bitmap.width(), ref_bitmap.height(),
            test_bitmap.width(), test_bitmap.height(),
            subdir, image, codec, quality, ratio, ppd
        ));
    }

    let dssim_start = Instant::now();
    let dssim = compute_dssim_from_images(ref_bitmap, &test_bitmap)?;
    let dssim_ms = dssim_start.elapsed().as_secs_f64() * 1000.0;

    // Sanity check: DSSIM should be non-negative
    if dssim < 0.0 {
        return Err(nerror!(
            imageflow_core::ErrorKind::InternalError,
            "Negative DSSIM {} for {}/{} {:?} q{} r{} ppd{}",
            dssim, subdir, image, codec, quality, ratio, ppd
        ));
    }

    // Sanity check: DSSIM should not be NaN or infinity
    if !dssim.is_finite() {
        return Err(nerror!(
            imageflow_core::ErrorKind::InternalError,
            "Non-finite DSSIM {} for {}/{} {:?} q{} r{} ppd{}",
            dssim, subdir, image, codec, quality, ratio, ppd
        ));
    }

    // Sanity check: DSSIM > 1.0 is unusual (warn but don't fail)
    // Values > 0.5 are already very bad quality
    if dssim > 1.0 {
        eprintln!(
            "WARNING: Unusually high DSSIM {} for {}/{} {:?} q{} r{} ppd{}",
            dssim, subdir, image, codec, quality, ratio, ppd
        );
    }

    // Relative path for CSV
    let rel_path = encoded_path
        .strip_prefix(&config.cache_dir)
        .unwrap_or(&encoded_path)
        .to_string_lossy()
        .to_string();

    let m = Measurement {
        image: image.to_string(),
        subdir: subdir.to_string(),
        codec: codec.name(),
        codec_speed: match codec {
            Codec::Avif { speed } => Some(speed),
            _ => None,
        },
        quality,
        ratio,
        ppd,
        dssim,
        file_size,
        reference_width: ref_w,
        reference_height: ref_h,
        encoded_width,
        encoded_height,
        encoded_path: rel_path,
    };

    // Cache measurement
    cache_measurement(config, &key, &m);

    Ok((m, encode_ms, decode_ms, dssim_ms))
}

/// Cache key for transformed reference images
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct RefTransformKey {
    ratio_x100: u32,
    ppd: u32,
}

impl RefTransformKey {
    fn new(ratio: f32, ppd: u32) -> Self {
        Self {
            ratio_x100: (ratio * 100.0).round() as u32,
            ppd,
        }
    }
}

/// Timing statistics for pipeline stages
#[derive(Debug, Default)]
struct TimingStats {
    encode_ms: f64,
    decode_ms: f64,
    dssim_ms: f64,
    total_ms: f64,
    count: usize,
}

impl TimingStats {
    fn add(&mut self, encode_ms: f64, decode_ms: f64, dssim_ms: f64, total_ms: f64) {
        self.encode_ms += encode_ms;
        self.decode_ms += decode_ms;
        self.dssim_ms += dssim_ms;
        self.total_ms += total_ms;
        self.count += 1;
    }

    fn print_summary(&self, name: &str) {
        if self.count > 0 {
            println!("    {} timing ({} samples):", name, self.count);
            println!("      Encode: {:.1}ms avg, {:.1}ms total",
                     self.encode_ms / self.count as f64, self.encode_ms);
            println!("      Decode+Sim: {:.1}ms avg, {:.1}ms total",
                     self.decode_ms / self.count as f64, self.decode_ms);
            println!("      DSSIM: {:.1}ms avg, {:.1}ms total",
                     self.dssim_ms / self.count as f64, self.dssim_ms);
            println!("      Total: {:.1}ms avg, {:.1}ms total",
                     self.total_ms / self.count as f64, self.total_ms);
        }
    }
}

/// Process all measurements for a single image
///
/// Precomputes and caches transformed reference images for each (ratio, ppd) combination
/// to avoid redundant work across codec/quality tests.
fn process_image(
    config: &PipelineConfig,
    ref_path: &Path,
    subdir: &str,
    stem: &str,
    meta: &ReferenceMetadata,
) -> Result<Vec<Measurement>, FlowError> {
    let ref_bytes = load_file(ref_path)?;
    let rw = meta.reference_width;
    let rh = meta.reference_height;

    let mut measurements = Vec::new();

    // Build work items
    let mut work: Vec<(Codec, u32, f32, u32)> = Vec::new();
    for &codec in &config.codecs {
        for &q in &config.quality_values {
            // Skip AVIF speed=5 except q=100 (per spec)
            if let Codec::Avif { speed: 5 } = codec {
                if q != 100 {
                    continue;
                }
            }
            for &r in &config.ratios {
                for &p in &config.ppd_values {
                    work.push((codec, q, r, p));
                }
            }
        }
    }

    // Precompute reference transforms for each unique (ratio, ppd) combination
    // This avoids redundant decode+resize for each codec/quality at the same viewing condition
    let ref_cache_start = Instant::now();
    let mut ref_cache: std::collections::HashMap<RefTransformKey, imgref::ImgVec<rgb::Rgba<f32>>> =
        std::collections::HashMap::new();

    for &ratio in &config.ratios {
        for &ppd in &config.ppd_values {
            let key = RefTransformKey::new(ratio, ppd);

            // Calculate reference transform params
            let view_scale = BASELINE_PPD as f32 / ppd as f32;

            let (ref_browser_dim, ref_browser_filter) = if ratio > 1.0 {
                let dw = (rw as f32 / ratio).round() as u32;
                let dh = (rh as f32 / ratio).round() as u32;
                (Some((dw, dh)), Some(s::Filter::CatmullRom))
            } else {
                (None, None)
            };

            let ref_view_dim = if view_scale < 1.0 {
                let base_w = ref_browser_dim.map(|(w, _)| w).unwrap_or(rw);
                let base_h = ref_browser_dim.map(|(_, h)| h).unwrap_or(rh);
                Some(((base_w as f32 * view_scale).round() as u32,
                      (base_h as f32 * view_scale).round() as u32))
            } else {
                None
            };

            let ref_params = ViewSimParams {
                browser_resize: ref_browser_dim.zip(ref_browser_filter).map(|((w, h), f)| (w, h, f)),
                view_resize: ref_view_dim,
            };

            match decode_with_view_sim(&ref_bytes, &ref_params) {
                Ok(bitmap) => { ref_cache.insert(key, bitmap); }
                Err(e) => {
                    eprintln!("    Error precomputing ref transform r{:.2} p{}: {:?}", ratio, ppd, e);
                }
            }
        }
    }
    let ref_cache_elapsed = ref_cache_start.elapsed();
    println!("    Reference cache: {} transforms in {:.1}ms",
             ref_cache.len(), ref_cache_elapsed.as_secs_f64() * 1000.0);

    let counter = AtomicUsize::new(0);
    let total = work.len();

    // Timing stats per codec
    let mut timing_stats: std::collections::HashMap<String, TimingStats> =
        std::collections::HashMap::new();

    for (codec, q, r, p) in work {
        let ref_key = RefTransformKey::new(r, p);
        let cached_ref = ref_cache.get(&ref_key);

        let measurement_start = Instant::now();
        match process_measurement_with_ref_timed(
            config, &ref_bytes, rw, rh, subdir, stem, codec, q, r, p, cached_ref,
        ) {
            Ok((m, encode_ms, decode_ms, dssim_ms)) => {
                let total_ms = measurement_start.elapsed().as_secs_f64() * 1000.0;
                timing_stats.entry(codec.name())
                    .or_default()
                    .add(encode_ms, decode_ms, dssim_ms, total_ms);
                measurements.push(m);
            }
            Err(e) => {
                eprintln!("    Error {}/{} q{} r{:.2} p{}: {:?}", stem, codec.name(), q, r, p, e);
            }
        }

        let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
        if n % 50 == 0 || n == total {
            print!("\r    Progress: {}/{} measurements", n, total);
            std::io::stdout().flush().ok();
        }
    }
    println!();

    // Print timing summary
    for codec_name in ["jpeg", "webp", "avif_s6"] {
        if let Some(stats) = timing_stats.get(codec_name) {
            stats.print_summary(codec_name);
        }
    }

    Ok(measurements)
}

//=============================================================================
// CSV Output
//=============================================================================

fn write_csv(config: &PipelineConfig, measurements: &[Measurement]) -> Result<PathBuf, FlowError> {
    let results_dir = config.results_dir();
    fs::create_dir_all(&results_dir).ok();

    let csv_path = results_dir.join(format!(
        "measurements_p{}.csv",
        config.pipeline_version
    ));

    let file = File::create(&csv_path)
        .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Create CSV: {}", e))?;
    let mut wtr = csv::Writer::from_writer(BufWriter::new(file));

    for m in measurements {
        wtr.serialize(m).map_err(|e| {
            nerror!(imageflow_core::ErrorKind::InternalError, "Write CSV row: {}", e)
        })?;
    }

    wtr.flush()
        .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Flush CSV: {}", e))?;

    Ok(csv_path)
}

//=============================================================================
// Main Pipeline
//=============================================================================

pub fn run_calibration(config: &PipelineConfig) -> Result<Vec<Measurement>, FlowError> {
    let start = Instant::now();

    println!("=== Codec Quality Calibration Pipeline ===");
    println!("Corpus:          {}", config.corpus_dir.display());
    println!("Cache:           {}", config.cache_dir.display());
    println!("Pipeline version: {}", config.pipeline_version);
    println!("Codec versions:   {:?}", config.codecs.iter().map(|c| format!("{}=v{}", c.name(), c.version())).collect::<Vec<_>>());
    println!();
    println!("Test matrix:");
    println!("  Codecs:    {:?}", config.codecs.iter().map(|c| c.name()).collect::<Vec<_>>());
    println!("  Qualities: {} values", config.quality_values.len());
    println!("  Ratios:    {:?}", config.ratios);
    println!("  PPDs:      {:?}", config.ppd_values);
    println!();

    // Stage 1: Prepare references
    println!("Stage 1: Preparing references...");
    let refs = prepare_references(config)?;
    println!("  Found {} images\n", refs.len());

    // Stage 2-6: Process each image
    println!("Stage 2-6: Processing images...");
    let mut all_measurements = Vec::new();

    for (i, (ref_path, subdir, stem, meta)) in refs.iter().enumerate() {
        println!(
            "[{}/{}] {}/{} ({}x{})",
            i + 1,
            refs.len(),
            subdir,
            stem,
            meta.reference_width,
            meta.reference_height
        );

        match process_image(config, ref_path, subdir, stem, meta) {
            Ok(ms) => {
                println!("    Completed: {} measurements", ms.len());
                all_measurements.extend(ms);
            }
            Err(e) => {
                eprintln!("    Error: {:?}", e);
            }
        }
    }

    // Write CSV
    println!("\nWriting results...");
    let csv_path = write_csv(config, &all_measurements)?;

    let elapsed = start.elapsed();
    println!("\n=== Complete ===");
    println!("Total measurements: {}", all_measurements.len());
    println!("CSV output: {}", csv_path.display());
    println!("Time: {:.1}s", elapsed.as_secs_f64());

    Ok(all_measurements)
}

//=============================================================================
// Tests
//=============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_calibration_full() {
        let config = PipelineConfig::default();
        match run_calibration(&config) {
            Ok(ms) => {
                println!("\nSample measurements:");
                for m in ms.iter().take(20) {
                    println!(
                        "  {} {} q={} r={:.2} p={} -> DSSIM={:.6} ({} bytes)",
                        m.image, m.codec, m.quality, m.ratio, m.ppd, m.dssim, m.file_size
                    );
                }
            }
            Err(e) => eprintln!("Error: {:?}", e),
        }
    }

    #[test]
    #[ignore]
    fn test_calibration_mini() {
        // Mini test with reduced matrix
        let config = PipelineConfig {
            quality_values: vec![50, 75, 90],
            ratios: vec![0.5, 1.0, 2.0],
            ppd_values: vec![40, 70],
            codecs: vec![Codec::Mozjpeg, Codec::WebP],
            ..PipelineConfig::default()
        };

        match run_calibration(&config) {
            Ok(ms) => {
                println!("\nMini calibration complete: {} measurements", ms.len());
            }
            Err(e) => eprintln!("Error: {:?}", e),
        }
    }
}
