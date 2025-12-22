//! Codec Quality Calibration Pipeline
//!
//! Builds a mapping database that translates JPEG quality values to equivalent
//! WebP and AVIF quality values across different viewing conditions.
//!
//! # Known Issues / TODOs
//!
//! ## Bug: Resample2D used instead of Constrain
//! The current implementation uses `Resample2D` which ignores aspect ratio and can
//! upsample images. Should use `Constrain` to preserve aspect ratio and only downsample.
//! Original images should be reduced to 1/4 size to minimize starting compression artifacts.
//!
//! ## Bug: Missing dimensions in output
//! The JSON and CSV output only includes the ratio, not the actual encoded dimensions.
//! Should include `encoded_width` and `encoded_height` for each measurement.
//!
//! ## Bug: Excessive JPEG q100 re-encoding in process_measurement
//! The current approach re-encodes to JPEG q100 at multiple stages:
//! - After decoding the compressed file
//! - After browser simulation resizing
//! - After viewing transform resizing
//!
//! This is wasteful and introduces unnecessary quality loss. DSSIM works fine with
//! in-memory bitmaps. Should:
//! 1. Decode once to bitmap
//! 2. Apply browser simulation resize (in-memory)
//! 3. Apply viewing transform resize (in-memory)
//! 4. Pass bitmaps directly to DSSIM
//!
//! ## Bug: Browser/viewer transformed references not cached
//! For each (ratio, ppd) combination, the reference image goes through the same
//! browser + viewing transform. These should be cached per (image, ratio, ppd) to
//! avoid redundant work when testing multiple codecs/qualities.
//!
//! ## Optimization: Use Constrain node with chained resizes
//! Could use `Constrain` node to decode → browser sim (CubicBSpline or CatmullRom)
//! → viewing transform (Lanczos), keeping everything in memory. The two-stage
//! resize could potentially be combined into a single operation.
//!
//! ## TODO: Extend quality and ratio coverage
//! Quality values should include:
//! - q0, q5, q10 for proper interpolation at the low end
//! - Popular JPEG values: 60, 75, 80, 85, 90 (already have most)
//! - Popular WebP values: 75, 80, 85 (WebP defaults to 75)
//!
//! Ratios should include:
//! - 0.25 (4x srcset, ultra-high DPI)
//! - 4.0 (very undersized, 0.25x)
//!
//! ## TODO: Add instrumentation
//! Need timing instrumentation to identify bottlenecks:
//! - Encoding time per codec/quality
//! - DSSIM computation time
//! - Reference preparation time
//! - Browser/viewing transform time
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
//! The pipeline caches at two levels for fast iteration:
//! 1. **Encoded files**: Cached with version stamp. Change `ENCODER_VERSION` to invalidate.
//! 2. **Measurements**: Cached with version stamp. Change `PIPELINE_VERSION` to invalidate.
//!
//! Browser simulation and viewing transforms run in-memory (fast, no caching needed).
//!
//! ## Cache-Breaking
//!
//! - Change `ENCODER_VERSION` when: encoder settings change (e.g., AVIF speed)
//! - Change `PIPELINE_VERSION` when: viewing model changes (browser kernels, PPD interpretation)
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

/// Bump when encoder settings change (e.g., AVIF speed, WebP method)
const ENCODER_VERSION: u32 = 1;

/// Bump when viewing model changes (browser kernels, PPD values, DSSIM method)
const PIPELINE_VERSION: u32 = 1;

//=============================================================================
// Test Matrix Configuration
//=============================================================================

/// Quality values to test (profile anchor points + interpolation)
/// Includes common values (25, 50, 75) plus fine-grained points for accurate curve fitting
const QUALITY_VALUES: &[u32] =
    &[15, 20, 25, 30, 34, 40, 45, 50, 55, 57, 60, 65, 70, 73, 76, 80, 85, 89, 91, 95, 96, 100];

