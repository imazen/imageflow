use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use io::IoProxy;
use ffi::BitmapBgra;
use imageflow_types::PixelFormat;
use ::{Context, Result, ErrorKind};
use io::IoProxyRef;
use std::slice;
use std::rc::Rc;
use std::cell::RefCell;
use std::os::raw::c_int;
use imagequant;
use lodepng;

pub struct PngquantEncoder {
    liq: imagequant::Attributes,
    io: IoProxy,
}

impl PngquantEncoder {
    pub(crate) fn create(c: &Context, speed: u8, quality: (u8, u8), io: IoProxy) -> Result<Self> {
        let mut liq = imagequant::new();
        liq.set_speed(speed.into());
        liq.set_quality(quality.0.into(), quality.1.into());
        Ok(PngquantEncoder {
            liq,
            io,
        })
    }
}

impl Encoder for PngquantEncoder {
    fn write_frame(&mut self, c: &Context, preset: &EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<EncodeResult> {
        let (pal, pixels) = Self::quantize(frame)?;

        let mut lode = lodepng::State::new();

        for c in pal {
            lode.info_raw_mut().palette_add(c).unwrap();
            lode.info_png_mut().color.palette_add(c).unwrap();
        }

        lode.info_raw_mut().colortype = lodepng::ColorType::PALETTE;
        lode.info_raw_mut().set_bitdepth(8);
        lode.info_png_mut().color.colortype = lodepng::ColorType::PALETTE;
        lode.info_png_mut().color.set_bitdepth(8);
        lode.set_auto_convert(false);
        lode.set_filter_strategy(lodepng::FilterStrategy::ZERO, false);

        let png = lode.encode(&pixels, frame.w as usize, frame.h as usize)?;
use std::io::Write;
        self.io.write_all(&png).unwrap();

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
    fn quantize(frame: &BitmapBgra) -> Result<(Vec<imagequant::Color>, Vec<u8>)> {
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

        let mut liq = imagequant::new();
        let stride_bytes = frame.stride as usize;
        let width_bytes = frame.w as usize * frame.fmt.bytes();
        let mut frame_sync = SyncBitmap {
            pixels: unsafe {
                slice::from_raw_parts(frame.pixels, stride_bytes * frame.h as usize - stride_bytes + width_bytes)
            },
            stride_bytes,
        };
        let mut img = imagequant::Image::new_unsafe_fn(&liq, convert_row, &mut frame_sync, frame.w as usize, frame.h as usize, 0.)?;
        let mut res = liq.quantize(&mut img)?;
        res.set_dithering_level(1.);
        Ok(res.remapped(&mut img)?)
    }
}
