use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::{Context, ErrorCategory, ErrorKind, FlowError, JsonResponse, Result};
use std::sync::*;

use crate::io::IoProxy;
use imageflow_types::collections::AddRemoveSet;
use imageflow_types::IoDirection;
use std::any::Any;
use std::borrow::BorrowMut;
use std::ops::DerefMut;
use uuid::Uuid;
mod gif;
mod lode;
mod pngquant;
pub use lode::write_png;

mod auto;
pub(crate) mod cms;
mod image_png_decoder;
pub(crate) mod moxcms_transform;
pub(crate) mod source_profile;
mod tiny_lru;

// C codec modules — require c-codecs feature
#[cfg(feature = "c-codecs")]
mod jpeg_decoder;
#[cfg(feature = "c-codecs")]
pub(crate) mod lcms2_transform;
#[cfg(feature = "c-codecs")]
mod libpng_decoder;
#[cfg(feature = "c-codecs")]
mod libpng_encoder;
#[cfg(feature = "c-codecs")]
mod mozjpeg;
#[cfg(feature = "c-codecs")]
mod mozjpeg_decoder;
#[cfg(feature = "c-codecs")]
mod mozjpeg_decoder_helpers;
#[cfg(feature = "c-codecs")]
mod webp;

// Zen codec modules — pure Rust, require zen-codecs feature
#[cfg(feature = "zen-codecs")]
mod zenjpeg_decoder;
#[cfg(feature = "zen-codecs")]
mod zenjpeg_encoder;
#[cfg(feature = "zen-codecs")]
mod zenwebp_codec;

use crate::graphics::bitmaps::BitmapKey;

pub trait DecoderFactory {
    fn create(c: &Context, io: &mut IoProxy, io_id: i32) -> Option<Result<Box<dyn Decoder>>>;
}
pub trait Decoder: Any {
    fn initialize(&mut self, c: &Context) -> Result<()>;
    fn get_unscaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo>;
    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo>;
    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>>;
    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()>;
    fn read_frame(&mut self, c: &Context) -> Result<BitmapKey>;
    fn has_more_frames(&mut self) -> Result<bool>;
    fn as_any(&self) -> &dyn Any;
}
pub trait Encoder {
    // GIF encoder will need to know if transparency is required (we could guess based on first input frame)
    // If not required, we can do frame shrinking and delta encoding. Otherwise we have to
    // encode entire frames and enable transparency (default)
    fn write_frame(
        &mut self,
        c: &Context,
        preset: &s::EncoderPreset,
        frame: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult>;

    fn into_io(self: Box<Self>) -> Result<IoProxy>;
}

enum CodecKind {
    EncoderPlaceholder,
    Encoder(Box<dyn Encoder>),
    EncoderFinished,
    Decoder(Box<dyn Decoder>),
}

#[derive(PartialEq, Copy, Clone)]
pub enum NamedDecoders {
    #[cfg(feature = "c-codecs")]
    MozJpegRsDecoder,
    #[cfg(feature = "c-codecs")]
    ImageRsJpegDecoder,
    ImageRsPngDecoder,
    #[cfg(feature = "c-codecs")]
    LibPngRsDecoder,
    GifRsDecoder,
    #[cfg(feature = "c-codecs")]
    WebPDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenJpegDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenWebPDecoder,
}
impl NamedDecoders {
    pub fn works_for_magic_bytes(&self, bytes: &[u8]) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            NamedDecoders::ImageRsJpegDecoder | NamedDecoders::MozJpegRsDecoder => {
                bytes.starts_with(b"\xFF\xD8\xFF")
            }
            NamedDecoders::GifRsDecoder => {
                bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a")
            }
            #[cfg(feature = "c-codecs")]
            NamedDecoders::LibPngRsDecoder => {
                bytes.starts_with(b"\x89\x50\x4E\x47\x0D\x0A\x1A\x0A")
            }
            NamedDecoders::ImageRsPngDecoder => {
                bytes.starts_with(b"\x89\x50\x4E\x47\x0D\x0A\x1A\x0A")
            }
            #[cfg(feature = "c-codecs")]
            NamedDecoders::WebPDecoder => {
                bytes.starts_with(b"RIFF") && bytes.len() >= 12 && bytes[8..12].starts_with(b"WEBP")
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJpegDecoder => bytes.starts_with(b"\xFF\xD8\xFF"),
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenWebPDecoder => {
                bytes.starts_with(b"RIFF") && bytes.len() >= 12 && bytes[8..12].starts_with(b"WEBP")
            }
        }
    }

