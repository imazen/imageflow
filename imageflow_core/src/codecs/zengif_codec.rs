use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, Result};

use super::*;
use crate::graphics::bitmaps::{BitmapCompositing, ColorSpace};
use crate::io::IoProxy;
use crate::io::IoProxyProxy;
use imageflow_types::PixelLayout;
use std::any::Any;
use std::rc::Rc;

// ============================================================================
// Decoder
// ============================================================================

pub struct ZenGifDecoder {
    decoder: Option<zengif::Decoder<'static, IoProxy>>,
    /// Stashed last frame for encoder to reference (delay, etc.)
    last_composed: Option<zengif::ComposedFrame>,
    /// Peeked next frame (used for has_more_frames)
    peeked_frame: Option<zengif::ComposedFrame>,
    metadata: Option<zengif::Metadata>,
    target_frame: Option<i32>,
    current_frame: i32,
    /// None = unknown, Some(true/false) = known
    has_more: Option<bool>,
    // Leaked Unstoppable to satisfy 'static lifetime on Decoder
    _stop: &'static zengif::Unstoppable,
}

impl ZenGifDecoder {
    pub fn create(c: &Context, io: IoProxy, _io_id: i32) -> Result<Self> {
        // Compute memory limit from security settings
        let limit = c.security.max_decode_size.as_ref().or(c.security.max_frame_size.as_ref());
        let max_bytes = if let Some(limit) = limit {
            // 4 bytes per pixel (RGBA) * megapixels * 1M
            (limit.megapixels * 1_000_000.0 * 4.0) as u64
        } else {
            // Default: 256 MB
            256 * 1024 * 1024
        };

        let limits =
            zengif::Limits::default().max_memory(max_bytes).max_total_pixels(max_bytes / 4);

        // Leak the Unstoppable to get a 'static reference.
        // This is fine — Unstoppable is a ZST.
        let stop: &'static zengif::Unstoppable = Box::leak(Box::new(zengif::Unstoppable));

        let decoder = zengif::Decoder::new(io, limits, stop)
            .map_err(|e| nerror!(ErrorKind::GifDecodingError, "zengif init error: {}", e))?;

        // Validate dimensions against security limits
        let meta = decoder.metadata().clone();
        if let Some(limit) = limit {
            let w = meta.width as i32;
            let h = meta.height as i32;
            if w > limit.w as i32 {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "GIF width {} exceeds max_decode_size.w {}",
                    w,
                    limit.w
                ));
            }
            if h > limit.h as i32 {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "GIF height {} exceeds max_decode_size.h {}",
                    h,
                    limit.h
                ));
            }
            let megapixels = w as f32 * h as f32 / 1_000_000f32;
            if megapixels > limit.megapixels {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "GIF megapixels {:.2} exceeds max_decode_size.megapixels {}",
                    megapixels,
                    limit.megapixels
                ));
            }
        }

        Ok(ZenGifDecoder {
            decoder: Some(decoder),
            last_composed: None,
            peeked_frame: None,
            metadata: Some(meta),
            target_frame: None,
            current_frame: 0,
            has_more: None,
            _stop: stop,
        })
    }

    pub fn get_repeat(&self) -> Option<zengif::Repeat> {
        self.metadata.as_ref().map(|m| m.repeat)
    }

    pub fn last_frame_delay(&self) -> Option<u16> {
        self.last_composed.as_ref().map(|f| f.delay)
    }
}

impl Decoder for ZenGifDecoder {
    fn initialize(&mut self, _c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_unscaled_image_info(&mut self, _c: &Context) -> Result<s::ImageInfo> {
        let meta = self.metadata.as_ref().unwrap();
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: meta.width as i32,
            image_height: meta.height as i32,
            preferred_mime_type: "image/gif".to_owned(),
            preferred_extension: "gif".to_owned(),
            lossless: false,
            multiple_frames: meta.frame_count > 1,
        })
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }

    fn get_exif_rotation_flag(&mut self, _c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, _c: &Context, tell: s::DecoderCommand) -> Result<()> {
        if let s::DecoderCommand::SelectFrame(frame) = tell {
            self.target_frame = Some(frame);
        }
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        let decoder = self.decoder.as_mut().ok_or_else(|| {
            nerror!(ErrorKind::InvalidOperation, "ZenGifDecoder already consumed")
        })?;

        // Helper to get next frame (from peeked or from decoder)
        let get_next = |peeked: &mut Option<zengif::ComposedFrame>,
                        dec: &mut zengif::Decoder<'static, IoProxy>|
         -> Result<Option<zengif::ComposedFrame>> {
            if let Some(f) = peeked.take() {
                Ok(Some(f))
            } else {
                dec.next_frame()
                    .map_err(|e| nerror!(ErrorKind::GifDecodingError, "zengif decode error: {}", e))
            }
        };

        // Skip frames to reach target
        if let Some(target) = self.target_frame {
            while self.current_frame < target {
                let frame = get_next(&mut self.peeked_frame, decoder)?;
                if frame.is_none() {
                    return Err(nerror!(
                        ErrorKind::InvalidArgument,
                        "frame={} requested but GIF only has {} frames",
                        target,
                        self.current_frame
                    ));
                }
                self.current_frame += 1;
            }
        }

        // Read the actual frame
        let frame = get_next(&mut self.peeked_frame, decoder)?;

        let frame = frame
            .ok_or_else(|| nerror!(ErrorKind::InvalidOperation, "No more frames available"))?;

        let w = frame.width as u32;
        let h = frame.height as u32;
        let pixels = &frame.pixels;

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
            let src_stride = w as usize * 4;

            let dst = window.slice_mut();

            // Convert RGBA → BGRA (swap R and B channels)
            for y in 0..h as usize {
                let dst_row_start = y * dst_stride;
                let src_row_start = y * w as usize;
                for x in 0..w as usize {
                    let pix = &pixels[src_row_start + x];
                    let di = dst_row_start + x * 4;
                    dst[di] = pix.b; // B
                    dst[di + 1] = pix.g; // G
                    dst[di + 2] = pix.r; // R
                    dst[di + 3] = pix.a; // A
                }
            }
        }

        self.current_frame += 1;
        self.last_composed = Some(frame);

        // Determine if there are more frames
        if self.target_frame.is_some() {
            self.has_more = Some(false);
        } else {
            // Peek at the next frame to know if there are more
            if let Some(decoder) = self.decoder.as_mut() {
                match decoder.next_frame() {
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
        }

        Ok(bitmap_key)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(self.has_more.unwrap_or(true))
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

// ============================================================================
// Encoder
// ============================================================================

pub struct ZenGifEncoder {
    io_id: i32,
    io_ref: Rc<RefCell<IoProxy>>,
    frames: Vec<zengif::FrameInput>,
    width: u16,
    height: u16,
    repeat: zengif::Repeat,
}

impl ZenGifEncoder {
    pub(crate) fn create(c: &Context, io: IoProxy, first_frame_key: BitmapKey) -> Result<Self> {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
        let bitmap = bitmaps.try_borrow_mut(first_frame_key).map_err(|e| e.at(here!()))?;

        if !c.enabled_codecs.encoders.contains(&NamedEncoders::ZenGifEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The zengif encoder has been disabled"
            ));
        }

        let io_id = io.io_id();
        let io_ref = Rc::new(RefCell::new(io));

        Ok(ZenGifEncoder {
            io_id,
            io_ref,
            frames: Vec::new(),
            width: bitmap.w() as u16,
            height: bitmap.h() as u16,
            repeat: zengif::Repeat::Infinite,
        })
    }
}

