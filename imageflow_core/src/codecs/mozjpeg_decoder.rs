use std;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::ffi;
use crate::{Context, CError,  Result, JsonResponse};
use crate::ffi::{CodecInstance, wrap_jpeg_get_custom_state, WrapJpegSourceManager};
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
extern crate mozjpeg_sys;
use ::mozjpeg_sys::*;
use gif::SetParameter;
use imageflow_helpers::preludes::from_std::ptr::null;


pub struct MozJpegDecoder{
    io: IoProxy,
    width: Option<i32>,
    height: Option<i32>,
    pixel_format: Option<jpeg::PixelFormat>
}

impl MozJpegDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<MozJpegDecoder> {
        Ok(MozJpegDecoder{
            io,
            width: None,
            height: None,
            pixel_format: None
        })
    }
}


impl Decoder for MozJpegDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }


    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {

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
            let _ = self.get_image_info(c)?;
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

#[repr(C)]
struct SourceManager{
    manager: ffi::WrapJpegSourceManager,
    bytes_have_been_read: bool,
    buffer: Vec<u8>
}

pub enum DecoderStage{

}
struct MzDec{
    error_state: Vec<u8>,
    codec_info: jpeg_decompress_struct,
    context: *mut Context,
    error: Option<FlowError>,
    io: IoProxy,
    source_manager: Option<Box<SourceManager>>
}
impl Drop for MzDec{
    fn drop(&mut self) {
        unsafe {
            jpeg_destroy_decompress(&mut self.codec_info)
        }
    }
}
impl MzDec{

    #[no_mangle]
    extern "C" fn jpeg_error_handler(custom_state: *mut c_void,
                                     codec_info: *mut mozjpeg_sys::jpeg_common_struct,
                                     error_code: i32,
                                     message_buffer: *u8,
                                     message_buffer_length: i32) -> bool{
        let decoder = custom_state as &mut MzDec;

        if decoder.error.is_none() {
            decoder.error = Some(nerror!(ErrorKind::JpegDecodingError, "Image malformed"));
        }

        return false; //false -> Fail, true -> ignore
    }

    fn new(context: &Context, io: IoProxy) -> Result<Box<MzDec>>{

        //Allocate space for the error state structure.
        let error_state_size = unsafe{ ffi::wrap_jpeg_error_state_bytes() };
        let mut error_state: Vec<u8> = Vec::with_capacity(error_state_size);
        for ix in 0..error_state_size{
            error_state.push(0u8);
        }

        let mut decoder = Box::new(MzDec{
            error_state,
            codec_info: mem::zeroed(),
            context: context as *mut Context,
            error: None,
            io,
            source_manager: None
        });

        unsafe {
            ffi::wrap_jpeg_setup_error_handler(
                &mut decoder.codec_info as *mut jpeg_common_struct,
                decoder.error_state.as_mut_ptr() as *mut c_void,
                decoder.as_mut() as *mut c_void,
                MzDec::jpeg_error_handler
            );

            if !ffi::wrap_jpeg_create_decompress(&mut decoder.codec_info){
                return Err(decoder.error.expect("error missing").at(here!()));
            }
        }

        Ok(decoder)
    }

    #[no_mangle]
    extern "C" fn source_fill_buffer(codec_info: &mut mozjpeg_sys::jpeg_decompress_struct, custom_state: *mut c_void, suspend_io: *mut bool) -> bool{

        let mut decoder = unsafe{custom_state as &mut MzDec };

        let mut source_manager = decoder.source_manager.unwrap().as_mut();


        let buffer = source_manager.buffer.as_mut_bytes();
        match decoder.io.read(buffer){
            Ok(size) => {
                if size == 0 {
                    if source_manager.bytes_have_been_read {
                        // Fake a correctly ended jpeg file so we can recover what's possible from this jpeg.
                        buffer[0] = 0xFF;
                        buffer[1] = 0xD9;
                        source_manager.manager.shared_mgr.next_input_byte = buffer.as_mut_ptr();
                        source_manager.manager.shared_mgr.bytes_in_buffer = 2;
                        return true;
                    }else{
                        decoder.error = Some(nerror!(ErrorKind::ImageDecodingError, "Empty source file"));
                        return false;
                    }
                }else{
                    source_manager.manager.shared_mgr.next_input_byte = buffer.as_mut_ptr();
                    source_manager.manager.shared_mgr.bytes_in_buffer = size;
                    source_manager.bytes_have_been_read = true;
                    return true;
                }
            },
            Err(err) => {
                decoder.error = Some(err.into());
                return false;
            }
        }

    }

    #[no_mangle]
    extern "C" fn source_skip_bytes(codec_info: &mut mozjpeg_sys::jpeg_decompress_struct, custom_state: *mut c_void, mut byte_count: c_long) -> bool{
        if byte_count > 0 {
            let mut decoder = unsafe{custom_state as &mut MzDec };
            let mut source_manager = decoder.source_manager.unwrap().as_mut();

            while byte_count > source_manager.manager.shared_mgr.bytes_in_buffer as c_long {
                byte_count -= source_manager.manager.shared_mgr.bytes_in_buffer as c_long;
                let mut suspend = false;
                if !source_fill_buffer(codec_info, custom_state, &mut suspend){
                    decoder.error = decoder.error.map(|e| e.at(here!()));
                    return false;
                }
            }

            source_manager.manager.shared_mgr.next_input_byte += byte_count as usize;
            source_manager.manager.shared_mgr.bytes_in_buffer -= byte_count as usize;
        }
        true
    }

    fn setup_source_manager(&mut self){
        if self.source_manager.is_none() {
            let mut mgr = Box::new(
                SourceManager {
                    manager: WrapJpegSourceManager {
                        shared_mgr: mem::zeroed(),
                        init_source_fn: None,
                        term_source_fn: None,
                        fill_input_buffer_fn: None,
                        skip_input_data_fn: None,
                        custom_state: &mut self as *mut c_void
                    },
                    bytes_have_been_read: false,
                    buffer: vec![0, 4096]
                }
            );
            unsafe {
                ffi::wrap_jpeg_setup_source_manager(&mut mgr.manager);
            }
            self.source_manager = Some(mgr)
        }
    }

    fn read_header(&mut self) -> Result<()>{
        self.setup_source_manager();

        if unsafe{ !ffi::wrap_jpeg_read_header(&mut self.codec_info) } {
            return Err(decoder.error.expect("error missing").at(here!()));
        }
        
        Ok(())

// Available only after `jpeg_read_header()`
//         let width = cinfo.image_width;
//         let height = cinfo.image_height;
//
// // Output settings be set before calling `jpeg_start_decompress()`
//         cinfo.out_color_space = J_COLOR_SPACE::JCS_RGB;
//         jpeg_start_decompress(&mut cinfo);
//         let row_stride = cinfo.image_width as usize * cinfo.output_components as usize;
//         let buffer_size = row_stride * cinfo.image_height as usize;
//         let mut buffer = vec![0u8; buffer_size];
//
//         while cinfo.output_scanline < cinfo.output_height {
//             let offset = cinfo.output_scanline as usize * row_stride;
//             let mut jsamparray = [buffer[offset..].as_mut_ptr()];
//             jpeg_read_scanlines(&mut cinfo, jsamparray.as_mut_ptr(), 1);
//         }
//
//         jpeg_finish_decompress(&mut cinfo);
//         jpeg_destroy_decompress(&mut cinfo);
//         libc::fclose(fh);
    }
}