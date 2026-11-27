use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, Result};

use super::*;
use crate::codecs::cms;
use crate::codecs::source_profile::SourceProfile;
use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use crate::io::IoProxy;
use imageflow_types::PixelLayout;
use std::borrow::Cow;

use zc::decode::{DecodeRowSink, DynAnimationFrameDecoder, DynDecoderConfig, RowStrip, SinkError};
use zc::how_far::StopOnly;
use zc::CodecErrorExt as _;
use zc::ImageFormat as ZenFormat;
use zc::ImageInfo as ZenImageInfo;
use zc::{OwnedAnimationFrame, Pulse, SharedPulse};
use zenpixels::{ChannelOrder, FormatCow, PixelFormat, PixelView, PixelViewMut, SampleType};

/// Fallback CMYK ICC profile used when a CMYK JPEG has no embedded profile.
/// Same profile used by the mozjpeg (C) decoder path for consistent output.
static FALLBACK_CMYK_PROFILE: &[u8] = include_bytes!("cmyk.icc");

/// Whether this format supports EXIF orientation metadata.
fn has_exif_orientation(fmt: ZenFormat) -> bool {
    matches!(fmt, ZenFormat::Jpeg | ZenFormat::Jxl)
}

/// Whether this format can produce CMYK source data (needing ICC-based conversion).
/// JPEG is the only zen codec today that can decode CMYK.
fn may_have_cmyk(fmt: ZenFormat) -> bool {
    matches!(fmt, ZenFormat::Jpeg)
}

/// Whether to always use the frame decoder path (even for single-frame files).
/// True for GIF because its probe can't detect animation status.
fn always_use_frame_decoder(fmt: ZenFormat) -> bool {
    matches!(fmt, ZenFormat::Gif)
}

/// Whether the format's buffered Decode::decode is faster than push_decode.
///
/// True for JPEG: push_decoder_direct has stride/output issues that cause
/// visual corruption when writing into SIMD-aligned bitmaps (159 test
/// failures). Until zenjpeg's push_decode path fully supports strided
/// output and matches the buffered path's metadata/orientation handling,
/// JPEG stays on the buffered path. The fused BGRA streaming kernel is
/// still active via Decoder::decode() → streaming_output_format.
///
/// TODO: investigate push_decoder_direct failures and switch JPEG to
/// push_decode for zero-copy bitmap writes.
fn prefers_buffered_decode(fmt: ZenFormat) -> bool {
    matches!(fmt, ZenFormat::Jpeg)
}

/// Unified decoder for all zen codec formats.
///
/// Uses zencodec dyn dispatch for all formats (JPEG, PNG, WebP, GIF, AVIF
/// JXL). Each format provides a DynDecoderConfig; the single-frame path uses
/// push_decode for zero-copy row streaming into the graph bitmap.
pub struct ZenDecoder {
    config: Box<dyn DynDecoderConfig>,
    io: IoProxy,
    data: Option<Vec<u8>>,
    cached_info: Option<ZenImageInfo>,
    // Persistent frame decoder for animation
    frame_dec: Option<Box<dyn DynAnimationFrameDecoder>>,
    frame_index: u32,
    // Peeked next frame (used for has_more_frames detection)
    peeked_frame: Option<OwnedAnimationFrame>,
    // Deferred error encountered while peeking the next animation frame
    peek_error: Option<FlowError>,
    // Exact rational duration from last decoded animation frame (None = unknown / still)
    last_frame_duration: Option<zc::animation::FrameDuration>,
    loop_count: Option<u32>,
    // Decoder options
    ignore_color_profile: bool,
    ignore_color_profile_errors: bool,
    // Resource limits for decode jobs
    resource_limits: Option<zc::ResourceLimits>,
    // Cooperative cancellation pulse threaded into every decode job
    pulse: Option<SharedPulse>,
    // Cached max_threads-sized rayon pool, reused across decode calls and animation frames
    // so the codec's internal par_iter is bounded without re-spawning a pool per frame.
    thread_pool: Option<rayon::ThreadPool>,
    thread_pool_threads: Option<u32>,
    // Image format (provides extension, mime type, animation support)
    format: ZenFormat,
    // Whether has_more_frames is known
    has_more: Option<bool>,
    // Target frame index for SelectFrame command
    target_frame: Option<u32>,
}

/// Map imageflow `ExecutionSecurity` decode caps → zencodec `ResourceLimits`
/// for decode jobs. Returns `None` when no decode-relevant limit is configured.
pub(crate) fn decode_limits_from_security(
    security: &imageflow_types::ExecutionSecurity,
) -> Option<zc::ResourceLimits> {
    let frame = security.max_decode_size.as_ref().or(security.max_frame_size.as_ref());
    if frame.is_none()
        && security.max_input_file_bytes.is_none()
        && security.max_total_file_pixels.is_none()
    {
        return None;
    }
    let mut limits = zc::ResourceLimits::default();
    if let Some(f) = frame {
        let max_pixels = (f.megapixels as f64 * 1_000_000.0) as u64;
        limits.max_pixels = Some(max_pixels);
        limits.max_memory_bytes = Some(max_pixels.saturating_mul(4));
        limits.max_width = Some(f.w);
        limits.max_height = Some(f.h);
    }
    if let Some(bytes) = security.max_input_file_bytes {
        limits.max_input_bytes = Some(bytes as u64);
    }
    if let Some(pixels) = security.max_total_file_pixels {
        limits.max_total_pixels = Some(pixels);
    }
    Some(limits)
}

/// Map imageflow `ExecutionSecurity` encode caps → zencodec `ResourceLimits`
/// for encode jobs. Returns `None` when no encode-relevant limit is configured.
pub(crate) fn encode_limits_from_security(
    security: &imageflow_types::ExecutionSecurity,
) -> Option<zc::ResourceLimits> {
    let frame = security.max_encode_size.as_ref().or(security.max_frame_size.as_ref())?;
    let mut limits = zc::ResourceLimits::default();
    let max_pixels = (frame.megapixels as f64 * 1_000_000.0) as u64;
    limits.max_pixels = Some(max_pixels);
    limits.max_memory_bytes = Some(max_pixels.saturating_mul(4));
    limits.max_width = Some(frame.w);
    limits.max_height = Some(frame.h);
    Some(limits)
}