/// Intrinsic/device pixel ratios for srcset simulation
///
/// ```text
/// ratio = encoded_size / display_size
///
/// 0.33 → 3x srcset (e.g., 600px image displayed at 1800px CSS width)
/// 0.50 → 2x srcset (retina, most common high-DPI case)
/// 0.67 → 1.5x srcset
/// 1.00 → 1x srcset (image matches display size exactly)
/// 1.50 → undersized (image is 1.5x larger than needed)
/// 2.00 → very undersized (image is 2x larger than needed, browser downsamples)
/// ```
const RATIOS: &[f32] = &[0.33, 0.5, 0.67, 1.0, 1.5, 2.0];

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

//=============================================================================
// Codec Configuration
//=============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Codec {
    Jpeg,
    WebP,
    Avif { speed: u8 },
}

impl Codec {
    fn extension(&self) -> &'static str {
        match self {
            Codec::Jpeg => "jpg",
            Codec::WebP => "webp",
            Codec::Avif { .. } => "avif",
        }
    }

    fn name(&self) -> String {
        match self {
            Codec::Jpeg => "jpeg".to_string(),
            Codec::WebP => "webp".to_string(),
            Codec::Avif { speed } => format!("avif_s{}", speed),
        }
    }

    /// All codecs to test
    fn all() -> Vec<Codec> {
        vec![Codec::Jpeg, Codec::WebP, Codec::Avif { speed: 6 }]
    }

    /// Get encoder preset for this codec
    fn encoder_preset(&self, quality: u32) -> s::EncoderPreset {
        match self {
            Codec::Jpeg => s::EncoderPreset::Mozjpeg {
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
///
/// FIXME: Missing encoded_width and encoded_height fields.
/// Currently only stores ratio, but actual dimensions are needed for analysis.
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
    // FIXME: Add these fields:
    // pub encoded_width: u32,
    // pub encoded_height: u32,
    /// Path to encoded file (relative to cache_dir)
    pub encoded_path: String,
}

/// Cache key for encoded files
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EncodedKey {
    image: String,
    codec: String,
    quality: u32,
    ratio_x100: u32,
}

impl EncodedKey {
    fn new(image: &str, codec: &Codec, quality: u32, ratio: f32) -> Self {
        Self {
            image: image.to_string(),
            codec: codec.name(),
            quality,
            ratio_x100: (ratio * 100.0).round() as u32,
        }
    }

    fn filename(&self, ext: &str) -> String {
        if self.ratio_x100 == 100 {
            format!("{}_q{}.{}", self.codec, self.quality, ext)
        } else {
            format!("{}_q{}_r{}.{}", self.codec, self.quality, self.ratio_x100, ext)
        }
    }
}

/// Cache key for measurements
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct MeasurementKey {
    image: String,
    codec: String,
    quality: u32,
    ratio_x100: u32,
    ppd: u32,
}

impl MeasurementKey {
    fn new(image: &str, codec: &Codec, quality: u32, ratio: f32, ppd: u32) -> Self {
        Self {
            image: image.to_string(),
            codec: codec.name(),
            quality,
            ratio_x100: (ratio * 100.0).round() as u32,
            ppd,
        }
    }

    fn filename(&self) -> String {
        format!(
            "{}_{}_q{}_r{}_p{}.json",
            self.image, self.codec, self.quality, self.ratio_x100, self.ppd
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
    pub encoder_version: u32,
    pub pipeline_version: u32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            corpus_dir: PathBuf::from("/mnt/v/work/corpus"),
            cache_dir: PathBuf::from("/mnt/v/work/corpus_cache"),
            quality_values: QUALITY_VALUES.to_vec(),
            ratios: RATIOS.to_vec(),
            ppd_values: PPD_VALUES.to_vec(),
            codecs: Codec::all(),
            max_reference_dim: MAX_REFERENCE_DIM,
            encoder_version: ENCODER_VERSION,
            pipeline_version: PIPELINE_VERSION,
        }
    }
}

impl PipelineConfig {
    fn references_dir(&self) -> PathBuf {
        self.cache_dir.join("references")
    }

    fn encoded_dir(&self) -> PathBuf {
        self.cache_dir.join(format!("encoded_v{}", self.encoder_version))
    }

    fn measurements_dir(&self) -> PathBuf {
        self.cache_dir.join(format!("measurements_v{}", self.pipeline_version))
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

fn get_dimensions(bytes: &[u8]) -> Result<(u32, u32), FlowError> {
    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    ctx.add_input_vector(0, bytes.to_vec()).map_err(|e| e.at(here!()))?;

    let resp = ctx
        .execute_1(s::Execute001 {
            security: None,
            graph_recording: None,
            framewise: s::Framewise::Steps(vec![Node::Decode { io_id: 0, commands: None }]),
        })
        .map_err(|e| e.at(here!()))?;

    match resp {
        ResponsePayload::JobResult(job) | ResponsePayload::BuildResult(job) => {
            if let Some(d) = job.decodes.first() {
                Ok((d.w as u32, d.h as u32))
            } else {
                Err(nerror!(imageflow_core::ErrorKind::InternalError, "No decode result"))
            }
        }
        _ => Err(nerror!(imageflow_core::ErrorKind::InternalError, "Unexpected response")),
    }
}

/// Resample and save as JPEG q100 (near-lossless intermediate)
///
/// FIXME: This function is overused - we re-encode to JPEG q100 multiple times
/// when we could keep bitmaps in memory. Each encode/decode cycle loses quality.
/// Should refactor to work with in-memory bitmaps where possible.
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
            // FIXME: Should use Constrain instead of Resample2D to:
            // 1. Preserve aspect ratio
            // 2. Only downsample, never upsample
            // 3. Handle non-square images correctly
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

/// Encode with specific codec/quality, optionally resizing first
fn encode_image(
    src: &[u8],
    codec: Codec,
    quality: u32,
    resize: Option<(u32, u32)>,
) -> Result<Vec<u8>, FlowError> {
    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    ctx.add_input_vector(0, src.to_vec()).map_err(|e| e.at(here!()))?;
    ctx.add_output_buffer(1).map_err(|e| e.at(here!()))?;

    let mut steps: Vec<Node> = vec![Node::Decode { io_id: 0, commands: None }];

    if let Some((w, h)) = resize {
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

fn compute_dssim(reference: &[u8], test: &[u8]) -> Result<f64, FlowError> {
    let ref_img = load_for_dssim(reference)?;
    let test_img = load_for_dssim(test)?;

    let d = dssim::new();
    let ref_d = d.create_image(&ref_img).ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "DSSIM failed to create reference image")
    })?;
    let test_d = d.create_image(&test_img).ok_or_else(|| {
        nerror!(imageflow_core::ErrorKind::InternalError, "DSSIM failed to create test image")
    })?;

    let (val, _) = d.compare(&ref_d, test_d);
    Ok(val.into())
}

fn load_for_dssim(bytes: &[u8]) -> Result<imgref::ImgVec<rgb::Rgba<f32>>, FlowError> {
    use dssim::*;
    use s::PixelLayout;

    let mut ctx = Context::create().map_err(|e| e.at(here!()))?;
    #[cfg(feature = "bad-avif-decoder")]
    ctx.enabled_codecs.enable_bad_avif_decoder();
    ctx.add_input_vector(0, bytes.to_vec()).map_err(|e| e.at(here!()))?;

    let mut bmp = BitmapBgraContainer::empty();

    ctx.execute_1(s::Execute001 {
        security: None,
        graph_recording: None,
        framewise: s::Framewise::Steps(vec![Node::Decode { io_id: 0, commands: None }, unsafe {
            bmp.as_mut().get_node()
        }]),
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

//=============================================================================
// Pipeline Stages
//=============================================================================

/// Stage 1: Prepare reference images
///
/// FIXME: Should reduce original images to 1/4 their size (not just max 1600px)
/// to minimize starting compression artifacts from the source JPEG.
/// Currently may use source images that are already heavily compressed.
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
        let (sw, sh) = get_dimensions(&src_bytes)?;

        let max_dim = sw.max(sh);
        let (rw, rh, factor) = if max_dim > config.max_reference_dim {
            let scale = config.max_reference_dim as f32 / max_dim as f32;
            ((sw as f32 * scale).round() as u32, (sh as f32 * scale).round() as u32, scale)
        } else {
            (sw, sh, 1.0)
        };

        // Create reference as JPEG q100
        let ref_bytes = if factor < 1.0 {
            resample_to_jpeg100(&src_bytes, rw, rh, s::Filter::Lanczos)?
        } else {
            decode_to_jpeg100(&src_bytes)?
        };

        // Atomic write: temp file + rename
        let temp_path = out_dir.join(format!("{}.tmp", stem));
        fs::write(&temp_path, &ref_bytes)
            .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Write ref: {}", e))?;
        fs::rename(&temp_path, &ref_path)
            .map_err(|e| nerror!(imageflow_core::ErrorKind::InternalError, "Rename ref: {}", e))?;

        let meta = ReferenceMetadata {
            source_path: src.to_string_lossy().to_string(),
            source_width: sw,
            source_height: sh,
            reference_width: rw,
            reference_height: rh,
            downsample_factor: factor,
        };

        fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).ok();
        results.push((ref_path, subdir, stem, meta));
    }

    Ok(results)
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
) -> Result<(Vec<u8>, PathBuf), FlowError> {
    let key = EncodedKey::new(image, &codec, quality, ratio);
    let out_dir = config.encoded_dir().join(subdir).join(image);
    fs::create_dir_all(&out_dir).ok();

    let filename = key.filename(codec.extension());
    let out_path = out_dir.join(&filename);

    // Check cache
    if out_path.exists() {
        let bytes = load_file(&out_path)?;
        return Ok((bytes, out_path));
    }

    // Calculate intrinsic size for ratio < 1.0
    let (intrinsic_w, intrinsic_h) = if ratio < 1.0 {
        ((ref_w as f32 * ratio).round() as u32, (ref_h as f32 * ratio).round() as u32)
    } else {
        (ref_w, ref_h)
    };

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

    Ok((encoded, out_path))
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

/// Stages 3-6: Browser sim + viewing transform + DSSIM (in-memory)
///
/// FIXME: This function has several inefficiencies:
/// 1. Re-encodes to JPEG q100 multiple times (decode_to_jpeg100, resample_to_jpeg100)
///    when DSSIM can work directly with in-memory bitmaps
/// 2. Does not cache the browser+viewing transformed reference image
///    (same transform is applied for every codec/quality at same ratio/ppd)
/// 3. Should use Constrain node with chained resizes instead of separate operations
///
/// Ideal approach:
/// - Cache reference transformed per (image, ratio, ppd)
/// - Decode test image once to bitmap
/// - Apply browser sim resize in-memory
/// - Apply viewing transform resize in-memory
/// - Pass bitmaps directly to DSSIM
fn process_measurement(
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
) -> Result<Measurement, FlowError> {
    let key = MeasurementKey::new(image, &codec, quality, ratio, ppd);

    // Check measurement cache
    if let Some(m) = get_cached_measurement(config, &key) {
        return Ok(m);
    }

    // Get or create encoded file
    let (encoded, encoded_path) = get_or_create_encoded(
        config, ref_bytes, ref_w, ref_h, subdir, image, codec, quality, ratio,
    )?;
    let file_size = encoded.len();

    // Decode to JPEG q100 for processing
    let decoded = decode_to_jpeg100(&encoded)?;

    //=========================================================================
    // Stage 3: Browser Scaling Simulation
    //=========================================================================
    // Simulates how the browser would display this srcset image.
    //
    // ratio < 1.0: Image is HIGHER resolution than display (e.g., 2x srcset)
    //   → Browser UPSAMPLES the decoded image to display size
    //   → Uses Mitchell (CubicBSpline) filter, which spreads artifacts
    //   → This makes compression artifacts MORE visible
    //
    // ratio > 1.0: Image is LOWER resolution than display (undersized)
    //   → Browser DOWNSAMPLES both image and reference
    //   → Uses Catmull-Rom filter
    //   → This hides some artifacts through downsampling
    //
    // ratio = 1.0: Image matches display size exactly, no scaling needed
    //=========================================================================
    let browser_sim = if ratio < 1.0 {
        // 2x/3x srcset case: browser upsamples small encoded image to display size
        // Mitchell filter is commonly used by browsers for upsampling
        resample_to_jpeg100(&decoded, ref_w, ref_h, s::Filter::CubicBSpline)?
    } else if ratio > 1.0 {
        // Undersized case: browser downsamples to fit display
        // Catmull-Rom is sharp and commonly used for downsampling
        let dw = (ref_w as f32 / ratio).round() as u32;
        let dh = (ref_h as f32 / ratio).round() as u32;
        resample_to_jpeg100(&decoded, dw, dh, s::Filter::CatmullRom)?
    } else {
        decoded
    };

    // Apply same browser transform to reference for fair comparison
    let browser_ref = if ratio > 1.0 {
        let dw = (ref_w as f32 / ratio).round() as u32;
        let dh = (ref_h as f32 / ratio).round() as u32;
        resample_to_jpeg100(ref_bytes, dw, dh, s::Filter::CatmullRom)?
    } else {
        ref_bytes.to_vec()
    };

    //=========================================================================
    // Stage 4: Viewing Condition Transform (Retinal Resolution Simulation)
    //=========================================================================
    // Simulates the limited resolving power of the human eye at different
    // viewing conditions. At high PPD (phone), multiple pixels fall within
    // one "visual pixel" (1 arcminute), so we downsample to simulate this.
    //
    // view_scale = BASELINE_PPD / target_ppd
    //
    // PPD=40 (desktop): scale=1.0, no change (baseline, artifacts most visible)
    // PPD=70 (laptop):  scale=0.57, downsample to 57%
    // PPD=95 (phone):   scale=0.42, downsample to 42% (artifacts blend together)
    //
    // We downsample BOTH reference and test equally, so this doesn't change
    // which codec "wins" - it only affects the absolute DSSIM values.
    //=========================================================================
    let view_scale = BASELINE_PPD as f32 / ppd as f32;
    let (browser_w, browser_h) = get_dimensions(&browser_sim)?;

    let (viewed, viewed_ref) = if view_scale < 1.0 {
        // Higher PPD than baseline: downsample to simulate reduced visual acuity
        // Using Lanczos for high-quality downsampling
        let vw = (browser_w as f32 * view_scale).round() as u32;
        let vh = (browser_h as f32 * view_scale).round() as u32;
        let v = resample_to_jpeg100(&browser_sim, vw, vh, s::Filter::Lanczos)?;

        let (brw, brh) = get_dimensions(&browser_ref)?;
        let vrw = (brw as f32 * view_scale).round() as u32;
        let vrh = (brh as f32 * view_scale).round() as u32;
        let vr = resample_to_jpeg100(&browser_ref, vrw, vrh, s::Filter::Lanczos)?;

        (v, vr)
    } else {
        // PPD <= baseline: no transform needed (or would need to upsample,
        // which we skip since it doesn't add information)
        (browser_sim, browser_ref)
    };

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
    let dssim = compute_dssim(&viewed_ref, &viewed)?;

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
        encoded_path: rel_path,
    };

    // Cache measurement
    cache_measurement(config, &key, &m);

    Ok(m)
}

/// Process all measurements for a single image
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

    let counter = AtomicUsize::new(0);
    let total = work.len();

    for (codec, q, r, p) in work {
        match process_measurement(config, &ref_bytes, rw, rh, subdir, stem, codec, q, r, p) {
            Ok(m) => measurements.push(m),
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

    Ok(measurements)
}

//=============================================================================
// CSV Output
//=============================================================================

fn write_csv(config: &PipelineConfig, measurements: &[Measurement]) -> Result<PathBuf, FlowError> {
    let results_dir = config.results_dir();
    fs::create_dir_all(&results_dir).ok();

    let csv_path = results_dir.join(format!(
        "measurements_enc{}_pipe{}.csv",
        config.encoder_version, config.pipeline_version
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
    println!("Encoder version: {}", config.encoder_version);
    println!("Pipeline version:{}", config.pipeline_version);
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
            codecs: vec![Codec::Jpeg, Codec::WebP],
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
