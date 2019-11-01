use std;
use ::{Context};
use io::IoProxy;
use super::*;
use self::bgra::BGRA8;
use self::screen::Screen;
use mozjpeg;

pub struct JpegDecoder {
    reader: IoProxy,
    jpeg_buffer: Screen,
    buffer: Option<Vec<u8>>,
}

impl JpegDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<JpegDecoder> {
        // TODO: Implement this
        let screen = Screen::new(&reader)

        // TODO: Throw error on CMYK from get_image_info... bcuz fuckin' really.
        Ok(None)
    }

    fn initialize(&mut self, c: &Context) -> Result<()> {
        // TODO: Implement this
        Ok(())
    }

    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        // TODO: Implement this
        Ok(None)
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        // TODO: Implement this
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        // TODO: Implement this
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {
        // TODO: Implement this
        Ok(None)
    }

    fn as_any(&self) -> &Any { }

    fn has_more_frames(&mut self) -> Result<bool> { Ok(None) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mozjpeg::*;
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
        let mut decompressed: Vec<u8> = Vec::new();

        // @TODO: later reduce the use of unsafe
        // @TODO: we will also make a wrapper
        unsafe {
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
                decompressed.as_mut_ptr(),
                width,
                pitch,
                height,
                TJPF_TJPF_RGBA,
                TJFLAG_NOREALLOC as i32,
            )
        }
    }
}
