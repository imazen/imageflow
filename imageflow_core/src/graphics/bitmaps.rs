use crate::{FlowError, ErrorKind};
use crate::ffi::{PixelFormat, BitmapFloat, BitmapBgra, BitmapCompositingMode};
use imageflow_types::{PixelBuffer, CompositingMode};
use imgref::ImgRef;
use std::slice;
use slotmap::*;
use crate::graphics::aligned_buffer::AlignedBuffer;
use std::cell::{RefCell, RefMut};
use std;
use std::ops::DerefMut;
use crate::ErrorKind::BitmapPointerNull;





new_key_type! {
    pub struct BitmapKey;
}

pub struct BitmapsContainer{
    map: ::slotmap::DenseSlotMap<BitmapKey, RefCell<Bitmap>>,
}

impl BitmapsContainer{
    pub fn with_capacity(capacity: usize) -> BitmapsContainer{
        BitmapsContainer{
            map: ::slotmap::DenseSlotMap::<BitmapKey, RefCell<Bitmap>>::with_capacity_and_key(capacity)
        }
    }
    pub fn get(&self, key: BitmapKey) -> Option<&RefCell<Bitmap>>{
        self.map.get(key)
    }

    pub fn try_borrow_mut(&self, key: BitmapKey) -> Result<RefMut<Bitmap>, FlowError> {
        self.get(key).ok_or_else(|| nerror!(ErrorKind::BitmapKeyNotFound))?
            .try_borrow_mut()
            .map_err(|e| nerror!(ErrorKind::FailedBorrow))
    }

    pub fn free(&mut self, key: BitmapKey) -> bool {
        self.map.remove(key).is_some()
    }

    pub fn create_bitmap_f32(&mut self,
                            w: u32,
                            h: u32,
                            pixel_layout: PixelLayout,
                            alpha_premultiplied: bool,
                            alpha_meaningful: bool,
                            color_space: ColorSpace) -> Result<BitmapKey, FlowError>{
        Ok(self.map.insert(RefCell::new(Bitmap::create_float(w,h,pixel_layout, alpha_premultiplied, alpha_meaningful, color_space)?)))
    }

    pub fn create_bitmap_u8(&mut self,
                             w: u32,
                             h: u32,
                             pixel_layout: PixelLayout,
                             alpha_premultiplied: bool,
                             alpha_meaningful: bool,
                             color_space: ColorSpace,
                             compose: BitmapCompositing) -> Result<BitmapKey, FlowError>{
        Ok(self.map.insert(RefCell::new(Bitmap::create_u8(w,h,pixel_layout, alpha_premultiplied, alpha_meaningful, color_space, compose)?)))
    }
}

#[test]
fn crop_bitmap(){
    let mut c = BitmapsContainer::with_capacity(2);
    let b1 =
        c.create_bitmap_f32(10,10, PixelLayout::BGRA, false, true, ColorSpace::LinearRGB)
            .unwrap();
    let b2 =
        c.create_bitmap_u8(10,10, PixelLayout::BGRA, false, true, ColorSpace::StandardRGB, BitmapCompositing::ReplaceSelf)
            .unwrap();

    let mut bitmap = c.get(b1).unwrap().borrow_mut();
    let mut full_window = bitmap.get_window_f32().unwrap();
    let mut window = full_window.window(1,1,6,6).unwrap();
    window.slice()[0] = 3f32;

    bitmap.set_alpha_meaningful(false);

}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ColorSpace{
    StandardRGB,
    LinearRGB
}



#[derive(Clone,Debug,PartialEq)]
pub enum BitmapCompositing {
    ReplaceSelf,
    BlendWithSelf,
    BlendWithMatte(imageflow_types::Color),
}
pub use imageflow_types::PixelLayout;


pub enum BitmapBuffer{
    Floats(AlignedBuffer<f32>),
    Bytes(AlignedBuffer<u8>),
}
impl BitmapBuffer{

}

pub struct BitmapWindowMut<'a, T>{
    slice: &'a mut [T],
    info: BitmapInfo
}

impl<'a> BitmapWindowMut<'a,u8> {
    pub fn apply_matte(&mut self, matte: imageflow_types::Color) -> Result<(), FlowError> {
        crate::graphics::blend::apply_matte(self, matte)
    }

