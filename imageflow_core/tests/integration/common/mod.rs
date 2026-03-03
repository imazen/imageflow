use imageflow_core::{here, nerror};
#[allow(unused_imports)]
use imageflow_helpers as hlp;
use imageflow_types as s;

#[macro_use]
pub mod macros;
pub mod bitmap_diff_stats;
pub mod checksum_adapter;
use bitmap_diff_stats::*;

use imageflow_core::graphics::bitmaps::BitmapWindowMut;
use imageflow_core::{Context, ErrorKind, FlowError};
use std::marker::PhantomPinned;
use std::path::Path;

use imageflow_core;
use s::PixelLayout;
use std::path::PathBuf;
use std::pin::Pin;
use std::{self, panic};

use imageflow_core::BitmapKey;
use imageflow_types::ResponsePayload;
use slotmap::Key;
use std::time::Duration;

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
        let checksums_dir = manifest.join("tests/integration/visuals/checksums");
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
        let petname = Self::to_petname(name);
        let path = self
            .output_storage
            .download_reference(&petname)
            .unwrap_or_else(|e| panic!("Download error for {petname}: {e}"))
            .unwrap_or_else(|| panic!("Reference image not found: {petname}"));

        let mut c = Context::create().unwrap();
        c.add_file(0, s::IoDirection::In, path.to_str().unwrap()).unwrap();

        let image = decode_image(&mut c, 0);
        (c, image)
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
            if let Err(e) = self.output_storage.upload_reference(&dest_path, &petname) {
                eprintln!("Warning: upload failed for {petname}: {e}");
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
            if let Err(e) = self.output_storage.upload_reference(&dest_path, &petname) {
                eprintln!("Warning: upload failed for {petname}: {e}");
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
    ) -> (ChecksumMatch, String, String) {
        let actual = Self::checksum_bytes(bytes);
        self.save_bytes(bytes, &actual);
        self.exact_match(actual, module, test_name, detail_name)
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
    ) -> (ChecksumMatch, String, String) {
        let adapter = checksum_adapter::ChecksumAdapter::new(&self.checksums_dir);
        if let Some((result, trusted)) =
            adapter.try_match(module, test_name, detail_name, &actual_checksum)
        {
            return (result, trusted, actual_checksum);
        }

        // No .checksums file found
        panic!(
            "No .checksums entry found for {module}/{test_name} {detail_name}. \
             Run with UPDATE_CHECKSUMS=1 to create it."
        );
    }
}

pub fn decode_image(c: &mut Context, io_id: i32) -> BitmapKey {
    let mut bit = BitmapBgraContainer::empty();
    let result = c.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        framewise: s::Framewise::Steps(vec![s::Node::Decode { io_id, commands: None }, unsafe {
            bit.as_mut().get_node()
        }]),
    });

    result.unwrap();
    bit.bitmap_key(c).unwrap()
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
    AllowDssimMatch(f64, f64),
}

impl Similarity {
    fn report_on_bytes(&self, stats: &BitmapDiffStats) -> Option<String> {
        let allowed_off_by_one_bytes: i64 = match *self {
            Similarity::AllowOffByOneBytesCount(v) => v,
            Similarity::AllowOffByOneBytesRatio(ratio) => (ratio * stats.values as f32) as i64,
            Similarity::AllowDssimMatch(..) => return None,
        };

        //TODO: This doesn't really work, since off-by-one errors are averaged and thus can hide +/- 4
        let bad_approx_of_differing_pixels = stats.values_abs_delta_sum as i64 / 4;

        if stats.pixels_differing < bad_approx_of_differing_pixels
            || stats.values_differing_by_more_than_1 > allowed_off_by_one_bytes
        {
            return Some(format!("Bitmaps mismatched: {}", stats.report()));
        }

        None
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
    ) -> (ChecksumMatch, String, String) {
        match *self {
            ResultKind::Bitmap { context, key } => {
                let actual = ChecksumCtx::checksum_bitmap(context, key);
                c.save_bitmap(context, key, &actual);
                c.exact_match(actual, module, test_name, detail_name)
            }
            ResultKind::Bytes(b) => c.bytes_match(b, module, test_name, detail_name),
        }
    }
}