/// `encode_limits_from_security`, additionally capping `max_memory_bytes` to the
/// `MemBudgetPolicy` ceiling minus the exact `w·h·4` source buffer (the per-stage
/// runtime backstop beneath the pre-flight estimate gate).
pub(crate) fn encode_limits_capped(
    security: &imageflow_types::ExecutionSecurity,
    w: u32,
    h: u32,
) -> Option<zc::ResourceLimits> {
    let mut limits = encode_limits_from_security(security);
    if let Some(ceiling) = security.mem_budget_policy.as_ref().and_then(|p| p.byte_ceiling()) {
        let buffer = (w as u64).saturating_mul(h as u64).saturating_mul(4);
        let cap = ceiling.saturating_sub(buffer).max(1);
        let l = limits.get_or_insert_with(zc::ResourceLimits::default);
        l.max_memory_bytes = Some(l.max_memory_bytes.map_or(cap, |m| m.min(cap)));
    }
    limits
}

/// Run `f` inside a freshly-built `max_threads`-sized rayon pool so the codec's
/// internal `par_iter` is bounded to that thread count; runs on the ambient pool
/// when unset or when pool construction fails. `f` and its return must be `Send`.
/// `DynDecodeJob`/`DynDecoder` are NOT `Send`, but a single-frame decode is still
/// bounded here by BUILDING the job+decoder *inside* `f`, so only the `Send`
/// config/data/output cross the pool boundary. For repeated calls (animation
/// frames) cache the pool with `build_pool` + `install_pooled` instead, so it is
/// spawned once rather than per frame.
pub(crate) fn run_pooled<R: Send>(max_threads: Option<u32>, f: impl FnOnce() -> R + Send) -> R {
    install_pooled(&build_pool(max_threads), f)
}

/// Build a `max_threads`-sized rayon pool (`None` ⇒ use the ambient pool). Cache
/// the result and reuse it via `install_pooled` across many calls — e.g. per-frame
/// animation decode/encode — so the pool is spawned once, not per call.
pub(crate) fn build_pool(max_threads: Option<u32>) -> Option<rayon::ThreadPool> {
    match max_threads {
        Some(n) if n >= 1 => rayon::ThreadPoolBuilder::new().num_threads(n as usize).build().ok(),
        _ => None,
    }
}

/// Run `f` on `pool` if present, else on the ambient pool. `f`/`R` must be `Send`.
pub(crate) fn install_pooled<R: Send>(
    pool: &Option<rayon::ThreadPool>,
    f: impl FnOnce() -> R + Send,
) -> R {
    match pool {
        Some(p) => p.install(f),
        None => f(),
    }
}

/// Pre-flight memory gate for one codec stage: the codec's working-set estimate
/// plus the EXACT BGRA frame buffer (`w·h·4`). Returns the violated
/// `(assertion, limit, actual)` when a present `MemBudgetPolicy` estimate
/// threshold is exceeded, else `Ok(())`. Codecs without a real estimator return
/// `unknown()` (all `None`) → treated as a zero working set, so only the exact
/// buffer is gated until real estimators ship. `peak = max(decode_peak,
/// encode_peak) + buffer` is enforced by each stage checking `codec_peak +
/// buffer` against the same policy (the peak lands at whichever stage is larger).
pub(crate) fn check_stage_memory(
    policy: &imageflow_types::MemBudgetPolicy,
    codec_est: &zc::estimate::ResourceEstimate,
    w: u32,
    h: u32,
) -> std::result::Result<(), (&'static str, u64, u64)> {
    let buffer_bytes = (w as u64).saturating_mul(h as u64).saturating_mul(4);
    let codec_avg = codec_est.peak_memory_bytes_est().unwrap_or(0);
    let codec_max = codec_est.peak_memory_bytes_max().unwrap_or(codec_avg);
    let peak_avg = codec_avg.saturating_add(buffer_bytes);
    let peak_max = codec_max.saturating_add(buffer_bytes);
    match policy.check_estimates(peak_avg, peak_max) {
        Some(v) => Err(v),
        None => Ok(()),
    }
}

impl ZenDecoder {
    fn new_zencodec(
        c: &Context,
        config: Box<dyn DynDecoderConfig>,
        io: IoProxy,
        format: ZenFormat,
    ) -> Self {
        let max_threads = c.security.max_threads;
        ZenDecoder {
            config,
            io,
            data: None,
            cached_info: None,
            frame_dec: None,
            frame_index: 0,
            peeked_frame: None,
            peek_error: None,
            last_frame_duration: None,
            loop_count: None,
            ignore_color_profile: false,
            ignore_color_profile_errors: false,
            resource_limits: decode_limits_from_security(&c.security),
            pulse: Some(SharedPulse::new(StopOnly::new(c.cancellation_token()))),
            thread_pool: build_pool(max_threads),
            thread_pool_threads: max_threads,
            format,
            has_more: None,
            target_frame: None,
        }
    }

    /// Keep the cached rayon pool aligned with `c.security.max_threads` (which may
    /// be updated via `Context::configure_security` after `add_input_*` created the
    /// decoder).
    fn ensure_thread_pool(&mut self, max_threads: Option<u32>) {
        if self.thread_pool_threads != max_threads {
            self.thread_pool = build_pool(max_threads);
            self.thread_pool_threads = max_threads;
        }
    }

    /// Codec working-set estimate (excludes the BGRA frame buffer) for decoding
    /// into `w×h`. Returns `unknown()` (zeros) until the codec ships an estimator;
    /// used by `v1/estimate` to mirror the run-path decode memory gate.
    pub(crate) fn estimate_decode(
        &self,
        env: &zc::estimate::ComputeEnvironment,
        w: u32,
        h: u32,
    ) -> zc::estimate::ResourceEstimate {
        let chars = zc::estimate::ImageCharacteristics::new(w, h, PixelFormat::BGRA8_SRGB);
        self.config.estimate_decode_resources(&chars, env)
    }

