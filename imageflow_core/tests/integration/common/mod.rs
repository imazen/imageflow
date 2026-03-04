use imageflow_core::{here, nerror};
#[allow(unused_imports)]
use imageflow_helpers as hlp;
use imageflow_types as s;

#[macro_use]
pub mod macros;
pub mod checksum_adapter;
pub mod upload_tracker;

use imageflow_core::graphics::bitmaps::BitmapWindowMut;
use imageflow_core::{Context, ErrorKind, FlowError};
use std::marker::PhantomPinned;
use std::path::Path;

use imageflow_core;
use std::path::PathBuf;
use std::pin::Pin;
use std::io::Write as _;
use std::{self, panic};

use imageflow_core::BitmapKey;
use imageflow_types::ResponsePayload;
use slotmap::Key;
use std::time::Duration;
pub use zensim_regress::Tolerance;
use zensim_regress::manifest::{ManifestEntry, ManifestStatus, ManifestWrite};

/// Global manifest writer, shared across all test threads.
///
/// Checks `REGRESS_MANIFEST_DIR` first (lock-free per-process files,
/// preferred for nextest), then falls back to `REGRESS_MANIFEST_PATH`
/// (lock-based single file).
fn global_manifest() -> &'static Option<Box<dyn ManifestWrite + Send + Sync>> {
    use std::sync::OnceLock;
    static MANIFEST: OnceLock<Option<Box<dyn ManifestWrite + Send + Sync>>> = OnceLock::new();
    MANIFEST.get_or_init(|| zensim_regress::manifest::writer_from_env())
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ChecksumMatch {
    Match,
    Mismatch,
    NewStored,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IoTestEnum {
    ByteArray(Vec<u8>),
    OutputBuffer,
    Url(String),
}

/// Source cache directory at workspace root `.image-cache/sources/`.
fn source_cache_dir() -> PathBuf {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent");
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
    let after_host = after_scheme
        .split_once('/')
        .map(|(_, p)| p)
        .unwrap_or(after_scheme);
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
    let mut bit = BitmapBgraContainer::empty();
    let mut steps = steps.to_vec();
    steps.push(unsafe { bit.as_mut().get_node() });

    let mut context = Context::create().unwrap();

    let result = build_steps(&mut context, &steps, io, None, debug).unwrap();

    if let Some((w, h)) = bit.bitmap_size(&context) {
        (w as u32, h as u32)
    } else {
        panic!("execution failed: {:?}", result);
    }
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

/// A context for getting/storing frames and frame checksums by test name.
///
/// Uses `ReferenceStorage` from zensim-regress for output image caching,
/// upload, and download. Petname-based naming for reference images.
pub struct ChecksumCtx {
    /// Directory containing `.checksums` files (one per test module).
    checksums_dir: PathBuf,
    /// Remote storage for output reference images (cache + S3).
    output_storage: zensim_regress::remote::ReferenceStorage,
    /// Local cache directory for output images (same dir given to ReferenceStorage).
    output_cache_dir: PathBuf,
}

impl ChecksumCtx {
    /// A checksum context configured for integration visual tests.
    pub fn visuals() -> ChecksumCtx {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let checksums_dir = manifest.join("tests/integration/visuals");
        let workspace_root = manifest.parent().expect("CARGO_MANIFEST_DIR has no parent");

        let output_cache_dir = workspace_root.join(".image-cache/outputs");
        std::fs::create_dir_all(&output_cache_dir).unwrap();

        let upload_prefix = std::env::var("REGRESS_UPLOAD_PREFIX")
            .ok()
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .or_else(|| {
                Some("s3://imageflow-resources/visual_test_checksums".to_string())
            });
        let upload_enabled = std::env::var("UPLOAD_REFERENCES")
            .is_ok_and(|v| v == "1" || v == "true");

        let output_storage = zensim_regress::remote::ReferenceStorage::new(
            "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums",
            upload_prefix,
            upload_enabled,
            &output_cache_dir,
        );

        ChecksumCtx { checksums_dir, output_storage, output_cache_dir }
    }

    /// Convert a name (petname or full hash) to a petname for storage.
    ///
    /// Petnames have dashes (e.g., `"north-axe-3d468:sea"`).
    /// Full hashes don't (e.g., `"sea:a4839401fabae99c.png"`).
    fn to_petname(name_or_hash: &str) -> String {
        if name_or_hash.contains('-') {
            // Already a petname
            name_or_hash.to_string()
        } else {
            // Full hash — strip file extension, convert to petname
            let bare = name_or_hash
                .rsplit_once('.')
                .filter(|(_, ext)| {
                    matches!(*ext, "png" | "jpg" | "jpeg" | "webp" | "gif" | "unknown")
                })
                .map(|(base, _)| base)
                .unwrap_or(name_or_hash);
            zensim_regress::petname::memorable_name(bare)
        }
    }

    /// Load a reference image from cache or remote storage.
    ///
    /// Accepts either a petname (`"north-axe-3d468:sea"`) or a full hash
    /// (`"sea:a4839401fabae99c.png"`). The bitmap will be destroyed when
    /// the returned Context goes out of scope.
    pub fn load_image(&self, name: &str) -> (Box<Context>, BitmapKey) {
        self.try_load_image(name)
            .unwrap_or_else(|| panic!("Failed to load reference image: {name}"))
    }

    /// Load a reference image, returning None on any failure.
    fn try_load_image(&self, name: &str) -> Option<(Box<Context>, BitmapKey)> {
        let petname = Self::to_petname(name);
        let path = self
            .output_storage
            .download_reference(&petname)
            .ok()?
            .or_else(|| {
                eprintln!("Reference image not found: {petname}");
                None
            })?;

        let mut c = Context::create().ok()?;
        c.add_file(0, s::IoDirection::In, path.to_str()?).ok()?;

        let image = try_decode_image(&mut c, 0)?;
        Some((c, image))
    }

    /// Save a bitmap frame to cache and upload to remote storage.
    pub fn save_frame(&self, window: &mut BitmapWindowMut<u8>, checksum: &str) {
        let petname = Self::to_petname(checksum);
        let filename =
            zensim_regress::remote::ReferenceStorage::remote_filename(&petname);
        let dest_path = self.output_cache_dir.join(&filename);
        if !dest_path.exists() {
            println!("Writing {}", dest_path.display());
            imageflow_core::helpers::write_png(&dest_path, window).unwrap();
            if self.output_storage.uploads_configured() {
                match self.output_storage.upload_reference(&dest_path, &petname) {
                    Ok(()) => upload_tracker::record_upload(&petname),
                    Err(e) => eprintln!("Warning: upload failed for {petname}: {e}"),
                }
            }
        }
    }

    /// Save encoded bytes to cache and upload to remote storage.
    pub fn save_bytes(&self, bytes: &[u8], checksum: &str) {
        let petname = Self::to_petname(checksum);
        let filename =
            zensim_regress::remote::ReferenceStorage::remote_filename(&petname);
        let dest_path = self.output_cache_dir.join(&filename);
        if !dest_path.exists() {
            println!("Writing {}", dest_path.display());
            std::fs::write(&dest_path, bytes).unwrap();
            if self.output_storage.uploads_configured() {
                match self.output_storage.upload_reference(&dest_path, &petname) {
                    Ok(()) => upload_tracker::record_upload(&petname),
                    Err(e) => eprintln!("Warning: upload failed for {petname}: {e}"),
                }
            }
        }
    }

    /// Checksum encoded bytes using seahash + file extension.
    pub fn checksum_bytes(bytes: &[u8]) -> String {
        let h = seahash::hash(bytes);
        format!("sea:{h:016x}.{}", Self::file_extension_for_bytes(bytes))
    }

    pub fn file_extension_for_bytes(bytes: &[u8]) -> &'static str {
        if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            "png"
        } else if bytes.starts_with(b"GIF8") {
            "gif"
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "jpg"
        } else if bytes.starts_with(b"RIFF")
            && bytes.len() >= 12
            && bytes[8..12].starts_with(b"WEBP")
        {
            "webp"
        } else {
            "unknown"
        }
    }

    /// Checksum bitmap pixels using seahash with dimensions baked in.
    ///
    /// Iterates scanlines to exclude stride padding (matching the old
    /// `short_hash_pixels` behavior). Dimensions are prepended to avoid
    /// collisions between differently-shaped images.
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

    pub fn checksum_bitmap(c: &Context, bitmap_key: BitmapKey) -> String {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!())).unwrap();

        let mut window = bitmap.get_window_u8().unwrap();

        window.normalize_unused_alpha().unwrap();
        Self::checksum_bitmap_window(&mut window)
    }
    pub fn save_bitmap(&self, c: &Context, bitmap_key: BitmapKey, checksum: &str) {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!())).unwrap();

        let mut window = bitmap.get_window_u8().unwrap();
        self.save_frame(&mut window, checksum)
    }

    /// Structured bytes match using (module, test_name, detail_name).
    pub fn bytes_match(
        &self,
        bytes: &[u8],
        module: &str,
        test_name: &str,
        detail_name: &str,
        tolerance: Option<&Tolerance>,
    ) -> (ChecksumMatch, String, String) {
        let actual = Self::checksum_bytes(bytes);
        self.save_bytes(bytes, &actual);
        self.exact_match(actual, module, test_name, detail_name, tolerance)
    }

    /// Structured checksum match using (module, test_name, detail_name).
    ///
    /// Returns (match_result, trusted_name, actual_checksum).
    /// `trusted_name` is a petname from the `.checksums` file.
    pub fn exact_match(
        &self,
        actual_checksum: String,
        module: &str,
        test_name: &str,
        detail_name: &str,
        tolerance: Option<&Tolerance>,
    ) -> (ChecksumMatch, String, String) {
        let adapter = checksum_adapter::ChecksumAdapter::new(&self.checksums_dir);
        if let Some((result, trusted)) =
            adapter.try_match(module, test_name, detail_name, &actual_checksum, tolerance)
        {
            return (result, trusted, actual_checksum);
        }

        // No .checksums file or entry found — report as mismatch so pixel
        // comparison can still run and potentially auto-accept within tolerance.
        eprintln!(
            "Warning: no .checksums entry for {module}/{test_name} {detail_name}"
        );
        (ChecksumMatch::Mismatch, String::new(), actual_checksum)
    }
}

