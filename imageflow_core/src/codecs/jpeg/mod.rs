use std;

pub struct JpegDecoder {
    reader: ::jpeg::Reader<IOProxy>,
    jpeg_buffer: Screen,
    buffer: Option<Vec<u8>>,
}

impl JpegDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<JpegDecoder> {
        // TODO: Implement this

        // TODO: Throw error on CMYK from get_image_info... bcuz fuckin' really.
        Ok(None)
    }

    fn initialize(&mut self, c: &Context) -> Result<()> {
        // TODO: Implement this
        Ok(None)
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