    pub fn create(&self, c: &Context, io: IoProxy, io_id: i32) -> Result<Box<dyn Decoder>> {
        return_if_cancelled!(c);
        match self {
            #[cfg(feature = "c-codecs")]
            NamedDecoders::MozJpegRsDecoder => {
                Ok(Box::new(mozjpeg_decoder::MozJpegDecoder::create(c, io, io_id)?))
            }
            #[cfg(feature = "c-codecs")]
            NamedDecoders::LibPngRsDecoder => {
                Ok(Box::new(libpng_decoder::LibPngDecoder::create(c, io, io_id)?))
            }
            NamedDecoders::GifRsDecoder => Ok(Box::new(gif::GifDecoder::create(c, io, io_id)?)),
            #[cfg(feature = "c-codecs")]
            NamedDecoders::ImageRsJpegDecoder => {
                Ok(Box::new(jpeg_decoder::JpegDecoder::create(c, io, io_id)?))
            }
            NamedDecoders::ImageRsPngDecoder => {
                Ok(Box::new(image_png_decoder::ImagePngDecoder::create(c, io, io_id)?))
            }
            #[cfg(feature = "c-codecs")]
            NamedDecoders::WebPDecoder => Ok(Box::new(webp::WebPDecoder::create(c, io, io_id)?)),
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJpegDecoder => {
                Ok(Box::new(zenjpeg_decoder::ZenJpegDecoder::create(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenWebPDecoder => {
                Ok(Box::new(zenwebp_codec::ZenWebPDecoder::create(c, io, io_id)?))
            }
        }
    }
}
#[derive(PartialEq, Copy, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum NamedEncoders {
    GifEncoder,
    #[cfg(feature = "c-codecs")]
    MozJpegEncoder,
    PngQuantEncoder,
    LodePngEncoder,
    #[cfg(feature = "c-codecs")]
    WebPEncoder,
    #[cfg(feature = "c-codecs")]
    LibPngRsEncoder,
    #[cfg(feature = "zen-codecs")]
    ZenJpegEncoder,
    #[cfg(feature = "zen-codecs")]
    ZenWebPEncoder,
}
pub struct EnabledCodecs {
    pub decoders: ::smallvec::SmallVec<[NamedDecoders; 4]>,
    pub encoders: ::smallvec::SmallVec<[NamedEncoders; 8]>,
}
impl Default for EnabledCodecs {
    fn default() -> Self {
        EnabledCodecs {
            decoders: smallvec::SmallVec::from_slice(&[
                // Zen (pure Rust) decoders preferred when available
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJpegDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenWebPDecoder,
                // C-based decoders as fallback
                #[cfg(all(feature = "c-codecs", not(feature = "zen-codecs")))]
                NamedDecoders::MozJpegRsDecoder,
                #[cfg(all(feature = "c-codecs", not(feature = "zen-codecs")))]
                NamedDecoders::WebPDecoder,
                // PNG: prefer libpng (c-codecs) if available, else image-rs
                #[cfg(feature = "c-codecs")]
                NamedDecoders::LibPngRsDecoder,
                #[cfg(not(feature = "c-codecs"))]
                NamedDecoders::ImageRsPngDecoder,
                NamedDecoders::GifRsDecoder,
            ]),
            encoders: smallvec::SmallVec::from_slice(&[
                NamedEncoders::GifEncoder,
                // Zen (pure Rust) encoders preferred
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJpegEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenWebPEncoder,
                // C-based WebP encoder always enabled for lossy (zenwebp lossy has quality issues)
                #[cfg(feature = "c-codecs")]
                NamedEncoders::WebPEncoder,
                // C-based JPEG encoder as fallback
                #[cfg(all(feature = "c-codecs", not(feature = "zen-codecs")))]
                NamedEncoders::MozJpegEncoder,
                NamedEncoders::PngQuantEncoder,
                NamedEncoders::LodePngEncoder,
                #[cfg(feature = "c-codecs")]
                NamedEncoders::LibPngRsEncoder,
            ]),
        }
    }
}

impl EnabledCodecs {
    pub fn prefer_decoder(&mut self, decoder: NamedDecoders) {
        self.decoders.retain(|item| item != &decoder);
        self.decoders.insert(0, decoder);
    }
    pub fn disable_decoder(&mut self, decoder: NamedDecoders) {
        self.decoders.retain(|item| item != &decoder);
    }
    pub fn create_decoder_for_magic_bytes(
        &self,
        bytes: &[u8],
        c: &Context,
        io: IoProxy,
        io_id: i32,
    ) -> Result<Box<dyn Decoder>> {
        for &decoder in self.decoders.iter() {
            if decoder.works_for_magic_bytes(bytes) {
                return decoder.create(c, io, io_id);
            }
        }
        Err(nerror!(
            ErrorKind::NoEnabledDecoderFound,
            "No ENABLED decoder found for file starting in {:X?}",
            bytes
        ))
    }
}

/// Tracks the lifecycle of an encoder's output buffer.
///
/// ```text
/// Ready(IoProxy) ──get_ptr()──→ Lent(IoProxy) ──get_ptr()──→ Lent (idempotent)
///        │                            │
///        │ take()                     │ take() → ERROR
///        ▼                            ▼
///      Taken                    "pointer was lent"
/// ```
enum OutputBufferState {
    /// No output buffer (decoder, or IoProxy loaned to an active encoder).
    None,
    /// Buffer is available for reading or taking.
    Ready(IoProxy),
    /// A raw pointer to the buffer was given out via C ABI.
    /// The IoProxy is kept alive; `take()` is blocked.
    Lent(IoProxy),
    /// The buffer Vec was moved out. All further access errors.
    Taken,
}

// We need a rust-friendly codec instance, codec definition, and a way to wrap C codecs
pub struct CodecInstanceContainer {
    pub io_id: i32,
    codec: CodecKind,
    output_state: OutputBufferState,
}

impl CodecInstanceContainer {
    pub fn get_decoder(&mut self) -> Result<&mut dyn Decoder> {
        if let CodecKind::Decoder(ref mut d) = self.codec {
            Ok(&mut **d)
        } else {
            Err(nerror!(ErrorKind::InvalidArgument, "Not a decoder"))
        }
    }

    pub fn create(
        c: &Context,
        mut io: IoProxy,
        io_id: i32,
        direction: IoDirection,
    ) -> Result<CodecInstanceContainer> {
        if direction == IoDirection::Out {
            Ok(CodecInstanceContainer {
                io_id,
                codec: CodecKind::EncoderPlaceholder,
                output_state: OutputBufferState::Ready(io),
            })
        } else {
            let mut buffer = [0u8; 12];
            let result =
                io.read(&mut buffer).map_err(|e| FlowError::from_decoder(e).at(here!()))?;

            io.seek(io::SeekFrom::Start(0)).map_err(|e| FlowError::from_decoder(e).at(here!()))?;

            Ok(CodecInstanceContainer {
                io_id,
                codec: CodecKind::Decoder(
                    c.enabled_codecs.create_decoder_for_magic_bytes(&buffer, c, io, io_id)?,
                ),
                output_state: OutputBufferState::None,
            })
        }
    }
}

impl CodecInstanceContainer {
    pub fn write_frame(
        &mut self,
        c: &Context,
        preset: &s::EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<s::EncodeResult> {
        // Pick encoder
        if let CodecKind::EncoderPlaceholder = self.codec {
            let io = match std::mem::replace(&mut self.output_state, OutputBufferState::None) {
                OutputBufferState::Ready(io) => io,
                _ => {
                    return Err(nerror!(
                        ErrorKind::InvalidState,
                        "Encoder {} output buffer not in Ready state for write_frame",
                        self.io_id
                    ))
                }
            };
            let encoder = auto::create_encoder(c, io, preset, bitmap_key, decoder_io_ids)
                .map_err(|e| e.at(here!()))?;

            self.codec = CodecKind::Encoder(encoder);
        };

        match self.codec {
            CodecKind::Encoder(ref mut e) => {
                match e
                    .write_frame(c, preset, bitmap_key, decoder_io_ids)
                    .map_err(|e| e.at(here!()))
                {
                    Err(e) => Err(e),
                    Ok(result) => match result.bytes {
                        s::ResultBytes::Elsewhere => Ok(result),
                        other => Err(nerror!(
                            ErrorKind::InternalError,
                            "Encoders must return s::ResultBytes::Elsewhere and write to their owned IO. Found {:?}",
                            other
                        )),
                    },
                }
            }
            CodecKind::EncoderPlaceholder => Err(nerror!(
                ErrorKind::InvalidState,
                "Encoder {} wasn't created somehow",
                self.io_id
            )),
            CodecKind::EncoderFinished => Err(nerror!(
                ErrorKind::InvalidState,
                "Encoder {} has already been finalized",
                self.io_id
            )),
            CodecKind::Decoder(_) => Err(unimpl!()),
        }
    }

    /// Finalize the encoder (if active), reclaiming the IoProxy into `output_state`.
    /// After this call, the encoder is consumed and no more frames can be written.
    /// This must be called before reading the output buffer, since some encoders
    /// (e.g., GIF) write trailing data when finalized.
    fn finalize_encoder(&mut self) -> Result<()> {
        if matches!(self.codec, CodecKind::Encoder(_)) {
            if let CodecKind::Encoder(encoder) =
                std::mem::replace(&mut self.codec, CodecKind::EncoderFinished)
            {
                let io = encoder.into_io().map_err(|e| e.at(here!()))?;
                self.output_state = OutputBufferState::Ready(io);
            }
        }
        Ok(())
    }

    /// Finalize the encoder and move the output buffer out as an owned `Vec<u8>`.
    /// After this call, the buffer is gone — further access will error.
    pub fn take_output_buffer(&mut self) -> Result<Vec<u8>> {
        self.finalize_encoder().map_err(|e| e.at(here!()))?;
        // Check for forbidden states before committing the replace,
        // so we don't destroy a Lent IoProxy (which would dangle the raw pointer).
        match self.output_state {
            OutputBufferState::Lent(_) => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "Cannot take output buffer for io_id {}: a raw pointer was already lent out",
                    self.io_id
                ))
            }
            OutputBufferState::Taken => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "Output buffer for io_id {} has already been taken",
                    self.io_id
                ))
            }
            OutputBufferState::None => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "io_id {} is not an output buffer",
                    self.io_id
                ))
            }
            OutputBufferState::Ready(_) => {} // proceed below
        }
        // Only Ready reaches here — safe to replace with Taken.
        match std::mem::replace(&mut self.output_state, OutputBufferState::Taken) {
            OutputBufferState::Ready(io) => io.into_output_vec().map_err(|e| e.at(here!())),
            _ => unreachable!(),
        }
    }

    /// Finalize the encoder and return raw pointer + length to the output buffer.
    /// Transitions to `Lent` state — the IoProxy is kept alive but `take()` is blocked.
    /// Idempotent: calling again on a `Lent` buffer returns the same pointer.
    ///
    /// The returned pointer is valid as long as this `CodecInstanceContainer` is alive
    /// and the buffer is not taken. Caller must ensure no mutable access occurs
    /// when dereferencing the pointer.
    pub fn output_buffer_raw_parts(&mut self) -> Result<(*const u8, usize)> {
        self.finalize_encoder().map_err(|e| e.at(here!()))?;
        match self.output_state {
            OutputBufferState::Ready(_) => {
                // Transition to Lent
                let io = match std::mem::replace(&mut self.output_state, OutputBufferState::None) {
                    OutputBufferState::Ready(io) => io,
                    _ => unreachable!(),
                };
                let (ptr, len) = io.output_buffer_raw_parts().map_err(|e| e.at(here!()))?;
                self.output_state = OutputBufferState::Lent(io);
                Ok((ptr, len))
            }
            OutputBufferState::Lent(ref io) => {
                io.output_buffer_raw_parts().map_err(|e| e.at(here!()))
            }
            OutputBufferState::Taken => Err(nerror!(
                ErrorKind::InvalidArgument,
                "Output buffer for io_id {} has already been taken",
                self.io_id
            )),
            OutputBufferState::None => Err(nerror!(
                ErrorKind::InvalidArgument,
                "io_id {} is not an output buffer",
                self.io_id
            )),
        }
    }
}
