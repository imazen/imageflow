use imageflow_core::{here, nerror};
#[allow(unused_imports)]
use imageflow_helpers as hlp;
use imageflow_types as s;

#[macro_use]
pub mod macros;
pub mod upload_tracker;

use imageflow_core::graphics::bitmaps::BitmapWindowMut;
use imageflow_core::{Context, ErrorKind, FlowError};
use std::io::Write as _;
use std::path::Path;

use imageflow_core;
use std::path::PathBuf;
use std::{self, panic};

use imageflow_core::BitmapKey;
use imageflow_types::ResponsePayload;
use std::time::Duration;
use zensim_regress::checksums::{CheckResult, ChecksumManager};
pub use zensim_regress::Tolerance;

/// Process-wide test init. Called lazily before any test needs a Context.
/// Sets CMS to Both mode so moxcms and lcms2 are compared on every transform.
pub fn test_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        imageflow_core::CmsBackend::set_process_default(imageflow_core::CmsBackend::Both);
        imageflow_core::CmsBackend::enable_stderr_diagnostics();
    });
}

/// Global `ChecksumManager`, shared across all test threads.
///
/// Configured with hardcoded S3 defaults for imageflow reference images,
/// env var overrides, diff output directory, and manifest from env.
/// Replaces the old `ChecksumCtx` + `ChecksumAdapter` + `global_manifest()`.
fn global_manager() -> &'static ChecksumManager {
    use std::sync::OnceLock;
    static MANAGER: OnceLock<ChecksumManager> = OnceLock::new();
    MANAGER.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let checksums_dir = manifest_dir.join("tests/integration/visuals");
        let workspace_root = manifest_dir.parent().expect("CARGO_MANIFEST_DIR has no parent");

        // Remote storage with hardcoded S3 defaults
        let cache_dir = checksums_dir.join(".remote-cache");
        let download_url = std::env::var("REGRESS_REFERENCE_URL")
            .ok()
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .unwrap_or_else(|| {
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums"
                    .to_string()
            });
        let upload_prefix = std::env::var("REGRESS_UPLOAD_PREFIX")
            .ok()
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .or_else(|| Some("s3://imageflow-resources/visual_test_checksums".to_string()));
        let upload_enabled =
            std::env::var("UPLOAD_REFERENCES").is_ok_and(|v| v == "1" || v == "true");

        let remote = zensim_regress::remote::ReferenceStorage::new(
            download_url,
            upload_prefix,
            upload_enabled,
            cache_dir,
        );

        // Diff output directory
        let diff_dir = workspace_root.join(".image-cache/diffs");
        let _ = std::fs::create_dir_all(&diff_dir);

        // Never use update_mode — it prunes old baselines and replaces them.
        // WithinTolerance passes tests without writing to disk.
        // New baselines are created only via CREATE_BASELINES=1 (handled in handle_check_result).
        ChecksumManager::with_modes(&checksums_dir, false)
            .with_remote_storage(remote)
            .with_diff_output(diff_dir)
            .with_manifest_from_env()
    })
}

/// Returns true if `CREATE_BASELINES=1` env var is set.
/// This only allows creating entries for brand-new tests — never replaces existing baselines.
fn create_baselines_mode() -> bool {
    static MODE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| std::env::var("CREATE_BASELINES").is_ok_and(|v| v == "1" || v == "true"))
}

