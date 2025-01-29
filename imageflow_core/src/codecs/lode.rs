use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use crate::io::IoProxy;

use imageflow_types::PixelFormat;
use crate::{Context, Result, ErrorKind, FlowError};
use crate::io::IoProxyRef;
use std::slice;
use std::io::Write;
use std::rc::Rc;
use std::cell::RefCell;
use std::os::raw::{c_int, c_uint, c_ulong};
use libc;
use rgb;
use lodepng;
use lodepng::{CompressSettings, DecompressSettings};
use flate2::Compression;
use crate::codecs::NamedEncoders::LodePngEncoder;
use crate::graphics::bitmaps::BitmapKey;

pub struct LodepngEncoder {
    io: IoProxy,
    use_highest_compression: Option<bool>
}

impl LodepngEncoder {
    pub(crate) fn create(c: &Context, io: IoProxy, use_highest_compression: Option<bool>) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::LodePngEncoder){
            return Err(nerror!(ErrorKind::CodecDisabledError, "The LodePNG encoder has been disabled"));
        }
        Ok(LodepngEncoder {
            io,
            use_highest_compression
        })
    }
}

impl Encoder for LodepngEncoder {
    fn write_frame(&mut self, c: &Context, _preset: &EncoderPreset, bitmap_key: BitmapKey, decoder_io_ids: &[i32]) -> Result<EncodeResult> {

        let bitmaps = c.borrow_bitmaps()
            .map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key)
            .map_err(|e| e.at(here!()))?;

            // check if the top-right pixel is transparent
            // let x= bitmap.w() - 1;
            // let y= 0;
            // let window = bitmap.get_window_u8()
            //         .unwrap();
            // let  top_right_pixel = window.get_pixel_bgra8(x, y).unwrap();
            // if top_right_pixel.a == 0 {
            //     eprintln!("lodepng: top-right pixel of image is transparent");
            // }

        let (w, h) = (bitmap.w(), bitmap.h());

        Self::write_png_auto(&mut self.io, &mut bitmap.get_window_u8().unwrap(), self.use_highest_compression)?;

        Ok(EncodeResult {
            w: w as i32,
            h: h as i32,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "png".to_owned(),
            preferred_mime_type: "image/png".to_owned(),
        })

    }

    fn get_io(&self) -> Result<IoProxyRef> {
        Ok(IoProxyRef::Borrow(&self.io))
    }
}

pub unsafe fn write_png<T: AsRef<std::path::Path>>(path: T, window: &mut crate::graphics::bitmaps::BitmapWindowMut<u8>) -> Result<()>{

    let file = std::fs::File::create(path)
        .map_err(|e| nerror!(ErrorKind::InvalidOperation))?;

    LodepngEncoder::write_png_auto(file, window, None)
        .map_err(|e| e.at(here!()))?;
    Ok(())
}

impl LodepngEncoder {

    pub fn write_png_auto<W: Write>(writer: W, window: &mut crate::graphics::bitmaps::BitmapWindowMut<u8>, use_highest_compression: Option<bool>) -> Result<()> {

        let bytes_per_pixel = window.items_per_pixel() as usize;
        let w = window.w() as usize;
        let h = window.h() as usize;
        let proper_len = w * h * bytes_per_pixel;

        let color_type = match (bytes_per_pixel, window.info().alpha_meaningful()){
            (4, true) => lodepng::ColorType::BGRA,
            (4, false) => lodepng::ColorType::BGRX,
            (3, _) => lodepng::ColorType::BGR,
            (1, _) => lodepng::ColorType::GREY,
            _ => return Err(nerror!(ErrorKind::InvalidState, "Unsupported pixel format"))
        };

        if window.stride_padding() > 0{
           let bytes = window.create_contiguous_vec().map_err(|e| e.at(here!()))?;
           assert_eq!(bytes.len(), proper_len);
           LodepngEncoder::write_png_auto_slice(writer, &bytes, w, h, color_type, use_highest_compression)
            .map_err(|e| e.at(here!()))
        }else{
            let slice = &window.get_slice()[..proper_len];
            LodepngEncoder::write_png_auto_slice(writer, slice, w, h, color_type, use_highest_compression)
            .map_err(|e| e.at(here!()))
        }

    }

    pub fn write_png_auto_slice<W: Write>(mut writer: W, pixels: &[u8], width: usize, height: usize, pixel_format: lodepng::ColorType, use_highest_compression: Option<bool>) -> Result<()> {

        if pixels.len() == 0{
            return Err(nerror!(ErrorKind::InvalidOperation, "No pixels to encode"));
        }

        let mut lode = lodepng::Encoder::new();
        lode.set_auto_convert(true);

        lode.info_raw_mut().colortype = pixel_format;
        lode.info_raw_mut().set_bitdepth(8);

        if use_highest_compression.unwrap_or(false){
            lode.set_custom_zlib(Some(zlib_compressor_9), std::ptr::null());
        }else{
            lode.set_custom_zlib(Some(zlib_compressor_6), std::ptr::null());
        }

        let png = lode.encode(pixels, width, height)?;

        writer.write_all(&png).map_err(|e| FlowError::from_encoder(e))?;
        Ok(())
    }

    pub fn write_png8<W: Write>(mut writer: W, pal: &[rgb::RGBA8], pixels: &[u8], width: usize, height: usize, use_highest_compression: Option<bool>) -> Result<()> {
        let mut lode = lodepng::Encoder::new();

        for &c in pal {
            lode.info_raw_mut().palette_add(c)?;
            lode.info_png_mut().color.palette_add(c)?;
        }

        lode.info_raw_mut().colortype = lodepng::ColorType::PALETTE;
        lode.info_raw_mut().set_bitdepth(8);
        lode.info_png_mut().color.colortype = lodepng::ColorType::PALETTE;
        lode.info_png_mut().color.set_bitdepth(8);
        lode.set_auto_convert(false);
        lode.set_filter_strategy(lodepng::FilterStrategy::ZERO, false);

        if use_highest_compression.unwrap_or(false){
            lode.set_custom_zlib(Some(zlib_compressor_9), std::ptr::null());
        }else{
            lode.set_custom_zlib(Some(zlib_compressor_6), std::ptr::null());
        }

        let png = lode.encode(&pixels, width, height)?;

        writer.write_all(&png).map_err(|e| FlowError::from_encoder(e))?;
        Ok(())
    }
}

extern "C" {
    /// zlib
    fn compress2(dest: *mut u8, dest_len: &mut c_ulong, source: *const u8, source_len: c_ulong, level: c_int) -> c_int;
}

fn zlib_compressor_6(input: &[u8], output: &mut dyn std::io::Write, context: &CompressSettings) -> std::result::Result<(), lodepng::Error> {
    zlib_compressor(input, output, context, 6)
}
fn zlib_compressor_9(input: &[u8], output: &mut dyn std::io::Write, context: &CompressSettings) -> std::result::Result<(), lodepng::Error> {
    zlib_compressor(input, output, context, 9)
}
fn zlib_compressor(input: &[u8], output: &mut dyn std::io::Write, context: &CompressSettings, zlib_level: u32) -> std::result::Result<(), lodepng::Error>{
    let mut compress = flate2::write::ZlibEncoder::new(output, flate2::Compression::new(zlib_level));
    if let Err(e) = compress.write_all(&input){
        return Err(lodepng::Error::new(1008));
    }
    if let Err(e) = compress.finish(){
        return Err(lodepng::Error::new(1009));
    }
    Ok(())
}
