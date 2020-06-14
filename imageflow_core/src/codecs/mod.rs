use std;
use std::sync::*;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError, Result, JsonResponse, ErrorKind, FlowError, ErrorCategory};
use crate::ffi::BitmapBgra;
use crate::ffi::DecoderColorInfo;
use crate::ffi::ColorProfileSource;
use imageflow_types::collections::AddRemoveSet;
use crate::io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;
use std::borrow::BorrowMut;
use std::ops::DerefMut;
use std::any::Any;
use lcms2::*;
use lcms2;
mod gif;
mod pngquant;
mod lode;
mod mozjpeg;
mod mozjpeg_decoder;
mod libpng_decoder;
mod libpng_encoder;
mod mozjpeg_decoder_helpers;
mod jpeg_decoder;

mod webp;
mod color_transform_cache;
use crate::io::IoProxyRef;
use crate::codecs::color_transform_cache::ColorTransformCache;
use crate::codecs::NamedEncoders::LibPngRsEncoder;

pub trait DecoderFactory{
    fn create(c: &Context, io: &mut IoProxy, io_id: i32) -> Option<Result<Box<dyn Decoder>>>;
}
pub trait Decoder : Any{
    fn initialize(&mut self, c: &Context) -> Result<()>;
    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo>;
    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>>;
    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()>;
    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra>;
    fn has_more_frames(&mut self) -> Result<bool>;
    fn as_any(&self) -> &dyn Any;
}
pub trait Encoder{
    // GIF encoder will need to know if transparency is required (we could guess based on first input frame)
    // If not required, we can do frame shrinking and delta encoding. Otherwise we have to
    // encode entire frames and enable transparency (default)
    fn write_frame(&mut self, c: &Context, preset: &s::EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<s::EncodeResult>;

    fn get_io(&self) -> Result<IoProxyRef>;
}




enum CodecKind{
    EncoderPlaceholder,
    Encoder(Box<dyn Encoder>),
    Decoder(Box<dyn Decoder>)
}

#[derive(PartialEq, Copy, Clone)]
pub enum NamedDecoders{
    MozJpegRsDecoder,
    WICJpegDecoder,
    ImageRsJpegDecoder,
    LibPngRsDecoder,
    GifRsDecoder,
    WebPDecoder,
}
impl NamedDecoders{
    pub fn works_for_magic_bytes(&self, bytes: &[u8]) -> bool{
        match self{
            NamedDecoders::WICJpegDecoder | NamedDecoders::ImageRsJpegDecoder| NamedDecoders::MozJpegRsDecoder => {
                bytes.starts_with(b"\xFF\xD8\xFF")
            },
            NamedDecoders::GifRsDecoder => {
                bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a")
            },
            NamedDecoders::LibPngRsDecoder => {
                bytes.starts_with( b"\x89\x50\x4E\x47\x0D\x0A\x1A\x0A")
            },
            NamedDecoders::WebPDecoder => {
                bytes.starts_with(b"RIFF") && bytes[8..12].starts_with(b"WEBP")
            }
        }
    }

    pub fn create(&self, c: &Context, io: IoProxy, io_id: i32) -> Result<Box<dyn Decoder>>{
        match self{
            NamedDecoders::MozJpegRsDecoder => Ok(Box::new(mozjpeg_decoder::MozJpegDecoder::create(c, io, io_id)?)),
            NamedDecoders::LibPngRsDecoder => Ok(Box::new(libpng_decoder::LibPngDecoder::create(c, io, io_id)?)),
            NamedDecoders::GifRsDecoder => Ok(Box::new(gif::GifDecoder::create(c, io, io_id)?)),
            NamedDecoders::ImageRsJpegDecoder => Ok(Box::new(jpeg_decoder::JpegDecoder::create(c, io, io_id)?)),
            NamedDecoders::WebPDecoder => Ok(Box::new(webp::WebPDecoder::create(c, io, io_id)?)),
            NamedDecoders::WICJpegDecoder => {
                panic!("WIC Jpeg Decoder not implemented"); //TODO, use actual error for this
            }
        }
    }

}
#[derive(PartialEq, Copy, Clone)]
pub enum NamedEncoders{
    GifEncoder,
    MozJpegEncoder,
    PngQuantEncoder,
    LodePngEncoder,
    WebPEncoder,
    LibPngRsEncoder,
}
pub struct EnabledCodecs{
    pub decoders: ::smallvec::SmallVec<[NamedDecoders;4]>,
    pub encoders: ::smallvec::SmallVec<[NamedEncoders;8]>,
}
impl Default for EnabledCodecs {
    fn default() -> Self {
        EnabledCodecs{
            decoders: smallvec::SmallVec::from_slice(
                &[NamedDecoders::MozJpegRsDecoder,
                    NamedDecoders::LibPngRsDecoder,
                    NamedDecoders::GifRsDecoder,
                    NamedDecoders::WebPDecoder]),
            encoders: smallvec::SmallVec::from_slice(
                &[NamedEncoders::GifEncoder,
                    NamedEncoders::MozJpegEncoder,
                    NamedEncoders::PngQuantEncoder,
                    NamedEncoders::LodePngEncoder,
                    NamedEncoders::WebPEncoder,
                    NamedEncoders::LibPngRsEncoder
                ])
        }
    }
}

impl EnabledCodecs{
    pub fn prefer_decoder(&mut self, decoder: NamedDecoders){
        self.decoders.retain( |item| item != &decoder);
        self.decoders.insert(0, decoder);
    }
    pub fn disable_decoder(&mut self, decoder: NamedDecoders){
        self.decoders.retain( |item| item != &decoder);
    }
    pub fn create_decoder_for_magic_bytes(&self, bytes: &[u8], c: &Context, io: IoProxy, io_id: i32) -> Result<Box<dyn Decoder>>{
        for &decoder in self.decoders.iter(){
            if decoder.works_for_magic_bytes(bytes){
                return decoder.create(c, io, io_id);
            }
        }
        return Err(nerror!(ErrorKind::NoEnabledDecoderFound,  "No ENABLED decoder found for file starting in {:X?}", bytes))
    }
}

// We need a rust-friendly codec instance, codec definition, and a way to wrap C codecs
pub struct CodecInstanceContainer{
    pub io_id: i32,
    codec: CodecKind,
    encode_io: Option<IoProxy>
}

impl CodecInstanceContainer {