pub fn decode_image(c: &mut Context, io_id: i32) -> BitmapKey {
    try_decode_image(c, io_id).expect("decode_image failed")
}

/// Decode an input image, returning None on failure instead of panicking.
fn try_decode_image(c: &mut Context, io_id: i32) -> Option<BitmapKey> {
    let mut bit = BitmapBgraContainer::empty();
    let result = c.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        framewise: s::Framewise::Steps(vec![s::Node::Decode { io_id, commands: None }, unsafe {
            bit.as_mut().get_node()
        }]),
    });

    result.ok()?;
    bit.bitmap_key(c)
}

pub fn decode_input(c: &mut Context, input: IoTestEnum) -> BitmapKey {
    let mut bit = BitmapBgraContainer::empty();

    let _result = build_steps(
        c,
        &[s::Node::Decode { io_id: 0, commands: None }, unsafe { bit.as_mut().get_node() }],
        vec![input],
        None,
        false,
    )
    .unwrap();

    bit.bitmap_key(c).unwrap()
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
            Similarity::AllowOffByOneBytesRatio(ratio) => {
                Tolerance {
                    max_delta: 1,
                    min_similarity: 99.0,
                    max_pixels_different: ratio as f64,
                    ..Tolerance::exact()
                }
            }
            Similarity::MaxZdsim(max_zdsim) => {
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
    fn to_regression_tolerance_for_comparison(&self) -> zensim_regress::testing::RegressionTolerance {
        let spec = self.to_tolerance_spec();
        spec.to_regression_tolerance(zensim_regress::arch::detect_arch_tag())
    }
}

#[derive(Clone)]
pub struct Constraints {
    pub similarity: Similarity,
    pub max_file_size: Option<usize>,
}

pub enum ResultKind<'a> {
    Bitmap { context: &'a Context, key: BitmapKey },
    Bytes(&'a [u8]),
}
impl<'a> ResultKind<'a> {
    fn exact_match(
        &mut self,
        c: &ChecksumCtx,
        module: &str,
        test_name: &str,
        detail_name: &str,
        tolerance: Option<&Tolerance>,
    ) -> (ChecksumMatch, String, String) {
        match *self {
            ResultKind::Bitmap { context, key } => {
                let actual = ChecksumCtx::checksum_bitmap(context, key);
                c.save_bitmap(context, key, &actual);
                c.exact_match(actual, module, test_name, detail_name, tolerance)
            }
            ResultKind::Bytes(b) => c.bytes_match(b, module, test_name, detail_name, tolerance),
        }
    }
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
    assert_eq!((aw, ah), (ew, eh), "bitmap dimensions differ: actual {aw}x{ah} vs expected {ew}x{eh}");

    let actual_stride = actual.info().t_stride() as usize;
    let expected_stride = expected.info().t_stride() as usize;

    let actual_img = StridedBytes::try_new(
        actual.get_slice(), aw, ah, actual_stride, PixelFormat::Srgb8Bgra,
    ).expect("actual bitmap: invalid for zensim");
    let expected_img = StridedBytes::try_new(
        expected.get_slice(), ew, eh, expected_stride, PixelFormat::Srgb8Bgra,
    ).expect("expected bitmap: invalid for zensim");

    let z = Zensim::new(ZensimProfile::latest());
    let report = check_regression(&z, &expected_img, &actual_img, tolerance)
        .expect("zensim comparison failed");

    let zdsim = zensim_regress::diff_summary::zdsim(report.score());

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
    let diffs_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join(".image-cache/diffs");
    std::fs::create_dir_all(&diffs_dir).ok()?;

    let sanitized = diff_name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
    let path = diffs_dir.join(format!("{sanitized}.png"));

    let exp_rgba = bitmap_to_rgba_bytes(expected, w, h);
    let act_rgba = bitmap_to_rgba_bytes(actual, w, h);
    zensim_regress::display::save_comparison_png(&exp_rgba, &act_rgba, w, h, amplification, Some(600), &path);
    eprintln!("Saved comparison diff to {} (amplification: {amplification}x)", path.display());

    // Also show sixel if requested
    if std::env::var("SIXEL_DIFF").is_ok_and(|v| v == "1") {
        let _ = std::io::stderr().flush();
        zensim_regress::display::print_comparison_raw(&exp_rgba, &act_rgba, w, h, amplification, Some(600));
    }

    Some(path)
}

pub fn check_size(result: &ResultKind, max_file_size: Option<usize>, panic: bool) -> bool {
    if let ResultKind::Bytes(actual_bytes) = *result {
        if actual_bytes.len() > max_file_size.unwrap_or(actual_bytes.len()) {
            let message = format!(
                "Encoded size ({}) exceeds limit ({})",
                actual_bytes.len(),
                max_file_size.unwrap()
            );
            if panic {
                panic!("{}", &message);
            } else {
                eprintln!("{}", &message);
                return false;
            }
        }
    }
    true
}

/// Evaluates the given result against known truth, applying the given constraints.
///
/// Uses zensim-regress for pixel comparison instead of the legacy BitmapDiffStats path.
pub fn compare_with<'a>(
    _c: &ChecksumCtx,
    expected_context: Box<Context>,
    expected_bitmap_key: BitmapKey,
    result: ResultKind<'a>,
    require: Constraints,
    do_panic: bool,
) -> bool {
    if !check_size(&result, require.max_file_size, do_panic) {
        return false;
    }

    let tolerance = require.similarity.to_regression_tolerance_for_comparison();

    let mut image_context = Context::create().unwrap();
    let (actual_context, actual_bitmap_key) = match result {
        ResultKind::Bitmap { context, key } => (context, key),
        ResultKind::Bytes(actual_bytes) => {
            unsafe { image_context.add_input_bytes(0, actual_bytes) }.unwrap();
            let key = decode_image(&mut image_context, 0);
            (image_context.as_ref(), key)
        }
    };

    let actual_bitmaps = actual_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
    let mut actual_bm = actual_bitmaps.try_borrow_mut(actual_bitmap_key)
        .map_err(|e| e.at(here!())).unwrap();
    let actual_window = actual_bm.get_window_u8().unwrap();

    let expected_bitmaps = expected_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
    let mut expected_bm = expected_bitmaps.try_borrow_mut(expected_bitmap_key)
        .map_err(|e| e.at(here!())).unwrap();
    let expected_window = expected_bm.get_window_u8().unwrap();

    let (passed, _zdsim) = compare_bitmaps_zensim(&actual_window, &expected_window, &tolerance, "compare_with", do_panic);
    passed
}

/// Compute zensim quality score of the actual output vs the original source.
///
/// Returns `None` if dimensions differ (pipeline resized/cropped), if either
/// image can't be loaded, or if the source URL can't be fetched. Only produces
/// a meaningful score for same-dimension comparisons (encode-only, passthrough).
fn compute_zensim_vs_source(
    c: &ChecksumCtx,
    actual_checksum: &str,
    source_url: &str,
) -> Option<f64> {
    use zensim::{PixelFormat, StridedBytes, Zensim, ZensimProfile};

    // Load actual output bitmap (already saved to disk during checksum)
    let (actual_ctx, actual_key) = c.try_load_image(actual_checksum)?;
    let actual_bitmaps = actual_ctx.borrow_bitmaps().ok()?;
    let mut actual_bm = actual_bitmaps.try_borrow_mut(actual_key).ok()?;
    let actual_window = actual_bm.get_window_u8()?;
    let (aw, ah) = (actual_window.w() as usize, actual_window.h() as usize);
    let actual_stride = actual_window.info().t_stride() as usize;

    // Load and decode source image
    let mut source_ctx = Context::create().ok()?;
    if source_url.starts_with("http://") || source_url.starts_with("https://") {
        let bytes = get_url_bytes_with_retry(source_url).ok()?;
        unsafe { source_ctx.add_input_bytes(0, &bytes) }.ok()?;
    } else {
        source_ctx.add_file(0, s::IoDirection::In, source_url).ok()?;
    };
    let source_key = try_decode_image(&mut source_ctx, 0)?;
    let source_bitmaps = source_ctx.borrow_bitmaps().ok()?;
    let mut source_bm = source_bitmaps.try_borrow_mut(source_key).ok()?;
    let source_window = source_bm.get_window_u8()?;
    let (sw, sh) = (source_window.w() as usize, source_window.h() as usize);
    let source_stride = source_window.info().t_stride() as usize;

    // Dimensions must match for zensim comparison
    if aw != sw || ah != sh {
        return None;
    }
    // Minimum 8×8 for zensim
    if aw < 8 || ah < 8 {
        return None;
    }

    let actual_slice = actual_window.get_slice();
    let source_slice = source_window.get_slice();

    let actual_img =
        StridedBytes::try_new(actual_slice, aw, ah, actual_stride, PixelFormat::Srgb8Bgra).ok()?;
    let source_img = StridedBytes::try_new(
        source_slice,
        sw,
        sh,
        source_stride,
        PixelFormat::Srgb8Bgra,
    )
    .ok()?;

    let z = Zensim::new(ZensimProfile::latest());
    let result = z.compute(&source_img, &actual_img).ok()?;
    Some(result.score)
}

/// Evaluates the given result against known truth using structured identity.
///
/// Uses `.checksums` files as the only lookup path.
/// On mismatch within tolerance, auto-accepts to the `.checksums` file.
/// When `source_url` is provided, computes zensim quality vs original source
/// (same-dimension only) and records it in the `.checksums` diff summary.
#[track_caller]
pub fn evaluate_result<'a>(
    c: &ChecksumCtx,
    module: &str,
    test_name: &str,
    detail_name: &str,
    mut result: ResultKind<'a>,
    tolerance: &Tolerance,
    max_file_size: Option<usize>,
    source_url: Option<&str>,
    do_panic: bool,
) -> bool {
    if !check_size(&result, max_file_size, do_panic) {
        return false;
    }
    let (exact, trusted, actual) = result.exact_match(c, module, test_name, detail_name, Some(tolerance));

    let flat_name = if detail_name.is_empty() {
        test_name.to_string()
    } else {
        format!("{test_name} {detail_name}")
    };

    // Compute tolerance zdsim for reporting
    let tolerance_zdsim = zensim_regress::diff_summary::zdsim(tolerance.min_similarity);

    if exact == ChecksumMatch::Match {
        // Exact hash match → zdsim=0 by definition
        eprintln!("  {flat_name}: zdsim=0 (match) tolerance={tolerance_zdsim:.4}");
        if let Some(m) = global_manifest() {
            m.write_entry(&ManifestEntry {
                test_name: &flat_name,
                status: ManifestStatus::Match,
                actual_zdsim: Some(0.0),
                tolerance_zdsim: Some(tolerance_zdsim),
                actual_hash: &actual,
                baseline_hash: Some(&actual),
                diff_summary: None,
            });
        }
        return true;
    }
    if exact == ChecksumMatch::NewStored {
        eprintln!("  {flat_name}: zdsim=- (novel)");
        if let Some(m) = global_manifest() {
            m.write_entry(&ManifestEntry {
                test_name: &flat_name,
                status: ManifestStatus::Novel,
                actual_zdsim: None,
                tolerance_zdsim: None,
                actual_hash: &actual,
                baseline_hash: None,
                diff_summary: None,
            });
        }
        return true;
    }

    eprintln!("--- Checksum mismatch for '{flat_name}' ---");

    // If there's no trusted reference to compare against, we can't do pixel comparison.
    // This happens when a brand-new test has no .checksums entry at all.
    if trusted.is_empty() {
        let msg = format!(
            "No reference baseline for '{flat_name}'. \
             Run with UPDATE_CHECKSUMS=1 to create the initial baseline."
        );
        eprintln!("{msg}");
        if let Some(m) = global_manifest() {
            m.write_entry(&ManifestEntry {
                test_name: &flat_name,
                status: ManifestStatus::Failed,
                actual_zdsim: None,
                tolerance_zdsim: Some(tolerance_zdsim),
                actual_hash: &actual,
                baseline_hash: None,
                diff_summary: Some("no baseline"),
            });
        }
        if do_panic {
            panic!("{msg}");
        }
        return false;
    }

    // Load both bitmaps and compare via zensim-regress
    let reg_tolerance = tolerance.to_regression_tolerance(zensim_regress::arch::detect_arch_tag());
    let (close_enough, measured_zdsim) = {
        let (expected_context, expected_bitmap_key) = c.load_image(&trusted);
        let expected_bitmaps = expected_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
        let mut expected_bm = expected_bitmaps.try_borrow_mut(expected_bitmap_key)
            .map_err(|e| e.at(here!())).unwrap();
        let expected_window = expected_bm.get_window_u8().unwrap();

        let mut image_context = Context::create().unwrap();
        let (actual_context, actual_bitmap_key) = match result {
            ResultKind::Bitmap { context, key } => (context, key),
            ResultKind::Bytes(actual_bytes) => {
                unsafe { image_context.add_input_bytes(0, actual_bytes) }.unwrap();
                let key = decode_image(&mut image_context, 0);
                (image_context.as_ref(), key)
            }
        };
        let actual_bitmaps = actual_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
        let mut actual_bm = actual_bitmaps.try_borrow_mut(actual_bitmap_key)
            .map_err(|e| e.at(here!())).unwrap();
        let actual_window = actual_bm.get_window_u8().unwrap();

        compare_bitmaps_zensim(&actual_window, &expected_window, &reg_tolerance, &flat_name, do_panic)
    };

    // Report measured zdsim for ratcheting
    let status_str = if close_enough { "accepted" } else { "FAILED" };
    eprintln!("  {flat_name}: zdsim={measured_zdsim:.6} ({status_str}) tolerance={tolerance_zdsim:.4}");

    // Write manifest entry
    if let Some(m) = global_manifest() {
        let diff_str = format!("zdsim={measured_zdsim:.6}");
        m.write_entry(&ManifestEntry {
            test_name: &flat_name,
            status: if close_enough { ManifestStatus::Accepted } else { ManifestStatus::Failed },
            actual_zdsim: Some(measured_zdsim),
            tolerance_zdsim: Some(tolerance_zdsim),
            actual_hash: &actual,
            baseline_hash: Some(&trusted),
            diff_summary: Some(&diff_str),
        });
    }

    if close_enough {
        eprintln!(
            "--- '{flat_name}': checksum mismatch within tolerance ({}) ---",
            zensim_regress::diff_summary::format_tolerance_shorthand(tolerance)
        );

        // Always auto-accept to .checksums when within tolerance.
        // This records the platform-specific hash so future runs get exact matches.
        {
            let adapter = checksum_adapter::ChecksumAdapter::new(&c.checksums_dir);
            let diff_summary = source_url.and_then(|url| {
                match compute_zensim_vs_source(c, &actual, url) {
                    Some(score) => {
                        eprintln!("  zensim vs source: {score:.1}");
                        Some(format!("src_zs:{score:.1}"))
                    }
                    None => None,
                }
            });

            if let Err(e) = adapter.accept(
                module,
                test_name,
                detail_name,
                &actual,
                Some(&trusted),
                None,
                diff_summary.as_deref(),
            ) {
                eprintln!("Warning: failed to auto-accept {flat_name}: {e}");
            }
        }
    }
    close_enough
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
/// It handles:
/// 1. Pipeline setup (input download, node execution)
/// 2. Output checksumming
/// 3. Checksum matching via `.checksums` files
/// 4. Pixel-level comparison on mismatch
/// 5. Auto-accept recording on tolerance match
#[track_caller]
pub fn compare_encoded(
    input: Option<IoTestEnum>,
    identity: &TestIdentity,
    detail: &str,
    source_url: Option<&str>,
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

    let ctx = ChecksumCtx::visuals();
    let tol_spec = require.similarity.to_tolerance_spec();

    evaluate_result(
        &ctx,
        identity.module,
        identity.func_name,
        detail,
        ResultKind::Bytes(&bytes),
        &tol_spec,
        require.max_file_size,
        source_url,
        true,
    )
}

/// Run a bitmap comparison test with structured identity.
///
/// This is the `#[track_caller]` function backing `visual_check_bitmap!`.
#[track_caller]
pub fn compare_bitmap(
    inputs: Vec<IoTestEnum>,
    identity: &TestIdentity,
    detail: &str,
    source_url: Option<&str>,
    mut steps: Vec<s::Node>,
    tolerance: &Tolerance,
) -> bool {
    let mut context = Context::create().unwrap();
    let mut bit = BitmapBgraContainer::empty();
    steps.push(unsafe { bit.as_mut().get_node() });

    let response = build_steps(&mut context, &steps, inputs, None, false).unwrap();

    let bitmap_key = bit
        .bitmap_key(&context)
        .unwrap_or_else(|| panic!("execution failed {:?}", response));

    let ctx = ChecksumCtx::visuals();
    evaluate_result(
        &ctx,
        identity.module,
        identity.func_name,
        detail,
        ResultKind::Bitmap { context: &context, key: bitmap_key },
        tolerance,
        None,
        source_url,
        true,
    )
}

pub fn default_graph_recording(debug: bool) -> Option<imageflow_types::Build001GraphRecording> {
    if debug {
        Some(s::Build001GraphRecording::debug_defaults())
    } else {
        None
    }
}

/// Simplifies access to raw bitmap data from Imageflow (when using imageflow_types::Node)
/// Consider this an unmovable type. If you move it, you will corrupt the heap.
pub struct BitmapBgraContainer {
    dest_bitmap: BitmapKey,
    _marker: PhantomPinned,
}
impl BitmapBgraContainer {
    pub fn empty() -> Pin<Box<Self>> {
        Box::pin(BitmapBgraContainer { dest_bitmap: BitmapKey::null(), _marker: PhantomPinned })
    }
    /// Creates an operation node containing a pointer to self. Do not move self!
    pub unsafe fn get_node(self: Pin<&mut Self>) -> s::Node {
        let key = unsafe {
            let this = self.get_unchecked_mut();
            &mut this.dest_bitmap
        };

        let ptr_to_key = key as *mut BitmapKey;
        s::Node::FlowBitmapKeyPtr { ptr_to_bitmap_key: ptr_to_key as usize }
    }

    /// Reads back the bitmap key written by the graph engine.
    /// Safe because `dest_bitmap` is `Copy` and read through `&self`.
    pub fn bitmap_key(&self, _c: &Context) -> Option<BitmapKey> {
        if self.dest_bitmap.is_null() {
            None
        } else {
            Some(self.dest_bitmap)
        }
    }

    /// Returns a reference the bitmap
    /// This reference is only valid for the duration of the context it was created within
    pub fn bitmap_size(&self, c: &Context) -> Option<(usize, usize)> {
        if self.dest_bitmap.is_null() {
            None
        } else {
            Some(c.borrow_bitmaps().unwrap().try_borrow_mut(self.dest_bitmap).unwrap().size())
        }
    }
}