/// Handle a `CheckResult` — return true on pass, panic on fail.
fn handle_check_result(result: &Result<CheckResult, zensim_regress::RegressError>) -> bool {
    match result {
        Ok(CheckResult::Match { .. }) | Ok(CheckResult::WithinTolerance { .. }) => true,
        Ok(CheckResult::NoBaseline { .. }) if create_baselines_mode() => true,
        Ok(CheckResult::NoBaseline { .. }) => {
            panic!("No baseline. Run with CREATE_BASELINES=1 to create the initial baseline.");
        }
        Ok(CheckResult::Failed { report, .. }) => {
            let msg = report
                .as_ref()
                .map(|r| format!("{r}"))
                .unwrap_or_else(|| "checksum mismatch, no pixel comparison available".to_string());
            panic!("{msg}");
        }
        Err(e) => {
            panic!("comparison error: {e}");
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum IoTestEnum {
    ByteArray(Vec<u8>),
    OutputBuffer,
    Url(String),
}

/// Source cache directory at workspace root `.image-cache/sources/`.
fn source_cache_dir() -> PathBuf {
    let workspace_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("CARGO_MANIFEST_DIR has no parent");
    let dir = workspace_root.join(".image-cache/sources");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Convert a URL to a relative cache path by stripping scheme and host.
///
/// `"https://s3.amazonaws.com/bucket/test_inputs/photo.jpg"`
/// → `"bucket/test_inputs/photo.jpg"`
fn url_to_cache_path(url: &str) -> PathBuf {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let after_host = after_scheme.split_once('/').map(|(_, p)| p).unwrap_or(after_scheme);
    PathBuf::from(after_host)
}

/// Fetch URL bytes with local caching and retry.
///
/// Caches downloaded files in `.image-cache/sources/` at the workspace root,
/// using the URL path as the cache key. Subsequent calls with the same URL
/// return the cached bytes without making a network request.
pub fn get_url_bytes_with_retry(url: &str) -> Result<Vec<u8>, FlowError> {
    let cache_dir = source_cache_dir();
    let rel_path = url_to_cache_path(url);
    let full_path = cache_dir.join(&rel_path);

    if full_path.exists() {
        return std::fs::read(&full_path)
            .map_err(|e| nerror!(ErrorKind::FetchError, "{}: {}", full_path.display(), e));
    }

    // Download with retry
    let bytes = fetch_url_with_retry(url)?;

    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&full_path, &bytes).unwrap();

    Ok(bytes)
}

/// Internal: download URL bytes with exponential backoff retry.
fn fetch_url_with_retry(url: &str) -> Result<Vec<u8>, FlowError> {
    let mut retry_count = 3;
    let mut retry_wait = 100;
    loop {
        match ::imageflow_http_helpers::fetch_bytes(url)
            .map_err(|e| nerror!(ErrorKind::FetchError, "{}: {}", url, e))
        {
            Err(e) => {
                if retry_count > 0 {
                    retry_count -= 1;
                    std::thread::sleep(Duration::from_millis(retry_wait));
                    retry_wait *= 5;
                } else {
                    return Err(e);
                }
            }
            Ok(bytes) => {
                return Ok(bytes);
            }
        }
    }
}

pub struct IoTestTranslator;
impl IoTestTranslator {
    pub fn add(&self, c: &mut Context, io_id: i32, io_enum: IoTestEnum) -> Result<(), FlowError> {
        match io_enum {
            IoTestEnum::ByteArray(vec) => {
                c.add_copied_input_buffer(io_id, &vec).map_err(|e| e.at(here!()))
            }
            IoTestEnum::Url(url) => {
                let bytes = get_url_bytes_with_retry(&url).map_err(|e| e.at(here!()))?;
                c.add_input_vector(io_id, bytes).map_err(|e| e.at(here!()))
            }

            IoTestEnum::OutputBuffer => c.add_output_buffer(io_id).map_err(|e| e.at(here!())),
        }
    }
}

pub fn build_steps(
    context: &mut Context,
    steps: &[s::Node],
    io: Vec<IoTestEnum>,
    security: Option<imageflow_types::ExecutionSecurity>,
    debug: bool,
) -> Result<ResponsePayload, FlowError> {
    build_framewise(context, s::Framewise::Steps(steps.to_vec()), io, security, debug)
        .map_err(|e| e.at(here!()))
}

pub fn build_framewise(
    context: &mut Context,
    framewise: s::Framewise,
    io: Vec<IoTestEnum>,
    security: Option<imageflow_types::ExecutionSecurity>,
    debug: bool,
) -> Result<ResponsePayload, FlowError> {
    test_init();
    for (ix, val) in io.into_iter().enumerate() {
        IoTestTranslator {}.add(context, ix as i32, val)?;
    }
    let build =
        s::Execute001 { security, graph_recording: default_graph_recording(debug), framewise };
    if debug {
        println!("{}", serde_json::to_string_pretty(&build).unwrap());
    }

    context.execute_1(build)
}

/// Executes the given steps (adding a frame buffer container to the end of them).
/// Returns the width and height of the resulting frame.
/// Steps must be open-ended - they cannot be terminated with an encoder.
pub fn get_result_dimensions(steps: &[s::Node], io: Vec<IoTestEnum>, debug: bool) -> (u32, u32) {
    let capture_id = 0;
    let mut steps = steps.to_vec();
    steps.push(s::Node::CaptureBitmapKey { capture_id });

    let mut context = Context::create().unwrap();

    let result = build_steps(&mut context, &steps, io, None, debug).unwrap();

    let bitmap_key = context
        .get_captured_bitmap_key(capture_id)
        .unwrap_or_else(|| panic!("execution failed: {:?}", result));
    let bitmaps = context.borrow_bitmaps().unwrap();
    let bm = bitmaps.try_borrow_mut(bitmap_key).unwrap();
    let (w, h) = bm.size();
    (w as u32, h as u32)
}

/// Just validates that no errors are thrown during job execution
pub fn smoke_test(
    input: Option<IoTestEnum>,
    output: Option<IoTestEnum>,
    security: Option<imageflow_types::ExecutionSecurity>,
    debug: bool,
    steps: Vec<s::Node>,
) -> Result<s::ResponsePayload, imageflow_core::FlowError> {
    let mut io_list = Vec::new();
    if input.is_some() {
        io_list.push(input.unwrap());
    }
    if output.is_some() {
        io_list.push(output.unwrap());
    }
    let mut context = Context::create().unwrap();
    build_steps(&mut context, &steps, io_list, security, debug)
}

/// Checksum encoded bytes using seahash + file extension.
pub fn checksum_bytes(bytes: &[u8]) -> String {
    let h = seahash::hash(bytes);
    format!("sea:{h:016x}.{}", file_extension_for_bytes(bytes))
}

pub fn file_extension_for_bytes(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        "png"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && bytes[8..12].starts_with(b"WEBP") {
        "webp"
    } else {
        "unknown"
    }
}

/// Checksum bitmap pixels using seahash with dimensions baked in.
///
/// Iterates scanlines to exclude stride padding. Dimensions are prepended
/// to avoid collisions between differently-shaped images.
pub fn checksum_bitmap_window(bitmap_window: &mut BitmapWindowMut<u8>) -> String {
    let w = bitmap_window.w() as u32;
    let h = bitmap_window.h() as u32;

    let mut buf = Vec::with_capacity(8 + (w as usize * h as usize * 4));
    buf.extend_from_slice(&w.to_le_bytes());
    buf.extend_from_slice(&h.to_le_bytes());
    for line in bitmap_window.scanlines() {
        buf.extend_from_slice(line.row());
    }

    let hash = seahash::hash(&buf);
    format!("sea:{hash:016x}")
}

pub fn decode_image(c: &mut Context, io_id: i32) -> BitmapKey {
    try_decode_image(c, io_id).expect("decode_image failed")
}

/// Decode an input image, returning None on failure instead of panicking.
fn try_decode_image(c: &mut Context, io_id: i32) -> Option<BitmapKey> {
    let capture_id = 0;
    let result = c.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id, commands: None },
            s::Node::CaptureBitmapKey { capture_id },
        ]),
    });

    result.ok()?;
    c.get_captured_bitmap_key(capture_id)
}