    pub fn slice_of_pixels(&mut self) -> Option<&mut [rgb::alt::BGRA8]>{
        if self.info().channels() != 4 || self.slice.len() %4 != 0{
            return None;
        }
        unsafe {
            Some(core::slice::from_raw_parts_mut(self.slice.as_mut_ptr() as *mut rgb::alt::BGRA8, self.slice.len() / 4))
        }
    }
}
impl<'a,T>  BitmapWindowMut<'a, T> {

    pub unsafe fn to_bitmap_float(&mut self) -> Result<BitmapFloat, FlowError>{
        if std::mem::size_of::<T>() != 4{
            return Err(nerror!(ErrorKind::InvalidState));
        }

        Ok(BitmapFloat {
            w: self.w() as u32,
            h: self.h() as u32,
            pixels:  self.slice.as_mut_ptr() as *mut f32,
            pixels_borrowed: true,
            channels: self.info().channels() as u32,
            alpha_meaningful: self.info().alpha_meaningful,
            alpha_premultiplied: self.info().alpha_premultiplied,
            float_stride: self.info().item_stride(),
            float_count: self.info().item_stride() * self.h()
        })
    }

    pub unsafe fn to_bitmap_bgra(&mut self) -> Result<BitmapBgra, FlowError>{
        if std::mem::size_of::<T>() != 1{
            return Err(nerror!(ErrorKind::InvalidState));
        }

        let fmt = self.info().calculate_pixel_format()
            .map_err(|e| e.at(here!()))?;


        let mut b = BitmapBgra {
            w: self.w() as u32,
            h: self.h() as u32,
            stride: self.info.item_stride,
            pixels: self.slice.as_mut_ptr() as *mut u8,
            fmt,
            matte_color: [0;4],
            compositing_mode: crate::ffi::BitmapCompositingMode::ReplaceSelf
        };


        match &self.info().compose{
            BitmapCompositing::ReplaceSelf =>
                {b.compositing_mode = crate::ffi::BitmapCompositingMode::ReplaceSelf},
            BitmapCompositing::BlendWithSelf =>
                {b.compositing_mode = crate::ffi::BitmapCompositingMode::BlendWithSelf},
            BitmapCompositing::BlendWithMatte(c) => {
                b.compositing_mode = crate::ffi::BitmapCompositingMode::BlendWithMatte;

                let color_val = c.clone();
                let color_srgb_argb = color_val.clone().to_u32_bgra().unwrap();

                b.matte_color = std::mem::transmute(color_srgb_argb);

                if c != &imageflow_types::Color::Transparent {
                    b.fill_rect(
                                          0,
                                          0,
                                          self.w(),
                                          self.h(),
                                          &color_val)?;
                }
            }
        }



        Ok(b)
    }


    pub fn w(&self) -> u32{
        self.info.width()
    }
    pub fn h(&self) -> u32{
        self.info.height()
    }

    /// Replaces all data with zeroes. Will zero data outside the window if this is a cropped window.
    pub fn clear_slice(&mut self){
        unsafe {
            std::ptr::write_bytes(self.slice.as_mut_ptr(), 0, self.slice.len() - 1);
        }
    }

    pub fn row(&mut self, index: u32) -> Option<&mut [T]>{
        if index >= self.info.h {
            None
        }else {
            let start_index = self.info.item_stride.checked_mul(index).unwrap() as usize;
            let end_index = start_index + self.info.w as usize * self.info.channels();
            Some(&mut self.slice[start_index..end_index])
        }
    }

    pub fn row_window(&mut self, index: u32) -> Option<BitmapWindowMut<T>>{
        let w= self.w();
        self.window(0, index, w, index + 1)
    }


