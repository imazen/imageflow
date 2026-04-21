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
pub(crate) mod substitution_measurements;
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

// Zen codec adapters (zencodec dyn dispatch)
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
    ZenPngDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenAvifDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenJxlDecoder,
    #[cfg(feature = "zen-codecs")]
    ZenBmpDecoder,
}
impl NamedDecoders {
    /// Wire-side `NamedDecoderName` (used by the codec killbits grid).
    /// Symmetric counterpart to `NamedDecoderName::image_format()`.
    pub fn wire_name(&self) -> imageflow_types::NamedDecoderName {
        use imageflow_types::NamedDecoderName as N;
        match self {
            #[cfg(feature = "c-codecs")]
            NamedDecoders::MozJpegRsDecoder => N::MozjpegRsDecoder,
            #[cfg(feature = "c-codecs")]
            NamedDecoders::ImageRsJpegDecoder => N::ImageRsJpegDecoder,
            NamedDecoders::ImageRsPngDecoder => N::ImageRsPngDecoder,
            #[cfg(feature = "c-codecs")]
            NamedDecoders::LibPngRsDecoder => N::LibpngDecoder,
            NamedDecoders::GifRsDecoder => N::GifRsDecoder,
            #[cfg(feature = "c-codecs")]
            NamedDecoders::WebPDecoder => N::WebpDecoder,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJpegDecoder => N::ZenJpegDecoder,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenWebPDecoder => N::ZenWebpDecoder,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenGifDecoder => N::ZenGifDecoder,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenPngDecoder => N::ZenPngDecoder,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenAvifDecoder => N::ZenAvifDecoder,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJxlDecoder => N::ZenJxlDecoder,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenBmpDecoder => N::ZenBmpDecoder,
        }
    }

    /// The killbits `ImageFormat` this decoder handles.
    pub fn image_format(&self) -> imageflow_types::ImageFormat {
        use imageflow_types::ImageFormat;
        match self {
            #[cfg(feature = "c-codecs")]
            NamedDecoders::MozJpegRsDecoder => ImageFormat::Jpeg,
            #[cfg(feature = "c-codecs")]
            NamedDecoders::ImageRsJpegDecoder => ImageFormat::Jpeg,
            NamedDecoders::ImageRsPngDecoder => ImageFormat::Png,
            #[cfg(feature = "c-codecs")]
            NamedDecoders::LibPngRsDecoder => ImageFormat::Png,
            NamedDecoders::GifRsDecoder => ImageFormat::Gif,
            #[cfg(feature = "c-codecs")]
            NamedDecoders::WebPDecoder => ImageFormat::Webp,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJpegDecoder => ImageFormat::Jpeg,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenWebPDecoder => ImageFormat::Webp,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenGifDecoder => ImageFormat::Gif,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenPngDecoder => ImageFormat::Png,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenAvifDecoder => ImageFormat::Avif,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJxlDecoder => ImageFormat::Jxl,
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenBmpDecoder => ImageFormat::Bmp,
        }
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
            NamedDecoders::ZenPngDecoder => bytes.starts_with(b"\x89\x50\x4E\x47\x0D\x0A\x1A\x0A"),
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenAvifDecoder => {
                bytes.len() >= 12
                    && &bytes[4..8] == b"ftyp"
                    && (bytes[8..12].starts_with(b"avif")
                        || bytes[8..12].starts_with(b"avis")
                        || bytes[8..12].starts_with(b"mif1"))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJxlDecoder => {
                // JXL bare codestream: FF 0A; container: 00 00 00 0C 4A 58 4C 20 0D 0A 87 0A
                bytes.starts_with(&[0xFF, 0x0A])
                    || (bytes.len() >= 12
                        && bytes.starts_with(&[0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20]))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenBmpDecoder => bytes.starts_with(b"BM"),
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
            NamedDecoders::ZenPngDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_png(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenAvifDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_avif(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenJxlDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_jxl(c, io, io_id)?))
            }
            #[cfg(feature = "zen-codecs")]
            NamedDecoders::ZenBmpDecoder => {
                Ok(Box::new(zen_decoder::ZenDecoder::create_bmp(c, io, io_id)?))
            }
        }
    }
}
#[derive(PartialEq, Copy, Clone, Debug)]
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
    ZenPngEncoder,
    /// ZenPng + zenquant (the default zenpng quantizer). Distinct from
    /// [`ZenPngEncoder`] so the substitution table can order this
    /// palette-reducing variant explicitly ahead of imagequant and
    /// pngquant. Gated on `zen-codecs` (zenpng's default `quantize`
    /// feature is enabled via the zenpng dep in Cargo.toml).
    #[cfg(feature = "zen-codecs")]
    ZenPngZenquantEncoder,
    /// ZenPng + imagequant. Shares the pngquant quantization kernel
    /// with `PngQuantEncoder` but routes through zenpng's encoder
    /// pipeline. Requires zenpng's `imagequant` feature; not enabled in
    /// the default imageflow build. Variant exists so the
    /// substitution-priority table is complete; `create_png_imagequant`
    /// returns a "not yet plumbed" error until the zenpng feature is
    /// turned on in imageflow's Cargo.toml.
    #[cfg(feature = "zen-codecs")]
    ZenPngImagequantEncoder,
    #[cfg(feature = "zen-codecs")]
    ZenAvifEncoder,
    #[cfg(feature = "zen-codecs")]
    ZenJxlEncoder,
    #[cfg(feature = "zen-codecs")]
    ZenBmpEncoder,
    #[cfg(feature = "zen-codecs")]
    MozjpegRsEncoder,
}

