use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use crate::io::IoProxy;
use crate::ffi::BitmapBgra;
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
    fn write_frame(&mut self, c: &Context, _preset: &EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<EncodeResult> {
        Self::write_png_auto(&mut self.io, frame, self.use_highest_compression)?;

        Ok(EncodeResult {
            w: frame.w as i32,
            h: frame.h as i32,
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

impl LodepngEncoder {
    pub fn write_png_auto<W: Write>(mut writer: W, frame: &BitmapBgra, use_highest_compression: Option<bool>) -> Result<()> {
        let mut lode = lodepng::State::new();
        lode.set_auto_convert(true);

        let pixels_slice = unsafe {frame.pixels_slice()}.ok_or(nerror!(ErrorKind::BitmapPointerNull))?;
        let mut pixels_buf;
        let pixels = if frame.stride != frame.w {
            let width_bytes = frame.width() * frame.fmt.bytes();
            pixels_buf = Vec::with_capacity(frame.width() * frame.height());
            pixels_buf.extend(pixels_slice.chunks(frame.stride())
                .flat_map(|s| s[0..width_bytes].iter()));
            &pixels_buf
        } else {
            pixels_slice
        };

        lode.info_raw_mut().colortype = match frame.fmt {
            PixelFormat::Bgra32 => lodepng::ColorType::BGRA,
            PixelFormat::Bgr32 => lodepng::ColorType::BGRX,
            PixelFormat::Bgr24 => lodepng::ColorType::BGR,
            PixelFormat::Gray8 => lodepng::ColorType::GREY,
        };
        lode.info_raw_mut().set_bitdepth(8);

        if use_highest_compression.unwrap_or(false){
            lode.encoder.zlibsettings.custom_zlib = Some(zlib_compress_adapter_slowest);
        }else{
            lode.encoder.zlibsettings.custom_zlib = Some(zlib_compress_adapter);
        }

        let png = lode.encode(pixels, frame.width(), frame.height())?;

        writer.write_all(&png).map_err(|e| FlowError::from_encoder(e))?;
        Ok(())
    }

    pub fn write_png8<W: Write>(mut writer: W, pal: &[rgb::RGBA8], pixels: &[u8], width: usize, height: usize, use_highest_compression: Option<bool>) -> Result<()> {
        let mut lode = lodepng::State::new();

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
            lode.encoder.zlibsettings.custom_zlib = Some(zlib_compress_adapter_slowest);
        }else{
            lode.encoder.zlibsettings.custom_zlib = Some(zlib_compress_adapter);
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

unsafe extern "C" fn zlib_compress_adapter(dest: &mut *mut u8, dest_size: &mut usize, source: *const u8, source_size: usize, info: *const lodepng::CompressSettings) -> c_uint {
    assert!(dest.is_null());
    let dest_buf_size = source_size * 1001/1000 + 12;
    *dest = libc::malloc(dest_buf_size) as *mut u8;
    let mut compressed_size = dest_buf_size as c_ulong;
    compress2(*dest, &mut compressed_size, source, source_size as c_ulong, 6);
    *dest_size = compressed_size as usize;
    0
}
unsafe extern "C" fn zlib_compress_adapter_slowest(dest: &mut *mut u8, dest_size: &mut usize, source: *const u8, source_size: usize, info: *const lodepng::CompressSettings) -> c_uint {
    assert!(dest.is_null());
    let dest_buf_size = source_size * 1001/1000 + 12;
    *dest = libc::malloc(dest_buf_size) as *mut u8;
    let mut compressed_size = dest_buf_size as c_ulong;
    compress2(*dest, &mut compressed_size, source, source_size as c_ulong, 9);
    *dest_size = compressed_size as usize;
    0
}