    pub fn slice(&'a mut self) -> &'a mut [T]{
        self.slice
    }


    pub(crate) fn slice_ptr(&mut self) -> *mut T {
        self.slice.as_mut_ptr()
    }

    pub fn info(&'a self) -> &'a BitmapInfo{
        &self.info
    }
    pub fn window(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) -> Option<BitmapWindowMut<T>>{
        if x1 >= x2 || y1 >= y2 || x2 > self.info.width() || y2 > self.info.height(){
            return None;// Err(nerror!(ErrorKind::InvalidArgument, "x1,y1,x2,y2 must be within window bounds"));
        }
        let offset = x1 + (y1 * self.info.item_stride());
        Some(BitmapWindowMut{
            slice: &mut self.slice[offset as usize..],
            info: BitmapInfo {
                w: x2 - x1,
                h: y2 - y1,
                item_stride: self.info.item_stride(),
                alpha_premultiplied: self.info.alpha_premultiplied(),
                alpha_meaningful: self.info.alpha_meaningful(),
                color_space: self.info.color_space(),
                pixel_layout: self.info.pixel_layout(),
                compose: self.info.compose.clone()
            }
        })
    }
}


impl Bitmap{
    pub fn get_window_u8(&mut self) -> Option<BitmapWindowMut<u8>>{
        let info = self.info().clone();
        let offset = self.offset() as usize;

        self.get_u8_slice().map(|s| {
            BitmapWindowMut{
                slice: &mut s[offset..],
                info
            }
        })
    }

    pub fn get_window_f32(&mut self) -> Option<BitmapWindowMut<f32>>{
        let info = self.info().clone();
        let offset = self.offset() as usize;

        self.get_f32_slice().map(|s| {
            BitmapWindowMut{
                slice: &mut s[offset..],
                info
            }
        })
    }

    fn get_u8_slice(&mut self) -> Option<&mut [u8]>{
        if let BitmapBuffer::Bytes(ref mut buf) = &mut (*(self)).buffer{
            return Some(buf.as_slice_mut())
        }else{
            None
        }
    }
    fn get_f32_slice(&mut self) -> Option<&mut [f32]>{
        if let BitmapBuffer::Floats(ref mut buf) = &mut (*(self)).buffer{
            return Some(buf.as_slice_mut())
        }else{
            None
        }
    }
}



#[derive(Clone, Debug)]
pub struct BitmapInfo{
    w: u32,
    h: u32,
    item_stride: u32,
    alpha_premultiplied: bool,
    alpha_meaningful: bool,
    color_space: ColorSpace,
    pixel_layout: PixelLayout,
    compose: BitmapCompositing
}

impl BitmapInfo {
    pub(crate) fn calculate_pixel_format(&self) -> Result<PixelFormat, FlowError> {
        Ok(match self.pixel_layout(){
            PixelLayout::BGR => PixelFormat::Bgr24,
            PixelLayout::BGRA if self.alpha_meaningful() => PixelFormat::Bgra32,
            PixelLayout::BGRA if !self.alpha_meaningful() => PixelFormat::Bgr32,
            PixelLayout::Gray => PixelFormat::Gray8,
            _ => { return Err(nerror!(ErrorKind::InvalidState)); }
        })
    }
}

impl BitmapInfo{
    #[inline]
    pub fn width(&self) -> u32{
        self.w
    }
    #[inline]
    pub fn height(&self) -> u32{
        self.h
    }
    #[inline]
    pub fn item_stride(&self) -> u32{
        self.item_stride
    }
    #[inline]
    pub fn pixel_layout(&self) -> PixelLayout{
        self.pixel_layout
    }
    #[inline]
    pub fn color_space(&self) -> ColorSpace{
        self.color_space
    }
    #[inline]
    pub fn channels(&self) -> usize{
        self.pixel_layout.channels()
    }
    #[inline]
    pub fn alpha_premultiplied(&self) -> bool{
        self.alpha_premultiplied
    }
    #[inline]
    pub fn alpha_meaningful(&self) -> bool{
        self.alpha_meaningful
    }
    #[inline]
    pub fn compose(&self) -> &BitmapCompositing{
        &self.compose
    }
}
pub struct Bitmap{
    buffer: BitmapBuffer,
    offset: u32,
    info: BitmapInfo
}
impl Bitmap{
    #[inline]
    pub fn offset(&self) -> u32{
        self.offset
    }
    #[inline]
    pub fn info(&self) -> &BitmapInfo{
        &self.info
    }
    #[inline]
    pub fn set_alpha_meaningful(&mut self, value: bool){
        self.info.alpha_meaningful = value;
    }
    #[inline]
    pub fn set_compositing(&mut self, value: BitmapCompositing){
        self.info.compose = value;
    }
    #[inline]
    pub fn w(&self) -> u32{
        self.info.w
    }
    #[inline]
    pub fn h(&self) -> u32{
        self.info.h
    }

    pub fn frame_info(&self) -> crate::flow::definitions::FrameInfo {
        crate::flow::definitions::FrameInfo {
            w: self.w() as i32,
            h: self.h() as i32,
            fmt: self.info().calculate_pixel_format()
                .expect("Only call frame_info() on classic bitmap_bgra formats")
        }
    }

    pub fn check_dimensions<T>(w: usize, h: usize) -> Result<(), FlowError>
    {
        if w == 0 || h == 0{
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap dimensions cannot be zero"))
        };
        if w.saturating_mul(std::mem::size_of::<T>()) >= i32::max_value() as usize / h{
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap dimensions cannot be so large they would cause i32 overflow"))
        }
        Ok(())
    }

    pub fn get_stride<T>(w: usize, h: usize, items_per_pixel: usize,  alignment_in_bytes: usize) -> Result<u32,FlowError>{

        Bitmap::check_dimensions::<T>(w, items_per_pixel)
            .map_err(|e| e.at(here!()))?;

        let un_padded_stride = w * items_per_pixel;
        let alignment = alignment_in_bytes / std::mem::size_of::<T>();
        if alignment_in_bytes % std::mem::size_of::<T>() != 0{
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap alignment must be multiple of type size"));
        }

        let padding = if un_padded_stride % alignment == 0 { 0 } else { alignment - (un_padded_stride % alignment)};

        let stride = un_padded_stride.saturating_add(padding);

        Bitmap::check_dimensions::<T>(stride, h).map_err(|e| e.at(here!()))?;

        Ok(stride as u32)
    }

   pub(crate) fn create_float(w: u32,
                            h: u32,
                            pixel_layout: PixelLayout,
                            alpha_premultiplied: bool,
                            alpha_meaningful: bool,
                            color_space: ColorSpace) -> Result<Bitmap, FlowError>{
        // Pad rows to 64 bytes (this does not guarantee memory alignment, just stride alignment)
        let stride = Bitmap::get_stride::<f32>(w as usize, h as usize, pixel_layout.channels(), 64)?;

       //TODO: Note that allocs could be aligned to 16 instead of 64 bytes.
        Ok(Bitmap{
            buffer: BitmapBuffer::Floats(AlignedBuffer::new(stride as usize * h as usize, 64)
                .map_err(|e| nerror!(ErrorKind::AllocationFailed))?),
            offset: 0,
            info: BitmapInfo {
                w,
                h,
                item_stride: stride,
                color_space,
                alpha_premultiplied,
                alpha_meaningful,
                pixel_layout,
                compose: BitmapCompositing::BlendWithSelf
            }
        })
    }
    pub fn create_u8(w: u32,
                    h: u32,
                    pixel_layout: PixelLayout,
                    alpha_premultiplied: bool,
                    alpha_meaningful: bool,
                    color_space: ColorSpace,
                    compositing_mode: BitmapCompositing) -> Result<Bitmap, FlowError>{

        // Pad rows to 64 bytes (this does not guarantee memory alignment, just stride alignment)
        let stride = Bitmap::get_stride::<u8>(w as usize, h as usize, pixel_layout.channels(), 64)?;

        //TODO: Note that allocs could be aligned to 16 instead of 64 bytes.
        let mut b = Bitmap{
            buffer: BitmapBuffer::Bytes(AlignedBuffer::new(stride as usize * h as usize, 64)
                .map_err(|e| nerror!(ErrorKind::AllocationFailed,
                 "Failed to allocate {}x{}x{} bitmap ({} bytes). Reduce dimensions or increase RAM.",
                  w, h, pixel_layout.channels(), w as usize * h as usize * pixel_layout.channels()))?),
            offset: 0,
            info: BitmapInfo {
                w,
                h,
                item_stride: stride,
                color_space,
                alpha_premultiplied,
                alpha_meaningful,
                pixel_layout,
                compose: compositing_mode.clone()
            }
        };

        if let BitmapCompositing::BlendWithMatte(c) = compositing_mode{
            let color_val = c.clone();
            let color_srgb_argb = color_val.clone().to_u32_bgra().unwrap();
            if color_val != imageflow_types::Color::Transparent {
                unsafe {
                    b.get_window_u8().unwrap().to_bitmap_bgra()
                        .map_err(|e| e.at(here!()))?
                        .fill_rect(0,
                                  0,
                                  w as u32,
                                  h as u32,
                                  &color_val)?;
                }
            }
        }
        Ok(b)
    }

    pub fn crop(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) -> Result<(), FlowError>{
        let (w, h) = (self.w(), self.h());
        if x2 <= x1 || y2 <= y1 || x2 > w || y2 > h {
            return Err(nerror!(ErrorKind::InvalidArgument,
            "Invalid crop bounds {:?} (image {}x{})", ((x1, y1), (x2, y2)), w, h));
        }

        self.offset = self.offset + (self.info.item_stride * y1) + (self.info.pixel_layout().channels() as u32 * x1);
        self.info.w = x2 - x1;
        self.info.h = y2 - y1;
        Ok(())
    }
}