    pub fn get_decoder(&mut self) -> Result<&mut dyn Decoder>{
        if let CodecKind::Decoder(ref mut d) = self.codec{
            Ok(&mut **d)
        }else{
            Err(nerror!(ErrorKind::InvalidArgument, "Not a decoder"))
        }

    }

    pub fn create(c: &Context, mut io: IoProxy, io_id: i32, direction: IoDirection) -> Result<CodecInstanceContainer>{
        if direction == IoDirection::Out {
            Ok(CodecInstanceContainer
                {
                    io_id,
                    codec: CodecKind::EncoderPlaceholder,
                    encode_io: Some(io),
                })
        }else {
            let mut buffer = [0u8; 12];
            let result = io.read(&mut buffer)
                .map_err(|e|  FlowError::from_decoder(e).at(here!()))?;

            io.seek( io::SeekFrom::Start(0))
                .map_err(|e|  FlowError::from_decoder(e).at(here!()))?;


            Ok(CodecInstanceContainer
            {
                io_id,
                codec: CodecKind::Decoder(c.enabled_codecs.create_decoder_for_magic_bytes(&buffer, c, io, io_id)?),
                encode_io: None
            })

        }
    }

}


impl CodecInstanceContainer{

     pub fn write_frame(&mut self, c: &Context, preset: &s::EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<s::EncodeResult>{

         // Pick encoder
         if let CodecKind::EncoderPlaceholder = self.codec {

             let io = self.encode_io.take().unwrap();

             let codec = match *preset {
                 s::EncoderPreset::Gif => {
                     //TODO: enforce killbits - if c.enabled_codecs.encoders.contains()
                     CodecKind::Encoder(Box::new(gif::GifEncoder::create(c, preset, io, frame)?))
                 },
                 s::EncoderPreset::Pngquant {speed, quality , minimum_quality, maximum_deflate} => {
                     CodecKind::Encoder(Box::new(pngquant::PngquantEncoder::create(c, speed, quality, minimum_quality, maximum_deflate, io)?))
                 },
                 s::EncoderPreset::Mozjpeg {quality, progressive} => {
                     CodecKind::Encoder(Box::new(mozjpeg::MozjpegEncoder::create(c, quality, progressive, io)?))
                 },
                 s::EncoderPreset::LibjpegTurbo {quality, progressive, optimize_huffman_coding} => {
                     CodecKind::Encoder(Box::new(mozjpeg::MozjpegEncoder::create_classic(c, quality.map(|q| q as u8), progressive, optimize_huffman_coding, io)?))
                 },
                 s::EncoderPreset::Lodepng { maximum_deflate }=> {
                     CodecKind::Encoder(Box::new(lode::LodepngEncoder::create(c, io, maximum_deflate)?))
                 },
                 s::EncoderPreset::Libpng {..}  => {
                     CodecKind::Encoder(Box::new(
                         libpng_encoder::LibPngEncoder::create(c, io)?))
                 },
                 s::EncoderPreset::WebPLossless => CodecKind::Encoder(Box::new(webp::WebPEncoder::create(c, io)?)),
                 s::EncoderPreset::WebPLossy {quality}=> CodecKind::Encoder(Box::new(webp::WebPEncoder::create(c, io)?)),

             };
             self.codec = codec;
         };


         if let CodecKind::Encoder(ref mut e) = self.codec {
             match e.write_frame(c, preset, frame, decoder_io_ids).map_err(|e| e.at(here!())){
                 Err(e) => Err(e),
                 Ok(result) => {
                     match result.bytes{
                         s::ResultBytes::Elsewhere => Ok(result),
                         other => Err(nerror!(ErrorKind::InternalError, "Encoders must return s::ResultBytes::Elsewhere and write to their owned IO. Found {:?}", other))

                     }
                 }
             }
         }else{
             Err(unimpl!())
             //Err(FlowError::ErrNotImpl)
         }
    }

    pub fn get_encode_io(&self) -> Result<Option<IoProxyRef>>{
        if let CodecKind::Encoder(ref e) = self.codec {
            Ok(Some(e.get_io().map_err(|e| e.at(here!()))?))
        }else if let Some(ref e) = self.encode_io{
            Ok(Some(IoProxyRef::Borrow(e)))
        } else {
            Ok(None)
        }
    }
}


