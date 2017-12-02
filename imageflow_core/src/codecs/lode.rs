use super::Encoder;
use super::s::{EncoderPreset, EncodeResult};
use io::IoProxy;
use ffi::BitmapBgra;
use imageflow_types::PixelFormat;
use ::{Context, Result, ErrorKind};
use io::IoProxyRef;
use std::io::Write;
use rgb;
use lodepng;

pub struct LodepngEncoder {
}

impl LodepngEncoder {
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

        writer.write_all(&png).map_err(|_| lodepng::Error(79))?;
        Ok(())
    }
}
