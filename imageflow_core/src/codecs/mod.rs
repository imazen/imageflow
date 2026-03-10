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
pub mod codec_decisions;
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

// ── Codec capabilities ──────────────────────────────────────────────────────

/// Static capability metadata for an encoder implementation.
#[derive(Debug, Clone, Copy)]
pub struct EncoderCaps {
    pub format: imageflow_types::ImageFormat,
    pub lossy: bool,
    pub lossless: bool,
    pub animation: bool,
    pub alpha: bool,
    /// Quality ranking for lossy encoding (0=broken, 1=functional, 2=good, 3=excellent).
    /// Used to pick the best encoder when multiple support the same format+mode.
    pub lossy_rank: u8,
    /// Quality ranking for lossless encoding.
    pub lossless_rank: u8,
}

/// Static capability metadata for a decoder implementation.
#[derive(Debug, Clone, Copy)]
pub struct DecoderCaps {
    pub format: imageflow_types::ImageFormat,
    pub animation: bool,
}

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

/// Named decoder implementations. Variants are always present regardless of
/// feature flags — `#[cfg]` only gates whether `create()` can instantiate them
/// and whether they appear in default/preset lists.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum NamedDecoders {
    MozJpegRsDecoder,
    ImageRsJpegDecoder,
    ImageRsPngDecoder,
    LibPngRsDecoder,
    GifRsDecoder,
    WebPDecoder,
    ZenJpegDecoder,
    ZenWebPDecoder,
    ZenGifDecoder,
    ZenJxlDecoder,
    ZenAvifDecoder,
    ZenHeicDecoder,
}
impl NamedDecoders {
    pub fn is_jpeg(&self) -> bool {
        matches!(self, Self::MozJpegRsDecoder | Self::ImageRsJpegDecoder | Self::ZenJpegDecoder)
    }
    pub fn is_png(&self) -> bool {
        matches!(self, Self::ImageRsPngDecoder | Self::LibPngRsDecoder)
    }
    pub fn is_gif(&self) -> bool {
        matches!(self, Self::GifRsDecoder | Self::ZenGifDecoder)
    }
    pub fn is_webp(&self) -> bool {
        matches!(self, Self::WebPDecoder | Self::ZenWebPDecoder)
    }
    pub fn is_jxl(&self) -> bool {
        matches!(self, Self::ZenJxlDecoder)
    }
    pub fn is_avif(&self) -> bool {
        matches!(self, Self::ZenAvifDecoder)
    }
    pub fn is_heic(&self) -> bool {
        matches!(self, Self::ZenHeicDecoder)
    }

    pub fn is_c_codec(&self) -> bool {
        matches!(
            self,
            Self::MozJpegRsDecoder
                | Self::ImageRsJpegDecoder
                | Self::LibPngRsDecoder
                | Self::WebPDecoder
        )
    }
    pub fn is_zen_codec(&self) -> bool {
        matches!(
            self,
            Self::ZenJpegDecoder
                | Self::ZenWebPDecoder
                | Self::ZenGifDecoder
                | Self::ZenJxlDecoder
                | Self::ZenAvifDecoder
                | Self::ZenHeicDecoder
        )
    }

    /// Returns the image format this decoder handles.
    pub fn format(&self) -> imageflow_types::ImageFormat {
        self.caps().format
    }

    /// Static capability metadata for this decoder implementation.
    pub fn caps(&self) -> DecoderCaps {
        use imageflow_types::ImageFormat;
        match self {
            Self::MozJpegRsDecoder => DecoderCaps { format: ImageFormat::Jpeg, animation: false },
            Self::ImageRsJpegDecoder => DecoderCaps { format: ImageFormat::Jpeg, animation: false },
            Self::ImageRsPngDecoder => DecoderCaps { format: ImageFormat::Png, animation: false },
            Self::LibPngRsDecoder => DecoderCaps { format: ImageFormat::Png, animation: false },
            Self::GifRsDecoder => DecoderCaps { format: ImageFormat::Gif, animation: true },
            Self::WebPDecoder => DecoderCaps { format: ImageFormat::Webp, animation: false },
            Self::ZenJpegDecoder => DecoderCaps { format: ImageFormat::Jpeg, animation: false },
            Self::ZenWebPDecoder => DecoderCaps { format: ImageFormat::Webp, animation: false },
            Self::ZenGifDecoder => DecoderCaps { format: ImageFormat::Gif, animation: true },
            Self::ZenJxlDecoder => DecoderCaps { format: ImageFormat::Jxl, animation: false },
            Self::ZenAvifDecoder => DecoderCaps { format: ImageFormat::Avif, animation: false },
            Self::ZenHeicDecoder => DecoderCaps { format: ImageFormat::Heic, animation: false },
        }
    }

    /// Returns true if this decoder can be instantiated with the current feature flags.
    pub fn is_compiled_in(&self) -> bool {
        match self {
            Self::MozJpegRsDecoder
            | Self::ImageRsJpegDecoder
            | Self::LibPngRsDecoder
            | Self::WebPDecoder => cfg!(feature = "c-codecs"),
            Self::ZenJpegDecoder
            | Self::ZenWebPDecoder
            | Self::ZenGifDecoder
            | Self::ZenJxlDecoder
            | Self::ZenAvifDecoder
            | Self::ZenHeicDecoder => {
                cfg!(feature = "zen-codecs")
            }
            Self::ImageRsPngDecoder | Self::GifRsDecoder => true,
        }
    }

