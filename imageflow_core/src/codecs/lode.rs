use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use io::IoProxy;
use ffi::BitmapBgra;
use imageflow_types::PixelFormat;
use ::{Context, Result, ErrorKind, FlowError};
use io::IoProxyRef;
use std::slice;
use std::io::Write;
use std::rc::Rc;
use std::cell::RefCell;
use std::os::raw::c_int;
use rgb;
use lodepng;

pub struct LodepngEncoder {
    io: IoProxy,
}

impl LodepngEncoder {
    pub(crate) fn create(c: &Context, io: IoProxy) -> Result<Self> {
        Ok(LodepngEncoder {
            io,
        })
    }
}

impl Encoder for LodepngEncoder {
    fn write_frame(&mut self, c: &Context, _preset: &EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<EncodeResult> {
        Self::write_png_auto(&mut self.io, frame)?;

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
    pub fn write_png_auto<W: Write>(mut writer: W, frame: &BitmapBgra) -> Result<()> {
        let mut lode = lodepng::State::new();
        lode.set_auto_convert(true);

        let pixels_slice = unsafe {frame.pixels_slice()}.ok_or(nerror!(ErrorKind::BitmapPointerNull))?;
        let mut pixels_buf;
        let pixels = if frame.stride != frame.w {
            pixels_buf = Vec::with_capacity(frame.width() * frame.height());
            pixels_buf.extend(pixels_slice.chunks(frame.stride())
                .flat_map(|s| s[0..frame.width()].iter()));
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

        let png = lode.encode(pixels, frame.width(), frame.height())?;

        writer.write_all(&png).map_err(|e| FlowError::from_encoder(e))?;
        Ok(())
    }

    pub fn write_png8<W: Write>(mut writer: W, pal: &[rgb::RGBA8], pixels: &[u8], width: usize, height: usize) -> Result<()> {
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

        let png = lode.encode(&pixels, width, height)?;

        writer.write_all(&png).map_err(|e| FlowError::from_encoder(e))?;
        Ok(())
    }
}
