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
use crate::codecs::lode;

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
            liq.set_speed(u8::min(10, u8::max(1, speed)).into());
        }
        let min = u8::min(100, minimum_quality.unwrap_or(0));
        let max = u8::min(100,quality.unwrap_or(100));
        liq.set_quality(min.into(), max.into());

        Ok(PngquantEncoder {
            liq,
            io,
            maximum_deflate
        })
    }
}

impl Encoder for PngquantEncoder {
    fn write_frame(&mut self, c: &Context, preset: &EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<EncodeResult> {
        if let Some((pal, pixels)) = self.quantize(frame)? {
            lode::LodepngEncoder::write_png8(&mut self.io, &pal, &pixels, frame.w as usize, frame.h as usize, self.maximum_deflate)?;
        } else {
            lode::LodepngEncoder::write_png_auto(&mut self.io, &frame, self.maximum_deflate)?;
        };

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

impl PngquantEncoder {
    fn quantize(&mut self, frame: &BitmapBgra) -> StdResult<Option<(Vec<imagequant::Color>, Vec<u8>)>, imagequant::liq_error> {
        // BitmapBgra contains a *mut pointer, which isn't Sync.
        struct SyncBitmap<'a> {
            pixels: &'a [u8],
            stride_bytes: usize,
        }

        let convert_row = match frame.fmt {
            PixelFormat::Bgra32 => {
                unsafe extern "C" fn convert_bgra32(output_row: *mut imagequant::Color, y: c_int, width: c_int, frame: *mut SyncBitmap) {
                    let output_row = slice::from_raw_parts_mut(output_row, width as usize);
                    let input_row = &(*frame).pixels[y as usize * (*frame).stride_bytes..];
                    for (i, px) in output_row.iter_mut().enumerate() {
                        *px = imagequant::Color {
                            b: input_row[i * 4 + 0],
                            g: input_row[i * 4 + 1],
                            r: input_row[i * 4 + 2],
                            a: input_row[i * 4 + 3],
                        }
                    }
                };
                convert_bgra32
            },
            PixelFormat::Bgr32 => {
                unsafe extern "C" fn convert_bgr32(output_row: *mut imagequant::Color, y: c_int, width: c_int, frame: *mut SyncBitmap) {
                    let output_row = slice::from_raw_parts_mut(output_row, width as usize);
                    let input_row = &(*frame).pixels[y as usize * (*frame).stride_bytes..];
                    for (i, px) in output_row.iter_mut().enumerate() {
                        *px = imagequant::Color {
                            b: input_row[i * 4 + 0],
                            g: input_row[i * 4 + 1],
                            r: input_row[i * 4 + 2],
                            a: 255,
                        }
                    }
                };
                convert_bgr32
            },
            PixelFormat::Bgr24 => {
                unsafe extern "C" fn convert_bgr24(output_row: *mut imagequant::Color, y: c_int, width: c_int, frame: *mut SyncBitmap) {
                    let output_row = slice::from_raw_parts_mut(output_row, width as usize);
                    let input_row = &(*frame).pixels[y as usize * (*frame).stride_bytes..];
                    for (i, px) in output_row.iter_mut().enumerate() {
                        *px = imagequant::Color {
                            b: input_row[i * 3 + 0],
                            g: input_row[i * 3 + 1],
                            r: input_row[i * 3 + 2],
                            a: 255,
                        }
                    }
                };
                convert_bgr24
            },
            PixelFormat::Gray8 => {
                unsafe extern "C" fn convert_gray8(output_row: *mut imagequant::Color, y: c_int, width: c_int, frame: *mut SyncBitmap) {
                    let output_row = slice::from_raw_parts_mut(output_row, width as usize);
                    let input_row = &(*frame).pixels[y as usize * (*frame).stride_bytes..];
                    for (px, g) in output_row.iter_mut().zip(input_row.iter().cloned()) {
                        *px = imagequant::Color {
                            b: g,
                            g: g,
                            r: g,
                            a: 255,
                        }
                    }
                };
                convert_gray8
            },
        };

        let stride_bytes = frame.stride as usize;
        let width_bytes = frame.w as usize * frame.fmt.bytes();
        let mut frame_sync = SyncBitmap {
            pixels: unsafe {
                slice::from_raw_parts(frame.pixels, stride_bytes * frame.h as usize - stride_bytes + width_bytes)
            },
            stride_bytes,
        };
        let mut img = imagequant::Image::new_unsafe_fn(&self.liq, convert_row, &mut frame_sync, frame.w as usize, frame.h as usize, 0.)?;
        let mut res = match self.liq.quantize(&mut img) {
            Ok(res) => res,
            Err(imagequant::liq_error::LIQ_QUALITY_TOO_LOW) => return Ok(None),
            Err(err) => return Err(err)?,
        };
        res.set_dithering_level(1.);
        Ok(Some(res.remapped(&mut img)?))
    }
}
