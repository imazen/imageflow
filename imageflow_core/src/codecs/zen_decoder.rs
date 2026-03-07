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

use zc::decode::{DecodeFrame, DynDecoderConfig, DynFrameDecoder};
use zc::ImageInfo as ZenImageInfo;

/// Format-specific metadata needed by the unified decoder.
struct FormatMeta {
    preferred_extension: &'static str,
    preferred_mime_type: &'static str,
    /// Whether this format supports EXIF orientation (JPEG, JXL).
    has_exif_orientation: bool,
    /// Whether this is a format where CMYK source data means we should skip CMS.
    /// (zenjpeg converts CMYK→RGB during decode, so ICC CMYK profile can't be applied after.)
    may_have_cmyk: bool,
    /// Whether this format may contain animation (GIF, WebP, JXL, APNG).
    may_have_animation: bool,
    /// Whether to always use the frame decoder path (even for single-frame files).
    /// True for GIF (probe can't detect animation status).
    /// False for WebP/JXL (probe accurately reports has_animation).
    always_use_frame_decoder: bool,
}

const JPEG_META: FormatMeta = FormatMeta {
    preferred_extension: "jpg",
    preferred_mime_type: "image/jpeg",
    has_exif_orientation: true,
    may_have_cmyk: true,
    may_have_animation: false,
    always_use_frame_decoder: false,
};

const WEBP_META: FormatMeta = FormatMeta {
    preferred_extension: "webp",
    preferred_mime_type: "image/webp",
    has_exif_orientation: false,
    may_have_cmyk: false,
    may_have_animation: true,
    always_use_frame_decoder: false,
};

const GIF_META: FormatMeta = FormatMeta {
    preferred_extension: "gif",
    preferred_mime_type: "image/gif",
    has_exif_orientation: false,
    may_have_cmyk: false,
    may_have_animation: true,
    // GIF probe doesn't report animation status; always use frame decoder
    always_use_frame_decoder: true,
};

const JXL_META: FormatMeta = FormatMeta {
    preferred_extension: "jxl",
    preferred_mime_type: "image/jxl",
    has_exif_orientation: true,
    may_have_cmyk: false,
    may_have_animation: true,
    always_use_frame_decoder: false,
};

const AVIF_META: FormatMeta = FormatMeta {
    preferred_extension: "avif",
    preferred_mime_type: "image/avif",
    has_exif_orientation: false,
    may_have_cmyk: false,
    may_have_animation: true,
    always_use_frame_decoder: false,
};

const HEIC_META: FormatMeta = FormatMeta {
    preferred_extension: "heic",
    preferred_mime_type: "image/heic",
    has_exif_orientation: false,
    may_have_cmyk: false,
    may_have_animation: false,
    always_use_frame_decoder: false,
};

/// Decoding strategy — native JPEG path for backward compat, zencodec for everything else.
enum DecodeMode {
    /// Zencodec dyn dispatch (WebP, GIF, JXL, etc.)
    Zencodec(Box<dyn DynDecoderConfig>),
    /// Native zenjpeg API (preserves exact output from old adapter)
    NativeJpeg,
}

/// Unified decoder for all zen codec formats.
///
/// Uses zencodec-types dyn dispatch to handle WebP, GIF, JXL (and
/// eventually AVIF, PNG) through a single adapter.
/// JPEG uses the native zenjpeg API for exact backward compatibility.
pub struct ZenDecoder {
    mode: DecodeMode,
    io: IoProxy,
    data: Option<Vec<u8>>,
    // Cached info from probe (zencodec path) or read_info (native JPEG path)
    cached_info: Option<ZenImageInfo>,
    cached_jpeg_info: Option<zenjpeg::decoder::JpegInfo>,
    // Persistent frame decoder for animation
    frame_dec: Option<Box<dyn DynFrameDecoder>>,
    frame_index: u32,
    // Peeked next frame (used for has_more_frames detection)
    peeked_frame: Option<DecodeFrame>,
    // Animation metadata from last decoded frame
    last_delay_ms: u32,
    loop_count: Option<u32>,
    // Decoder options
    ignore_color_profile: bool,
    ignore_color_profile_errors: bool,
    // Resource limits for decode jobs
    resource_limits: Option<zc::ResourceLimits>,
    // Format metadata
    meta: FormatMeta,
    // Whether has_more_frames is known
    has_more: Option<bool>,
    // Target frame index for SelectFrame command
    target_frame: Option<u32>,
}