impl NamedEncoders {
    /// Wire-side `NamedEncoderName` (used by the codec killbits grid).
    pub fn wire_name(&self) -> imageflow_types::NamedEncoderName {
        use imageflow_types::NamedEncoderName as N;
        match self {
            NamedEncoders::GifEncoder => N::GifEncoder,
            #[cfg(feature = "c-codecs")]
            NamedEncoders::MozJpegEncoder => N::MozjpegEncoder,
            NamedEncoders::PngQuantEncoder => N::PngquantEncoder,
            NamedEncoders::LodePngEncoder => N::LodepngEncoder,
            #[cfg(feature = "c-codecs")]
            NamedEncoders::WebPEncoder => N::WebpEncoder,
            #[cfg(feature = "c-codecs")]
            NamedEncoders::LibPngRsEncoder => N::LibpngEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenJpegEncoder => N::ZenJpegEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenWebPEncoder => N::ZenWebpEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenGifEncoder => N::ZenGifEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenPngEncoder => N::ZenPngEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenPngZenquantEncoder => N::ZenPngZenquantEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenPngImagequantEncoder => N::ZenPngImagequantEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenAvifEncoder => N::ZenAvifEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenJxlEncoder => N::ZenJxlEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenBmpEncoder => N::ZenBmpEncoder,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::MozjpegRsEncoder => N::MozjpegRsEncoder,
        }
    }

