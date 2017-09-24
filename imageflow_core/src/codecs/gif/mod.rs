use ::std;
use ::for_other_imageflow_crates::preludes::external_without_std::*;
use ::ffi;
use ::{Context, CError,  Result, JsonResponse};
use ::ffi::CodecInstance;
use ::ffi::BitmapBgra;
use ::imageflow_types::collections::AddRemoveSet;
use io::IoProxy;
use uuid::Uuid;
use imageflow_types::IoDirection;
use super::*;
use ::std::any::Any;
mod disposal;
mod subimage;
mod screen;
mod bgra;
use self::bgra::BGRA8;
use self::screen::Screen;
use ::gif::Frame;
use ::gif::SetParameter;

pub struct GifDecoder{
    reader: ::gif::Reader<IoProxy>,
    screen: Screen,
    buffer: Option<Vec<u8>>,
    last_frame: Option<Frame<'static>>,
    next_frame: Option<Frame<'static>>
}

impl GifDecoder {
    pub fn create(c: &Context, io: IoProxy, io_id: i32) -> Result<GifDecoder> {

        let mut decoder = ::gif::Decoder::new(io);

        // Important:
        decoder.set(::gif::ColorOutput::Indexed);

        let reader = decoder.read_info().map_err(|e| FlowError::from(e).at(here!()))?;

        let screen = Screen::new(&reader);

        Ok(GifDecoder{
            reader,
            screen,
            buffer: None,
            last_frame: None,
            next_frame: None
        })
    }

    fn read_next_frame_info(&mut self) -> Result<()>{
        self.last_frame = self.next_frame.take();
        // Currently clones local palette
        self.next_frame = self.reader.next_frame_info().map_err(|e| FlowError::from(e).at(here!()))?.map(|v|v.clone());
        Ok(())
    }


    fn create_bitmap_from_screen(&self, c: &Context) -> Result<*mut BitmapBgra>{
        // Create output bitmap and copy to it
        unsafe {
            let w = self.screen.width;
            let h = self.screen.height;
            let copy = ffi::flow_bitmap_bgra_create(c.flow_c(), w as i32, h as i32, false, ffi::PixelFormat::Bgra32);
            if copy == ptr::null_mut() {
                cerror!(c).panic();
            }
            let copy_mut = &mut *copy;

            for row in 0..h{
                let to_row: &mut [BGRA8] = std::slice::from_raw_parts_mut(copy_mut.pixels.offset(copy_mut.stride as isize * row as isize) as *mut BGRA8, w as usize);
                to_row.copy_from_slice(&self.screen.pixels[row * w..(row + 1) * w]);
            }
            Ok(copy)
        }
    }
    pub fn current_frame(&self) -> Option<&Frame>{
        self.last_frame.as_ref()
    }

    pub fn get_repeat(&self) -> Option<::gif::Repeat>{
        // TODO: Fix hack - gif crate doesn't allow reading this
        Some(::gif::Repeat::Infinite)
    }
}


impl Decoder for GifDecoder {
    fn initialize(&mut self, c: &Context) -> Result<()> {
        Ok(())
    }


    fn get_image_info(&mut self, c: &Context) -> Result<s::ImageInfo> {
        Ok(s::ImageInfo {
            frame_decodes_into: s::PixelFormat::Bgra32,
            image_width: self.reader.width() as i32,
            image_height: self.reader.height() as i32,
//            current_frame_index: 0,
//            frame_count: 1,
            // We would have to read in the entire GIF to know!
            preferred_mime_type: "image/gif".to_owned(),
            preferred_extension: "gif".to_owned()
        })
    }

    fn get_exif_rotation_flag(&mut self, c: &Context) -> Result<Option<i32>> {
        Ok(None)
    }

    fn tell_decoder(&mut self, c: &Context, tell: s::DecoderCommand) -> Result<()> {
        Ok(())
    }

    fn read_frame(&mut self, c: &Context) -> Result<*mut BitmapBgra> {
        // Ensure next_frame is present (only called for first frame)
        if self.next_frame.is_none() {
            self.read_next_frame_info().map_err(|e| e.at(here!()))?;
        }

        {
            // Grab a reference
            let frame = self.next_frame.as_ref().ok_or_else(|| nerror!(ErrorKind::InvalidOperation, "read_frame was called without a frame available"))?;

            //Prepare our resuable buffer
            let buf_size = self.reader.width() as usize * self.reader.height() as usize;

            let buf_mut = self.buffer.get_or_insert_with(|| vec![0; buf_size]);
            let mut slice = &mut buf_mut[..self.reader.buffer_size()];

            unsafe {
                ptr::write_bytes(slice.as_mut_ptr(), 0, slice.len() - 1);
            }
            //Read into that buffer
            self.reader.read_into_buffer(slice).map_err(|e| FlowError::from(e).at(here!()))?;

            // Render / apply disposal
            //TODO: allocs: Disposal currently allocates a new copy every blit (for previous frame)
            self.screen.blit(frame, slice).map_err(|e| nerror!(ErrorKind::GifDecodingError, "{:?}", e))?; //Missing palette?
        }
        // Try to read the next frame;
        self.read_next_frame_info().map_err(|e| e.at(here!()))?;


        self.create_bitmap_from_screen(c)
    }
    fn has_more_frames(&mut self) -> Result<bool> {
        Ok(self.next_frame.is_some())
    }
    fn as_any(&self) -> &Any {
        self as &Any
    }
}