fn get_imgref_bgra32(b: &mut BitmapWindowMut<u8>) -> imgref::ImgVec<rgb::Rgba<f32>> {
    use dssim::*;

    b.normalize_unused_alpha().unwrap();
    if b.info().pixel_layout() != PixelLayout::BGRA {
        panic!("Pixel layout is not BGRA");
    }

    let (w, h) = (b.w() as usize, b.h() as usize);

    let slice = b.get_slice();
    let new_stride = b.info().t_stride() as usize / 4;

    let cast_to_bgra8 = bytemuck::cast_slice::<u8, rgb::alt::BGRA8>(slice);

    imgref::Img::new_stride(cast_to_bgra8.to_rgbaplu(), w, h, new_stride)
}

#[allow(dead_code)]
pub struct CompareBitmapsResult {
    pub stats: Option<BitmapDiffStats>,
    pub dssim: Option<f64>,
    pub close_enough: bool,
    pub exact_match: bool,
    pub failure_message: Option<String>,
    pub actual_checksum: Option<String>,
}
/// Compare two bgra32 or bgr32 frames using the given similarity requirements
pub fn compare_bitmaps(
    _c: &ChecksumCtx,
    actual: &mut BitmapWindowMut<u8>,
    expected: &mut BitmapWindowMut<u8>,
    require: Similarity,
    panic: bool,
) -> CompareBitmapsResult {
    let stats = BitmapDiffStats::diff_bitmap_windows(actual, expected);
    if stats.pixels_differing == 0 {
        return CompareBitmapsResult {
            stats: Some(stats),
            dssim: None,
            close_enough: true,
            exact_match: true,
            failure_message: None,
            actual_checksum: None,
        };
    }
    // Always report pixel diff stats when pixels differ
    eprintln!("{}", stats.report());

    if let Similarity::AllowDssimMatch(minval, maxval) = require {
        let actual_ref = get_imgref_bgra32(actual);
        let expected_ref = get_imgref_bgra32(expected);
        let d = dssim::new();

        let actual_img = d.create_image(&actual_ref).unwrap();
        let expected_img = d.create_image(&expected_ref).unwrap();

        let (dssim, _) = d.compare(&expected_img, actual_img);

        eprintln!("dssim = {} (allowed range [{}, {}])", dssim, minval, maxval);

        let failure = if dssim > maxval {
            Some(format!("The dssim {} is greater than the permitted value {}", dssim, maxval))
        } else if dssim < minval {
            Some(format!("The dssim {} is lower than expected minimum value {}", dssim, minval))
        } else {
            None
        };
        let result = CompareBitmapsResult {
            stats: Some(stats),
            dssim: Some(dssim.into()),
            close_enough: dssim >= minval && dssim <= maxval,
            exact_match: false,
            failure_message: failure.clone(),
            actual_checksum: None,
        };

        if let Some(message) = failure {
            if panic {
                panic!("{}", message);
            } else {
                eprintln!("{}", message);
            }
        }
        result
    } else {
        let failure = require.report_on_bytes(&stats);

        let result = CompareBitmapsResult {
            stats: Some(stats),
            dssim: None,
            close_enough: failure.is_none(),
            exact_match: false,
            failure_message: failure.clone(),
            actual_checksum: None,
        };

        if let Some(message) = failure {
            if panic {
                panic!("{}", message);
            } else {
                eprintln!("{}", message);
            }
        }
        result
    }
}