    /// Returns true if this encoder writes JPEG.
    pub fn is_jpeg(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            NamedEncoders::MozJpegEncoder => true,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenJpegEncoder | NamedEncoders::MozjpegRsEncoder => true,
            _ => false,
        }
    }
    /// Returns true if this encoder writes PNG.
    pub fn is_png(&self) -> bool {
        match self {
            NamedEncoders::PngQuantEncoder | NamedEncoders::LodePngEncoder => true,
            #[cfg(feature = "c-codecs")]
            NamedEncoders::LibPngRsEncoder => true,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenPngEncoder
            | NamedEncoders::ZenPngZenquantEncoder
            | NamedEncoders::ZenPngImagequantEncoder => true,
            _ => false,
        }
    }
    /// Returns true if this encoder writes WebP.
    pub fn is_webp(&self) -> bool {
        match self {
            #[cfg(feature = "c-codecs")]
            NamedEncoders::WebPEncoder => true,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenWebPEncoder => true,
            _ => false,
        }
    }
    /// Returns true if this encoder writes GIF.
    pub fn is_gif(&self) -> bool {
        match self {
            NamedEncoders::GifEncoder => true,
            #[cfg(feature = "zen-codecs")]
            NamedEncoders::ZenGifEncoder => true,
            _ => false,
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
                // JPEG: C mozjpeg preferred when available (stable baseline)
                #[cfg(feature = "c-codecs")]
                NamedDecoders::MozJpegRsDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJpegDecoder,
                // PNG: libpng (C) first, else zenpng, else image-rs
                #[cfg(feature = "c-codecs")]
                NamedDecoders::LibPngRsDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenPngDecoder,
                #[cfg(all(not(feature = "zen-codecs"), not(feature = "c-codecs")))]
                NamedDecoders::ImageRsPngDecoder,
                // WebP: libwebp (C) first, else zenwebp
                #[cfg(feature = "c-codecs")]
                NamedDecoders::WebPDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenWebPDecoder,
                // GIF: gif-rs baseline, zen as alternative
                NamedDecoders::GifRsDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenGifDecoder,
                // Zen-only formats
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenAvifDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenJxlDecoder,
                #[cfg(feature = "zen-codecs")]
                NamedDecoders::ZenBmpDecoder,
            ]),
            encoders: smallvec::SmallVec::from_slice(&[
                // GIF: use built-in gif crate first (zen as alternative)
                NamedEncoders::GifEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenGifEncoder,
                // JPEG: C mozjpeg preferred when available (stable output)
                #[cfg(feature = "c-codecs")]
                NamedEncoders::MozJpegEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJpegEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::MozjpegRsEncoder,
                // WebP: C libwebp preferred when available
                #[cfg(feature = "c-codecs")]
                NamedEncoders::WebPEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenWebPEncoder,
                // PNG: pngquant/lodepng baseline, libpng (C) or zenpng as alternatives
                NamedEncoders::PngQuantEncoder,
                NamedEncoders::LodePngEncoder,
                #[cfg(feature = "c-codecs")]
                NamedEncoders::LibPngRsEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenPngEncoder,
                // ZenPng palette-reducing variants: zenquant (default)
                // preferred over imagequant (requires zenpng feature
                // toggle). Ordered after the truecolor ZenPngEncoder so
                // the priority-indexed table leads with whichever the
                // requested preset actually needs.
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenPngZenquantEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenPngImagequantEncoder,
                // Zen-only formats
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenAvifEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenJxlEncoder,
                #[cfg(feature = "zen-codecs")]
                NamedEncoders::ZenBmpEncoder,
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
    pub fn prefer_encoder(&mut self, encoder: NamedEncoders) {
        self.encoders.retain(|item| item != &encoder);
        self.encoders.insert(0, encoder);
    }
    pub fn disable_encoder(&mut self, encoder: NamedEncoders) {
        self.encoders.retain(|item| item != &encoder);
    }
    /// Find the first enabled encoder matching a predicate on NamedEncoders.
    pub fn first_encoder<F>(&self, pred: F) -> Option<NamedEncoders>
    where
        F: Fn(NamedEncoders) -> bool,
    {
        self.encoders.iter().copied().find(|&e| pred(e))
    }

    /// Find a preferred encoder if enabled, else fall back to the first
    /// enabled encoder matching a predicate.
    pub fn preferred_or_first<F>(&self, preferred: NamedEncoders, pred: F) -> Option<NamedEncoders>
    where
        F: Fn(NamedEncoders) -> bool,
    {
        if self.encoders.contains(&preferred) {
            Some(preferred)
        } else {
            self.first_encoder(pred)
        }
    }
    pub fn create_decoder_for_magic_bytes(
        &self,
        bytes: &[u8],
        c: &Context,
        io: IoProxy,
        io_id: i32,
    ) -> Result<Box<dyn Decoder>> {
        let trusted = c.trusted_policy.as_deref();
        let active = c.active_job_security.as_deref();
        for &decoder in self.decoders.iter() {
            if decoder.works_for_magic_bytes(bytes) {
                let format = decoder.image_format();
                let grid = c.net_support(None);
                crate::killbits::enforce(
                    grid.grid(),
                    imageflow_types::KillbitsOp::Decode,
                    format,
                )?;
                // Codec-level kill: if this particular decoder is
                // denied, skip it and try the next one matching the
                // magic bytes. Only error out with
                // `codec_not_available` when no live decoder remains.
                let wire = decoder.wire_name();
                if !crate::killbits::codec_decoder_allowed(wire, trusted, active) {
                    continue;
                }
                return decoder.create(c, io, io_id);
            }
        }
        // Distinguish "no decoder handled magic bytes" from "every
        // matching decoder was killed by codec killbits".
        for &decoder in self.decoders.iter() {
            if decoder.works_for_magic_bytes(bytes) {
                let format = decoder.image_format();
                // All matching decoders for this format were denied.
                return Err(nerror!(
                    ErrorKind::CodecDisabledError,
                    "{{\"error\": \"codec_not_available\", \"format\": \"{}\", \"reasons\": [\"no_available_decoder\"]}}",
                    format.as_snake()
                ));
            }
        }
        Err(nerror!(
            ErrorKind::NoEnabledDecoderFound,
            "No ENABLED decoder found for file starting in {:X?}",
            bytes
        ))
    }
}

/// Merge a pending [`s::EncodeAnnotations`] into an `Option` slot,
/// prioritizing whatever is already set on the result. Used by
/// `CodecInstanceContainer::write_frame` to attach substitution
/// notices without clobbering annotations the encoder itself produced
/// (today none do, but the merge keeps future annotation channels
/// compositional).
fn merge_annotations(
    target: &mut Option<s::EncodeAnnotations>,
    incoming: s::EncodeAnnotations,
) {
    match target {
        Some(existing) => {
            // Per-encoder substitution wins over any pre-existing
            // substitution on the encoder-produced slot — the incoming
            // annotation comes from the dispatcher, which has the
            // authoritative view of which codec actually ran.
            if incoming.codec_substitution.is_some() {
                existing.codec_substitution = incoming.codec_substitution;
            }
        }
        None => {
            if !incoming.is_empty() {
                *target = Some(incoming);
            }
        }
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
    /// Annotation captured at encoder-creation time when the dispatcher
    /// substituted a different codec than the preset asked for. Merged
    /// into the [`s::EncodeResult`] returned by the first
    /// `write_frame` call so the response carries the substitution
    /// notice. Cleared (`take`d) after attachment.
    ///
    /// Boxed so the idle container footprint is `Option<Box>` (8 bytes
    /// on 64-bit) rather than a full annotation struct. The
    /// `calculate_heap_allocations` counter treats the box allocation
    /// as a lazy-init rather than a fixed cost.
    pending_encode_annotation: Option<Box<s::EncodeAnnotations>>,
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
                pending_encode_annotation: None,
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
                pending_encode_annotation: None,
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
            let created = auto::create_encoder(c, io, preset, bitmap_key, decoder_io_ids)
                .map_err(|e| e.at(here!()))?;

            // Stash any substitution annotation so the first successful
            // write_frame can merge it into the EncodeResult.
            self.pending_encode_annotation = created.annotation.map(Box::new);
            self.codec = CodecKind::Encoder(created.inner);
        };

        match self.codec {
            CodecKind::Encoder(ref mut e) => {
                match e
                    .write_frame(c, preset, bitmap_key, decoder_io_ids)
                    .map_err(|e| e.at(here!()))
                {
                    Err(e) => Err(e),
                    Ok(mut result) => match result.bytes {
                        s::ResultBytes::Elsewhere => {
                            // Merge the substitution annotation into the
                            // first frame's result. Subsequent frames
                            // (animation) carry no annotation — the
                            // substitution is a per-encoder property,
                            // not per-frame.
                            if let Some(pending) = self.pending_encode_annotation.take() {
                                merge_annotations(&mut result.annotations, *pending);
                            }
                            Ok(result)
                        }
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