    #[cfg(feature = "zen-codecs")]
    pub fn create_jpeg(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        // CmykHandling::Passthrough — emit raw inverted CMYK bytes (ChannelOrder::CmykInverted)
        // for 4-component JPEGs instead of applying zenjpeg's internal CMYK→RGB
        // matrix. The CMS stage then applies the source ICC profile (or the
        // bundled fallback CMYK profile) for an accurate conversion. No effect
        // on 3-component JPEGs. (Passthrough is the default in zenjpeg post-
        // ebb0e24f, but set explicitly so the intent survives any default flip.)
        let config =
            zenjpeg::JpegDecoderConfig::new().cmyk_handling(zenjpeg::CmykHandling::Passthrough);
        Ok(Self::new_zencodec(c, Box::new(config), io, ZenFormat::Jpeg))
    }

    #[cfg(feature = "zen-codecs")]
    pub fn create_webp(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenwebp::zencodec::WebpDecoderConfig::new();
        Ok(Self::new_zencodec(c, Box::new(config), io, ZenFormat::WebP))
    }

    #[cfg(feature = "zen-codecs")]
    pub fn create_png(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenpng::PngDecoderConfig::new();
        Ok(Self::new_zencodec(c, Box::new(config), io, ZenFormat::Png))
    }

    #[cfg(feature = "zen-codecs")]
    pub fn create_gif(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zengif::GifDecoderConfig::new();
        Ok(Self::new_zencodec(c, Box::new(config), io, ZenFormat::Gif))
    }

    #[cfg(feature = "zen-codecs")]
    pub fn create_avif(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenavif::AvifDecoderConfig::new();
        Ok(Self::new_zencodec(c, Box::new(config), io, ZenFormat::Avif))
    }

    #[cfg(feature = "zen-codecs")]
    pub fn create_jxl(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenjxl::JxlDecoderConfig::new();
        Ok(Self::new_zencodec(c, Box::new(config), io, ZenFormat::Jxl))
    }

    pub fn create_bmp(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenbitmaps::BmpDecoderConfig::new();
        Ok(Self::new_zencodec(c, Box::new(config), io, ZenFormat::Bmp))
    }

    /// Ensure input bytes are available, either from a memory-backed IoProxy
    /// (zero-copy) or by buffering a file-backed IoProxy into self.data.
    fn ensure_data_available(&mut self) -> Result<()> {
        // Memory-backed: bytes are already accessible via try_peek_all()
        if self.io.try_peek_all().is_some() {
            return Ok(());
        }
        // File-backed: read into self.data
        if self.data.is_none() {
            let mut bytes = Vec::with_capacity(8192);
            self.io.read_to_end(&mut bytes).map_err(FlowError::from_decoder)?;
            self.data = Some(bytes);
        }
        Ok(())
    }

    /// Get a reference to the input bytes. For memory-backed IoProxy this is
    /// zero-copy; for file-backed, returns the buffered data.
    /// Must call ensure_data_available() first.
    fn data_slice(&self) -> &[u8] {
        self.io.try_peek_all().unwrap_or_else(|| {
            self.data.as_ref().expect("ensure_data_available not called").as_slice()
        })
    }

    /// Get owned input bytes for animation decoder (which needs ownership).
    /// For memory-backed IoProxy, copies the bytes. For file-backed, takes
    /// the already-buffered Vec.
    fn take_data_owned(&mut self) -> Vec<u8> {
        if let Some(data) = self.data.take() {
            data
        } else if let Some(slice) = self.io.try_peek_all() {
            slice.to_vec()
        } else {
            panic!("ensure_data_available not called")
        }
    }

    fn ensure_info_probed(&mut self) -> Result<()> {
        self.ensure_data_available()?;
        if self.cached_info.is_none() {
            let data = self.data_slice();
            let mut job = self.config.dyn_job();
            if let Some(ref limits) = self.resource_limits {
                job.set_limits(*limits);
            }
            if let Some(ref pulse) = self.pulse {
                job.set_pulse(pulse.clone());
            }
            let info = job.probe(data).map_err(|e| {
                FlowError::from_codec_error(
                    ErrorKind::ImageDecodingError,
                    &format!("{} probe", self.format.extension()),
                    &*e,
                )
            })?;
            self.cached_info = Some(info);
        }
        Ok(())
    }

    /// Extract SourceProfile from zencodec-types ImageInfo for CMS.
    fn source_profile_from_info(&self, info: &ZenImageInfo) -> SourceProfile {
        // CMYK JPEGs: zenjpeg emits raw CMYK bytes (PixelDescriptor::CMYK8) when
        // cmyk_output_raw is enabled. Route to CmykIcc so the CMS stage applies
        // the embedded ICC profile (or a fallback CMYK profile) for accurate
        // conversion to sRGB. CMYK always needs color management — we apply it
        // even when ignore_color_profile is set, matching the mozjpeg (C) path.
        if may_have_cmyk(self.format)
            && info.source_color.channel_count == Some(4)
            && !info.has_alpha
        {
            let icc = info
                .source_color
                .icc_profile
                .as_ref()
                .map(|p| p.to_vec())
                .unwrap_or_else(|| FALLBACK_CMYK_PROFILE.to_vec());
            return SourceProfile::CmykIcc(icc);
        }

        if self.ignore_color_profile {
            return SourceProfile::Srgb;
        }

        // Priority: ICC profile > CICP > sRGB
        if let Some(ref icc) = info.source_color.icc_profile {
            // Determine if grayscale from channel count
            if info.source_color.channel_count == Some(1) {
                return SourceProfile::IccProfileGray(icc.to_vec());
            }
            return SourceProfile::IccProfile(icc.to_vec());
        }

        if let Some(cicp) = info.source_color.cicp {
            return SourceProfile::Cicp {
                color_primaries: cicp.color_primaries,
                transfer_characteristics: cicp.transfer_characteristics,
                matrix_coefficients: cicp.matrix_coefficients,
                full_range: cicp.full_range,
            };
        }

        SourceProfile::Srgb
    }

