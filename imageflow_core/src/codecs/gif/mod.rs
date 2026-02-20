use super::*;
use crate::ffi;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::io::IoProxy;
use crate::{Context, JsonResponse, Result};
use imageflow_types::collections::AddRemoveSet;
use imageflow_types::{IoDirection, PixelLayout};
use std::any::Any;
use uuid::Uuid;
mod bgra;
mod disposal;
mod screen;
mod subimage;
use self::bgra::BGRA8;
use self::screen::Screen;
use crate::gif::Frame;
use crate::graphics::bitmaps::{BitmapCompositing, BitmapKey, ColorSpace};
use crate::io::IoProxyProxy;
use crate::io::IoProxyRef;
use lcms2_sys::cmsAllocProfileSequenceDescription;
use std::rc::Rc;

pub struct GifDecoder {
    reader: ::gif::Decoder<IoProxy>,
    screen: Screen,
    buffer: Option<Vec<u8>>,
    last_frame: Option<Frame<'static>>,
    next_frame: Option<Frame<'static>>,
}

impl GifDecoder {
    fn is_animated(&self) -> Option<bool> {
        match (self.last_frame.is_some(), self.next_frame.is_some()) {
            (true, true) => Some(true),
            (false, false) => None, // No frames read yet
            (true, false) => Some(false),
            (false, true) => {
                panic!("GifDecoder::is_animated called during the middle of read_frame")
            }
        }
    }

    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<GifDecoder> {
        let mut options = ::gif::Decoder::<IoProxy>::build();
        options.allow_unknown_blocks(true);
        options.set_memory_limit(::gif::MemoryLimit::Bytes(
            std::num::NonZeroU64::new(8000 * 8000).unwrap(),
        ));
        options.set_color_output(::gif::ColorOutput::Indexed); // Important

        let reader = options.read_info(io).map_err(|e| FlowError::from(e).at(here!()))?;

        // Validate dimensions BEFORE allocating Screen buffer to prevent excessive memory use
        let w = reader.width() as i32;
        let h = reader.height() as i32;
        let limit = c.security.max_decode_size.as_ref().or(c.security.max_frame_size.as_ref());
        if let Some(limit) = limit {
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

        let screen = Screen::new(&reader);

        Ok(GifDecoder { reader, screen, buffer: None, last_frame: None, next_frame: None })
    }

    fn read_next_frame_info(&mut self) -> Result<()> {
        self.last_frame = self.next_frame.take();
        // Currently clones local palette
        self.next_frame =
            self.reader.next_frame_info().map_err(|e| FlowError::from(e).at(here!()))?.cloned();
        Ok(())
    }

    fn create_bitmap_from_screen(&self, c: &Context) -> Result<BitmapKey> {
        // Create output bitmap and copy to it

        let w = self.screen.width;
        let h = self.screen.height;

        let mut bitmaps = c.borrow_bitmaps_mut().map_err(|e| e.at(here!()))?;

        let bitmap_key = bitmaps
            .create_bitmap_u8(
                w as u32,
                h as u32,
                PixelLayout::BGRA,
                false,
                true,
                ColorSpace::StandardRGB,
                BitmapCompositing::ReplaceSelf,
            )
            .map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        let mut window = bitmap.get_window_u8().unwrap();

        for mut line in window.scanlines_bgra().unwrap() {
            let y = line.y();
            line.row_mut().copy_from_slice(&self.screen.pixels[y * w..(y + 1) * w]);
        }
        Ok(bitmap_key)
    }
    pub fn current_frame(&self) -> Option<&Frame<'static>> {
        self.last_frame.as_ref()
    }

    pub fn get_repeat(&self) -> Option<::gif::Repeat> {
        // TODO: Fix hack - gif crate doesn't allow reading this
        Some(::gif::Repeat::Infinite)
    }
}

impl Decoder for GifDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        self.get_unscaled_image_info(c)
    }
    fn get_unscaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: i32::from(self.reader.width()),
            image_height: i32::from(self.reader.height()),
            //            current_frame_index: 0,
            //            frame_count: 1,
            // We would have to read in the entire GIF to know!
            preferred_mime_type: "image/gif".to_owned(),
            preferred_extension: "gif".to_owned(),
            lossless: false,
            multiple_frames: self.is_animated().unwrap_or(false), //false if we didn't read any frames yet
        })
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey> {
        // Ensure next_frame is present (only called for first frame)
        if self.next_frame.is_none() {
            self.read_next_frame_info().map_err(|e| e.at(here!()))?;
        }

        {
            // Grab a reference
            let frame = self.next_frame.as_ref().ok_or_else(|| {
                nerror!(
                    ErrorKind::InvalidOperation,
                    "read_frame was called without a frame available"
                )
            })?;

            //Prepare our reusable buffer
            let buf_size = self.reader.width() as usize * self.reader.height() as usize;

            let buf_mut = self.buffer.get_or_insert_with(|| vec![0; buf_size]);
            let slice = &mut buf_mut[..self.reader.buffer_size()];

            slice.fill(0);
            //Read into that buffer
            //Read into that buffer
            self.reader.read_into_buffer(slice).map_err(|e| FlowError::from(e).at(here!()))?;

            // Render / apply disposal
            //TODO: allocs: Disposal currently allocates a new copy every blit (for previous frame)
            self.screen
                .blit(frame, slice)
                .map_err(|e| nerror!(ErrorKind::GifDecodingError, "{:?}", e))?; //Missing palette?
        }
        // Try to read the next frame;
        self.read_next_frame_info().map_err(|e| e.at(here!()))?;

        self.create_bitmap_from_screen(c)
    }
    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(self.next_frame.is_some())
    }
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