pub struct GifEncoder{
    io_id: i32,
    encoder: ::gif::Encoder<IoProxy>,
    io_ref: &'static IoProxy, //unsafe self-referential,
    frame_ix: i32
}

impl GifEncoder{
    pub(crate) fn create(c: &Context, preset: &s::EncoderPreset, io: IoProxy, first_frame: &BitmapBgra) -> Result<GifEncoder>{
        Ok(GifEncoder{
            io_id: io.io_id(),
            io_ref: unsafe { &*(&io as *const IoProxy) },
            // Global color table??
            encoder: ::gif::Encoder::new(io, first_frame.w as u16, first_frame.h as u16, &[]).map_err(|e| FlowError::from_gif_encoder(e).at(here!()))?,
            frame_ix: 0
        })
    }
}

impl Encoder for GifEncoder{
    fn write_frame(&mut self, c: &Context, preset: &s::EncoderPreset, frame: &mut BitmapBgra, decoder_io_ids: &[i32]) -> Result<s::EncodeResult> {

        let mut decoded_frame = None;
        let mut repeat = None;
        for io_id in decoder_io_ids{

            let mut codec = c.get_codec(*io_id).map_err(|e| e.at(here!()))?;
            let gif_decoder = codec.get_decoder().map_err(|e| e.at(here!()))?.as_any().downcast_ref::<GifDecoder>();

            if let Some(d) = gif_decoder {

                repeat = d.get_repeat();
                decoded_frame = d.last_frame.clone(); //TODO: clones local palette; expensive, not used
                break;
            }
        }

//        eprintln!("decoders: {:?}, found_frame: {}", decoder_io_ids, decoded_frame.is_some() );

        unsafe {
            let mut pixels = Vec::new();
            pixels.extend_from_slice(frame.pixels_slice_mut().expect("Frame must have pixel buffer"));

            let mut f = match frame.fmt {
                ::ffi::PixelFormat::Bgr24 => Ok(from_bgr_with_stride(frame.w as u16, frame.h as u16, &mut pixels, frame.stride as usize)),
                ::ffi::PixelFormat::Bgra32 => Ok(from_bgra_with_stride(frame.w as u16, frame.h as u16, &mut pixels, frame.stride as usize)),
                ::ffi::PixelFormat::Bgr32 => Ok(from_bgrx_with_stride(frame.w as u16, frame.h as u16, &mut pixels, frame.stride as usize)),
                other =>  Err(nerror!(ErrorKind::InvalidArgument, "PixelFormat {:?} not supported for gif encoding", frame.fmt))
            }?;

            if let Some(from) = decoded_frame{
                f.delay = from.delay;
                f.needs_user_input = from.needs_user_input;
            }
            if self.frame_ix == 0 {
                // Only write before any frames
                if let Some(r) = repeat {
//                    eprintln!("Writing repeat");
                    self.encoder.write_extension(::gif::ExtensionData::Repetitions(r)).map_err(|e| FlowError::from_gif_encoder(e).at(here!()))?;
                }else{
//                    eprintln!("Skipping repeat");
                }
            }




            // TODO: Overhaul encoding
            // delay
            // dispose method
            // rect
            // transparency??

            self.encoder.write_frame(&f).map_err(|e| FlowError::from_gif_encoder(e).at(here!()))?;

            self.frame_ix+=1;
            Ok(
                s::EncodeResult{
                    w: frame.w as i32,
                    h: frame.h as i32,
                    io_id: self.io_id,
                    bytes: ::imageflow_types::ResultBytes::Elsewhere,
                    preferred_extension: "gif".to_owned(),
                    preferred_mime_type: "image/gif".to_owned()
                }
            )
        }
    }
    fn get_io(&self) -> Result<&IoProxy> {
        Ok(self.io_ref)
    }
}


fn remove_padding(width: u16, pixels: &[u8], stride: usize) -> Vec<u8>{
    pixels.chunks(stride).flat_map(|s| s[0..width as usize * 4].iter().map(|v| *v)).collect()
}
/// Creates a frame from pixels in RGBA format.
///
/// *Note: This method is not optimized for speed.*
pub fn from_bgra_with_stride(width: u16, height: u16, pixels: &mut [u8], stride: usize) -> ::gif::Frame<'static> {
    let mut without_padding = remove_padding(width, pixels, stride);
    for pix in without_padding.chunks_mut(4) {
        let a = pix[0];
        pix[0] = pix[2];
        pix[2] = a;
    }
    ::gif::Frame::from_rgba(width, height, &mut without_padding)
}

pub fn from_bgrx_with_stride(width: u16, height: u16, pixels: &mut [u8], stride: usize) -> ::gif::Frame<'static> {
    let mut without_padding = remove_padding(width, pixels, stride);

    for pix in without_padding.chunks_mut(4) {
        let a = pix[0];
        pix[0] = pix[2];
        pix[2] = a;
        pix[3] = 0xFF;
    }
    ::gif::Frame::from_rgba(width, height, &mut without_padding)
}



/// Creates a frame from pixels in RGB format.
///
/// *Note: This method is not optimized for speed.*
pub fn from_bgr_with_stride(width: u16, height: u16, pixels: &[u8], stride: usize) -> ::gif::Frame<'static> {
    let mut without_padding = remove_padding(width, pixels, stride);
    for pix in without_padding.chunks_mut(3) {
        let a = pix[0];
        pix[0] = pix[2];
        pix[2] = a;
    }
    ::gif::Frame::from_rgb(width, height, &mut without_padding)
}