pub fn decode_input(c: &mut Context, input: IoTestEnum) -> BitmapKey {
    let capture_id = 0;

    let _result = build_steps(
        c,
        &[s::Node::Decode { io_id: 0, commands: None }, s::Node::CaptureBitmapKey { capture_id }],
        vec![input],
        None,
        false,
    )
    .unwrap();

    c.get_captured_bitmap_key(capture_id).unwrap()
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Similarity {
    AllowOffByOneBytesCount(i64),
    AllowOffByOneBytesRatio(f32),
    /// Maximum allowed zdsim (zen dissimilarity, 0-1 scale).
    /// 0.0 = exact match, higher = more different.
    /// Mirrors DSSIM semantics: drop-in replacement for AllowDssimMatch.
    /// zdsim = (100 - zensim_score) / 100.
    MaxZdsim(f64),
}

impl Similarity {
    /// Convert to a `Tolerance` for `.checksums` files and pixel comparison.
    pub fn to_tolerance_spec(&self) -> Tolerance {
        match *self {
            Similarity::AllowOffByOneBytesCount(n) => {
                if n == 0 {
                    Tolerance::exact()
                } else {
                    // Can't convert absolute byte count to pixel fraction without
                    // knowing image size; use 1% as a reasonable upper bound.
                    Tolerance {
                        max_delta: 1,
                        min_similarity: 99.0,
                        max_pixels_different: 0.01,
                        ..Tolerance::exact()
                    }
                }
            }
            Similarity::AllowOffByOneBytesRatio(ratio) => Tolerance {
                max_delta: 1,
                min_similarity: 99.0,
                max_pixels_different: ratio as f64,
                ..Tolerance::exact()
            },
            Similarity::MaxZdsim(max_zdsim) => {
                assert!(
                    max_zdsim < 1.0,
                    "MaxZdsim({max_zdsim}) is >= 1.0 — this disables similarity checking entirely. \
                     Use a meaningful threshold or restructure the test."
                );
                if max_zdsim <= 0.0 {
                    Tolerance::exact()
                } else {
                    // Perceptual metric only — no per-pixel delta constraints.
                    // zdsim is a perceptual dissimilarity score; converting it to
                    // byte-level deltas would corrupt the original intent.
                    let min_similarity = (100.0 * (1.0 - max_zdsim)).max(0.0);
                    Tolerance {
                        max_delta: 255,
                        min_similarity,
                        max_pixels_different: 1.0,
                        ..Tolerance::exact()
                    }
                }
            }
        }
    }

    /// Convert to a `RegressionTolerance` for direct bitmap comparison.
    fn to_regression_tolerance_for_comparison(
        &self,
    ) -> zensim_regress::testing::RegressionTolerance {
        let spec = self.to_tolerance_spec();
        spec.to_regression_tolerance(zensim_regress::arch::detect_arch_tag())
    }
}

#[derive(Clone)]
pub struct Constraints {
    /// If `None`, skip pixel similarity comparison (file-size-only test).
    pub similarity: Option<Similarity>,
    pub max_file_size: Option<usize>,
}

/// Compare two BGRA bitmaps using zensim-regress.
///
/// Returns `(passed, zdsim)` — the pass/fail result and the measured zdsim
/// dissimilarity (0 = identical, higher = worse).
///
/// Prints the regression report to stderr. On failure, saves a comparison
/// diff PNG to `.image-cache/diffs/`. On failure with `do_panic`, panics.
fn compare_bitmaps_zensim(
    actual: &BitmapWindowMut<u8>,
    expected: &BitmapWindowMut<u8>,
    tolerance: &zensim_regress::testing::RegressionTolerance,
    diff_name: &str,
    do_panic: bool,
) -> (bool, f64) {
    use zensim::{PixelFormat, StridedBytes, Zensim, ZensimProfile};
    use zensim_regress::testing::check_regression;

    let (aw, ah) = (actual.w() as usize, actual.h() as usize);
    let (ew, eh) = (expected.w() as usize, expected.h() as usize);
    assert_eq!(
        (aw, ah),
        (ew, eh),
        "bitmap dimensions differ: actual {aw}x{ah} vs expected {ew}x{eh}"
    );

    let actual_stride = actual.info().t_stride() as usize;
    let expected_stride = expected.info().t_stride() as usize;

    let actual_img =
        StridedBytes::try_new(actual.get_slice(), aw, ah, actual_stride, PixelFormat::Srgb8Bgra)
            .expect("actual bitmap: invalid for zensim");
    let expected_img = StridedBytes::try_new(
        expected.get_slice(),
        ew,
        eh,
        expected_stride,
        PixelFormat::Srgb8Bgra,
    )
    .expect("expected bitmap: invalid for zensim");

    let z = Zensim::new(ZensimProfile::latest());
    let report = check_regression(&z, &expected_img, &actual_img, tolerance)
        .expect("zensim comparison failed");

    let zdsim = zensim::score_to_dissimilarity(report.score());

    // Compute ideal diff amplification: min(10, 255/max_diff_pixel)
    let max_delta = *report.max_channel_delta().iter().max().unwrap_or(&0);
    let amplification = zensim_regress::report::ideal_amplification(max_delta);

    if !report.passed() {
        let msg = format!("{report}");
        eprintln!("{msg}");

        // Always save diff image on failure for debugging
        save_diff_image(expected, actual, aw as u32, ah as u32, diff_name, amplification);

        if do_panic {
            panic!("{msg}");
        }
    } else if report.pixels_differing() > 0 {
        // Print informational report even on pass when pixels differ
        eprintln!("{report}");
        // Save diff image for accepted tests too (for CI reports)
        save_diff_image(expected, actual, aw as u32, ah as u32, diff_name, amplification);
    }
    (report.passed(), zdsim)
}

/// Convert a strided BGRA BitmapWindowMut to packed RGBA bytes.
fn bitmap_to_rgba_bytes(bm: &BitmapWindowMut<u8>, w: u32, h: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        let slice = bm.row(row).unwrap();
        for x in 0..w as usize {
            let b = slice[x * 4];
            let g = slice[x * 4 + 1];
            let r = slice[x * 4 + 2];
            let a = slice[x * 4 + 3];
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    rgba
}

/// Save a 3-panel comparison PNG (Expected | Actual | Diff) to the diffs directory.
///
/// Amplification is `min(10, 255/max_diff_pixel)` so diffs are always visible
/// without clipping.
///
/// Returns the path to the saved PNG, or None if saving failed.
fn save_diff_image(
    expected: &BitmapWindowMut<u8>,
    actual: &BitmapWindowMut<u8>,
    w: u32,
    h: u32,
    diff_name: &str,
    amplification: u8,
) -> Option<PathBuf> {
    let diffs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.join(".image-cache/diffs");
    std::fs::create_dir_all(&diffs_dir).ok()?;

    let sanitized = diff_name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
    let path = diffs_dir.join(format!("{sanitized}.png"));

    let exp_rgba = bitmap_to_rgba_bytes(expected, w, h);
    let act_rgba = bitmap_to_rgba_bytes(actual, w, h);
    zensim_regress::display::save_comparison_png(
        &exp_rgba,
        &act_rgba,
        w,
        h,
        amplification,
        Some(600),
        &path,
    );
    eprintln!("Saved comparison diff to {} (amplification: {amplification}x)", path.display());

    // Also show sixel if requested
    if std::env::var("SIXEL_DIFF").is_ok_and(|v| v == "1") {
        let _ = std::io::stderr().flush();
        zensim_regress::display::print_comparison_raw(
            &exp_rgba,
            &act_rgba,
            w,
            h,
            amplification,
            Some(600),
        );
    }

    Some(path)
}

/// Direct bitmap comparison of encoded bytes against a pre-decoded expected image.
///
/// Used by `compare_encoded_to_source` in `encoders.rs` for live comparison
/// without `.checksums` files.
pub fn compare_with(
    expected_context: Box<Context>,
    expected_bitmap_key: BitmapKey,
    actual_bytes: &[u8],
    require: Constraints,
    do_panic: bool,
) -> bool {
    test_init();
    // Check file size
    if let Some(max) = require.max_file_size {
        if actual_bytes.len() > max {
            let message = format!("Encoded size ({}) exceeds limit ({max})", actual_bytes.len());
            if do_panic {
                panic!("{}", &message);
            } else {
                eprintln!("{}", &message);
                return false;
            }
        }
    }

    let Some(similarity) = require.similarity else {
        return true; // file-size-only test, no pixel comparison
    };
    let tolerance = similarity.to_regression_tolerance_for_comparison();

    let mut image_context = Context::create().unwrap();
    image_context.add_copied_input_buffer(0, actual_bytes).unwrap();
    let actual_bitmap_key = decode_image(&mut image_context, 0);

    let actual_bitmaps = image_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
    let mut actual_bm =
        actual_bitmaps.try_borrow_mut(actual_bitmap_key).map_err(|e| e.at(here!())).unwrap();
    let actual_window = actual_bm.get_window_u8().unwrap();

    let expected_bitmaps = expected_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
    let mut expected_bm =
        expected_bitmaps.try_borrow_mut(expected_bitmap_key).map_err(|e| e.at(here!())).unwrap();
    let expected_window = expected_bm.get_window_u8().unwrap();

    let (passed, _zdsim) = compare_bitmaps_zensim(
        &actual_window,
        &expected_window,
        &tolerance,
        "compare_with",
        do_panic,
    );
    passed
}

/// Check a bitmap result against known baselines via `ChecksumManagerV2`.
///
/// Lower-level than `compare_bitmap` — for tests with custom pipeline setup.
/// Hashes the BGRA bitmap, wraps it as `StridedBytes`, and delegates to
/// `global_manager().check_hash_with_image()`.
pub fn check_visual_bitmap(
    identity: &TestIdentity,
    detail: &str,
    context: &Context,
    bitmap_key: BitmapKey,
    tolerance: &Tolerance,
) -> bool {
    let bitmaps = context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!())).unwrap();
    let mut window = bm.get_window_u8().unwrap();

    // Normalize alpha for bitmaps where alpha is not meaningful (e.g., JPEG output).
    // Without this, the alpha channel contains platform-dependent values — some SIMD
    // paths in mozjpeg write 0xFF, others leave the zero-initialized canvas value.
    // Normalizing to 255 makes checksums deterministic across platforms.
    window.normalize_unused_alpha().unwrap();

    let hash = checksum_bitmap_window(&mut window);
    let w = window.w() as usize;
    let h = window.h() as usize;
    let stride = window.info().t_stride() as usize;

    let actual_img = zensim::StridedBytes::try_new(
        window.get_slice(),
        w,
        h,
        stride,
        zensim::PixelFormat::Srgb8Bgra,
    )
    .expect("BGRA bitmap invalid for zensim");

    // Ensure reference image exists (for pixel comparison fallback on new platforms).
    // The manager only saves references on hash mismatch; this covers the hash-match case.
    let manager = global_manager();
    manager.save_reference_if_missing(identity.module, identity.func_name, detail, &actual_img);

    let result = manager.check_hash_with_image(
        identity.module,
        identity.func_name,
        detail,
        &hash,
        &actual_img,
        Some(tolerance),
    );
    let passed = handle_check_result(&result);
    // In CREATE_BASELINES mode, write the entry for brand-new tests
    if passed {
        if let Ok(CheckResult::NoBaseline { .. }) = &result {
            manager
                .accept(
                    identity.module,
                    identity.func_name,
                    detail,
                    &hash,
                    None,
                    None,
                    None,
                    "new-baseline",
                )
                .expect("Failed to write new baseline entry");
        }
    }
    passed
}