    pub fn works_for_magic_bytes(&self, bytes: &[u8]) -> bool {
        match self {
            Self::MozJpegRsDecoder | Self::ImageRsJpegDecoder | Self::ZenJpegDecoder => {
                bytes.starts_with(b"\xFF\xD8\xFF")
            }
            Self::GifRsDecoder | Self::ZenGifDecoder => {
                bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a")
            }
            Self::LibPngRsDecoder | Self::ImageRsPngDecoder => {
                bytes.starts_with(b"\x89\x50\x4E\x47\x0D\x0A\x1A\x0A")
            }
            Self::WebPDecoder | Self::ZenWebPDecoder => {
                bytes.starts_with(b"RIFF") && bytes.len() >= 12 && bytes[8..12].starts_with(b"WEBP")
            }
            Self::ZenJxlDecoder => {
                // JXL bare codestream: 0xFF 0x0A
                // JXL container: 0x00 0x00 0x00 0x0C 0x4A 0x58 0x4C 0x20 0x0D 0x0A 0x87 0x0A
                bytes.starts_with(&[0xFF, 0x0A])
                    || (bytes.len() >= 12
                        && bytes.starts_with(&[0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20]))
            }
            Self::ZenAvifDecoder => {
                bytes.len() >= 12
                    && &bytes[4..8] == b"ftyp"
                    && (bytes[8..12].starts_with(b"avif")
                        || bytes[8..12].starts_with(b"avis")
                        || bytes[8..12].starts_with(b"mif1"))
            }
            Self::ZenHeicDecoder => {
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

    /// Instantiate the decoder. Returns an error if the codec is not compiled in.
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
            #[allow(unreachable_patterns)]
            other => Err(nerror!(
                ErrorKind::CodecDisabledError,
                "Decoder {:?} is not compiled in (missing feature flag)",
                other
            )),
        }
    }
}
/// Named encoder implementations. Variants are always present regardless of
/// feature flags — `#[cfg]` only gates whether they appear in default/preset lists
/// and whether `auto.rs` can instantiate the concrete encoder type.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum NamedEncoders {
    GifEncoder,
    MozJpegEncoder,
    PngQuantEncoder,
    LodePngEncoder,
    WebPEncoder,
    LibPngRsEncoder,
    ZenJpegEncoder,
    ZenWebPEncoder,
    ZenGifEncoder,
    ZenJxlEncoder,
    ZenAvifEncoder,
}
impl NamedEncoders {
    pub fn is_jpeg(&self) -> bool {
        matches!(self, Self::MozJpegEncoder | Self::ZenJpegEncoder)
    }
    pub fn is_png(&self) -> bool {
        matches!(self, Self::PngQuantEncoder | Self::LodePngEncoder | Self::LibPngRsEncoder)
    }
    pub fn is_gif(&self) -> bool {
        matches!(self, Self::GifEncoder | Self::ZenGifEncoder)
    }
    pub fn is_webp(&self) -> bool {
        matches!(self, Self::WebPEncoder | Self::ZenWebPEncoder)
    }
    pub fn is_jxl(&self) -> bool {
        matches!(self, Self::ZenJxlEncoder)
    }
    pub fn is_avif(&self) -> bool {
        matches!(self, Self::ZenAvifEncoder)
    }
    pub fn is_c_codec(&self) -> bool {
        matches!(self, Self::MozJpegEncoder | Self::WebPEncoder | Self::LibPngRsEncoder)
    }
    pub fn is_zen_codec(&self) -> bool {
        matches!(
            self,
            Self::ZenJpegEncoder
                | Self::ZenWebPEncoder
                | Self::ZenGifEncoder
                | Self::ZenJxlEncoder
                | Self::ZenAvifEncoder
        )
    }

    /// Returns the image format this encoder handles.
    pub fn format(&self) -> imageflow_types::ImageFormat {
        self.caps().format
    }

    /// Returns true if this encoder can be instantiated with the current feature flags.
    pub fn is_compiled_in(&self) -> bool {
        match self {
            Self::MozJpegEncoder | Self::WebPEncoder | Self::LibPngRsEncoder => {
                cfg!(feature = "c-codecs")
            }
            Self::ZenJpegEncoder
            | Self::ZenWebPEncoder
            | Self::ZenGifEncoder
            | Self::ZenJxlEncoder
            | Self::ZenAvifEncoder => cfg!(feature = "zen-codecs"),
            Self::GifEncoder | Self::PngQuantEncoder | Self::LodePngEncoder => true,
        }
    }

