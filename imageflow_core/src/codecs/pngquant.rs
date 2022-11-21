use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use crate::io::IoProxy;
use crate::ffi::BitmapBgra;
use imageflow_types::PixelFormat;
use crate::{Context, Result, ErrorKind};
use std::result::Result as StdResult;
use crate::io::IoProxyRef;
use std::slice;
use std::rc::Rc;
use std::cell::RefCell;
use std::os::raw::c_int;
use imagequant;
use rgb::ComponentSlice;
use crate::codecs::lode;
use crate::graphics::bitmaps::BitmapKey;

pub struct PngquantEncoder {
    liq: imagequant::Attributes,
    io: IoProxy,
    maximum_deflate: Option<bool>,
}

impl PngquantEncoder {
    pub(crate) fn create(c: &Context, speed: Option<u8>, quality: Option<u8>, minimum_quality: Option<u8>, maximum_deflate: Option<bool>, io: IoProxy) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::PngQuantEncoder){
            return Err(nerror!(ErrorKind::CodecDisabledError, "The PNGQuant encoder has been disabled"));
        }
        let mut liq = imagequant::new();
        if let Some(speed) = speed {
            liq.set_speed(u8::min(10, u8::max(1, speed)).into()).unwrap();
        }
        let min = u8::min(100, minimum_quality.unwrap_or(0));
        let max = u8::min(100,quality.unwrap_or(100));
        liq.set_quality(min.into(), max.into()).unwrap();

        Ok(PngquantEncoder {
            liq,
            io,
            maximum_deflate
        })
    }
}
impl PngquantEncoder{
    unsafe fn raw_byte_access(rgba: &[rgb::RGBA8]) -> &[u8] {
        use std::slice;
        slice::from_raw_parts(rgba.as_ptr() as *const u8, rgba.len() * 4)
    }
}
impl Encoder for PngquantEncoder {
    fn write_frame(&mut self, c: &Context, preset: &EncoderPreset, bitmap_key: BitmapKey, decoder_io_ids: &[i32]) -> Result<EncodeResult> {

        let bitmaps = c.borrow_bitmaps()
            .map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key)
            .map_err(|e| e.at(here!()))?;

        {
            let mut bitmap_bgra = unsafe { bitmap.get_window_u8().unwrap().to_bitmap_bgra()? };
            bitmap_bgra.normalize_alpha().map_err(|e| e.at(here!()))?;
        }

        unsafe {
            let (vec,w,h) = bitmap.get_window_u8()
                .ok_or_else(|| nerror!(ErrorKind::InvalidBitmapType))?
                .to_vec_rgba()
                .map_err(|e| e.at(here!()))?;

            let mut img = imagequant::Image::new_borrowed(&self.liq, &vec ,w, h,0.)?;

            let res = match self.liq.quantize(&mut img) {
                Ok(mut res) => {
                    res.set_dithering_level(1.).unwrap();

                    let (pal, pixels) = res.remapped(&mut img).unwrap(); // could have alloc failure here, should map

                    lode::LodepngEncoder::write_png8(&mut self.io, &pal, &pixels, w, h, self.maximum_deflate)?;
                },
                Err(imagequant::liq_error::QualityTooLow) => {
                    lode::LodepngEncoder::write_png_auto_slice(&mut self.io, PngquantEncoder::raw_byte_access(vec.as_slice()), w, h, lodepng::ColorType::RGBA, self.maximum_deflate)?;
                }
                Err(err) => return Err(err)?,
            };


            Ok(EncodeResult {
                w: w as i32,
                h: h as i32,
                io_id: self.io.io_id(),
                bytes: ::imageflow_types::ResultBytes::Elsewhere,
                preferred_extension: "png".to_owned(),
                preferred_mime_type: "image/png".to_owned(),
            })
        }
    }

    fn get_io(&self) -> Result<IoProxyRef> {
        Ok(IoProxyRef::Borrow(&self.io))
    }
}
