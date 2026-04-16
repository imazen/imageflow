use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, Result};

use super::*;
use crate::codecs::cms;
use crate::codecs::source_profile::SourceProfile;
use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use crate::io::IoProxy;
use imageflow_types::PixelLayout;
use std::any::Any;
use std::borrow::Cow;

use zc::decode::{DecodeRowSink, DynAnimationFrameDecoder, DynDecoderConfig, SinkError};
use zc::ImageFormat as ZenFormat;
use zc::ImageInfo as ZenImageInfo;
use zc::OwnedAnimationFrame;

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
/// False for all formats: zenjpeg's push_decode routes through
/// `push_decoder_direct` → `decode_into()` with the fused streaming BGRA
/// path, writing directly into the bitmap sink with stride support
/// (811d824d fixed the bytes_per_pixel vs num_channels bug + added stride
/// scatter). No intermediate buffer, no extra copy.
/// Whether to skip push_decode and use the buffered Decode::decode path.
///
/// True for JPEG: push_decoder_direct has been fixed for baseline BGRA
/// (bytes_per_pixel, stride scatter, grayscale streaming, progressive
/// guard), but 4 edge cases remain (gray roundtrip sizing, CMS dual-
/// backend, ICC P3 roundtrip). The buffered path handles all cases.
///
/// When push_decode is enabled for JPEG, baseline 4096² decode reaches
/// parity with mozjpeg C (112ms vs 112ms) via zero-copy bitmap writes.
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
    // Animation metadata from last decoded frame
    last_delay_ms: u32,
    loop_count: Option<u32>,
    // Decoder options
    ignore_color_profile: bool,
    ignore_color_profile_errors: bool,
    // Resource limits for decode jobs
    resource_limits: Option<zc::ResourceLimits>,
    // Image format (provides extension, mime type, animation support)
    format: ZenFormat,
    // Whether has_more_frames is known
    has_more: Option<bool>,
    // Target frame index for SelectFrame command
    target_frame: Option<u32>,
}

impl ZenDecoder {
    fn new_zencodec(config: Box<dyn DynDecoderConfig>, io: IoProxy, format: ZenFormat) -> Self {
        ZenDecoder {
            config,
            io,
            data: None,
            cached_info: None,
            frame_dec: None,
            frame_index: 0,
            peeked_frame: None,
            last_delay_ms: 0,
            loop_count: None,
            ignore_color_profile: false,
            ignore_color_profile_errors: false,
            resource_limits: None,
            format,
            has_more: None,
            target_frame: None,
        }
    }

    pub fn create_jpeg(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        // CmykHandling::Passthrough — emit raw CMYK bytes (PixelDescriptor::CMYK8)
        // for 4-component JPEGs instead of applying zenjpeg's internal CMYK→RGB
        // matrix. The CMS stage then applies the source ICC profile (or the
        // bundled fallback CMYK profile) for an accurate conversion. No effect
        // on 3-component JPEGs. (Passthrough is the default in zenjpeg post-
        // ebb0e24f, but set explicitly so the intent survives any default flip.)
        let config =
            zenjpeg::JpegDecoderConfig::new().cmyk_handling(zenjpeg::CmykHandling::Passthrough);
        Ok(Self::new_zencodec(Box::new(config), io, ZenFormat::Jpeg))
    }

