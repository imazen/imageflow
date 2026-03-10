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

// Unified zen codec adapters (zencodec-types dyn dispatch)
#[cfg(feature = "zen-codecs")]
pub(crate) mod zen_decoder;
#[cfg(feature = "zen-codecs")]
pub(crate) mod zen_encoder;

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
    #[cfg(feature = "zen-codecs")]
    ZenGifDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenJxlDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenAvifDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenHeicDecoder,
}
impl NamedDecoders {
    pub fn is_jpeg(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            Self::MozJpegRsDecoder | Self::ImageRsJpegDecoder => true,
            #[cfg(feature = "zen-codecs")]
            Self::ZenJpegDecoder => true,
            _ => false,
        }
    }
    pub fn is_png(&self) -> bool {
        match self {
            Self::ImageRsPngDecoder => true,
            #[cfg(feature = "c-codecs")]
            Self::LibPngRsDecoder => true,
            _ => false,
        }
    }
    pub fn is_gif(&self) -> bool {
        match self {
            Self::GifRsDecoder => true,
            #[cfg(feature = "zen-codecs")]
            Self::ZenGifDecoder => true,
            _ => false,
        }
    }
    pub fn is_webp(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            Self::WebPDecoder => true,
            #[cfg(feature = "zen-codecs")]
            Self::ZenWebPDecoder => true,
            _ => false,
        }
    }
    pub fn is_jxl(&self) -> bool {
        match self {
            #[cfg(feature = "zen-codecs")]
            Self::ZenJxlDecoder => true,
            _ => false,
        }
    }
    pub fn is_avif(&self) -> bool {
        match self {
            #[cfg(feature = "zen-codecs")]
            Self::ZenAvifDecoder => true,
            _ => false,
        }
    }
    pub fn is_heic(&self) -> bool {
        match self {
            #[cfg(feature = "zen-codecs")]
            Self::ZenHeicDecoder => true,
            _ => false,
        }
    }

    pub fn is_c_codec(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            Self::MozJpegRsDecoder
            | Self::ImageRsJpegDecoder
            | Self::LibPngRsDecoder
            | Self::WebPDecoder => true,
            _ => false,
        }
    }
    pub fn is_zen_codec(&self) -> bool {
        match self {
            #[cfg(feature = "zen-codecs")]
            Self::ZenJpegDecoder
            | Self::ZenWebPDecoder
            | Self::ZenGifDecoder
            | Self::ZenJxlDecoder
            | Self::ZenAvifDecoder
            | Self::ZenHeicDecoder => true,
            _ => false,
        }
    }
    /// Returns the image format this decoder handles.
    pub fn format(&self) -> imageflow_types::ImageFormat {
        self.codec_name().format()
    }

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
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenGifDecoder => {
                bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a")
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJxlDecoder => {
                // JXL bare codestream: 0xFF 0x0A
                // JXL container: 0x00 0x00 0x00 0x0C 0x4A 0x58 0x4C 0x20 0x0D 0x0A 0x87 0x0A
                bytes.starts_with(&[0xFF, 0x0A])
                    || (bytes.len() >= 12
                        && bytes.starts_with(&[0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20]))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenAvifDecoder => {
                // ISO BMFF ftyp box with AVIF brands
                bytes.len() >= 12
                    && &bytes[4..8] == b"ftyp"
                    && (bytes[8..12].starts_with(b"avif")
                        || bytes[8..12].starts_with(b"avis")
                        || bytes[8..12].starts_with(b"mif1"))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenHeicDecoder => {
                // ISO BMFF ftyp box with HEIC brands
                bytes.len() >= 12
                    && &bytes[4..8] == b"ftyp"
                    && (bytes[8..12].starts_with(b"heic")
                        || bytes[8..12].starts_with(b"heix")
                        || bytes[8..12].starts_with(b"heim")
                        || bytes[8..12].starts_with(b"heis")
                        || bytes[8..12].starts_with(b"hevc")
                        || bytes[8..12].starts_with(b"hevx"))
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
                Ok(Box::new(zen_decoder::ZenDecoder::create_jpeg(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenWebPDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_webp(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenGifDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_gif(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJxlDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_jxl(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenAvifDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_avif(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenHeicDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_heic(c, io, io_id)?))
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
    #[cfg(feature = "zen-codecs")]
    ZenGifEncoder,
    #[cfg(feature = "zen-codecs")]
    ZenJxlEncoder,
    #[cfg(feature = "zen-codecs")]
    ZenAvifEncoder,
}
impl NamedEncoders {
    pub fn is_jpeg(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            Self::MozJpegEncoder => true,
            #[cfg(feature = "zen-codecs")]
            Self::ZenJpegEncoder => true,
            _ => false,
        }
    }
    pub fn is_png(&self) -> bool {
        match self {
            Self::PngQuantEncoder | Self::LodePngEncoder => true,
            #[cfg(feature = "c-codecs")]
            Self::LibPngRsEncoder => true,
            _ => false,
        }
    }
    pub fn is_gif(&self) -> bool {
        match self {
            Self::GifEncoder => true,
            #[cfg(feature = "zen-codecs")]
            Self::ZenGifEncoder => true,
            _ => false,
        }
    }
    pub fn is_webp(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            Self::WebPEncoder => true,
            #[cfg(feature = "zen-codecs")]
            Self::ZenWebPEncoder => true,
            _ => false,
        }
    }
    pub fn is_jxl(&self) -> bool {
        match self {
            #[cfg(feature = "zen-codecs")]
            Self::ZenJxlEncoder => true,
            _ => false,
        }
    }
    pub fn is_avif(&self) -> bool {
        match self {
            #[cfg(feature = "zen-codecs")]
            Self::ZenAvifEncoder => true,
            _ => false,
        }
    }
    pub fn is_c_codec(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            Self::MozJpegEncoder | Self::WebPEncoder | Self::LibPngRsEncoder => true,
            _ => false,
        }
    }
    pub fn is_zen_codec(&self) -> bool {
        match self {
            #[cfg(feature = "zen-codecs")]
            Self::ZenJpegEncoder
            | Self::ZenWebPEncoder
            | Self::ZenGifEncoder
            | Self::ZenJxlEncoder
            | Self::ZenAvifEncoder => true,
            _ => false,
        }
    }
    /// Returns the image format this encoder handles.
    pub fn format(&self) -> imageflow_types::ImageFormat {
        self.codec_name().format()
    }
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
                // Zen GIF decoder preferred when available
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenGifDecoder,
                #[cfg(not(feature = "zen-codecs"))]
                NamedDecoders::GifRsDecoder,
                // JXL decoder (zen-codecs only)
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJxlDecoder,
                // AVIF decoder (zen-codecs only)
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenAvifDecoder,
                // HEIC decoder (zen-codecs only)
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenHeicDecoder,
            ]),
            encoders: smallvec::SmallVec::from_slice(&[
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenGifEncoder,
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
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJxlEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenAvifEncoder,
            ]),
        }
    }
}

impl NamedDecoders {
    /// Convert to the stable `CodecName` used in the JSON API.
    pub fn codec_name(&self) -> imageflow_types::CodecName {
        use imageflow_types::CodecName;
        match self {
            #[cfg(feature = "c-codecs")]
            Self::MozJpegRsDecoder => CodecName::MozjpegDecoder,
            #[cfg(feature = "c-codecs")]
            Self::ImageRsJpegDecoder => CodecName::ImageRsJpegDecoder,
            Self::ImageRsPngDecoder => CodecName::ImageRsPngDecoder,
            #[cfg(feature = "c-codecs")]
            Self::LibPngRsDecoder => CodecName::LibpngDecoder,
            Self::GifRsDecoder => CodecName::GifRsDecoder,
            #[cfg(feature = "c-codecs")]
            Self::WebPDecoder => CodecName::LibwebpDecoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenJpegDecoder => CodecName::ZenjpegDecoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenWebPDecoder => CodecName::ZenwebpDecoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenGifDecoder => CodecName::ZengifDecoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenJxlDecoder => CodecName::ZenjxlDecoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenAvifDecoder => CodecName::ZenavifDecoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenHeicDecoder => CodecName::ZenheicDecoder,
        }
    }

    /// Try to convert from a `CodecName`. Returns `None` if the name
    /// refers to an encoder or a codec not compiled in.
    pub fn from_codec_name(name: imageflow_types::CodecName) -> Option<Self> {
        use imageflow_types::CodecName;
        match name {
            #[cfg(feature = "c-codecs")]
            CodecName::MozjpegDecoder => Some(Self::MozJpegRsDecoder),
            #[cfg(feature = "c-codecs")]
            CodecName::ImageRsJpegDecoder => Some(Self::ImageRsJpegDecoder),
            CodecName::ImageRsPngDecoder => Some(Self::ImageRsPngDecoder),
            #[cfg(feature = "c-codecs")]
            CodecName::LibpngDecoder => Some(Self::LibPngRsDecoder),
            CodecName::GifRsDecoder => Some(Self::GifRsDecoder),
            #[cfg(feature = "c-codecs")]
            CodecName::LibwebpDecoder => Some(Self::WebPDecoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenjpegDecoder => Some(Self::ZenJpegDecoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenwebpDecoder => Some(Self::ZenWebPDecoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZengifDecoder => Some(Self::ZenGifDecoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenjxlDecoder => Some(Self::ZenJxlDecoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenavifDecoder => Some(Self::ZenAvifDecoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenheicDecoder => Some(Self::ZenHeicDecoder),
            _ => None,
        }
    }
}

impl NamedEncoders {
    /// Convert to the stable `CodecName` used in the JSON API.
    pub fn codec_name(&self) -> imageflow_types::CodecName {
        use imageflow_types::CodecName;
        match self {
            Self::GifEncoder => CodecName::GifEncoder,
            #[cfg(feature = "c-codecs")]
            Self::MozJpegEncoder => CodecName::MozjpegEncoder,
            Self::PngQuantEncoder => CodecName::PngquantEncoder,
            Self::LodePngEncoder => CodecName::LodepngEncoder,
            #[cfg(feature = "c-codecs")]
            Self::WebPEncoder => CodecName::LibwebpEncoder,
            #[cfg(feature = "c-codecs")]
            Self::LibPngRsEncoder => CodecName::LibpngEncoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenJpegEncoder => CodecName::ZenjpegEncoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenWebPEncoder => CodecName::ZenwebpEncoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenGifEncoder => CodecName::ZengifEncoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenJxlEncoder => CodecName::ZenjxlEncoder,
            #[cfg(feature = "zen-codecs")]
            Self::ZenAvifEncoder => CodecName::ZenavifEncoder,
        }
    }

    /// Try to convert from a `CodecName`. Returns `None` if the name
    /// refers to a decoder or a codec not compiled in.
    pub fn from_codec_name(name: imageflow_types::CodecName) -> Option<Self> {
        use imageflow_types::CodecName;
        match name {
            CodecName::GifEncoder => Some(Self::GifEncoder),
            #[cfg(feature = "c-codecs")]
            CodecName::MozjpegEncoder => Some(Self::MozJpegEncoder),
            CodecName::PngquantEncoder => Some(Self::PngQuantEncoder),
            CodecName::LodepngEncoder => Some(Self::LodePngEncoder),
            #[cfg(feature = "c-codecs")]
            CodecName::LibwebpEncoder => Some(Self::WebPEncoder),
            #[cfg(feature = "c-codecs")]
            CodecName::LibpngEncoder => Some(Self::LibPngRsEncoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenjpegEncoder => Some(Self::ZenJpegEncoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenwebpEncoder => Some(Self::ZenWebPEncoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZengifEncoder => Some(Self::ZenGifEncoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenjxlEncoder => Some(Self::ZenJxlEncoder),
            #[cfg(feature = "zen-codecs")]
            CodecName::ZenavifEncoder => Some(Self::ZenAvifEncoder),
            _ => None,
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

    /// Build an `EnabledCodecs` from the preset definitions.
    pub fn from_preset(preset: imageflow_types::CodecPreset) -> Self {
        use imageflow_types::CodecPreset;
        match preset {
            CodecPreset::Legacy => Self::preset_legacy(),
            CodecPreset::Transitional => Self::preset_transitional(),
            CodecPreset::Modern => Self::preset_modern(),
            CodecPreset::Experimental => Self::preset_experimental(),
        }
    }

    /// Legacy: C codecs only. Always-available Rust fallbacks kept.
    fn preset_legacy() -> Self {
        EnabledCodecs {
            decoders: smallvec::SmallVec::from_slice(&[
                #[cfg(feature = "c-codecs")]
                NamedDecoders::MozJpegRsDecoder,
                #[cfg(feature = "c-codecs")]
                NamedDecoders::LibPngRsDecoder,
                #[cfg(feature = "c-codecs")]
                NamedDecoders::WebPDecoder,
                NamedDecoders::ImageRsPngDecoder,
                NamedDecoders::GifRsDecoder,
            ]),
            encoders: smallvec::SmallVec::from_slice(&[
                #[cfg(feature = "c-codecs")]
                NamedEncoders::MozJpegEncoder,
                #[cfg(feature = "c-codecs")]
                NamedEncoders::LibPngRsEncoder,
                #[cfg(feature = "c-codecs")]
                NamedEncoders::WebPEncoder,
                NamedEncoders::PngQuantEncoder,
                NamedEncoders::LodePngEncoder,
                NamedEncoders::GifEncoder,
            ]),
        }
    }

    /// Transitional: C primary, zen supplements for new formats + fallbacks.
    fn preset_transitional() -> Self {
        EnabledCodecs {
            decoders: smallvec::SmallVec::from_slice(&[
                #[cfg(feature = "c-codecs")]
                NamedDecoders::MozJpegRsDecoder,
                #[cfg(feature = "c-codecs")]
                NamedDecoders::LibPngRsDecoder,
                #[cfg(feature = "c-codecs")]
                NamedDecoders::WebPDecoder,
                NamedDecoders::GifRsDecoder,
                NamedDecoders::ImageRsPngDecoder,
                // Zen decoders as fallback for C-covered formats
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJpegDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenWebPDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenGifDecoder,
                // Zen-only formats
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJxlDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenAvifDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenHeicDecoder,
            ]),
            encoders: smallvec::SmallVec::from_slice(&[
                #[cfg(feature = "c-codecs")]
                NamedEncoders::MozJpegEncoder,
                #[cfg(feature = "c-codecs")]
                NamedEncoders::LibPngRsEncoder,
                #[cfg(feature = "c-codecs")]
                NamedEncoders::WebPEncoder,
                NamedEncoders::GifEncoder,
                NamedEncoders::PngQuantEncoder,
                NamedEncoders::LodePngEncoder,
                // Zen encoder fallbacks for C-covered formats
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJpegEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenWebPEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenGifEncoder,
                // Zen-only formats
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJxlEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenAvifEncoder,
            ]),
        }
    }

    /// Modern: Zen primary, C as fallback. Default behavior.
    fn preset_modern() -> Self {
        // This matches the current Default::default() ordering
        Self::default()
    }

    /// Experimental: Pure Rust, no C codecs.
    fn preset_experimental() -> Self {
        EnabledCodecs {
            decoders: smallvec::SmallVec::from_slice(&[
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJpegDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenWebPDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenGifDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJxlDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenAvifDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenHeicDecoder,
                NamedDecoders::ImageRsPngDecoder,
                NamedDecoders::GifRsDecoder,
            ]),
            encoders: smallvec::SmallVec::from_slice(&[
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJpegEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenWebPEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenGifEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJxlEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenAvifEncoder,
                NamedEncoders::PngQuantEncoder,
                NamedEncoders::LodePngEncoder,
                NamedEncoders::GifEncoder,
            ]),
        }
    }

    /// Add codec implementations that are not already present.
    /// Uses the default ordering to determine where each codec is inserted
    /// (appended at end if not in default list). Codecs not compiled in are
    /// silently skipped.
    pub fn apply_enable(&mut self, enable: &[imageflow_types::CodecName]) {
        for &name in enable {
            if let Some(d) = NamedDecoders::from_codec_name(name) {
                if !self.decoders.contains(&d) {
                    self.decoders.push(d);
                }
            }
            if let Some(e) = NamedEncoders::from_codec_name(name) {
                if !self.encoders.contains(&e) {
                    self.encoders.push(e);
                }
            }
        }
    }

    /// Apply `disable` list: remove named codec implementations.
    pub fn apply_disable(&mut self, disable: &[imageflow_types::CodecName]) {
        for &name in disable {
            if let Some(d) = NamedDecoders::from_codec_name(name) {
                self.decoders.retain(|item| item != &d);
            }
            if let Some(e) = NamedEncoders::from_codec_name(name) {
                self.encoders.retain(|item| item != &e);
            }
        }
    }

    /// Apply `prefer` list: move named codecs to front of their respective lists.
    pub fn apply_prefer(&mut self, prefer: &[imageflow_types::CodecName]) {
        // Process in reverse so first item in `prefer` ends up at position 0
        for &name in prefer.iter().rev() {
            if let Some(d) = NamedDecoders::from_codec_name(name) {
                if self.decoders.contains(&d) {
                    self.decoders.retain(|item| item != &d);
                    self.decoders.insert(0, d);
                }
            }
            if let Some(e) = NamedEncoders::from_codec_name(name) {
                if self.encoders.contains(&e) {
                    self.encoders.retain(|item| item != &e);
                    self.encoders.insert(0, e);
                }
            }
        }
    }

    /// Disable all decoders for the given formats.
    pub fn apply_disable_decode_formats(&mut self, formats: &[imageflow_types::ImageFormat]) {
        for &fmt in formats {
            self.decoders.retain(|d| d.codec_name().format() != fmt);
        }
    }

    /// Disable all encoders for the given formats.
    pub fn apply_disable_encode_formats(&mut self, formats: &[imageflow_types::ImageFormat]) {
        for &fmt in formats {
            self.encoders.retain(|e| e.codec_name().format() != fmt);
        }
    }

    /// Re-enable decoders for the given formats using the default decoder list.
    /// Only adds decoders that are compiled in and not already present.
    pub fn apply_enable_decode_formats(&mut self, formats: &[imageflow_types::ImageFormat]) {
        let defaults = Self::default();
        for &fmt in formats {
            for d in &defaults.decoders {
                if d.codec_name().format() == fmt && !self.decoders.contains(d) {
                    self.decoders.push(*d);
                }
            }
        }
    }

    /// Re-enable encoders for the given formats using the default encoder list.
    /// Only adds encoders that are compiled in and not already present.
    pub fn apply_enable_encode_formats(&mut self, formats: &[imageflow_types::ImageFormat]) {
        let defaults = Self::default();
        for &fmt in formats {
            for e in &defaults.encoders {
                if e.codec_name().format() == fmt && !self.encoders.contains(e) {
                    self.encoders.push(*e);
                }
            }
        }
    }

    // ── Query methods ────────────────────────────────────────────────

    /// Returns true if any encoder for the given format is enabled.
    pub fn has_encoder_for_format(&self, format: imageflow_types::ImageFormat) -> bool {
        self.encoders.iter().any(|e| e.codec_name().format() == format)
    }

    /// Returns true if any decoder for the given format is enabled.
    pub fn has_decoder_for_format(&self, format: imageflow_types::ImageFormat) -> bool {
        self.decoders.iter().any(|d| d.codec_name().format() == format)
    }

    /// Returns the highest-priority encoder for the given format, if any.
    pub fn first_encoder_for_format(
        &self,
        format: imageflow_types::ImageFormat,
    ) -> Option<NamedEncoders> {
        self.encoders.iter().copied().find(|e| e.codec_name().format() == format)
    }

    /// Returns the highest-priority decoder for the given format, if any.
    pub fn first_decoder_for_format(
        &self,
        format: imageflow_types::ImageFormat,
    ) -> Option<NamedDecoders> {
        self.decoders.iter().copied().find(|d| d.codec_name().format() == format)
    }

    /// Returns true if the specific encoder is enabled.
    pub fn has_encoder(&self, encoder: NamedEncoders) -> bool {
        self.encoders.contains(&encoder)
    }

    /// Returns true if the specific decoder is enabled.
    pub fn has_decoder(&self, decoder: NamedDecoders) -> bool {
        self.decoders.contains(&decoder)
    }

    /// Returns the deduplicated set of formats that have at least one encoder,
    /// in priority order (first encoder for each format determines ordering).
    pub fn available_encode_formats(
        &self,
    ) -> smallvec::SmallVec<[imageflow_types::ImageFormat; 8]> {
        let mut formats = smallvec::SmallVec::new();
        for e in &self.encoders {
            let fmt = e.codec_name().format();
            if !formats.contains(&fmt) {
                formats.push(fmt);
            }
        }
        formats
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