impl ZenDecoder {
    fn new_zencodec(config: Box<dyn DynDecoderConfig>, io: IoProxy, meta: FormatMeta) -> Self {
        ZenDecoder {
            mode: DecodeMode::Zencodec(config),
            io,
            data: None,
            cached_info: None,
            cached_jpeg_info: None,
            frame_dec: None,
            frame_index: 0,
            peeked_frame: None,
            last_delay_ms: 0,
            loop_count: None,
            ignore_color_profile: false,
            ignore_color_profile_errors: false,
            resource_limits: None,
            meta,
            has_more: None,
            target_frame: None,
        }
    }

    pub fn create_jpeg(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        Ok(ZenDecoder {
            mode: DecodeMode::NativeJpeg,
            io,
            data: None,
            cached_info: None,
            cached_jpeg_info: None,
            frame_dec: None,
            frame_index: 0,
            peeked_frame: None,
            last_delay_ms: 0,
            loop_count: None,
            ignore_color_profile: false,
            ignore_color_profile_errors: false,
            resource_limits: None,
            meta: JPEG_META,
            has_more: None,
            target_frame: None,
        })
    }

    pub fn create_webp(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenwebp::WebpDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, WEBP_META))
    }

    pub fn create_gif(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zengif::GifDecoderConfig::new();

        // Limits will be applied via DynDecodeJob::set_limits when decoding
        let mut decoder = Self::new_zencodec(Box::new(config), io, GIF_META);

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

    pub fn create_jxl(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenjxl::JxlDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, JXL_META))
    }

    pub fn create_avif(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = zenavif::AvifDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, AVIF_META))
    }

    pub fn create_heic(_c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        let config = heic_decoder::HeicDecoderConfig::new();
        Ok(Self::new_zencodec(Box::new(config), io, HEIC_META))
    }

    fn ensure_data_buffered(&mut self) -> Result<()> {
        if self.data.is_none() {
            let mut bytes = Vec::with_capacity(8192);
            self.io.read_to_end(&mut bytes).map_err(FlowError::from_decoder)?;
            self.data = Some(bytes);
        }
        Ok(())
    }

    fn ensure_info_probed(&mut self) -> Result<()> {
        self.ensure_data_buffered()?;
        match &self.mode {
            DecodeMode::NativeJpeg => {
                if self.cached_jpeg_info.is_none() {
                    let data = self.data.as_ref().unwrap();
                    let decoder = zenjpeg::decoder::Decoder::new().apply_icc(false).preserve_all();
                    let info = decoder.read_info(data).map_err(|e| {
                        nerror!(ErrorKind::ImageDecodingError, "zenjpeg info error: {}", e)
                    })?;
                    self.cached_jpeg_info = Some(info);
                }
            }
            DecodeMode::Zencodec(_) => {
                if self.cached_info.is_none() {
                    let data = self.data.as_ref().unwrap();
                    let mut job = match &self.mode {
                        DecodeMode::Zencodec(config) => config.dyn_job(),
                        _ => unreachable!(),
                    };
                    if let Some(ref limits) = self.resource_limits {
                        job.set_limits(limits.clone());
                    }
                    let info = job.probe(data).map_err(|e| {
                        nerror!(
                            ErrorKind::ImageDecodingError,
                            "{} probe error: {}",
                            self.meta.preferred_extension,
                            e
                        )
                    })?;
                    self.cached_info = Some(info);
                }
            }
        }
        Ok(())
    }

    /// Extract SourceProfile from zencodec-types ImageInfo for CMS.
    fn source_profile_from_info(&self, info: &ZenImageInfo) -> SourceProfile {
        if self.ignore_color_profile {
            return SourceProfile::Srgb;
        }

        // CMYK JPEGs: zenjpeg converts to RGB during decode, so ICC CMYK profile
        // can't be applied after. Skip CMS.
        if self.meta.may_have_cmyk {
            // If channel_count is 4 and no alpha, it was CMYK
            if info.source_color.channel_count == Some(4) && !info.has_alpha {
                return SourceProfile::Srgb;
            }
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

    /// Extract SourceProfile from native JpegInfo for JPEG CMS.
    fn source_profile_from_jpeg_info(
        &self,
        jpeg_info: &zenjpeg::decoder::JpegInfo,
        extras: Option<&zenjpeg::decoder::DecodedExtras>,
    ) -> SourceProfile {
        if self.ignore_color_profile {
            return SourceProfile::Srgb;
        }

        // CMYK: zenjpeg already converted to RGB, skip CMS
        let is_cmyk = matches!(
            jpeg_info.color_space,
            zenjpeg::decoder::ColorSpace::Cmyk | zenjpeg::decoder::ColorSpace::Ycck
        );
        if is_cmyk {
            return SourceProfile::Srgb;
        }

        if let Some(extras) = extras {
            if let Some(icc_data) = extras.icc_profile() {
                return match jpeg_info.color_space {
                    zenjpeg::decoder::ColorSpace::Grayscale => {
                        SourceProfile::IccProfileGray(icc_data.to_vec())
                    }
                    _ => SourceProfile::IccProfile(icc_data.to_vec()),
                };
            }
        }

        SourceProfile::Srgb
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

    let row_bytes = w as usize * 4;

    if is_bgra {
        // Direct copy, row by row
        for y in 0..h {
            let src_row = ps.row(y);
            let dst_start = y as usize * dst_stride;
            dst[dst_start..dst_start + row_bytes].copy_from_slice(&src_row[..row_bytes]);
        }
    } else if is_rgba {
        // Copy row by row, then swizzle RGBA→BGRA in-place
        for y in 0..h {
            let src_row = ps.row(y);
            let dst_start = y as usize * dst_stride;
            dst[dst_start..dst_start + row_bytes].copy_from_slice(&src_row[..row_bytes]);
        }
        let _ = garb::bytes::rgba_to_bgra_inplace_strided(dst, w as usize, h as usize, dst_stride);
    } else {
        // Fallback: per-pixel conversion for other formats
        let channels = descriptor.channels() as usize;
        for y in 0..h {
            let src_row = ps.row(y);
            let dst_start = y as usize * dst_stride;
            for x in 0..w as usize {
                let si = x * channels;
                let di = dst_start + x * 4;
                if channels >= 4 {
                    dst[di] = src_row[si + 2]; // B
                    dst[di + 1] = src_row[si + 1]; // G
                    dst[di + 2] = src_row[si]; // R
                    dst[di + 3] = src_row[si + 3]; // A
                } else if channels == 3 {
                    dst[di] = src_row[si + 2]; // B
                    dst[di + 1] = src_row[si + 1]; // G
                    dst[di + 2] = src_row[si]; // R
                    dst[di + 3] = 255;
                } else {
                    let v = src_row[si];
                    dst[di] = v;
                    dst[di + 1] = v;
                    dst[di + 2] = v;
                    dst[di + 3] = 255;
                }
            }
        }
    }
}

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
        match &self.mode {
            DecodeMode::NativeJpeg => {
                let info = self.cached_jpeg_info.as_ref().unwrap();
                Ok(s::ImageInfo {
                    frame_decodes_into: s::PixelFormat::Bgra32,
                    image_width: info.dimensions.width as i32,
                    image_height: info.dimensions.height as i32,
                    preferred_mime_type: self.meta.preferred_mime_type.to_owned(),
                    preferred_extension: self.meta.preferred_extension.to_owned(),
                    lossless: false,
                    multiple_frames: false,
                })
            }
            DecodeMode::Zencodec(_) => {
                let info = self.cached_info.as_ref().unwrap();
                Ok(s::ImageInfo {
                    frame_decodes_into: if info.has_alpha {
                        s::PixelFormat::Bgra32
                    } else {
                        s::PixelFormat::Bgr32
                    },
                    image_width: info.width as i32,
                    image_height: info.height as i32,
                    preferred_mime_type: self.meta.preferred_mime_type.to_owned(),
                    preferred_extension: self.meta.preferred_extension.to_owned(),
                    lossless: false, // conservative default
                    // Use may_have_animation from format metadata since probe()
                    // doesn't scan frames. The actual frame count is determined
                    // during decode via the frame decoder.
                    multiple_frames: self.meta.may_have_animation,
                })
            }
        }
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_exif_rotation_flag(&mut self, _c: &Context) -> Result<Option<i32>> {
        if !self.meta.has_exif_orientation {
            return Ok(None);
        }
        self.ensure_info_probed()?;
        match &self.mode {
            DecodeMode::NativeJpeg => {
                let info = self.cached_jpeg_info.as_ref().unwrap();
                if let Some(ref exif_data) = info.exif {
                    if let Some(orientation) = zenjpeg::lossless::parse_exif_orientation(exif_data)
                    {
                        return Ok(Some(orientation as i32));
                    }
                }
                Ok(None)
            }
            DecodeMode::Zencodec(_) => {
                let info = self.cached_info.as_ref().unwrap();
                let orient = info.orientation as u8;
                if orient <= 1 {
                    Ok(None)
                } else {
                    Ok(Some(orient as i32))
                }
            }
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
        self.ensure_data_buffered()?;
        self.ensure_info_probed()?;

        // ── Native JPEG path ──
        if matches!(self.mode, DecodeMode::NativeJpeg) {
            let data = self.data.as_ref().unwrap();

            let decoder = zenjpeg::decoder::Decoder::new()
                .output_format(zenjpeg::decoder::PixelFormat::Bgra)
                .apply_icc(false)
                .auto_orient(false)
                .preserve_all();

            let result = decoder.decode(data, c.stop()).map_err(|e| {
                nerror!(ErrorKind::ImageDecodingError, "zenjpeg decode error: {}", e)
            })?;

            let w = result.width();
            let h = result.height();
            let pixels = result.pixels_u8().ok_or_else(|| {
                nerror!(ErrorKind::ImageDecodingError, "zenjpeg returned no u8 pixels")
            })?;

            let source_profile = self.source_profile_from_jpeg_info(
                self.cached_jpeg_info.as_ref().unwrap(),
                result.extras(),
            );

            let mut bitmaps = c.borrow_bitmaps_mut().map_err(|e| e.at(here!()))?;
            let bitmap_key = bitmaps
                .create_bitmap_u8(
                    w,
                    h,
                    PixelLayout::BGRA,
                    false,
                    false,
                    ColorSpace::StandardRGB,
                    BitmapCompositing::ReplaceSelf,
                )
                .map_err(|e| e.at(here!()))?;

            {
                let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
                let mut window = bitmap.get_window_u8().unwrap();
                let dst_stride = window.info().t_stride() as usize;
                let src_stride = w as usize * 4;
                let dst = window.slice_mut();

                if dst_stride == src_stride {
                    dst[..pixels.len()].copy_from_slice(pixels);
                } else {
                    for y in 0..h as usize {
                        let src_row = &pixels[y * src_stride..(y + 1) * src_stride];
                        let dst_row = &mut dst[y * dst_stride..y * dst_stride + src_stride];
                        dst_row.copy_from_slice(src_row);
                    }
                }
            }

            // Apply CMS transform if needed
            if !matches!(source_profile, SourceProfile::Srgb) {
                let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
                let mut window = bitmap.get_window_u8().unwrap();
                let result = cms::transform_to_srgb(&mut window, &source_profile);
                if let Err(e) = result {
                    if !self.ignore_color_profile_errors {
                        return Err(e);
                    }
                }
            }

            self.has_more = Some(false);
            return Ok(bitmap_key);
        }

        // ── Zencodec path (WebP, GIF, JXL, etc.) ──
        let info = self.cached_info.as_ref().unwrap().clone();
        let source_profile = self.source_profile_from_info(&info);
        let has_alpha = info.has_alpha;
        let preferred = [zenpixels::PixelDescriptor::BGRA8_SRGB];

        // Animation path: use persistent frame decoder
        // - always_use_frame_decoder: GIF (probe can't detect animation)
        // - info.has_animation: WebP/JXL (probe reports animation accurately)
        // - frame_dec.is_some(): already in animation mode from prior call
        if self.meta.always_use_frame_decoder || info.has_animation || self.frame_dec.is_some() {
            if self.frame_dec.is_none() {
                // Create frame decoder on first call
                let data = self.data.take().unwrap();
                let config = match &self.mode {
                    DecodeMode::Zencodec(config) => config,
                    _ => unreachable!(),
                };
                let mut job = config.dyn_job();
                if let Some(ref limits) = self.resource_limits {
                    job.set_limits(limits.clone());
                }
                let frame_dec =
                    job.into_frame_decoder(Cow::Owned(data), &preferred).map_err(|e| {
                        nerror!(
                            ErrorKind::ImageDecodingError,
                            "{} frame decoder error: {}",
                            self.meta.preferred_extension,
                            e
                        )
                    })?;
                self.loop_count = frame_dec.loop_count();
                self.frame_dec = Some(frame_dec);
            }

            let frame_dec = self.frame_dec.as_mut().unwrap();

            // Helper: get next frame from peeked buffer or from decoder
            let get_next = |peeked: &mut Option<DecodeFrame>,
                            dec: &mut Box<dyn DynFrameDecoder>|
             -> Result<Option<DecodeFrame>> {
                if let Some(f) = peeked.take() {
                    Ok(Some(f))
                } else {
                    dec.next_frame().map_err(|e| {
                        nerror!(ErrorKind::ImageDecodingError, "frame decode error: {}", e)
                    })
                }
            };

            // Skip frames to reach target
            if let Some(target) = self.target_frame {
                while self.frame_index < target {
                    let skipped = get_next(&mut self.peeked_frame, frame_dec)?;
                    if skipped.is_none() {
                        return Err(nerror!(
                            ErrorKind::InvalidArgument,
                            "frame={} requested but {} only has {} frames",
                            target,
                            self.meta.preferred_extension,
                            self.frame_index
                        ));
                    }
                    self.frame_index += 1;
                }
            }

            let frame = get_next(&mut self.peeked_frame, frame_dec)?;

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
                    true,
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
                // SelectFrame → only one frame output
                self.has_more = Some(false);
            } else {
                // Peek at the next frame to know if there are more
                let frame_dec = self.frame_dec.as_mut().unwrap();
                match frame_dec.next_frame() {
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

        // Single-frame path: use push_decode for zero-copy into bitmap
        let data = self.data.as_ref().unwrap();
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

        {
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let mut window = bitmap.get_window_u8().unwrap();
            let dst_stride = window.info().t_stride() as usize;

            // Decode via DynDecoder, then copy into bitmap
            let config = match &self.mode {
                DecodeMode::Zencodec(config) => config,
                _ => unreachable!(),
            };
            let mut job = config.dyn_job();
            if let Some(ref limits) = self.resource_limits {
                job.set_limits(limits.clone());
            }
            let decoder = job.into_decoder(Cow::Borrowed(data), &preferred).map_err(|e| {
                nerror!(
                    ErrorKind::ImageDecodingError,
                    "{} decoder error: {}",
                    self.meta.preferred_extension,
                    e
                )
            })?;

            let output = decoder.decode().map_err(|e| {
                nerror!(
                    ErrorKind::ImageDecodingError,
                    "{} decode error: {}",
                    self.meta.preferred_extension,
                    e
                )
            })?;

            let ps = output.pixels();
            let dst = window.slice_mut();

            copy_pixel_slice_to_bitmap(dst, dst_stride, &ps);
        }

        // Apply CMS transform if needed
        if !matches!(source_profile, SourceProfile::Srgb) {
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let mut window = bitmap.get_window_u8().unwrap();
            let result = cms::transform_to_srgb(&mut window, &source_profile);
            if let Err(e) = result {
                if !self.ignore_color_profile_errors {
                    return Err(e);
                }
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