/// Check encoded bytes against known baselines via `ChecksumManagerV2`.
///
/// Lower-level than `compare_encoded` — for tests with custom pipeline setup.
/// Hashes the raw encoded bytes, decodes to BGRA for pixel comparison,
/// and delegates to `global_manager().check_hash_with_image()`.
pub fn check_visual_bytes(
    identity: &TestIdentity,
    detail: &str,
    bytes: &[u8],
    tolerance: &Tolerance,
) -> bool {
    test_init();
    let hash = checksum_bytes(bytes);

    // Decode encoded output to BGRA bitmap for pixel comparison
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, bytes).unwrap();
    let bitmap_key = decode_image(&mut ctx, 0);

    let bitmaps = ctx.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
    let mut bm = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!())).unwrap();
    let mut window = bm.get_window_u8().unwrap();
    window.normalize_unused_alpha().unwrap();
    let w = window.w() as usize;
    let h = window.h() as usize;
    let stride = window.info().t_stride() as usize;

    let actual_img = zensim::StridedBytes::try_new(
        window.get_slice(),
        w,
        h,
        stride,
        zensim::PixelFormat::Srgb8Bgra,
    )
    .expect("BGRA bitmap invalid for zensim");

    // Ensure reference image exists (for pixel comparison fallback on new platforms).
    let manager = global_manager();
    manager.save_reference_if_missing(identity.module, identity.func_name, detail, &actual_img);

    let result = manager.check_hash_with_image(
        identity.module,
        identity.func_name,
        detail,
        &hash,
        &actual_img,
        Some(tolerance),
    );
    let passed = handle_check_result(&result);
    // In CREATE_BASELINES mode, write the entry for brand-new tests
    if passed {
        if let Ok(CheckResult::NoBaseline { .. }) = &result {
            manager
                .accept(
                    identity.module,
                    identity.func_name,
                    detail,
                    &hash,
                    None,
                    None,
                    None,
                    "new-baseline",
                )
                .expect("Failed to write new baseline entry");
        }
    }
    passed
}