    /// Create a DynDecodeJob for pixel decoding with resource limits, pulse, and
    /// ICC policy applied via [`configure_job`].
    fn make_job(&self) -> Box<dyn zc::decode::DynDecodeJob<'_> + '_> {
        let mut job = self.config.dyn_job();
        configure_job(&mut *job, self.resource_limits, self.pulse.clone());
        job
    }
}

/// Apply resource limits, cancellation pulse, and ICC policy to a pixel-decode job.
///
/// Disables ICC attachment on the pixel-decode job (`allow_icc = false`):
/// `ensure_info_probed()` already extracted `source_color.icc_profile` for
/// imageflow's CMS stage (`cms::transform_to_srgb`), which owns policy for
/// `DiscardColorProfile`, `IgnoreColorProfileErrors`, and ignoring mismatched
/// profiles such as CMYK ICC on YCbCr/grayscale JPEGs (imageflow#737).
fn configure_job(
    job: &mut dyn zc::decode::DynDecodeJob<'_>,
    limits: Option<zc::ResourceLimits>,
    pulse: Option<SharedPulse>,
) {
    if let Some(l) = limits {
        job.set_limits(l);
    }
    if let Some(p) = pulse {
        job.set_pulse(p);
    }
    let _ = job.try_set_policy(zc::decode::DecodePolicy::none().with_allow_icc(false));
}

/// Convert a zencodec error into a `FlowError`, preserving cancellation as
/// `ErrorKind::OperationCancelled`.
#[track_caller]
fn decode_error_to_flow(
    context: &str,
    e: &(dyn std::error::Error + Send + Sync + 'static),
) -> FlowError {
    if let Some(zc::ErrorCategory::Stopped(reason)) = e.error_category() {
        return FlowError::from(reason);
    }
    FlowError::from_codec_error(ErrorKind::ImageDecodingError, context, e)
}

/// Convert one row of arbitrary U8 pixels into BGRA.
#[inline]
fn row_to_bgra(dst: &mut [u8], src: &[u8], width: usize, channels: usize) {
    match channels {
        4 => {
            garb::bytes::rgba_to_bgra(&src[..width * 4], &mut dst[..width * 4]).unwrap();
        }
        3 => {
            garb::bytes::rgb_to_bgra(&src[..width * 3], &mut dst[..width * 4]).unwrap();
        }
        1 => {
            garb::bytes::gray_to_rgba(&src[..width], &mut dst[..width * 4]).unwrap();
        }
        _ => {
            for x in 0..width {
                let si = x * channels;
                let di = x * 4;
                if channels >= 4 {
                    dst[di] = src[si + 2]; // B
                    dst[di + 1] = src[si + 1]; // G
                    dst[di + 2] = src[si]; // R
                    dst[di + 3] = src[si + 3]; // A
                } else {
                    let v = src[si];
                    dst[di] = v;
                    dst[di + 1] = v;
                    dst[di + 2] = v;
                    dst[di + 3] = 255;
                }
            }
        }
    }
}

/// Convert a `zencodec::animation::FrameDuration` to whole milliseconds
/// (truncating sub-millisecond fractions, saturating at `u32::MAX`).
#[inline]
fn frame_duration_to_ms(dur: zc::animation::FrameDuration) -> u32 {
    ((u128::from(dur.numerator()) * 1000 / u128::from(dur.denominator())).min(u128::from(u32::MAX)))
        as u32
}

/// Copy decoded pixels from a PixelView into a bitmap, handling stride and BGRA swizzle.
fn copy_pixel_slice_to_bitmap(dst: &mut [u8], dst_stride: usize, ps: &PixelView<'_>) {
    let w = ps.width();
    let h = ps.height();
    let format = ps.format();

    let is_bgra =
        format.layout.order == ChannelOrder::Bgra && format.layout.sample == SampleType::U8;
    let is_rgba =
        format.layout.order == ChannelOrder::Rgba && format.layout.sample == SampleType::U8;
    // CMYK: pass through bytes verbatim into the BGRA bitmap slot — the
    // downstream CMS stage (SourceProfile::CmykIcc) interprets the inverted CMYK
    // bytes and applies the ICC transform to BGRA. Swizzling here would corrupt
    // the C/M/Y/K channel order that CMS expects.
    let is_cmyk = matches!(format.layout.order, ChannelOrder::Cmyk | ChannelOrder::CmykInverted)
        && format.layout.sample == SampleType::U8;

    let row_bytes = w as usize * 4;

    if is_bgra || is_cmyk {
        for y in 0..h {
            let src_row = ps.row(y);
            let dst_start = y as usize * dst_stride;
            dst[dst_start..dst_start + row_bytes].copy_from_slice(&src_row[..row_bytes]);
        }
    } else if is_rgba {
        for y in 0..h {
            let src_row = ps.row(y);
            let dst_start = y as usize * dst_stride;
            dst[dst_start..dst_start + row_bytes].copy_from_slice(&src_row[..row_bytes]);
        }
        let _ = garb::bytes::rgba_to_bgra_inplace_strided(dst, w as usize, h as usize, dst_stride);
    } else {
        let channels = format.layout.channels();
        for y in 0..h {
            let src_row = ps.row(y);
            let dst_start = y as usize * dst_stride;
            row_to_bgra(
                &mut dst[dst_start..dst_start + w as usize * 4],
                src_row,
                w as usize,
                channels,
            );
        }
    }
}

// ─── BitmapRowSink ───────────────────────────────────────────────────────────

/// Row sink that writes decoded strips directly into a bitmap's BGRA strided buffer.
///
/// For 8-bit 4bpp formats (BGRA, RGBA): writes directly into the bitmap buffer
/// using the bitmap stride. RGBA is swizzled to BGRA in `finish()`.
///
/// For 8-bit non-4bpp formats (RGB, Gray): uses a temporary buffer per strip,
/// then converts into the bitmap with pixel expansion.
///
/// Non-U8 formats (e.g., 16-bit RGB) are rejected in `begin()` before any
/// decode work happens. The caller falls back to `into_decoder` with format
/// negotiation.
struct BitmapRowSink<'a> {
    data: &'a mut [u8],
    stride: usize,
    width: u32,
    height: u32,
    temp: Vec<u8>,
    /// True for 4bpp U8 (BGRA/RGBA) — writes directly into bitmap.
    is_4bpp: bool,
    /// True if output is RGBA and needs BGRA swizzle in finish().
    needs_swizzle: bool,
    /// True if `begin()` rejected a non-U8 pixel format (signals fallback to `into_decoder`).
    rejected_format: bool,
    /// Metadata of the previous non-4bpp strip (for deferred conversion).
    prev_strip: Option<StripMeta>,
}