pub trait EasyEncoder {
    fn write_frame(
        &mut self,
        w: &mut dyn Write,
        c: &Context,
        bitmap_key: BitmapKey,
    ) -> Result<s::EncodeResult>;
}

pub struct EncoderAdapter<T>
where
    T: EasyEncoder,
{
    io_id: i32,
    encoder: T,
    io_ref: Rc<RefCell<IoProxy>>,
}
impl<T> EncoderAdapter<T>
where
    T: EasyEncoder,
{
    pub(crate) fn create(io: IoProxy, encoder: T) -> EncoderAdapter<T> {
        let io_id = io.io_id();
        let io_ref = Rc::new(RefCell::new(io));

        EncoderAdapter { io_id, io_ref: io_ref.clone(), encoder }
    }

    fn get_io_ref(&self) -> Rc<RefCell<IoProxy>> {
        self.io_ref.clone()
    }
}

impl<T> Encoder for EncoderAdapter<T>
where
    T: EasyEncoder,
{
    fn write_frame(
        &mut self,
        c: &Context,
        preset: &s::EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult> {
        let io_proxy = IoProxyProxy(self.io_ref.clone());

        self.encoder
            .write_frame(&mut IoProxyProxy(self.io_ref.clone()), c, bitmap_key)
            .map_err(|e| e.at(here!()))
            .and_then(|mut r| {
                r.io_id = self.io_id;
                match r.bytes {
                    s::ResultBytes::ByteArray(vec) => {
                        IoProxyProxy(self.io_ref.clone())
                            .write_all(&vec)
                            .map_err(|e| FlowError::from_encoder(e).at(here!()))?;
                        r.bytes = s::ResultBytes::Elsewhere;
                        Ok(r)
                    }
                    _ => Ok(r),
                }
            })
    }

    fn get_io(&self) -> Result<IoProxyRef<'_>> {
        Ok(IoProxyRef::Ref(self.io_ref.borrow()))
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        match std::rc::Rc::try_unwrap(self.io_ref) {
            Ok(cell) => Ok(cell.into_inner()),
            Err(_) => Err(nerror!(
                ErrorKind::InternalError,
                "EncoderAdapter IoProxy has multiple references"
            )),
        }
    }
}
pub struct GifEncoder {
    io_id: i32,
    encoder: Option<::gif::Encoder<IoProxyProxy>>,
    io_ref: Rc<RefCell<IoProxy>>,
    frame_ix: i32,
}

impl GifEncoder {
    pub(crate) fn create(
        c: &Context,
        io: IoProxy,
        first_frame_key: BitmapKey,
    ) -> Result<GifEncoder> {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;

        let bitmap = bitmaps.try_borrow_mut(first_frame_key).map_err(|e| e.at(here!()))?;

        if !c.enabled_codecs.encoders.contains(&NamedEncoders::GifEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The gif encoder has been disabled"
            ));
        }
        let io_id = io.io_id();
        let io_ref = Rc::new(RefCell::new(io));

        Ok(GifEncoder {
            io_id,
            io_ref: io_ref.clone(),
            // Global color table??
            encoder: Some(
                ::gif::Encoder::new(
                    IoProxyProxy(io_ref),
                    bitmap.w() as u16,
                    bitmap.h() as u16,
                    &[],
                )
                .map_err(|e| FlowError::from(e).at(here!()))?,
            ),
            frame_ix: 0,
        })
    }
}

impl Encoder for GifEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        preset: &s::EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult> {
        let mut decoded_frame = None;
        let mut repeat = None;
        for io_id in decoder_io_ids {
            let mut codec = c.get_codec(*io_id).map_err(|e| e.at(here!()))?;
            let gif_decoder = codec
                .get_decoder()
                .map_err(|e| e.at(here!()))?
                .as_any()
                .downcast_ref::<GifDecoder>();

            if let Some(d) = gif_decoder {
                repeat = d.get_repeat();
                decoded_frame = d.last_frame.clone(); //TODO: clones local palette; expensive, not used
                break;
            }
        }

