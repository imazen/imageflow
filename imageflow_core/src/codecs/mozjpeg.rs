use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use io::IoProxy;
use ffi::BitmapBgra;
use imageflow_types::PixelFormat;
use ::{Context, Result, ErrorKind, FlowError};
use std::result::Result as StdResult;
use io::IoProxyRef;
use std::slice;
use std::rc::Rc;
use std::cell::RefCell;
use std::os::raw::c_int;
use mozjpeg;
use codecs::lode;
use std::io::Write;

pub struct MozjpegEncoder {
    io: IoProxy,
    quality: Option<u8>,
}

impl MozjpegEncoder {
    // Quality is in range 0-100
    pub(crate) fn create(c: &Context, quality: Option<u8>, io: IoProxy) -> Result<Self> {
        Ok(MozjpegEncoder {
            io, quality
        })
    }
}

impl Encoder for MozjpegEncoder {
    fn write_frame(&mut self, c: &Context, _preset: &EncoderPreset, frame: &mut BitmapBgra, _decoder_io_ids: &[i32]) -> Result<EncodeResult> {
        let in_color_space = match frame.fmt {
            PixelFormat::Bgra32 => mozjpeg::ColorSpace::JCS_EXT_BGRA,
            PixelFormat::Bgr32 => mozjpeg::ColorSpace::JCS_EXT_BGRX,
            PixelFormat::Bgr24 => mozjpeg::ColorSpace::JCS_EXT_BGR,
            PixelFormat::Gray8 => mozjpeg::ColorSpace::JCS_GRAYSCALE,
        };
        let mut cinfo = mozjpeg::Compress::new(in_color_space);
        cinfo.set_size(frame.width(), frame.height());
        if let Some(q) = self.quality {
            cinfo.set_quality(q.into());
        }
        cinfo.set_mem_dest();
        cinfo.start_compress();
        let pixels_slice = unsafe {frame.pixels_slice()}.ok_or(nerror!(ErrorKind::BitmapPointerNull))?;
        if frame.width() == frame.stride() {
            cinfo.write_scanlines(pixels_slice);
        } else {
            let width_bytes = frame.width() * frame.fmt.bytes();
            for row in pixels_slice.chunks(frame.stride()) {
                cinfo.write_scanlines(&row[0..width_bytes]);
            }
        }
        cinfo.finish_compress();
        let data = cinfo.data_as_mut_slice()
            .map_err(|_| nerror!(ErrorKind::MozjpegEncodingError, "Internal error"))?;
        self.io.write_all(data).map_err(|e| FlowError::from_encoder(e))?;

        Ok(EncodeResult {
            w: frame.w as i32,
            h: frame.h as i32,
            io_id: self.io.io_id(),
            bytes: ::imageflow_types::ResultBytes::Elsewhere,
            preferred_extension: "jpg".to_owned(),
            preferred_mime_type: "image/jpeg".to_owned(),
        })
    }

    fn get_io(&self) -> Result<IoProxyRef> {
        Ok(IoProxyRef::Borrow(&self.io))
    }
}