#[derive(Clone)]
struct StripMeta {
    y: u32,
    height: u32,
    width: u32,
    format: PixelFormat,
}

impl BitmapRowSink<'_> {
    fn new(data: &mut [u8], stride: usize) -> BitmapRowSink<'_> {
        BitmapRowSink {
            data,
            stride,
            width: 0,
            height: 0,
            temp: Vec::new(),
            is_4bpp: false,
            needs_swizzle: false,
            rejected_format: false,
            prev_strip: None,
        }
    }

    /// Convert a non-4bpp U8 strip from `self.temp` into the bitmap.
    fn convert_strip(&mut self, meta: StripMeta) {
        let bpp = meta.format.layout.bytes_per_pixel();
        let src_stride = meta.width as usize * bpp;
        let channels = meta.format.layout.channels();

        for row in 0..meta.height as usize {
            let src_start = row * src_stride;
            let dst_start = (meta.y as usize + row) * self.stride;
            let w = meta.width as usize;
            row_to_bgra(
                &mut self.data[dst_start..dst_start + w * 4],
                &self.temp[src_start..src_start + w * channels],
                w,
                channels,
            );
        }
    }
}

impl DecodeRowSink for BitmapRowSink<'_> {
    fn begin(
        &mut self,
        width: u32,
        height: u32,
        format: PixelFormat,
    ) -> std::result::Result<(), SinkError> {
        if format.layout.sample != SampleType::U8 {
            self.rejected_format = true;
            return Err(format!("BitmapRowSink requires U8 channels, got {:?}", format).into());
        }
        self.width = width;
        self.height = height;
        self.is_4bpp = format.layout.bytes_per_pixel() == 4;
        self.needs_swizzle = format.layout.order == ChannelOrder::Rgba;
        Ok(())
    }

    fn provide_next_buffer(
        &mut self,
        strip: RowStrip,
        pixel_format: PixelFormat,
    ) -> std::result::Result<PixelViewMut<'_>, SinkError> {
        let (y, width, height) = (strip.y, strip.width, strip.height);
        // Bounds check: ensure strip fits within allocated bitmap
        let end_row = y as usize + height as usize;
        if end_row > self.height as usize || width as usize > self.width as usize {
            return Err(format!(
                "strip y={y} h={height} w={width} exceeds bitmap {}x{}",
                self.width, self.height
            )
            .into());
        }

        if self.is_4bpp {
            // 4bpp (BGRA/RGBA): write directly into bitmap at the correct row offset.
            let row_bytes = width as usize * 4;
            let row_start = y as usize * self.stride;
            let needed =
                if height > 0 { (height as usize - 1) * self.stride + row_bytes } else { 0 };
            if row_start + needed > self.data.len() {
                return Err(format!(
                    "strip at y={y} needs {} bytes but bitmap has {}",
                    row_start + needed,
                    self.data.len()
                )
                .into());
            }
            let slice = &mut self.data[row_start..row_start + needed];
            let format_cow = match pixel_format.as_static_ref() {
                Some(s) => FormatCow::Borrowed(s),
                None => FormatCow::from_owned(pixel_format),
            };
            PixelViewMut::new_with_format_cow(slice, width, height, self.stride, format_cow)
                .map_err(|e| -> SinkError { format!("{e}").into() })
        } else {
            // Non-4bpp U8: convert the previous strip before reusing temp buffer
            if let Some(meta) = self.prev_strip.take() {
                self.convert_strip(meta);
            }

            let bpp = pixel_format.layout.bytes_per_pixel();
            let src_stride = width as usize * bpp;
            let needed = height as usize * src_stride;
            self.temp.resize(needed, 0);

            let format_cow = match pixel_format.as_static_ref() {
                Some(s) => FormatCow::Borrowed(s),
                None => FormatCow::from_owned(pixel_format.clone()),
            };
            self.prev_strip = Some(StripMeta { y, height, width, format: pixel_format });

            PixelViewMut::new_with_format_cow(&mut self.temp, width, height, src_stride, format_cow)
                .map_err(|e| -> SinkError { format!("{e}").into() })
        }
    }

    fn finish(&mut self) -> std::result::Result<(), SinkError> {
        // Flush last non-4bpp strip
        if let Some(meta) = self.prev_strip.take() {
            self.convert_strip(meta);
        }
        // Swizzle RGBA→BGRA in-place
        if self.needs_swizzle {
            let _ = garb::bytes::rgba_to_bgra_inplace_strided(
                self.data,
                self.width as usize,
                self.height as usize,
                self.stride,
            );
        }
        Ok(())
    }
}

// ─── Decoder trait impl ──────────────────────────────────────────────────────

