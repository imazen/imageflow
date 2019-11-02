use std;
use ::{Context};
use io::IoProxy;
use super::*;
use rgb::alt::*;
use mozjpeg_sys::*;

use ffi;
use ffi::CodecInstance;
use ffi::BitmapBgra;

pub struct JpegDecoder {
    reader: IoProxy,
    jpeg_buffer: Box<Vec<u8>>,
    buffer: Option<Vec<u8>>,
}

impl JpegDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<JpegDecoder> {
        // TODO: Implement this
        let decoder = JpegDecoder {
            reader: IoProxy::create(c, io_id),
            jpeg_buffer: Box::new(Vec::new()),
            buffer: None,
        };

        // TODO: Throw error on CMYK from get_image_info... bcuz fuckin' really.
        Ok(decoder)
    }
}

impl Decoder for JpegDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        // TODO: Implement this


        let _ = unsafe {
            let tjhandle = tjInitDecompress();
        };

        Ok(())
    }

    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: i32::from(0),
            image_height: i32::from(0),
            preferred_mime_type: "image/jpeg".to_owned(),
            preferred_extension: "jpeg".to_owned()
        })
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        // TODO: Implement this
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> { Ok(()) }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {
        // TODO: Implement this
        unsafe {
            let w = 0;
            let h = 0;
            let copy = ffi::flow_bitmap_bgra_create(c.flow_c(), w, h, false, ffi::PixelFormat::Bgra32);
            if copy.is_null() {
                cerror!(c).panic();
            }

            Ok(copy)
        }

    }

    fn has_more_frames(&mut self) -> Result<bool> { Ok(false) }

    fn as_any(&self) -> &Any {
        self as &Any
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mozjpeg_sys::*;
    use std::convert::TryInto;
    use std::fs::File;
    use std::io;
    use std::io::prelude::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    fn read_from_file() -> io::Result<Vec<u8>> {
        let mut f = File::open("tests/test.jpg")?;
        let mut buffer: Vec<u8> = Vec::new();

        // read the whole file
        f.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    #[test]
    fn decompress_test() {
        let mut jpegBuffer = read_from_file().expect("couldn't read file");
        let size: u64 = jpegBuffer.len().try_into().unwrap();
        let mut width: i32 = 0;
        let mut height: i32 = 0;
        let mut jpegSubsamp: i32 = 0;
        let mut jpegColorspace: i32 = 0;

        // @TODO: later reduce the use of unsafe
        // @TODO: we will also make a wrapper
        let tj_slice = unsafe {
            let allocated_size: usize = height as usize * width as usize * 4;
            let decompressed: *mut u8 = tjAlloc(allocated_size as i32);
            let tjhandle = tjInitDecompress();
            tjDecompressHeader3(
                tjhandle,
                jpegBuffer.as_mut_ptr(),
                size,
                &mut width,
                &mut height,
                &mut jpegSubsamp,
                &mut jpegColorspace,
            );
            let pitch = 0;
            tjDecompress2(
                tjhandle,
                jpegBuffer.as_mut_ptr(),
                size,
                decompressed,
                width,
                pitch,
                height,
                TJPF_TJPF_BGRA,
                TJFLAG_NOREALLOC as i32,
            );

            drop(Box::from_raw(tjhandle));
            slice::from_raw_parts(decompressed, allocated_size);
        };

    }
}