    pub fn create_webp(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenwebp::zencodec::WebpDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, ZenFormat::WebP))
    }

    pub fn create_png(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenpng::PngDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, ZenFormat::Png))
    }

    pub fn create_gif(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zengif::GifDecoderConfig::new();

        // Limits will be applied via DynDecodeJob::set_limits when decoding
        let mut decoder = Self::new_zencodec(Box::new(config), io, ZenFormat::Gif);

        // Store limits info for later use during decode
        let limit = c.security.max_decode_size.as_ref().or(c.security.max_frame_size.as_ref());
        if let Some(limit) = limit {
            let max_bytes = (limit.megapixels * 1_000_000.0 * 4.0) as u64;
            let mut limits = zc::ResourceLimits::default();
            limits.max_pixels = Some(max_bytes / 4);
            limits.max_memory_bytes = Some(max_bytes);
            decoder.resource_limits = Some(limits);
        }

        Ok(decoder)
    }

    pub fn create_avif(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenavif::AvifDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, ZenFormat::Avif))
    }

    pub fn create_jxl(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenjxl::JxlDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, ZenFormat::Jxl))
    }

    pub fn create_bmp(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenbitmaps::BmpDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, ZenFormat::Bmp))
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
            let info = job.probe(data).map_err(|e| {
                nerror!(
                    ErrorKind::ImageDecodingError,
                    "{} probe error: {}",
                    self.format.extension(),
                    e
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

    /// Create a DynDecodeJob with resource limits applied.
    fn make_job(&self) -> Box<dyn zc::decode::DynDecodeJob<'_> + '_> {
        let mut job = self.config.dyn_job();
        if let Some(ref limits) = self.resource_limits {
            job.set_limits(*limits);
        }
        job
    }
}

/// Convert one row of arbitrary U8 pixels into BGRA.
#[inline]
fn row_to_bgra(dst: &mut [u8], src: &[u8], width: usize, channels: usize) {
    for x in 0..width {
        let si = x * channels;
        let di = x * 4;
        if channels >= 4 {
            dst[di] = src[si + 2]; // B
            dst[di + 1] = src[si + 1]; // G
            dst[di + 2] = src[si]; // R
            dst[di + 3] = src[si + 3]; // A
        } else if channels >= 3 {
            dst[di] = src[si + 2]; // B
            dst[di + 1] = src[si + 1]; // G
            dst[di + 2] = src[si]; // R
            dst[di + 3] = 255;
        } else {
            let v = src[si];
            dst[di] = v;
            dst[di + 1] = v;
            dst[di + 2] = v;
            dst[di + 3] = 255;
        }
    }
}

/// Copy decoded pixels from a PixelSlice into a bitmap, handling stride and BGRA swizzle.
fn copy_pixel_slice_to_bitmap(dst: &mut [u8], dst_stride: usize, ps: &zenpixels::PixelSlice<'_>) {
    let w = ps.width();
    let h = ps.rows();
    let descriptor = ps.descriptor();

    let is_bgra = descriptor.layout() == zenpixels::ChannelLayout::Bgra
        && descriptor.channel_type() == zenpixels::ChannelType::U8;
    let is_rgba = descriptor.layout() == zenpixels::ChannelLayout::Rgba
        && descriptor.channel_type() == zenpixels::ChannelType::U8;
    // CMYK: pass through bytes verbatim into the BGRA bitmap slot — the
    // downstream CMS stage (SourceProfile::CmykIcc) interprets the CMYK bytes
    // and applies the ICC transform to BGRA. Swizzling here would corrupt
    // the C/M/Y/K channel order that CMS expects.
    let is_cmyk = descriptor.layout() == zenpixels::ChannelLayout::Cmyk
        && descriptor.channel_type() == zenpixels::ChannelType::U8;

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
        let channels = descriptor.channels();
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
    /// Metadata of the previous non-4bpp strip (for deferred conversion).
    prev_strip: Option<StripMeta>,
}

#[derive(Clone, Copy)]
struct StripMeta {
    y: u32,
    height: u32,
    width: u32,
    descriptor: zenpixels::PixelDescriptor,
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
            prev_strip: None,
        }
    }

    /// Convert a non-4bpp U8 strip from `self.temp` into the bitmap.
    fn convert_strip(&mut self, meta: StripMeta) {
        let bpp = meta.descriptor.bytes_per_pixel();
        let src_stride = meta.width as usize * bpp;
        let channels = meta.descriptor.channels();

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
        descriptor: zenpixels::PixelDescriptor,
    ) -> std::result::Result<(), SinkError> {
        if descriptor.channel_type() != zenpixels::ChannelType::U8 {
            return Err(format!("BitmapRowSink requires U8 channels, got {:?}", descriptor).into());
        }
        self.width = width;
        self.height = height;
        self.is_4bpp = descriptor.bytes_per_pixel() == 4;
        self.needs_swizzle = descriptor.layout() == zenpixels::ChannelLayout::Rgba;
        Ok(())
    }

    fn provide_next_buffer(
        &mut self,
        y: u32,
        height: u32,
        width: u32,
        descriptor: zenpixels::PixelDescriptor,
    ) -> std::result::Result<zenpixels::PixelSliceMut<'_>, SinkError> {
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
            PixelSliceMut::new(slice, width, height, self.stride, descriptor)
                .map_err(|e| -> SinkError { format!("{e}").into() })
        } else {
            // Non-4bpp U8: convert the previous strip before reusing temp buffer
            if let Some(meta) = self.prev_strip.take() {
                self.convert_strip(meta);
            }

            let bpp = descriptor.bytes_per_pixel();
            let src_stride = width as usize * bpp;
            let needed = height as usize * src_stride;
            self.temp.resize(needed, 0);

            self.prev_strip = Some(StripMeta { y, height, width, descriptor });

            PixelSliceMut::new(&mut self.temp, width, height, src_stride, descriptor)
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

use zenpixels::PixelSliceMut;

impl ZenDecoder {
    /// Get frame delay in centiseconds (GIF convention) from milliseconds.
    pub fn last_frame_delay(&self) -> Option<u16> {
        if self.last_delay_ms > 0 {
            Some((self.last_delay_ms / 10) as u16)
        } else {
            None
        }
    }

    pub fn get_loop_count(&self) -> Option<u32> {
        self.loop_count
    }
}

impl Decoder for ZenDecoder {
    fn initialize(&mut self, _c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, _c: &Context) -> Result<s::ImageInfo> {
        self.ensure_info_probed()?;
        let info = self.cached_info.as_ref().unwrap();
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
            lossless: false,
            multiple_frames: self.format.supports_animation(),
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
        if orient <= 1 {
            Ok(None)
        } else {
            Ok(Some(orient as i32))
        }
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
        self.ensure_data_available()?;
        self.ensure_info_probed()?;

        // ── Zencodec path (all formats via push_decode) ──
        let info = self.cached_info.as_ref().unwrap().clone();
        let source_profile = self.source_profile_from_info(&info);
        let has_alpha = info.has_alpha;
        // Request BGRA8_SRGB only — every zen codec supports it natively, so
        // the codec does the (possibly vectorized) swizzle internally and
        // writes straight into the imageflow BGRA bitmap. No RGBA/RGB/Gray
        // fallback path: if a codec ever drops BGRA support that's a bug
        // upstream, not something to silently paper over here.
        // (CMYK JPEGs with CmykHandling::Passthrough still emit CMYK8 and
        // bypass this negotiation — handled as a special case downstream.)
        let preferred = [zenpixels::PixelDescriptor::BGRA8_SRGB];

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
                let frame_dec = job
                    .into_animation_frame_decoder(Cow::Owned(data), &preferred)
                    .map_err(|e| {
                        nerror!(
                            ErrorKind::ImageDecodingError,
                            "{} frame decoder error: {}",
                            self.format.extension(),
                            e
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
                frame_dec.render_next_frame_owned(None).map_err(|e| {
                    nerror!(ErrorKind::ImageDecodingError, "frame decode error: {}", e)
                })?
            };

            let frame = frame
                .ok_or_else(|| nerror!(ErrorKind::InvalidOperation, "No more frames available"))?;

            self.last_delay_ms = frame.duration_ms();
            self.frame_index += 1;

            let ps = frame.pixels();
            let w = ps.width();
            let h = ps.rows();

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

            // Determine if more frames remain
            if self.target_frame.is_some() {
                self.has_more = Some(false);
            } else {
                let frame_dec = self.frame_dec.as_mut().unwrap();
                match frame_dec.render_next_frame_owned(None) {
                    Ok(Some(next)) => {
                        self.peeked_frame = Some(next);
                        self.has_more = Some(true);
                    }
                    Ok(None) => {
                        self.has_more = Some(false);
                    }
                    Err(_) => {
                        self.has_more = Some(false);
                    }
                }
            }

            return Ok(bitmap_key);
        }

        // ── Single-frame path ──
        // Try push_decode with BitmapRowSink. If begin() rejects the format
        // (non-U8), fall back to into_decoder which does format negotiation.
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
        let push_result: std::result::Result<_, zc::decode::SinkError> =
            if prefers_buffered_decode(self.format) {
                Err("skip push_decode; format prefers buffered decode".into())
            } else {
                let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
                let mut window = bitmap.get_window_u8().unwrap();
                let dst_stride = window.info().t_stride() as usize;
                let dst = window.slice_mut();

                let mut sink = BitmapRowSink::new(dst, dst_stride);
                let job = self.make_job();
                job.push_decode(Cow::Borrowed(data), &mut sink, &preferred)
                    .map_err(|e| -> zc::decode::SinkError { format!("{e}").into() })
            };

        match push_result {
            Ok(_) => { /* sink.finish() was called by codec — swizzle done */ }
            Err(_) => {
                // Format rejected, codec error, or buffered-decode-preferred — use into_decoder.
                let job = self.make_job();
                let dec = job.into_decoder(Cow::Borrowed(data), &preferred).map_err(|e| {
                    nerror!(
                        ErrorKind::ImageDecodingError,
                        "{} decode error: {}",
                        self.format.extension(),
                        e
                    )
                })?;
                let output = dec.decode().map_err(|e| {
                    nerror!(
                        ErrorKind::ImageDecodingError,
                        "{} decode error: {}",
                        self.format.extension(),
                        e
                    )
                })?;
                let ps = output.pixels();

                // Verify decoded dimensions match what we allocated
                if ps.width() != w || ps.rows() != h {
                    return Err(nerror!(
                        ErrorKind::ImageDecodingError,
                        "{} decoded {}x{} but info reported {}x{}",
                        self.format.extension(),
                        ps.width(),
                        ps.rows(),
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
        Ok(self.has_more.unwrap_or(true))
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}