        //        eprintln!("decoders: {:?}, found_frame: {}", decoder_io_ids, decoded_frame.is_some() );

        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        let window = bitmap.get_window_u8().unwrap();
        let (w, h) = window.size_16()?;
        let stride = window.t_stride();
        let fmt = window.pixel_format();

        // We gotta copy to mutate
        let mut pixels = Vec::new();
        pixels.extend_from_slice(window.get_slice());

        let mut f = match fmt {
            crate::ffi::PixelFormat::Bgr24 => Ok(from_bgr_with_stride(w, h, &pixels, stride)),
            crate::ffi::PixelFormat::Bgra32 => Ok(from_bgra_with_stride(w, h, &mut pixels, stride)),
            crate::ffi::PixelFormat::Bgr32 => Ok(from_bgrx_with_stride(w, h, &mut pixels, stride)),
            other => Err(nerror!(
                ErrorKind::InvalidArgument,
                "PixelFormat {:?} not supported for gif encoding",
                fmt
            )),
        }?;

        if let Some(from) = decoded_frame {
            f.delay = from.delay;
            f.needs_user_input = from.needs_user_input;
        }
        if self.frame_ix == 0 {
            // Only write before any frames
            if let Some(r) = repeat {
                //                    eprintln!("Writing repeat");
                self.encoder
                    .as_mut()
                    .ok_or_else(|| {
                        nerror!(ErrorKind::InternalError, "Gif encoder not initialized")
                    })?
                    .write_extension(::gif::ExtensionData::Repetitions(r))
                    .map_err(|e| FlowError::from(e).at(here!()))?;
            } else {
                //                    eprintln!("Skipping repeat");
            }
        }

        // TODO: Overhaul encoding
        // delay
        // dispose method
        // rect
        // transparency??

        self.encoder
            .as_mut()
            .ok_or_else(|| nerror!(ErrorKind::InternalError, "Gif encoder not initialized"))?
            .write_frame(&f)
            .map_err(|e| FlowError::from(e).at(here!()))?;

        self.frame_ix += 1;
        Ok(s::EncodeResult {
            w: w as i32,
            h: h as i32,
            io_id: self.io_id,
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "gif".to_owned(),
            preferred_mime_type: "image/gif".to_owned(),
        })
    }
    fn get_io(&self) -> Result<IoProxyRef<'_>> {
        Ok(IoProxyRef::Ref(self.io_ref.borrow()))
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        // Consume the encoder to write the GIF trailer before reclaiming the IoProxy.
        // If the encoder was already consumed (shouldn't happen), skip this step.
        if let Some(encoder) = self.encoder {
            let _flushed_writer =
                encoder.into_inner().map_err(|e| FlowError::from(e).at(here!()))?;
        }
        match std::rc::Rc::try_unwrap(self.io_ref) {
            Ok(cell) => Ok(cell.into_inner()),
            Err(_) => {
                Err(nerror!(ErrorKind::InternalError, "GifEncoder IoProxy has multiple references"))
            }
        }
    }
}

fn remove_padding(width: u16, pixels: &[u8], stride: usize) -> Vec<u8> {
    pixels.chunks(stride).flat_map(|s| s[0..width as usize * 4].iter().cloned()).collect()
}
/// Creates a frame from pixels in RGBA format.
///
/// *Note: This method is not optimized for speed.*
pub fn from_bgra_with_stride(
    width: u16,
    height: u16,
    pixels: &mut [u8],
    stride: usize,
) -> ::gif::Frame<'static> {
    let mut without_padding = remove_padding(width, pixels, stride);
    for pix in without_padding.chunks_mut(4) {
        pix.swap(0, 2);
        if pix[3] < 0x10 {
            pix[0] = 0;
            pix[1] = 0;
            pix[2] = 0;
            pix[3] = 0;
        }
    }
    ::gif::Frame::from_rgba(width, height, &mut without_padding)
}

pub fn from_bgrx_with_stride(
    width: u16,
    height: u16,
    pixels: &mut [u8],
    stride: usize,
) -> ::gif::Frame<'static> {
    let mut without_padding = remove_padding(width, pixels, stride);

    for pix in without_padding.chunks_mut(4) {
        pix.swap(0, 2);
        pix[3] = 0xFF;
    }
    ::gif::Frame::from_rgba(width, height, &mut without_padding)
}

/// Creates a frame from pixels in RGB format.
///
/// *Note: This method is not optimized for speed.*
pub fn from_bgr_with_stride(
    width: u16,
    height: u16,
    pixels: &[u8],
    stride: usize,
) -> ::gif::Frame<'static> {
    let mut without_padding = remove_padding(width, pixels, stride);
    for pix in without_padding.chunks_mut(3) {
        pix.swap(0, 2);
    }
    ::gif::Frame::from_rgb(width, height, &without_padding)
}
