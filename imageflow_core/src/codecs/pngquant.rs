use super::s::{EncodeResult, EncoderPreset};
use super::Encoder;
use crate::io::IoProxy;

use crate::codecs::lode;
use crate::graphics::bitmaps::BitmapKey;
use crate::io::IoProxyRef;
use crate::{Context, ErrorKind, Result};
use imageflow_types::{Color, PixelFormat};
use rgb::ComponentSlice;
use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::os::raw::c_int;
use std::rc::Rc;
use std::result::Result as StdResult;
use std::slice;
pub struct PngquantEncoder {
    liq: imagequant::Attributes,
    io: IoProxy,
    maximum_deflate: Option<bool>,
    matte: Option<Color>,
}

impl PngquantEncoder {
    pub(crate) fn create(
        c: &Context,
        io: IoProxy,
        speed: Option<u8>,
        quality: Option<u8>,
        minimum_quality: Option<u8>,
        maximum_deflate: Option<bool>,
        matte: Option<Color>,
    ) -> Result<Self> {
        if !c.enabled_codecs.encoders.contains(&crate::codecs::NamedEncoders::PngQuantEncoder) {
            return Err(nerror!(
                ErrorKind::CodecDisabledError,
                "The PNGQuant encoder has been disabled"
            ));
        }
        let mut liq = imagequant::new();
        if let Some(speed) = speed {
            liq.set_speed(speed.clamp(1, 10).into()).unwrap();
        }
        let target_quality = quality.unwrap_or(100).clamp(0, 100);
        let min: u8 = minimum_quality.unwrap_or(0).clamp(0, target_quality);

        liq.set_quality(min, target_quality).unwrap();

        Ok(PngquantEncoder { liq, io, maximum_deflate, matte })
    }
}
impl Encoder for PngquantEncoder {
    fn write_frame(
        &mut self,
        c: &Context,
        _preset: &EncoderPreset,
        bitmap_key: BitmapKey,
        decoder_io_ids: &[i32],
    ) -> Result<EncodeResult> {
        let bitmaps = c.borrow_bitmaps().map_err(|e| e.at(here!()))?;

        let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

        if self.matte.is_some() {
            bitmap.apply_matte(self.matte.clone().unwrap()).map_err(|e| e.at(here!()))?;
        }

        bitmap.get_window_u8().unwrap().normalize_unused_alpha().map_err(|e| e.at(here!()))?;
        let mut window = bitmap.get_window_bgra32().unwrap();

        let (w, h) = window.size_usize();

        let error = {
            // SAFETY: w and h match the bitmap dimensions, and the callback
            // only reads from the valid window rows within those bounds.
            let mut img = unsafe {
                imagequant::Image::new_fn(
                    &self.liq,
                    |row: &mut [MaybeUninit<imagequant::RGBA>], row_index: usize| {
                        let from = window.row(row_index).unwrap();
                        from.iter().zip(row).for_each(|(from, to)| {
                            to.write(imagequant::RGBA {
                                r: from.r,
                                g: from.g,
                                b: from.b,
                                a: from.a,
                            });
                        });
                    },
                    w,
                    h,
                    0.0,
                )
            }
            .map_err(|e| crate::FlowError::from(e).at(here!()))?;
            match self.liq.quantize(&mut img) {
                Ok(mut res) => {
                    res.set_dithering_level(1.).unwrap();

                    let (pal, pixels) = res.remapped(&mut img).unwrap(); // could have alloc failure here, should map

                    lode::LodepngEncoder::write_png8(
                        &mut self.io,
                        &pal,
                        &pixels,
                        w,
                        h,
                        self.maximum_deflate,
                    )?;
                    None
                }
                Err(e) => Some(e),
            }
        };
        match error {
            Some(imagequant::liq_error::QualityTooLow) => {
                if window.info().alpha_meaningful() {
                    let (vec, w, h) = window.to_vec_rgba().map_err(|e| e.at(here!()))?;

                    let slice_as_u8 = bytemuck::cast_slice::<rgb::RGBA8, u8>(vec.as_slice());

                    lode::LodepngEncoder::write_png_auto_slice(
                        &mut self.io,
                        slice_as_u8,
                        w,
                        h,
                        lodepng::ColorType::RGBA,
                        self.maximum_deflate,
                    )
                    .map_err(|e| e.at(here!()))?;

                    // data.add("result.format", "png32");
                } else {
                    let (vec, w, h) = window.to_vec_rgb().map_err(|e| e.at(here!()))?;

                    let slice_as_u8 = bytemuck::cast_slice::<rgb::RGB8, u8>(vec.as_slice());

                    lode::LodepngEncoder::write_png_auto_slice(
                        &mut self.io,
                        slice_as_u8,
                        w,
                        h,
                        lodepng::ColorType::RGB,
                        self.maximum_deflate,
                    )
                    .map_err(|e| e.at(here!()))?;

                    // data.add("result.format", "png24");
                }
            }
            Some(err) => return Err(err)?,
            None => {}
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

    fn get_io(&self) -> Result<IoProxyRef<'_>> {
        Ok(IoProxyRef::Borrow(&self.io))
    }

    fn into_io(self: Box<Self>) -> Result<IoProxy> {
        Ok(self.io)
    }
}