pub fn check_size(result: &ResultKind, require: Constraints, panic: bool) -> bool {
    if let ResultKind::Bytes(actual_bytes) = *result {
        if actual_bytes.len() > require.max_file_size.unwrap_or(actual_bytes.len()) {
            let message = format!(
                "Encoded size ({}) exceeds limit ({})",
                actual_bytes.len(),
                require.max_file_size.unwrap()
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

/// Evaluates the given result against known truth, applying the given constraints
pub fn compare_with<'a>(
    c: &ChecksumCtx,
    expected_context: Box<Context>,
    expected_bitmap_key: BitmapKey,
    result: ResultKind<'a>,
    require: Constraints,
    panic: bool,
) -> bool {
    if !check_size(&result, require.clone(), panic) {
        return false;
    }

    let res = compare_bitmaps_result_to_expected(
        c,
        result,
        true,
        expected_context,
        expected_bitmap_key,
        require.similarity,
        panic,
    );
    res.close_enough
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
    let (actual_ctx, actual_key) = c.load_image(actual_checksum);
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
    let source_key = decode_image(&mut source_ctx, 0);
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
    require: Constraints,
    source_url: Option<&str>,
    do_panic: bool,
) -> bool {
    if !check_size(&result, require.clone(), do_panic) {
        return false;
    }
    let (exact, trusted, actual) = result.exact_match(c, module, test_name, detail_name);
    if exact == ChecksumMatch::Match {
        return true;
    }
    if exact == ChecksumMatch::NewStored {
        return true;
    }

    let flat_name = if detail_name.is_empty() {
        test_name.to_string()
    } else {
        format!("{test_name} {detail_name}")
    };

    eprintln!("--- Checksum mismatch for '{flat_name}' ---");
    let (expected_context, expected_bitmap_key) = c.load_image(&trusted);
    let res = compare_bitmaps_result_to_expected(
        c,
        result,
        false,
        expected_context,
        expected_bitmap_key,
        require.similarity,
        do_panic,
    );
    if res.close_enough {
        eprintln!(
            "--- '{flat_name}': checksum mismatch within tolerance ({:?}) ---",
            require.similarity
        );

        // Auto-accept to .checksums if within tolerance
        if std::env::var("UPDATE_CHECKSUMS").is_ok_and(|v| v == "1") {
            let adapter = checksum_adapter::ChecksumAdapter::new(&c.checksums_dir);
            // Compute zensim quality vs original source (same-dimension only)
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
    res.close_enough
}

pub fn compare_bitmaps_result_to_expected<'a>(
    c: &ChecksumCtx,
    result: ResultKind<'a>,
    calculate_checksum: bool,
    expected_context: Box<Context>,
    expected_bitmap_key: BitmapKey,
    require: Similarity,
    panic: bool,
) -> CompareBitmapsResult {
    let mut image_context = Context::create().unwrap();
    let (actual_context, actual_bitmap_key) = match result {
        ResultKind::Bitmap { context, key } => (context, key),
        ResultKind::Bytes(actual_bytes) => {
            // SAFETY: `actual_bytes` is a parameter that outlives local `image_context`
            unsafe { image_context.add_input_bytes(0, actual_bytes) }.unwrap();
            let key = decode_image(&mut image_context, 0);
            (image_context.as_ref(), key)
        }
    };

    let actual_bitmaps = actual_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();
    let mut actual_bitmap =
        actual_bitmaps.try_borrow_mut(actual_bitmap_key).map_err(|e| e.at(here!())).unwrap();
    let mut actual = actual_bitmap.get_window_u8().unwrap();

    let actual_checksum = if calculate_checksum {
        Some(ChecksumCtx::checksum_bitmap_window(&mut actual))
    } else {
        None
    };

    let mut res;
    {
        let expected_bitmaps =
            expected_context.borrow_bitmaps().map_err(|e| e.at(here!())).unwrap();

        let mut expected_bitmap = expected_bitmaps
            .try_borrow_mut(expected_bitmap_key)
            .map_err(|e| e.at(here!()))
            .unwrap();
        let mut expected = expected_bitmap.get_window_u8().unwrap();
        res = compare_bitmaps(c, &mut actual, &mut expected, require, panic);
    }
    drop(expected_context); // Context must remain in scope until we are done with expected_bitmap
    res.actual_checksum = actual_checksum;
    res
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

    evaluate_result(
        &ctx,
        identity.module,
        identity.func_name,
        detail,
        ResultKind::Bytes(&bytes),
        require,
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
    allowed_off_by_one_bytes: usize,
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
        Constraints {
            similarity: Similarity::AllowOffByOneBytesCount(allowed_off_by_one_bytes as i64),
            max_file_size: None,
        },
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