impl Decoder for ZenDecoder {
    fn initialize(&mut self, _c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, _c: &Context) -> Result<s::ImageInfo> {
        self.ensure_info_probed()?;
        let info = self.cached_info.as_ref().unwrap();
        let lossless =
            info.source_encoding_details().map(|d| d.is_lossless()).unwrap_or_else(|| {
                matches!(
                    self.format,
                    ZenFormat::Png
                        | ZenFormat::Gif
                        | ZenFormat::Bmp
                        | ZenFormat::Pnm
                        | ZenFormat::Qoi
                        | ZenFormat::Tga
                        | ZenFormat::Farbfeld
                )
            });
        let multiple_frames = matches!(
            info.sequence,
            zc::ImageSequence::Animation { frame_count: None, .. }
                | zc::ImageSequence::Animation { frame_count: Some(2..), .. }
        );
        Ok(s::ImageInfo {
            frame_decodes_into: if info.has_alpha {
                s::PixelFormat::Bgra32
            } else {
                s::PixelFormat::Bgr32
            },
            image_width: info.width as i32,
            image_height: info.height as i32,
            preferred_mime_type: self.format.mime_type().to_owned(),
            preferred_extension: self.format.extension().to_owned(),
            lossless,
            multiple_frames,
        })
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_exif_rotation_flag(&mut self, _c: &Context) -> Result<Option<i32>> {
        if !has_exif_orientation(self.format) {
            return Ok(None);
        }
        self.ensure_info_probed()?;
        let info = self.cached_info.as_ref().unwrap();
        let orient = info.orientation as u8;
        if orient > 1 {
            return Ok(Some(orient as i32));
        }
        // `ImageInfo.orientation` defaults to `Orientation::Identity` (1) even when
        // no EXIF orientation tag is present. Inspect the probed EXIF blob so an
        // explicit Orientation=1 tag reports `Some(1)` (matching mozjpeg) while
        // absent orientation tags still report `None`.
        if let Some(ref exif) = info.embedded_metadata.exif
            && let Some(parsed) = zc::exif::Exif::parse(exif).and_then(|e| e.orientation())
        {
            return Ok(Some(parsed as u8 as i32));
        }
        Ok(None)
    }

    fn tell_decoder(&mut self, _c: &Context, tell: s::DecoderCommand) -> Result<()> {
        match tell {
            s::DecoderCommand::DiscardColorProfile => {
                self.ignore_color_profile = true;
            }
            s::DecoderCommand::IgnoreColorProfileErrors => {
                self.ignore_color_profile_errors = true;
            }
            s::DecoderCommand::SelectFrame(frame) => {
                self.target_frame = Some(frame.max(0) as u32);
            }
            _ => {}
        }
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        return_if_cancelled!(c);
        if let Some(err) = self.peek_error.take() {
            return Err(err);
        }
        self.ensure_thread_pool(c.security.max_threads);
        self.resource_limits = decode_limits_from_security(&c.security);
        self.ensure_data_available()?;
        self.ensure_info_probed()?;

        // ── Zencodec path (all formats via push_decode) ──
        let info = self.cached_info.as_ref().unwrap().clone();
        let source_profile = self.source_profile_from_info(&info);
        let has_alpha = info.has_alpha;

        // Pre-flight decode memory gate: codec working-set estimate + decoded buffer.
        if let Some(policy) = c.security.mem_budget_policy.as_ref() {
            let chars = zc::estimate::ImageCharacteristics::new(
                info.width,
                info.height,
                PixelFormat::BGRA8_SRGB,
            );
            let mut env = zc::estimate::ComputeEnvironment::host();
            if let Some(n) = c.security.max_threads {
                env = env.with_cores(n as usize);
            }
            let est = self.config.estimate_decode_resources(&chars, &env);
            if let Err((name, limit, actual)) =
                check_stage_memory(policy, &est, info.width, info.height)
            {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "decode memory budget {} exceeded: estimated {} bytes >= limit {} bytes",
                    name,
                    actual,
                    limit
                ));
            }
            // Cap the codec's runtime working set to the budget ceiling minus the
            // exact decoded buffer (per-stage backstop beneath the estimate gate).
            if let Some(ceiling) = policy.byte_ceiling() {
                let buffer =
                    (info.width as u64).saturating_mul(info.height as u64).saturating_mul(4);
                let codec_cap = ceiling.saturating_sub(buffer).max(1);
                let limits = self.resource_limits.get_or_insert_with(zc::ResourceLimits::default);
                limits.max_memory_bytes =
                    Some(limits.max_memory_bytes.map_or(codec_cap, |m| m.min(codec_cap)));
            }
        }
        // Request BGRA8_SRGB only — every zen codec supports it natively, so
        // the codec does the (possibly vectorized) swizzle internally and
        // writes straight into the imageflow BGRA bitmap. No RGBA/RGB/Gray
        // fallback path: if a codec ever drops BGRA support that's a bug
        // upstream, not something to silently paper over here.
        // (CMYK JPEGs with CmykHandling::Passthrough still emit CmykInverted and
        // bypass this negotiation — handled as a special case downstream.)
        let preferred: &[PixelFormat] = &[PixelFormat::BGRA8_SRGB];