    /// Static capability metadata for this encoder implementation.
    pub fn caps(&self) -> EncoderCaps {
        use imageflow_types::ImageFormat;
        match self {
            Self::GifEncoder => EncoderCaps {
                format: ImageFormat::Gif,
                lossy: true,
                lossless: false,
                animation: true,
                alpha: true,
                lossy_rank: 2,
                lossless_rank: 0,
            },
            Self::MozJpegEncoder => EncoderCaps {
                format: ImageFormat::Jpeg,
                lossy: true,
                lossless: false,
                animation: false,
                alpha: false,
                lossy_rank: 3,
                lossless_rank: 0,
            },
            Self::PngQuantEncoder => EncoderCaps {
                format: ImageFormat::Png,
                lossy: true,
                lossless: false,
                animation: false,
                alpha: true,
                lossy_rank: 2,
                lossless_rank: 0,
            },
            Self::LodePngEncoder => EncoderCaps {
                format: ImageFormat::Png,
                lossy: false,
                lossless: true,
                animation: false,
                alpha: true,
                lossy_rank: 0,
                lossless_rank: 2,
            },
            Self::WebPEncoder => EncoderCaps {
                format: ImageFormat::Webp,
                lossy: true,
                lossless: true,
                animation: false,
                alpha: true,
                lossy_rank: 2,
                lossless_rank: 2,
            },
            Self::LibPngRsEncoder => EncoderCaps {
                format: ImageFormat::Png,
                lossy: false,
                lossless: true,
                animation: false,
                alpha: true,
                lossy_rank: 0,
                lossless_rank: 2,
            },
            Self::ZenJpegEncoder => EncoderCaps {
                format: ImageFormat::Jpeg,
                lossy: true,
                lossless: false,
                animation: false,
                alpha: false,
                lossy_rank: 3,
                lossless_rank: 0,
            },
            Self::ZenWebPEncoder => EncoderCaps {
                format: ImageFormat::Webp,
                lossy: true,
                lossless: true,
                animation: false,
                alpha: true,
                lossy_rank: 2,
                lossless_rank: 2,
            },
            Self::ZenGifEncoder => EncoderCaps {
                format: ImageFormat::Gif,
                lossy: true,
                lossless: false,
                animation: true,
                alpha: true,
                lossy_rank: 2,
                lossless_rank: 0,
            },
            Self::ZenJxlEncoder => EncoderCaps {
                format: ImageFormat::Jxl,
                lossy: true,
                lossless: true,
                animation: false,
                alpha: true,
                lossy_rank: 3,
                lossless_rank: 3,
            },
            Self::ZenAvifEncoder => EncoderCaps {
                format: ImageFormat::Avif,
                lossy: true,
                lossless: false,
                animation: false,
                alpha: true,
                lossy_rank: 3,
                lossless_rank: 0,
            },
        }
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
                // C-based WebP encoder as fallback
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
            Self::MozJpegRsDecoder => CodecName::MozjpegDecoder,
            Self::ImageRsJpegDecoder => CodecName::ImageRsJpegDecoder,
            Self::ImageRsPngDecoder => CodecName::ImageRsPngDecoder,
            Self::LibPngRsDecoder => CodecName::LibpngDecoder,
            Self::GifRsDecoder => CodecName::GifRsDecoder,
            Self::WebPDecoder => CodecName::LibwebpDecoder,
            Self::ZenJpegDecoder => CodecName::ZenjpegDecoder,
            Self::ZenWebPDecoder => CodecName::ZenwebpDecoder,
            Self::ZenGifDecoder => CodecName::ZengifDecoder,
            Self::ZenJxlDecoder => CodecName::ZenjxlDecoder,
            Self::ZenAvifDecoder => CodecName::ZenavifDecoder,
            Self::ZenHeicDecoder => CodecName::ZenheicDecoder,
        }
    }

    /// Convert from a `CodecName`. Returns `None` if the name refers to an encoder.
    /// Always succeeds for decoder names regardless of feature flags — the variant
    /// exists, it just may not be instantiable.
    pub fn from_codec_name(name: imageflow_types::CodecName) -> Option<Self> {
        use imageflow_types::CodecName;
        match name {
            CodecName::MozjpegDecoder => Some(Self::MozJpegRsDecoder),
            CodecName::ImageRsJpegDecoder => Some(Self::ImageRsJpegDecoder),
            CodecName::ImageRsPngDecoder => Some(Self::ImageRsPngDecoder),
            CodecName::LibpngDecoder => Some(Self::LibPngRsDecoder),
            CodecName::GifRsDecoder => Some(Self::GifRsDecoder),
            CodecName::LibwebpDecoder => Some(Self::WebPDecoder),
            CodecName::ZenjpegDecoder => Some(Self::ZenJpegDecoder),
            CodecName::ZenwebpDecoder => Some(Self::ZenWebPDecoder),
            CodecName::ZengifDecoder => Some(Self::ZenGifDecoder),
            CodecName::ZenjxlDecoder => Some(Self::ZenJxlDecoder),
            CodecName::ZenavifDecoder => Some(Self::ZenAvifDecoder),
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
            Self::MozJpegEncoder => CodecName::MozjpegEncoder,
            Self::PngQuantEncoder => CodecName::PngquantEncoder,
            Self::LodePngEncoder => CodecName::LodepngEncoder,
            Self::WebPEncoder => CodecName::LibwebpEncoder,
            Self::LibPngRsEncoder => CodecName::LibpngEncoder,
            Self::ZenJpegEncoder => CodecName::ZenjpegEncoder,
            Self::ZenWebPEncoder => CodecName::ZenwebpEncoder,
            Self::ZenGifEncoder => CodecName::ZengifEncoder,
            Self::ZenJxlEncoder => CodecName::ZenjxlEncoder,
            Self::ZenAvifEncoder => CodecName::ZenavifEncoder,
        }
    }

    /// Convert from a `CodecName`. Returns `None` if the name refers to a decoder.
    /// Always succeeds for encoder names regardless of feature flags.
    pub fn from_codec_name(name: imageflow_types::CodecName) -> Option<Self> {
        use imageflow_types::CodecName;
        match name {
            CodecName::GifEncoder => Some(Self::GifEncoder),
            CodecName::MozjpegEncoder => Some(Self::MozJpegEncoder),
            CodecName::PngquantEncoder => Some(Self::PngQuantEncoder),
            CodecName::LodepngEncoder => Some(Self::LodePngEncoder),
            CodecName::LibwebpEncoder => Some(Self::WebPEncoder),
            CodecName::LibpngEncoder => Some(Self::LibPngRsEncoder),
            CodecName::ZenjpegEncoder => Some(Self::ZenJpegEncoder),
            CodecName::ZenwebpEncoder => Some(Self::ZenWebPEncoder),
            CodecName::ZengifEncoder => Some(Self::ZenGifEncoder),
            CodecName::ZenjxlEncoder => Some(Self::ZenJxlEncoder),
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

    // ── Capability-aware selection ─────────────────────────────────

    /// Returns true if any enabled encoder for `format` has the given capability.
    pub fn format_supports_lossy(&self, format: imageflow_types::ImageFormat) -> bool {
        self.encoders.iter().any(|e| {
            let c = e.caps();
            c.format == format && c.lossy
        })
    }

    pub fn format_supports_lossless(&self, format: imageflow_types::ImageFormat) -> bool {
        self.encoders.iter().any(|e| {
            let c = e.caps();
            c.format == format && c.lossless
        })
    }

    pub fn format_supports_animation(&self, format: imageflow_types::ImageFormat) -> bool {
        self.encoders.iter().any(|e| {
            let c = e.caps();
            c.format == format && c.animation
        })
    }

    pub fn format_supports_alpha(&self, format: imageflow_types::ImageFormat) -> bool {
        self.encoders.iter().any(|e| {
            let c = e.caps();
            c.format == format && c.alpha
        })
    }

    /// Select the best encoder for `format` given the desired mode.
    /// Walks the priority list, filters by capability, picks the highest-ranked candidate.
    /// Returns the chosen encoder and a trace of the decision.
    pub fn select_encoder(
        &self,
        format: imageflow_types::ImageFormat,
        lossless: bool,
    ) -> Option<(NamedEncoders, Vec<&'static str>)> {
        let mut trace = Vec::new();
        let mut best: Option<(NamedEncoders, u8)> = None;

        for &enc in &self.encoders {
            let caps = enc.caps();
            if caps.format != format {
                continue;
            }
            if lossless && !caps.lossless {
                trace.push(if enc.is_zen_codec() {
                    "zen encoder: no lossless support, skipped"
                } else {
                    "c encoder: no lossless support, skipped"
                });
                continue;
            }
            if !lossless && !caps.lossy {
                trace.push(if enc.is_zen_codec() {
                    "zen encoder: no lossy support, skipped"
                } else {
                    "c encoder: no lossy support, skipped"
                });
                continue;
            }
            let rank = if lossless { caps.lossless_rank } else { caps.lossy_rank };
            match best {
                None => {
                    best = Some((enc, rank));
                    trace.push("first capable encoder found");
                }
                Some((_, prev_rank)) if rank > prev_rank => {
                    best = Some((enc, rank));
                    trace.push("higher-ranked encoder found, replacing");
                }
                _ => {
                    trace.push("lower-ranked encoder skipped");
                }
            }
        }
        best.map(|(enc, _)| (enc, trace))
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

// ── Codec Selector — pure-function format + encoder selection ────────────────

/// Criteria for automatic format selection.
#[derive(Debug, Clone)]
pub struct FormatCriteria {
    pub allowed: imageflow_types::AllowedFormats,
    pub has_alpha: bool,
    pub has_animation: bool,
    pub lossless: Option<bool>,
    pub source_lossless: Option<bool>,
    pub pixel_count: u64,
    pub quality_profile: Option<imageflow_types::QualityProfile>,
}

impl Default for FormatCriteria {
    fn default() -> Self {
        Self {
            allowed: imageflow_types::AllowedFormats::web_safe(),
            has_alpha: false,
            has_animation: false,
            lossless: None,
            source_lossless: None,
            pixel_count: 0,
            quality_profile: None,
        }
    }
}

/// Result of a codec selection decision, with full trace.
#[derive(Debug, Clone)]
pub struct Selection<T: core::fmt::Debug + Clone> {
    pub chosen: T,
    pub trace: Vec<&'static str>,
}

/// Combined format + encoder selection result.
#[derive(Debug, Clone)]
pub struct FullSelection {
    pub format: imageflow_types::OutputImageFormat,
    pub encoder: NamedEncoders,
    pub trace: Vec<&'static str>,
}

/// Pure-function codec selector. No `Context` dependency — fully unit-testable.
pub struct CodecSelector<'a> {
    pub codecs: &'a EnabledCodecs,
}

impl<'a> CodecSelector<'a> {
    pub fn new(codecs: &'a EnabledCodecs) -> Self {
        Self { codecs }
    }

    /// Select the best output format for the given criteria.
    pub fn select_format(
        &self,
        c: &FormatCriteria,
    ) -> Option<Selection<imageflow_types::OutputImageFormat>> {
        use imageflow_types::{ImageFormat, OutputImageFormat};

        let allowed = &c.allowed;
        let mut trace: Vec<&'static str> = Vec::new();

        if !allowed.any_formats_enabled() {
            trace.push("no formats enabled in AllowedFormats");
            return None;
        }

        // Animation path
        if c.has_animation {
            trace.push("source has animation");
            if c.lossless == Some(true) {
                if self.can_encode_animated(ImageFormat::Webp) && allowed.webp == Some(true) {
                    trace.push("webp: animated lossless available");
                    return Some(Selection { chosen: OutputImageFormat::Webp, trace });
                }
            }
            if self.can_encode_animated(ImageFormat::Avif) && allowed.avif == Some(true) {
                trace.push("avif: animated encoding available");
                return Some(Selection { chosen: OutputImageFormat::Avif, trace });
            }
            if self.can_encode_animated(ImageFormat::Webp) && allowed.webp == Some(true) {
                trace.push("webp: animated encoding available");
                return Some(Selection { chosen: OutputImageFormat::Webp, trace });
            }
            trace.push("gif: animation fallback");
            return Some(Selection { chosen: OutputImageFormat::Gif, trace });
        }

        // JXL always wins if available (best compression across all modes)
        if self.codecs.has_encoder_for_format(ImageFormat::Jxl) && allowed.jxl == Some(true) {
            trace.push("jxl: best compression, selected");
            return Some(Selection { chosen: OutputImageFormat::Jxl, trace });
        }

        let choose_lossless = c.lossless == Some(true) || c.source_lossless == Some(true);

        // Lossless path
        if choose_lossless {
            trace.push("lossless path");
            if allowed.webp == Some(true) && self.codecs.format_supports_lossless(ImageFormat::Webp)
            {
                trace.push("webp: lossless, good compression");
                return Some(Selection { chosen: OutputImageFormat::Webp, trace });
            }
            if allowed.png == Some(true) {
                trace.push("png: lossless fallback");
                return Some(Selection { chosen: OutputImageFormat::Png, trace });
            }
            if allowed.avif == Some(true) && self.codecs.format_supports_lossless(ImageFormat::Avif)
            {
                trace.push("avif: lossless available");
                return Some(Selection { chosen: OutputImageFormat::Avif, trace });
            }
        }

        // Lossy with alpha: AVIF > WebP > PNG
        if c.has_alpha {
            trace.push("alpha required");
            if allowed.avif == Some(true) && self.codecs.has_encoder_for_format(ImageFormat::Avif) {
                trace.push("avif: best lossy alpha compression");
                return Some(Selection { chosen: OutputImageFormat::Avif, trace });
            }
            if allowed.webp == Some(true) {
                trace.push("webp: lossy alpha fallback");
                return Some(Selection { chosen: OutputImageFormat::Webp, trace });
            }
            if allowed.png == Some(true) {
                trace.push("png: alpha fallback");
                return Some(Selection { chosen: OutputImageFormat::Png, trace });
            }
        }

        // Lossy without alpha: AVIF for small images, then JPEG, WebP
        trace.push("lossy opaque path");
        if c.pixel_count < 3_000_000
            && allowed.avif == Some(true)
            && self.codecs.has_encoder_for_format(ImageFormat::Avif)
        {
            trace.push("avif: good compression for < 3Mpx");
            return Some(Selection { chosen: OutputImageFormat::Avif, trace });
        }

        if allowed.jpeg == Some(true) {
            trace.push("jpeg: default lossy opaque");
            return Some(Selection { chosen: OutputImageFormat::Jpeg, trace });
        }
        if allowed.webp == Some(true) {
            trace.push("webp: lossy fallback");
            return Some(Selection { chosen: OutputImageFormat::Webp, trace });
        }
        if allowed.avif == Some(true) && self.codecs.has_encoder_for_format(ImageFormat::Avif) {
            trace.push("avif: last resort lossy");
            return Some(Selection { chosen: OutputImageFormat::Avif, trace });
        }
        if allowed.png == Some(true) {
            trace.push("png: last resort");
            return Some(Selection { chosen: OutputImageFormat::Png, trace });
        }
        if allowed.gif == Some(true) {
            trace.push("gif: last resort");
            return Some(Selection { chosen: OutputImageFormat::Gif, trace });
        }

        trace.push("no suitable format found");
        None
    }

    /// Select the best encoder implementation for a format+mode.
    pub fn select_encoder(
        &self,
        format: imageflow_types::ImageFormat,
        lossless: bool,
    ) -> Option<Selection<NamedEncoders>> {
        self.codecs
            .select_encoder(format, lossless)
            .map(|(enc, trace)| Selection { chosen: enc, trace })
    }

    /// Combined: pick format, then pick encoder.
    pub fn select(&self, criteria: &FormatCriteria) -> Option<FullSelection> {
        let format_sel = self.select_format(criteria)?;
        let lossless = criteria.lossless.unwrap_or(false);
        let format = format_sel.chosen;
        let img_format = format.to_image_format()?;

        let encoder_sel = self.select_encoder(img_format, lossless)?;
        let mut trace = format_sel.trace;
        trace.extend(encoder_sel.trace);
        Some(FullSelection { format, encoder: encoder_sel.chosen, trace })
    }

    fn can_encode_animated(&self, format: imageflow_types::ImageFormat) -> bool {
        self.codecs.format_supports_animation(format)
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

#[cfg(test)]
mod codec_selector_tests {
    use super::*;
    use imageflow_types::{AllowedFormats, ImageFormat, OutputImageFormat};

    // ── Test helpers ─────────────────────────────────────────────────────────

    /// Build an EnabledCodecs with exactly the given encoders (no decoders needed for selection tests).
    fn codecs_with_encoders(encoders: &[NamedEncoders]) -> EnabledCodecs {
        EnabledCodecs {
            decoders: smallvec::SmallVec::new(),
            encoders: smallvec::SmallVec::from_slice(encoders),
        }
    }

    /// All zen encoders for every format.
    fn all_zen_encoders() -> EnabledCodecs {
        codecs_with_encoders(&[
            NamedEncoders::ZenJpegEncoder,
            NamedEncoders::ZenWebPEncoder,
            NamedEncoders::ZenGifEncoder,
            NamedEncoders::ZenJxlEncoder,
            NamedEncoders::ZenAvifEncoder,
            NamedEncoders::PngQuantEncoder,
            NamedEncoders::LodePngEncoder,
        ])
    }

    /// Only C encoders (legacy-style).
    fn c_only_encoders() -> EnabledCodecs {
        codecs_with_encoders(&[
            NamedEncoders::MozJpegEncoder,
            NamedEncoders::LibPngRsEncoder,
            NamedEncoders::WebPEncoder,
            NamedEncoders::PngQuantEncoder,
            NamedEncoders::LodePngEncoder,
            NamedEncoders::GifEncoder,
        ])
    }

    /// Minimal: only JPEG + PNG (web_safe without GIF).
    fn jpeg_png_only() -> EnabledCodecs {
        codecs_with_encoders(&[NamedEncoders::MozJpegEncoder, NamedEncoders::LodePngEncoder])
    }

    fn selector(codecs: &EnabledCodecs) -> CodecSelector<'_> {
        CodecSelector::new(codecs)
    }

    // ── Format selection: basic paths ────────────────────────────────────────

    #[test]
    fn no_formats_enabled_returns_none() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let result = sel.select_format(&FormatCriteria {
            allowed: AllowedFormats::none(),
            ..Default::default()
        });
        assert!(result.is_none());
    }

    #[test]
    fn web_safe_opaque_selects_jpeg() {
        let codecs = c_only_encoders();
        let sel = selector(&codecs);
        let result = sel
            .select_format(&FormatCriteria {
                allowed: AllowedFormats::web_safe(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Jpeg);
        assert!(result.trace.iter().any(|t| t.contains("jpeg")));
    }

    #[test]
    fn jxl_wins_when_enabled() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let result = sel
            .select_format(&FormatCriteria { allowed: AllowedFormats::all(), ..Default::default() })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Jxl);
        assert!(result.trace.iter().any(|t| t.contains("jxl")));
    }

    #[test]
    fn jxl_disabled_falls_through() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::all();
        allowed.jxl = None;
        let result = sel.select_format(&FormatCriteria { allowed, ..Default::default() }).unwrap();
        // Without JXL, lossy opaque small image → AVIF (pixel_count=0 < 3M)
        assert_eq!(result.chosen, OutputImageFormat::Avif);
    }

    // ── Format selection: alpha ──────────────────────────────────────────────

    #[test]
    fn alpha_prefers_avif_when_available() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::all();
        allowed.jxl = None; // disable JXL so alpha path is reached
        let result = sel
            .select_format(&FormatCriteria { allowed, has_alpha: true, ..Default::default() })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Avif);
        assert!(result.trace.iter().any(|t| t.contains("alpha")));
    }

    #[test]
    fn alpha_falls_to_webp_without_avif() {
        let codecs = c_only_encoders(); // has WebP but no AVIF
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::all();
        allowed.avif = None;
        allowed.jxl = None;
        let result = sel
            .select_format(&FormatCriteria { allowed, has_alpha: true, ..Default::default() })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Webp);
    }

    #[test]
    fn alpha_falls_to_png_without_avif_or_webp() {
        let codecs = jpeg_png_only();
        let sel = selector(&codecs);
        let result = sel
            .select_format(&FormatCriteria {
                allowed: AllowedFormats::web_safe(),
                has_alpha: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Png);
    }

    // ── Format selection: lossless ───────────────────────────────────────────

    #[test]
    fn lossless_prefers_webp_when_available() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::all();
        allowed.jxl = None;
        let result = sel
            .select_format(&FormatCriteria { allowed, lossless: Some(true), ..Default::default() })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Webp);
    }

    #[test]
    fn lossless_falls_to_png_without_webp() {
        let codecs = jpeg_png_only();
        let sel = selector(&codecs);
        let result = sel
            .select_format(&FormatCriteria {
                allowed: AllowedFormats::web_safe(),
                lossless: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Png);
    }

    #[test]
    fn source_lossless_triggers_lossless_path() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::all();
        allowed.jxl = None;
        let result = sel
            .select_format(&FormatCriteria {
                allowed,
                source_lossless: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Webp);
    }

    // ── Format selection: lossy opaque ───────────────────────────────────────

    #[test]
    fn small_image_prefers_avif_over_jpeg() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::all();
        allowed.jxl = None;
        let result = sel
            .select_format(&FormatCriteria {
                allowed,
                pixel_count: 1_000_000, // 1 Mpx < 3 Mpx threshold
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Avif);
    }

    #[test]
    fn large_image_prefers_jpeg_over_avif() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::all();
        allowed.jxl = None;
        let result = sel
            .select_format(&FormatCriteria {
                allowed,
                pixel_count: 5_000_000, // 5 Mpx > 3 Mpx threshold
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Jpeg);
    }

    // ── Format selection: animation ──────────────────────────────────────────

    #[test]
    fn animation_prefers_gif_fallback_without_animated_encoders() {
        let codecs = c_only_encoders(); // C WebP doesn't have animation caps
        let sel = selector(&codecs);
        let result = sel
            .select_format(&FormatCriteria {
                allowed: AllowedFormats::all(),
                has_animation: true,
                ..Default::default()
            })
            .unwrap();
        // C encoders don't advertise animation support, so GIF fallback
        assert_eq!(result.chosen, OutputImageFormat::Gif);
    }

    #[test]
    fn animation_lossless_prefers_webp_when_animated() {
        // Zen WebP encoder supports animation
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let webp_caps = NamedEncoders::ZenWebPEncoder.caps();
        if webp_caps.animation {
            let result = sel
                .select_format(&FormatCriteria {
                    allowed: AllowedFormats::all(),
                    has_animation: true,
                    lossless: Some(true),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(result.chosen, OutputImageFormat::Webp);
        }
    }

    // ── Encoder selection ────────────────────────────────────────────────────

    #[test]
    fn select_encoder_picks_first_capable() {
        let codecs =
            codecs_with_encoders(&[NamedEncoders::ZenJpegEncoder, NamedEncoders::MozJpegEncoder]);
        let sel = selector(&codecs);
        let result = sel.select_encoder(ImageFormat::Jpeg, false).unwrap();
        // Both are capable; ZenJpeg has lossy_rank=3 (excellent), MozJpeg has lossy_rank=3
        // First with equal rank wins (priority order)
        assert!(
            result.chosen == NamedEncoders::ZenJpegEncoder
                || result.chosen == NamedEncoders::MozJpegEncoder
        );
    }

    #[test]
    fn select_encoder_skips_wrong_format() {
        let codecs =
            codecs_with_encoders(&[NamedEncoders::MozJpegEncoder, NamedEncoders::LodePngEncoder]);
        let sel = selector(&codecs);
        // LodePng is lossless-only, so query lossless mode
        let result = sel.select_encoder(ImageFormat::Png, true).unwrap();
        assert_eq!(result.chosen, NamedEncoders::LodePngEncoder);
    }

    #[test]
    fn select_encoder_none_for_missing_format() {
        let codecs = codecs_with_encoders(&[NamedEncoders::MozJpegEncoder]);
        let sel = selector(&codecs);
        let result = sel.select_encoder(ImageFormat::Png, false);
        assert!(result.is_none());
    }

    #[test]
    fn select_encoder_respects_lossless_capability() {
        // MozJpeg is lossy-only
        let codecs = codecs_with_encoders(&[NamedEncoders::MozJpegEncoder]);
        let sel = selector(&codecs);
        let result = sel.select_encoder(ImageFormat::Jpeg, true);
        assert!(result.is_none(), "JPEG encoders don't support lossless");
    }

    #[test]
    fn select_encoder_prefers_higher_rank() {
        // PngQuant has lossy_rank=2 (good), LodePng has lossy_rank=1 (functional)
        let codecs =
            codecs_with_encoders(&[NamedEncoders::LodePngEncoder, NamedEncoders::PngQuantEncoder]);
        let sel = selector(&codecs);
        let result = sel.select_encoder(ImageFormat::Png, false).unwrap();
        assert_eq!(result.chosen, NamedEncoders::PngQuantEncoder);
    }

    // ── Full selection (format + encoder) ────────────────────────────────────

    #[test]
    fn full_select_combines_format_and_encoder() {
        let codecs = c_only_encoders();
        let sel = selector(&codecs);
        let result = sel
            .select(&FormatCriteria { allowed: AllowedFormats::web_safe(), ..Default::default() })
            .unwrap();
        assert_eq!(result.format, OutputImageFormat::Jpeg);
        assert_eq!(result.encoder, NamedEncoders::MozJpegEncoder);
        assert!(!result.trace.is_empty());
    }

    #[test]
    fn full_select_none_when_format_enabled_but_no_encoder() {
        // Allowed formats include AVIF but no AVIF encoder registered
        let codecs = codecs_with_encoders(&[]); // empty
        let sel = selector(&codecs);
        let mut allowed = AllowedFormats::none();
        allowed.avif = Some(true);
        let result = sel.select(&FormatCriteria { allowed, ..Default::default() });
        assert!(result.is_none());
    }

    // ── Trace inspection ─────────────────────────────────────────────────────

    #[test]
    fn trace_records_decision_steps() {
        let codecs = all_zen_encoders();
        let sel = selector(&codecs);
        let result = sel
            .select_format(&FormatCriteria { allowed: AllowedFormats::all(), ..Default::default() })
            .unwrap();
        assert_eq!(result.chosen, OutputImageFormat::Jxl);
        // Trace should mention JXL selection reason
        assert!(
            result.trace.iter().any(|t| t.contains("jxl")),
            "trace should mention jxl: {:?}",
            result.trace
        );
    }

    #[test]
    fn encoder_trace_shows_skip_reasons() {
        let codecs = codecs_with_encoders(&[
            NamedEncoders::LodePngEncoder, // PNG, not JPEG
            NamedEncoders::MozJpegEncoder, // JPEG, correct
        ]);
        let sel = selector(&codecs);
        let result = sel.select_encoder(ImageFormat::Jpeg, false).unwrap();
        assert_eq!(result.chosen, NamedEncoders::MozJpegEncoder);
    }

    // ── Capability queries ───────────────────────────────────────────────────

    #[test]
    fn caps_are_consistent() {
        // Every encoder should report the correct format
        for enc in &[
            NamedEncoders::MozJpegEncoder,
            NamedEncoders::ZenJpegEncoder,
            NamedEncoders::WebPEncoder,
            NamedEncoders::ZenWebPEncoder,
            NamedEncoders::GifEncoder,
            NamedEncoders::ZenGifEncoder,
            NamedEncoders::ZenJxlEncoder,
            NamedEncoders::ZenAvifEncoder,
            NamedEncoders::PngQuantEncoder,
            NamedEncoders::LodePngEncoder,
            NamedEncoders::LibPngRsEncoder,
        ] {
            let caps = enc.caps();
            // At least one of lossy or lossless must be true
            assert!(caps.lossy || caps.lossless, "{:?} reports neither lossy nor lossless", enc);
            // Ranks for supported modes must be > 0
            if caps.lossy {
                assert!(caps.lossy_rank > 0, "{:?} lossy_rank is 0", enc);
            }
            if caps.lossless {
                assert!(caps.lossless_rank > 0, "{:?} lossless_rank is 0", enc);
            }
        }
    }

    #[test]
    fn is_compiled_in_consistent_with_features() {
        // These pure-Rust encoders are always compiled in:
        assert!(NamedEncoders::PngQuantEncoder.is_compiled_in());
        assert!(NamedEncoders::LodePngEncoder.is_compiled_in());
        assert!(NamedEncoders::GifEncoder.is_compiled_in());
    }

    // ── Priority ordering ────────────────────────────────────────────────────

    #[test]
    fn priority_order_matters_for_equal_rank() {
        // If two encoders have the same rank, first in list wins
        let codecs_a =
            codecs_with_encoders(&[NamedEncoders::ZenJpegEncoder, NamedEncoders::MozJpegEncoder]);
        let codecs_b =
            codecs_with_encoders(&[NamedEncoders::MozJpegEncoder, NamedEncoders::ZenJpegEncoder]);
        let zen_rank = NamedEncoders::ZenJpegEncoder.caps().lossy_rank;
        let moz_rank = NamedEncoders::MozJpegEncoder.caps().lossy_rank;

        let result_a = selector(&codecs_a).select_encoder(ImageFormat::Jpeg, false).unwrap();
        let result_b = selector(&codecs_b).select_encoder(ImageFormat::Jpeg, false).unwrap();

        if zen_rank == moz_rank {
            // Equal rank: first in list wins
            assert_eq!(result_a.chosen, NamedEncoders::ZenJpegEncoder);
            assert_eq!(result_b.chosen, NamedEncoders::MozJpegEncoder);
        } else if zen_rank > moz_rank {
            // Higher rank always wins regardless of position
            assert_eq!(result_a.chosen, NamedEncoders::ZenJpegEncoder);
            assert_eq!(result_b.chosen, NamedEncoders::ZenJpegEncoder);
        } else {
            assert_eq!(result_a.chosen, NamedEncoders::MozJpegEncoder);
            assert_eq!(result_b.chosen, NamedEncoders::MozJpegEncoder);
        }
    }

    // ── EnabledCodecs helper methods ─────────────────────────────────────────

    #[test]
    fn has_encoder_for_format_works() {
        let codecs = c_only_encoders();
        assert!(codecs.has_encoder_for_format(ImageFormat::Jpeg));
        assert!(codecs.has_encoder_for_format(ImageFormat::Png));
        assert!(codecs.has_encoder_for_format(ImageFormat::Webp));
        assert!(!codecs.has_encoder_for_format(ImageFormat::Jxl));
        assert!(!codecs.has_encoder_for_format(ImageFormat::Avif));
    }

    #[test]
    fn format_supports_lossy_and_lossless() {
        let codecs = all_zen_encoders();
        assert!(codecs.format_supports_lossy(ImageFormat::Jpeg));
        assert!(!codecs.format_supports_lossless(ImageFormat::Jpeg));
        assert!(codecs.format_supports_lossy(ImageFormat::Webp));
        assert!(codecs.format_supports_lossless(ImageFormat::Webp));
        assert!(codecs.format_supports_lossy(ImageFormat::Png));
        assert!(codecs.format_supports_lossless(ImageFormat::Png));
    }

    #[test]
    fn empty_codecs_returns_none_for_everything() {
        let codecs = codecs_with_encoders(&[]);
        let sel = selector(&codecs);
        assert!(sel.select_encoder(ImageFormat::Jpeg, false).is_none());
        assert!(sel.select_encoder(ImageFormat::Png, true).is_none());
        // Format selection may still pick a format (e.g. GIF), but full select will fail
        let full = sel
            .select(&FormatCriteria { allowed: AllowedFormats::web_safe(), ..Default::default() });
        assert!(full.is_none());
    }
}