impl Encoder for ZenGifEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &s::EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult> {
        // Check if the source decoder is a ZenGifDecoder to get frame metadata
        let mut delay = 10u16; // default 100ms
        for io_id in decoder_io_ids {
            let mut codec = c.get_codec(*io_id).map_err(|e| e.at(here!()))?;
            let decoder = codec.get_decoder().map_err(|e| e.at(here!()))?;
            if let Some(d) = decoder.as_any().downcast_ref::<ZenGifDecoder>() {
                if let Some(r) = d.get_repeat() {
                    self.repeat = r;
                }
                if let Some(d) = d.last_frame_delay() {
                    delay = d;
                }
                break;
            }
            // Also check old GifDecoder for compatibility
            if let Some(d) = decoder.as_any().downcast_ref::<gif::GifDecoder>() {
                if let Some(r) = d.get_repeat() {
                    self.repeat = zengif::Repeat::from(r);
                }
                if let Some(f) = d.current_frame() {
                    delay = f.delay;
                }
                break;
            }
        }

        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
        let window = bitmap.get_window_u8().unwrap();
        let (w, h) = window.size_16()?;
        let stride = window.t_stride();
        let fmt = window.pixel_format();

        let slice = window.get_slice();

        // Convert from BGRA/BGR to RGBA for zengif
        let n = w as usize * h as usize;
        let mut rgba_pixels = Vec::with_capacity(n);

        match fmt {
            crate::ffi::PixelFormat::Bgra32 => {
                for y in 0..h as usize {
                    let row = &slice[y * stride..y * stride + w as usize * 4];
                    for pix in row.chunks_exact(4) {
                        rgba_pixels.push(zengif::Rgba {
                            r: pix[2],
                            g: pix[1],
                            b: pix[0],
                            a: pix[3],
                        });
                    }
                }
            }
            crate::ffi::PixelFormat::Bgr32 => {
                for y in 0..h as usize {
                    let row = &slice[y * stride..y * stride + w as usize * 4];
                    for pix in row.chunks_exact(4) {
                        rgba_pixels.push(zengif::Rgba { r: pix[2], g: pix[1], b: pix[0], a: 255 });
                    }
                }
            }
            crate::ffi::PixelFormat::Bgr24 => {
                for y in 0..h as usize {
                    let row = &slice[y * stride..y * stride + w as usize * 3];
                    for pix in row.chunks_exact(3) {
                        rgba_pixels.push(zengif::Rgba { r: pix[2], g: pix[1], b: pix[0], a: 255 });
                    }
                }
            }
            other => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "PixelFormat {:?} not supported for gif encoding",
                    other
                ));
            }
        }

        self.frames.push(zengif::FrameInput::new(w, h, delay, rgba_pixels));

        Ok(s::EncodeResult {
            w: w as i32,
            h: h as i32,
            io_id: self.io_id,
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "gif".to_owned(),
            preferred_mime_type: "image/gif".to_owned(),
        })
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        // Encode all accumulated frames
        let config =
            zengif::EncoderConfig::new().repeat(self.repeat).quantizer(zengif::Quantizer::auto());

        let output = zengif::encode_gif(
            self.frames,
            self.width,
            self.height,
            config,
            zengif::Limits::default(),
            &zengif::Unstoppable,
        )
        .map_err(|e| nerror!(ErrorKind::ImageEncodingError, "zengif encode error: {}", e))?;

        IoProxyProxy(self.io_ref.clone())
            .write_all(&output)
            .map_err(|e| FlowError::from_encoder(e).at(here!()))?;

        match std::rc::Rc::try_unwrap(self.io_ref) {
            Ok(cell) => Ok(cell.into_inner()),
            Err(_) => Err(nerror!(
                ErrorKind::InternalError,
                "ZenGifEncoder IoProxy has multiple references"
            )),
        }
    }
}