        // Animation path: use persistent full-frame decoder
        let is_animated = matches!(info.sequence, zc::ImageSequence::Animation { .. });
        if always_use_frame_decoder(self.format) || is_animated || self.frame_dec.is_some() {
            if self.frame_dec.is_none() {
                // Animation decoder needs ownership; use Cow::Owned.
                // For memory-backed IoProxy this copies; for file-backed it takes the buffer.
                let data = self.take_data_owned();
                let target = self.target_frame;
                if let Some(t) = target {
                    self.frame_index = t;
                }
                let mut job = self.make_job();
                // Use set_start_frame_index to let the codec skip internally
                if let Some(t) = target {
                    job.set_start_frame_index(t);
                }
                let frame_dec =
                    job.into_animation_frame_decoder(Cow::Owned(data), preferred).map_err(|e| {
                        FlowError::from_codec_error(
                            ErrorKind::ImageDecodingError,
                            &format!("{} frame decoder", self.format.extension()),
                            &*e,
                        )
                    })?;
                self.loop_count = frame_dec.loop_count();
                self.frame_dec = Some(frame_dec);
            }

            let frame_dec = self.frame_dec.as_mut().unwrap();

            // Get next frame from peeked buffer or from decoder
            let frame = if let Some(f) = self.peeked_frame.take() {
                Some(f)
            } else {
                // Thread the per-call pulse: the animation frame decoders for
                // GIF/WebP/AVIF drop the job pulse and honor ONLY this argument,
                // so passing None here leaves animated decode uncancellable.
                let pulse = self.pulse.clone();
                install_pooled(&self.thread_pool, move || {
                    frame_dec.render_next_frame_owned(pulse.as_ref().map(|p| p as &dyn Pulse))
                })
                .map_err(|e| decode_error_to_flow("frame decode", &*e))?
            };

            let frame = frame
                .ok_or_else(|| nerror!(ErrorKind::InvalidOperation, "No more frames available"))?;

            self.last_frame_duration = Some(frame.duration());
            self.frame_index += 1;

            let ps = frame.pixels();
            let w = ps.width();
            let h = ps.height();

            let mut bitmaps = c.borrow_bitmaps_mut().map_err(|e| e.at(here!()))?;
            let bitmap_key = bitmaps
                .create_bitmap_u8(
                    w,
                    h,
                    PixelLayout::BGRA,
                    false,
                    info.has_alpha,
                    ColorSpace::StandardRGB,
                    BitmapCompositing::ReplaceSelf,
                )
                .map_err(|e| e.at(here!()))?;

            {
                let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
                let mut window = bitmap.get_window_u8().unwrap();
                let dst_stride = window.info().t_stride() as usize;
                let dst = window.slice_mut();

                copy_pixel_slice_to_bitmap(dst, dst_stride, &ps);
            }

            // Apply CMS transform if needed — mirror the single-frame path below.
            // Animated WebP/AVIF/JXL can carry a non-sRGB ICC profile or CICP;
            // without this every frame would be encoded in its source color space
            // while tagged StandardRGB. (GIF reaches here too but carries no
            // profile, so source_profile is Srgb and this is a no-op.)
            if !matches!(source_profile, SourceProfile::Srgb) {
                let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
                let mut window = bitmap.get_window_u8().unwrap();
                let result = cms::transform_to_srgb(&mut window, &source_profile);
                if let Err(e) = result
                    && !self.ignore_color_profile_errors
                {
                    return Err(e);
                }
            }

            // Determine if more frames remain
            if self.target_frame.is_some() {
                self.has_more = Some(false);
            } else {
                let frame_dec = self.frame_dec.as_mut().unwrap();
                let pulse = self.pulse.clone();
                match install_pooled(&self.thread_pool, move || {
                    frame_dec.render_next_frame_owned(pulse.as_ref().map(|p| p as &dyn Pulse))
                }) {
                    Ok(Some(next)) => {
                        self.peeked_frame = Some(next);
                        self.has_more = Some(true);
                    }
                    Ok(None) => {
                        self.has_more = Some(false);
                    }
                    Err(e) => {
                        self.peek_error = Some(decode_error_to_flow("frame peek", &*e));
                        self.has_more = Some(false);
                    }
                }
            }

            return Ok(bitmap_key);
        }

        // ── Single-frame path ──
        // Try push_decode with BitmapRowSink. If begin() rejects the format
        // (non-U8) or the codec does not support push_decode, fall back to
        // into_decoder which does format negotiation.
        let data = self.data_slice();
        let w = info.width;
        let h = info.height;

        let mut bitmaps = c.borrow_bitmaps_mut().map_err(|e| e.at(here!()))?;
        let bitmap_key = bitmaps
            .create_bitmap_u8(
                w,
                h,
                PixelLayout::BGRA,
                false,
                has_alpha,
                ColorSpace::StandardRGB,
                BitmapCompositing::ReplaceSelf,
            )
            .map_err(|e| e.at(here!()))?;

        // For formats that have a faster buffered decode (currently JPEG, with
        // rayon-parallel output above ~2 MPx), skip push_decode and use
        // into_decoder().decode() directly. push_decode still runs row-by-row
        // sequentially regardless of image size; for JPEG that costs ~2.5× at
        // 4096² and is only marginally cheaper at smaller sizes.
        let needs_buffered_fallback = if prefers_buffered_decode(self.format) {
            true
        } else {
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let mut window = bitmap.get_window_u8().unwrap();
            let dst_stride = window.info().t_stride() as usize;
            let dst = window.slice_mut();

            let ext = self.format.extension();
            let config = &self.config;
            let limits = self.resource_limits;
            let pulse = self.pulse.clone();
            let (push_result, sink_rejected_format) =
                install_pooled(&self.thread_pool, move || {
                    let mut sink = BitmapRowSink::new(dst, dst_stride);
                    let mut job = config.dyn_job();
                    configure_job(&mut *job, limits, pulse);
                    let res = job.push_decode(Cow::Borrowed(data), &mut sink, preferred);
                    (res, sink.rejected_format)
                });
            match push_result {
                Ok(_) => false, // sink.finish() was called by codec — swizzle done
                Err(e) if sink_rejected_format || e.unsupported_operation().is_some() => true,
                Err(e) => {
                    return Err(decode_error_to_flow(&format!("{ext} decode"), &*e));
                }
            }
        };