/// Test identity: (module_name, function_name) derived from test context.
///
/// Used by macros to pass structured names to `#[track_caller]` functions.
pub struct TestIdentity {
    pub module: &'static str,
    pub func_name: &'static str,
}

/// Run a visual comparison test with structured identity.
///
/// This is the primary `#[track_caller]` entry point that macros call.
/// It handles pipeline execution, output checksumming, and delegation
/// to `ChecksumManagerV2` for matching, comparison, and auto-accept.
#[track_caller]
pub fn compare_encoded(
    input: Option<IoTestEnum>,
    identity: &TestIdentity,
    detail: &str,
    _source_url: Option<&str>,
    require: Constraints,
    steps: Vec<s::Node>,
) -> bool {
    let mut io_vec = Vec::new();
    if let Some(i) = input {
        io_vec.push(i);
    }
    io_vec.push(IoTestEnum::OutputBuffer);
    let output_io_id = (io_vec.len() - 1) as i32;

    let mut context = Context::create().unwrap();
    let _ = build_framewise(
        &mut context,
        imageflow_types::Framewise::Steps(steps),
        io_vec,
        None,
        false,
    )
    .unwrap();

    let bytes = context.take_output_buffer(output_io_id).unwrap();

    // Check file size
    if let Some(max) = require.max_file_size {
        assert!(bytes.len() <= max, "Encoded size ({}) exceeds limit ({max})", bytes.len());
    }

    let similarity = require.similarity.expect("compare_encoded requires a similarity threshold");
    let tol_spec = similarity.to_tolerance_spec();
    check_visual_bytes(identity, detail, &bytes, &tol_spec)
}

/// Run a bitmap comparison test with structured identity.
///
/// This is the `#[track_caller]` function backing `visual_check_bitmap!`.
#[track_caller]
pub fn compare_bitmap(
    inputs: Vec<IoTestEnum>,
    identity: &TestIdentity,
    detail: &str,
    _source_url: Option<&str>,
    mut steps: Vec<s::Node>,
    tolerance: &Tolerance,
) -> bool {
    let capture_id = 0;
    let mut context = Context::create().unwrap();
    steps.push(s::Node::CaptureBitmapKey { capture_id });

    let response = build_steps(&mut context, &steps, inputs, None, false).unwrap();

    let bitmap_key = context
        .get_captured_bitmap_key(capture_id)
        .unwrap_or_else(|| panic!("execution failed {:?}", response));

    check_visual_bitmap(identity, detail, &context, bitmap_key, tolerance)
}

pub fn default_graph_recording(debug: bool) -> Option<imageflow_types::Build001GraphRecording> {
    if debug {
        Some(s::Build001GraphRecording::debug_defaults())
    } else {
        None
    }
}
