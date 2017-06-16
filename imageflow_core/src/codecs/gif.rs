use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::job::Job;
use ::{Context, FlowErr,FlowError, Result, JsonResponse};
use ::ffi::CodecInstance;
use ::ffi::BitmapBgra;
use ::imageflow_types::collections::AddRemoveSet;
use io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;
use super::*;
use ::gif_dispose::Screen;
use ::gif::SetParameter;

pub struct GifDecoder{
    proxy_uuid: Uuid
}

impl GifDecoder {
    pub fn create(c: &Context, io: &IoProxy, io_id: i32) -> Result<GifDecoder> {
        Ok(GifDecoder{
            proxy_uuid: io.uuid
        })

    }
    fn read_size(io: &mut IoProxy) -> Result<(i32,i32)>{
        let mut decoder = ::gif::Decoder::new(io);

        // Important:
        decoder.set(::gif::ColorOutput::Indexed);

        let mut reader = decoder.read_info().unwrap();
        Ok((reader.width() as i32, reader.height() as i32))

    }
}
impl Decoder for GifDecoder {
    fn initialize(&mut self, c: &Context, job: &Job) -> Result<()> {
        Ok(())
    }


    fn get_image_info(&mut self, c: &Context, job: &Job, io: &mut IoProxy) -> Result<s::ImageInfo> {

        let (w,h) = GifDecoder::read_size(io)?;
        io.seek(c, 0).unwrap();

        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: w,
            image_height: h,
            current_frame_index: 0,
            frame_count: 1,
            // We would have to read in the entire GIF to know!
            preferred_mime_type: "image/gif".to_owned(),
            preferred_extension: "gif".to_owned()
        })
    }

    fn get_exif_rotation_flag(&mut self, c: &Context, job: &Job) -> Result<i32> {
        Ok(0)
    }

    fn tell_decoder(&mut self, c: &Context, job: &Job, tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context, job: &Job, io: &mut IoProxy) -> Result<*mut BitmapBgra> {
        let mut decoder = ::gif::Decoder::new(io);

        // Important:
        decoder.set(::gif::ColorOutput::Indexed);

        let mut reader = decoder.read_info().unwrap();
        let mut screen = ::gif_dispose::Screen::new(&reader);
        if let Some(frame) = reader.read_next_frame().unwrap() {
            screen.blit(&frame).unwrap();

            unsafe {
                let copy = ffi::flow_bitmap_bgra_create(c.flow_c(), screen.width as i32, screen.height as i32, false, ffi::PixelFormat::Bgra32);
                if copy == ptr::null_mut() {
                    c.error().assert_ok();
                }
                let pixel_count = (*copy).stride * (*copy).h / 4;
                let copy_buffer: &mut [Bgra32] = std::slice::from_raw_parts_mut((*copy).pixels as *mut Bgra32, pixel_count as usize);

                for (dst, &src) in copy_buffer.iter_mut().zip(screen.pixels.iter()) {
                    dst.b = src.b;
                    dst.g = src.g;
                    dst.r = src.r;
                    dst.a = src.a;
                }

                Ok(copy)
            }
        }else{
            panic!("");
            Err(FlowError::ErrNotImpl)
        }

    }
}

    #[repr(C, packed)]
    struct Bgra32 {
        b: u8,
        g: u8,
        r: u8,
        a: u8
    }

pub struct GifEncoder{

}

impl GifEncoder{
    pub(crate) fn create(c: &Context, job: &Job, io: &mut IoProxy, preset: &s::EncoderPreset, io_id: i32) -> GifEncoder{
        unimplemented!()
    }
}

impl Encoder for GifEncoder{
    fn write_frame(&mut self, c: &Context, job: &Job, io: &mut IoProxy, preset: &s::EncoderPreset, frame: &mut BitmapBgra) -> Result<s::EncodeResult> {
        unimplemented!()
    }
}