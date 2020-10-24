use crate::{FlowError, ErrorKind};
use crate::ffi::PixelFormat;
use imageflow_types::PixelBuffer;
use imgref::ImgRef;
use std::slice;
use slotmap::*;
use crate::graphics::aligned_buffer::AlignedBuffer;
use std::cell::RefCell;
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
    pub fn new() -> BitmapsContainer{
        BitmapsContainer{
            map: ::slotmap::DenseSlotMap::<BitmapKey, RefCell<Bitmap>>::with_capacity_and_key(16)
        }
    }
    pub fn get(&self, key: BitmapKey) -> Option<&RefCell<Bitmap>>{
        self.map.get(key)
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
                             color_space: ColorSpace) -> Result<BitmapKey, FlowError>{
        Ok(self.map.insert(RefCell::new(Bitmap::create_u8(w,h,pixel_layout, alpha_premultiplied, alpha_meaningful, color_space)?)))
    }
}

#[test]
fn crop_bitmap(){
    let mut c = BitmapsContainer::new();
    let b1 =
        c.create_bitmap_f32(10,10, PixelLayout::BGRA, false, true, ColorSpace::LinearRGB)
            .unwrap();
    let b2 =
        c.create_bitmap_u8(10,10, PixelLayout::BGRA, false, true, ColorSpace::StandardRGB)
            .unwrap();

    let mut bitmap = c.get(b1).unwrap().borrow_mut();
    let mut window = bitmap.get_window_f32().unwrap().window(1,1,6,6).unwrap();
    window.slice()[0] = 3f32;

    bitmap.set_alpha_meaningful(false);

}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ColorSpace{
    StandardRGB,
    LinearRGB
}
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PixelLayout{
    BGR,
    BGRA,
    Gray
}


#[derive(Clone,Debug,PartialEq)]
pub enum BitmapCompositing {
    ReplaceSelf,
    BlendWithSelf,
    BlendWithMatte(imageflow_types::Color),
}


impl PixelLayout{
    pub fn channels(&self) -> usize{
        match self{
            PixelLayout::BGR => 3,
            PixelLayout::BGRA => 4,
            PixelLayout::Gray => 1
        }
    }
}
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
impl<'a,T>  BitmapWindowMut<'a, T> {
    pub fn slice(&'a mut self) -> &'a mut [T]{
        self.slice
    }
    pub fn info(&'a self) -> &'a BitmapInfo{
        &self.info
    }
    pub fn window(self, x1: u32, y1: u32, x2: u32, y2: u32) -> Result<BitmapWindowMut<'a, T>, FlowError>{
        if x1 >= x2 || y1 >= y2 || x2 > self.info.width() || y2 > self.info.height(){
            return Err(nerror!(ErrorKind::InvalidArgument, "x1,y1,x2,y2 must be within window bounds"));
        }
        let offset = x1 + (y1 * self.info.item_stride());
        Ok(BitmapWindowMut{
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
    fn get_window_u8(&mut self) -> Option<BitmapWindowMut<u8>>{
        let info = self.info().clone();
        let offset = self.offset() as usize;

        self.get_u8_slice().map(|s| {
            BitmapWindowMut{
                slice: &mut s[offset..],
                info
            }
        })
    }

    fn get_window_f32(&mut self) -> Option<BitmapWindowMut<f32>>{
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
    pub fn info(&self) -> &BitmapInfo{
        &self.info
    }
    pub fn set_alpha_meaningful(&mut self, value: bool){
        self.info.alpha_meaningful = value;
    }


    fn check_dimensions<T>(w: usize, h: usize) -> Result<(), FlowError>
    {
        if w == 0 || h == 0{
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap dimensions cannot be zero"))
        };
        if w.saturating_mul(std::mem::size_of::<T>()) >= i32::max_value() as usize / h{
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap dimensions cannot be so large they would cause i32 overflow"))
        }
        Ok(())
    }

    fn get_stride<T>(w: usize, h: usize, items_per_pixel: usize,  alignment_in_bytes: usize) -> Result<u32,FlowError>{

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

   fn create_float(w: u32,
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
    fn create_u8(w: u32,
                    h: u32,
                    pixel_layout: PixelLayout,
                    alpha_premultiplied: bool,
                    alpha_meaningful: bool,
                    color_space: ColorSpace) -> Result<Bitmap, FlowError>{

        // Pad rows to 64 bytes (this does not guarantee memory alignment, just stride alignment)
        let stride = Bitmap::get_stride::<u8>(w as usize, h as usize, pixel_layout.channels(), 64)?;


        //TODO: Note that allocs could be aligned to 16 instead of 64 bytes.
        Ok(Bitmap{
            buffer: BitmapBuffer::Bytes(AlignedBuffer::new(stride as usize * h as usize, 64)
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
}