        if needs_buffered_fallback {
            // Format rejected by sink, push_decode unsupported, or buffered-decode-preferred —
            // use into_decoder. Build the job + decoder + run decode entirely INSIDE the pool:
            // the `!Send` DynDecodeJob/DynDecoder never cross the thread boundary (they're
            // local to the closure); only `config` (Send+Sync), `data`, `preferred` (Send)
            // are captured and the `DecodeOutput` (Send) is returned. Bounds the
            // rayon-parallel decode to max_threads.
            let ext = self.format.extension();
            let config = &self.config;
            let limits = self.resource_limits;
            let pulse = self.pulse.clone();
            let output = install_pooled(&self.thread_pool, move || {
                let mut job = config.dyn_job();
                configure_job(&mut *job, limits, pulse);
                job.into_decoder(Cow::Borrowed(data), preferred)?.decode()
            })
            .map_err(|e| decode_error_to_flow(&format!("{ext} decode"), &*e))?;
            let ps = output.pixels();

            // Verify decoded dimensions match what we allocated
            if ps.width() != w || ps.height() != h {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "{} decoded {}x{} but info reported {}x{}",
                    self.format.extension(),
                    ps.width(),
                    ps.height(),
                    w,
                    h
                ));
            }

            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let mut window = bitmap.get_window_u8().unwrap();
            let dst_stride = window.info().t_stride() as usize;
            let dst = window.slice_mut();
            copy_pixel_slice_to_bitmap(dst, dst_stride, &ps);
        }

        // Apply CMS transform if needed
        if !matches!(source_profile, SourceProfile::Srgb) {
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let mut window = bitmap.get_window_u8().unwrap();
            let result = cms::transform_to_srgb(&mut window, &source_profile);
            if let Err(e) = result
                && !self.ignore_color_profile_errors
            {
                return Err(e);
            }
        }

        self.has_more = Some(false);
        Ok(bitmap_key)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        if let Some(err) = self.peek_error.take() {
            return Err(err);
        }
        Ok(self.has_more.unwrap_or(true))
    }

    fn frame_delay_ms(&self) -> Option<u32> {
        self.last_frame_duration.map(frame_duration_to_ms)
    }

    fn frame_duration(&self) -> Option<zc::animation::FrameDuration> {
        self.last_frame_duration
    }

    fn loop_count(&self) -> Option<u32> {
        self.loop_count
    }

    fn estimate_decode_memory(&self, c: &Context, w: u32, h: u32) -> (u64, u64) {
        let mut env = zc::estimate::ComputeEnvironment::host();
        if let Some(n) = c.security.max_threads {
            env = env.with_cores(n as usize);
        }
        let est = self.estimate_decode(&env, w, h);
        let avg = est.peak_memory_bytes_est().unwrap_or(0);
        (avg, est.peak_memory_bytes_max().unwrap_or(avg))
    }
}

#[cfg(all(test, feature = "zen-codecs"))]
mod zen_redesign_tests {
    use super::*;
    use crate::codecs::NamedDecoders;

    const PNG_1X1_RGBA: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    fn make_2frame_gif(delay_cs: u16) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut enc = ::gif::Encoder::new(&mut buf, 2, 2, &[]).unwrap();
            enc.set_repeat(::gif::Repeat::Finite(3)).unwrap();
            for rgb in [[255u8, 0, 0, 255], [0, 255, 0, 255]] {
                let mut px = rgb.repeat(4);
                let mut frame = ::gif::Frame::from_rgba(2, 2, &mut px);
                frame.delay = delay_cs;
                enc.write_frame(&frame).unwrap();
            }
        }
        buf
    }

    #[test]
    fn zen_decoder_image_info_reports_static_vs_animated_and_lossless() {
        // Static PNG: multiple_frames = false, lossless = true
        let mut ctx = Context::create().unwrap();
        ctx.enabled_codecs.prefer_decoder(NamedDecoders::ZenPngDecoder);
        ctx.add_copied_input_buffer(0, PNG_1X1_RGBA).unwrap();
        let png_info = ctx.get_unscaled_unrotated_image_info(0).unwrap();
        assert!(!png_info.multiple_frames, "static PNG must report multiple_frames=false");
        assert!(png_info.lossless, "PNG must report lossless=true");

        // Animated 2-frame GIF via ZenGifDecoder: multiple_frames = true, lossless = true
        let mut ctx_gif = Context::create().unwrap();
        ctx_gif.enabled_codecs.prefer_decoder(NamedDecoders::ZenGifDecoder);
        ctx_gif.add_input_vector(0, make_2frame_gif(0)).unwrap();
        let gif_info = ctx_gif.get_unscaled_unrotated_image_info(0).unwrap();
        assert!(gif_info.multiple_frames, "2-frame GIF must report multiple_frames=true");
        assert!(gif_info.lossless, "GIF must report lossless=true");

        // Read first frame of 0cs-delay GIF: frame_delay_ms() is Some(0), NOT None!
        {
            let mut codec = ctx_gif.get_codec(0).unwrap();
            let dec = codec.get_decoder().unwrap();
            let _key = dec.read_frame(&ctx_gif).unwrap();
            assert_eq!(dec.frame_delay_ms(), Some(0));
            assert_eq!(dec.frame_duration(), Some(zc::animation::FrameDuration::from_millis(0)));
        }
    }

    #[test]
    fn native_jpeg_enforces_mem_budget_policy() {
        let mut ctx = Context::create().unwrap();
        // Use ImageRsPngDecoder so the budget gate triggers specifically on NativeJpeg encode.
        ctx.enabled_codecs.prefer_decoder(NamedDecoders::ImageRsPngDecoder);
        ctx.enabled_codecs.prefer_encoder(crate::codecs::NamedEncoders::ZenJpegEncoder);
        #[cfg(feature = "c-codecs")]
        ctx.enabled_codecs.disable_encoder(crate::codecs::NamedEncoders::MozJpegEncoder);
        ctx.add_copied_input_buffer(0, PNG_1X1_RGBA).unwrap();
        ctx.add_output_buffer(1).unwrap();

        let mut policy = s::MemBudgetPolicy::default();
        policy.require_est_bytes_below = Some(1); // 1 byte — must reject!
        let mut sec = s::ExecutionSecurity::unspecified();
        sec.mem_budget_policy = Some(Box::new(policy));

        let err = ctx
            .execute_1(s::Execute001 {
                graph_recording: None,
                security: Some(sec),
                job_options: None,
                framewise: s::Framewise::Steps(vec![
                    s::Node::Decode { io_id: 0, commands: None },
                    s::Node::Encode {
                        io_id: 1,
                        preset: s::EncoderPreset::Mozjpeg {
                            quality: Some(80),
                            progressive: Some(true),
                            matte: None,
                        },
                    },
                ]),
            })
            .unwrap_err();
        assert!(
            err.message.contains("encode memory budget"),
            "expected NativeJpeg encode memory budget rejection, got: {}",
            err.message
        );
    }
}
