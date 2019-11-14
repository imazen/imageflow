use super::*;
use io::IoProxy;
use std::io::BufReader;
use mozjpeg_sys::*;
use rgb::alt::*;
use std;
use Context;

use ffi;
use ffi::BitmapBgra;
use ffi::CodecInstance;
use ffi::PixelFormat;
use ffi::BitmapCompositingMode;
use gif::DecodingError;

pub struct JpegDecoder {
    tj_handle: tjhandle,
    reader: BufReader<IoProxy>,
    buffer: Option<BitmapBgra>,
    header: JpegHeader,
}

pub struct JpegHeader {
    width: i32,
    height: i32,
    // TODO: These should be rust enums
    jpegSubsamp: i32,
    jpegColorspace: i32,
}

impl JpegHeader {
    pub fn create(jpeg_buffer: &[u8], tj_handle: tjhandle) -> JpegHeader {

        let size: u64 = jpeg_buffer.len() as u64;
        let mut width: i32 = 0;
        let mut height: i32 = 0;
        let mut jpegSubsamp: i32 = 0;
        let mut jpegColorspace: i32 = 0;

        unsafe {
            tjDecompressHeader3(
                tj_handle,
                jpeg_buffer.as_ptr(),
                size,
                &mut width,
                &mut height,
                &mut jpegSubsamp,
                &mut jpegColorspace,
            );
        }

        JpegHeader { width, height, jpegSubsamp, jpegColorspace }
    }
}

impl Drop for JpegDecoder {
    fn drop(&mut self) {
        unsafe { tjDestroy(self.tj_handle as *mut _) };
    }
}

impl JpegDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<JpegDecoder> {
        let mut reader = io::BufReader::new(io);
        let mut jpeg_buffer = reader.fill_buf().map_err(|e| FlowError::from(e).at(here!()))?;

        if jpeg_buffer.len() == 0 {
            return Err(FlowError::from(std::io::ErrorKind::UnexpectedEof).at(here!()))
        }

        let tj_handle = unsafe { tjInitDecompress() };
        let jpeg_header = JpegHeader::create(&jpeg_buffer, tj_handle);

        let pitch = jpeg_header.width * 4;
        let allocated_size: usize = jpeg_header.height as usize * pitch as usize;
        let mut decompressed = Vec::with_capacity(allocated_size).into_boxed_slice();

        let size: u64 = jpeg_buffer.len() as u64;

        unsafe {
            tjDecompress2(
                tj_handle,
                jpeg_buffer.as_ptr(),
                size,
                decompressed.as_mut_ptr(),
                jpeg_header.width,
                pitch,
                jpeg_header.height,
                TJPF_TJPF_BGRA,
                TJFLAG_NOREALLOC as i32,
            );
        };

        let bitmap = BitmapBgra {
            w: jpeg_header.width as u32,
            h: jpeg_header.height as u32,
            stride: allocated_size as u32 / jpeg_header.height as u32,
            pixels: decompressed.as_mut_ptr(),
            fmt: PixelFormat::Bgra32,
            matte_color: [255; 4],
            compositing_mode: BitmapCompositingMode::ReplaceSelf
        };

        let decoder = JpegDecoder {
            tj_handle,
            reader,
            header: jpeg_header,
            buffer: Some(bitmap),
        };

        // TODO: Throw error on CMYK from get_image_info... bcuz fuckin' really.
        Ok(decoder)
    }
}

impl Decoder for JpegDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> { Ok(()) }

    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: self.header.width,
            image_height: self.header.height,
            preferred_mime_type: "image/jpeg".to_owned(),
            preferred_extension: "jpeg".to_owned(),
        })
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        // TODO: Implement this
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {
        let buffer: *mut BitmapBgra = self.buffer.as_mut().unwrap();
        Ok(buffer)
    }

    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(false)
    }

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
    use ffi::BitmapCompositingMode;

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
        let bitmap = unsafe {
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
            let allocated_size: usize = height as usize * width as usize * 4;
            let decompressed: *mut u8 = tjAlloc(allocated_size as i32);
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

            tjDestroy(tjhandle);
            slice::from_raw_parts(decompressed, allocated_size);

            BitmapBgra {
                w: width as u32,
                h: height as u32,
                stride: allocated_size as u32 / height as u32,
                pixels: decompressed,
                fmt: PixelFormat::Bgra32,
                matte_color: [255; 4],
                compositing_mode: BitmapCompositingMode::ReplaceSelf
            }
        };
    }
}
