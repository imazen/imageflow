use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError,  Result, JsonResponse};
use crate::ffi::BitmapBgra;
use imageflow_types::collections::AddRemoveSet;
use crate::io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;
use super::*;
use std::any::Any;
use std::rc::Rc;
use crate::io::IoProxyProxy;
use crate::io::IoProxyRef;
use rgb::alt::BGRA8;


extern crate jpeg_decoder as jpeg;


pub struct JpegDecoder{
    decoder:  jpeg::Decoder<IoProxy>,
    width: Option<i32>,
    height: Option<i32>,
    pixel_format: Option<jpeg::PixelFormat>
}

impl JpegDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<JpegDecoder> {

        let decoder =  jpeg::Decoder::new(io);

        Ok(JpegDecoder{
            decoder,
            width: None,
            height: None,
            pixel_format: None
        })
    }
}


impl Decoder for JpegDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }

    fn get_scaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo>{
        self.get_unscaled_image_info(c)
    }

    fn get_unscaled_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {

        self.decoder.read_info()?;
        let info = self.decoder.info().expect("error handling not yet implemented for jpeg");

        self.width = Some( i32::from(info.width));
        self.height = Some( i32::from(info.height));
        self.pixel_format = Some(info.pixel_format);


        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: i32::from(info.width),
            image_height: i32::from(info.height),
            preferred_mime_type: "image/jpeg".to_owned(),
            preferred_extension: "jpg".to_owned()
        })
    }

    //TODO! Support exif rotation
    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {

        if self.width.is_none() {
            let _ = self.get_scaled_image_info(c)?;
        }
        let pixels = self.decoder.decode()?;

        //TODO! Support color profiles

        unsafe {
            let w = self.width.unwrap();
            let h = self.height.unwrap();
            let copy = ffi::flow_bitmap_bgra_create(c.flow_c(), w as i32, h as i32, false, ffi::PixelFormat::Bgra32);
            if copy.is_null() {
                cerror!(c).panic();
            }
            let copy_mut = &mut *copy;

            match self.pixel_format.unwrap(){
                jpeg::PixelFormat::RGB24 => {
                    for row in 0..h{
                        let to_row: &mut [BGRA8] = std::slice::from_raw_parts_mut(copy_mut.pixels.offset(copy_mut.stride as isize * row as isize) as *mut BGRA8, w as usize);

                        let mut x = 0;
                        for mut pixel in to_row{
                            pixel.r = pixels[x * 3];
                            pixel.b = pixels[x * 3 + 1];
                            pixel.g = pixels[x * 3 + 2];
                            x+=1;
                        }
                    }
                },
                jpeg::PixelFormat::L8 => {
                    for row in 0..h{
                        let to_row: &mut [BGRA8] = std::slice::from_raw_parts_mut(copy_mut.pixels.offset(copy_mut.stride as isize * row as isize) as *mut BGRA8, w as usize);

                        let mut x = 0;
                        for mut pixel in to_row{
                            pixel.r = pixels[x];
                            pixel.b = pixel.r;
                            pixel.g = pixel.r;
                            x+=1;
                        }
                    }
                }

                _ => {
                    panic!("Unsupported jpeg type (grayscale or CMYK")
                }

            }

            Ok(copy)
        }
    }
    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}
