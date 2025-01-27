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
use std::fmt;




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
        let lookup = self.get(key);
        if lookup.is_none(){
            // collect all the slotmap keys
            let keys = self.map.keys().map(|key| format!("{:?}",key)).collect::<Vec<String>>().join(",");
            return  Err(nerror!(ErrorKind::BitmapKeyNotFound, "Could not find key {:?} in slotmap {:p} of length {:?} and keys {:?}", key, &self.map, self.map.len(), keys));
        }
        lookup.unwrap()
            .try_borrow_mut()
            .map_err(|e| nerror!(ErrorKind::FailedBorrow))
    }

    pub fn free(&mut self, key: BitmapKey) -> bool {
        // eprintln!("Freeing {:?} from slotmap {:p}", key, &self.map);
        self.map.remove(key).is_some()
    }

    pub fn create_bitmap_f32(&mut self,
                            w: u32,
                            h: u32,
                            pixel_layout: PixelLayout,
                            alpha_premultiplied: bool,
                            alpha_meaningful: bool,
                            color_space: ColorSpace) -> Result<BitmapKey, FlowError>{
        let key = self.map.insert(RefCell::new(Bitmap::create_float(w,h,pixel_layout, alpha_premultiplied, alpha_meaningful, color_space)?));
        // eprintln!("Creating bitmap {:?} in slotmap {:p}", key, &self.map);
        Ok(key)
    }

    pub fn create_bitmap_u8(&mut self,
                             w: u32,
                             h: u32,
                             pixel_layout: PixelLayout,
                             alpha_premultiplied: bool,
                             alpha_meaningful: bool,
                             color_space: ColorSpace,
                             compose: BitmapCompositing) -> Result<BitmapKey, FlowError>{
        let key = self.map.insert(RefCell::new(Bitmap::create_u8(w,h,pixel_layout, alpha_premultiplied, alpha_meaningful, color_space, compose)?));
        // eprintln!("Creating bitmap {:?} in slotmap {:p}", key, &self.map);
        Ok(key)
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
    window.slice_mut()[0] = 3f32;

    bitmap.set_alpha_meaningful(false);

    let _ = c.get(b1).unwrap();
    let _ = c.get(b2).unwrap();

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
    info: BitmapInfo,
    is_sub_window: bool
}

// impl debug for Bitmap
impl<'a, T> fmt::Debug for BitmapWindowMut<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = std::any::type_name::<T>();

        write!(f, "BitmapWindowMut<{}> {{ slice: {} w: {}, h: {}, channels: {}, info: {:?} }}", type_name, self.slice.len(), self.w(), self.h(), self.channels(), self.info )
    }
}

impl<'a> BitmapWindowMut<'a,u8> {
    pub fn apply_matte(&mut self, matte: imageflow_types::Color) -> Result<(), FlowError> {
        crate::graphics::blend::apply_matte(self, matte)
    }

    pub fn slice_of_pixels_first_row(&mut self) -> Option<&mut [rgb::alt::BGRA8]>{
        if self.info().channels() != 4 || self.slice.len() %4 != 0{
            return None;
        }
        unsafe {
            Some(core::slice::from_raw_parts_mut(self.slice.as_mut_ptr() as *mut rgb::alt::BGRA8, (self.slice.len() / 4).min(self.info.w as usize)))
        }
    }

    pub fn get_pixel_bgra8(&self, x: u32, y: u32) -> Option<rgb::alt::BGRA<u8>>   {
        if self.info().channels() != 4 || self.slice.len() %4 != 0{
            return None;
        }
        let index = (y * self.info.item_stride + x * 4) as usize;
        let pixel =  bytemuck::cast_slice::<u8,rgb::alt::BGRA8>(&self.slice[index..index+4]);
        Some(pixel[0])
    }

    pub unsafe fn to_vec_rgba(&self) -> Result<(Vec<rgb::RGBA8>, usize, usize), FlowError>{

        let w = self.w() as usize;
        let h = self.h() as usize;

        match &self.info().compose(){
            BitmapCompositing::ReplaceSelf | BitmapCompositing::BlendWithSelf =>{
                let mut v = vec![rgb::RGBA8::new(0,0,0,255);w * h];

                if self.info().channels() != 4 || self.slice.len() %4 != 0{
                    return Err(unimpl!("Only Bgr(a)32 supported"));
                }

                // TODO: if alpha might be random, we should clear it if self.info.alpha_meaningful(){

                let mut y = 0;
                for stride_row in self.slice.chunks(self.info.item_stride as usize){
                    for x in 0..w{
                        v[y * w + x].b = stride_row[x * 4 + 0];
                        v[y * w + x].g = stride_row[x * 4 + 1];
                        v[y * w + x].r = stride_row[x * 4 + 2];
                        v[y * w + x].a = stride_row[x * 4 + 3];
                    }
                    y = y + 1;
                }


                Ok((v, w, h))
            } BitmapCompositing::BlendWithMatte(c) => {
                let matte = c.clone().to_color_32().unwrap().to_rgba8();
                Ok((vec![matte;w * h], w, h))
            }
        }
    }

}

impl<'a,T>  BitmapWindowMut<'a, T> {

    #[inline]
    pub fn is_cropped(&self) -> bool{
        self.is_sub_window
    }

    pub fn stride_padding(&self) -> usize{
        self.info.item_stride as usize - self.info.w as usize * self.info.channels() as usize
    }
    pub fn create_contiguous_vec(&mut self) -> Result<Vec<T>, FlowError>
    where T: Clone{
        let width_in_t = self.w() as usize * self.channels() as usize;
        let final_len = self.h() as usize * width_in_t;
        let mut v = Vec::new();
        v.try_reserve(final_len).map_err(|e| nerror!(ErrorKind::InvalidOperation, "Failed to reserve memory for contiguous vec"))?;
        for row in self.slice.chunks(self.info.item_stride as usize).take(self.h() as usize){
            v.extend_from_slice(&row[0..width_in_t]);
        }
        assert_eq!(v.len(), final_len);
        Ok(v)
    }

    #[deprecated(since = "0.1.0", note = "Stop using BitmapBgra")]
    pub unsafe fn to_bitmap_bgra(&mut self) -> Result<BitmapBgra, FlowError>{
        if std::mem::size_of::<T>() != 1{
            return Err(nerror!(ErrorKind::InvalidState));
        }

        // zero width and zero height are invalid
        if self.w() == 0 || self.h() == 0 {
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap dimensions cannot be zero"));
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


        match &self.info().compose(){
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
    pub fn channels(&self) -> usize{
        self.info.channels()
    }


    pub fn row_mut(&mut self, index: u32) -> Option<&mut [T]>{
        if index >= self.info.h {
            None
        }else {
            let start_index = self.info.item_stride.checked_mul(index).unwrap() as usize;
            let end_index = start_index + self.info.w as usize * self.info.channels();
            Some(&mut self.slice[start_index..end_index])
        }
    }

    pub fn row(&self, index: u32) -> Option<&[T]>{
        if index >= self.info.h {
            None
        }else {
            let start_index = self.info.item_stride.checked_mul(index).unwrap() as usize;
            let end_index = start_index + self.info.w as usize * self.info.channels();
            Some(&self.slice[start_index..end_index])
        }
    }



    pub fn row_window(&mut self, index: u32) -> Option<BitmapWindowMut<T>>{
        let w= self.w();
        self.window(0, index, w, index + 1)
    }


    pub fn underlying_slice_mut(&mut self) -> &mut [T]{
        self.slice
    }

    pub fn slice_mut(&mut self) -> &mut [T]{
        //Exclude padding/alignment/stride after last pixel
        let last_pixel_offset = self.info.item_stride() * (self.info.h -1) + self.info.w * self.info.channels() as u32;
        self.slice[0..last_pixel_offset as usize].as_mut()
    }
    pub fn get_slice(&self) -> &[T]{
        //Exclude padding/alignment/stride after last pixel
        let last_pixel_offset = self.info.item_stride() * (self.info.h -1) + self.info.w * self.info.channels() as u32;
        self.slice[0..last_pixel_offset as usize].as_ref()
    }
    pub fn underlying_slice(&self) -> &[T]{
        self.slice
    }
    pub(crate) fn slice_ptr(&mut self) -> *mut T {
        self.slice.as_mut_ptr()
    }

    pub fn info(&'a self) -> &'a BitmapInfo{
        &self.info
    }
    pub fn clone_mut(&'a mut self) -> BitmapWindowMut<'a, T>{
        BitmapWindowMut{
            info: self.info.clone(),
            slice: self.slice,
            is_sub_window: self.is_sub_window
        }
    }

    pub fn window(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) -> Option<BitmapWindowMut<T>>{
        if x1 >= x2 || y1 >= y2 || x2 > self.info.width() || y2 > self.info.height(){
            return None;// Err(nerror!(ErrorKind::InvalidArgument, "x1,y1,x2,y2 must be within window bounds"));
        }
        let offset = (x1 * self.info.channels() as u32) + (y1 * self.info.item_stride());
        let (orig_w, orig_h) = (self.w(), self.h());
        Some(BitmapWindowMut{
            slice: &mut self.slice[offset as usize..],
            info: BitmapInfo {
                w: x2 - x1,
                h: y2 - y1,
                item_stride: self.info.item_stride(),
                info: self.info.info.clone()
            },
            is_sub_window: (x1,y1,x2,y2) != (0,0,orig_w,orig_h)
        })
    }

    // Split the window into two separate non-overlapping windows at the given y coordinate
    pub fn split_off(&mut self, y: u32) -> Option<BitmapWindowMut<'a, T>>{
        if y >= self.h() {
            return None;
        }
        // create 2 new BitmapInfo, but with different h values
        let mut info2 = self.info.clone();
        info2.h = self.h() - y;

        let s = std::mem::replace(&mut self.slice,  &mut []);

        let (top, bottom) = s.split_at_mut(y as usize * self.info.item_stride as usize);
        self.slice = top;
        self.info.h = y;
        Some(BitmapWindowMut{
            slice: bottom,
            info: info2,
            is_sub_window: true
        })
    }

}

impl<'a>  BitmapWindowMut<'a, u8> {

    pub fn fill_rect(&mut self, x: u32, y: u32, x2: u32, y2: u32, color: &imageflow_types::Color) -> Result<(), FlowError>{


        let color_srgb_argb = color.to_color_32()?;
        self.fill_rectangle(color_srgb_argb, x, y, x2, y2).map_err(|e| e.at(here!()))
    }
    pub fn fill_rectangle(&mut self, color: imageflow_helpers::colors::Color32, x: u32, y: u32, x2: u32, y2: u32) -> Result<(), FlowError>{
        if let BitmapCompositing::BlendWithMatte(_) = self.info().compose(){
            if self.is_sub_window || (x,y,x2,y2) != (0,0,self.w(),self.h()){
                return Err(nerror!(ErrorKind::InvalidArgument, "Cannot draw a rectangle on a sub-rectangle of a bitmap in BlendWithMatte mode"));
            }
        }
        if y2 == y || x2 == x { return Ok(()); } // Don't fail on zero width rect
        if y2 <= y || x2 <= x || x2 > self.w() || y2 > self.h(){
            return Err(nerror!(ErrorKind::InvalidArgument, "Coordinates {},{} {},{} must be within image dimensions {}x{}", x, y, x2, y2, self.w(), self.h()));        }
        if  self.info().pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(ErrorKind::InvalidArgument, "Only BGRA supported for fill_rectangle"));
        }
        let bgra = color.to_bgra8();


        let mut top = self.window(x, y, x2, y2).unwrap();

        // if y2 > y + 2{
        //     // Supposed to be a bit faster to memcpy than memset?
        //     let mut rest = top.split_off(1).unwrap();
        //     for top_lines in top.scanlines_bgra().unwrap(){
        //         top_lines.row.fill(bgra);
        //     }

        //     for line in rest.scanlines(){
        //         line.row.copy_from_slice(&top.slice_mut()[0..line.row.len()]);
        //     }
        // }else{
            for line in top.scanlines_bgra().unwrap(){
                line.row.fill(bgra);
            }
        //}

        Ok(())
    }

    pub fn set_alpha_to_255(&mut self) -> Result<(), FlowError>{
        for line in self.scanlines_bgra()?{
            for pix in line.row{
                pix.a = 255;
            }
        }
        Ok(())
    }

    pub fn normalize_unused_alpha(&mut self) -> Result<(), FlowError>{
        if self.info().alpha_meaningful(){
            return Ok(());
        }
        self.set_alpha_to_255()?;
        Ok(())
    }
}

impl Bitmap{
    pub fn get_window_u8(&mut self) -> Option<BitmapWindowMut<u8>>{
        let info = self.info().clone();
        let offset = self.offset() as usize;

        self.get_u8_slice().map(|s| {
            BitmapWindowMut{
                slice: &mut s[offset..],
                info,
                is_sub_window: false
            }
        })
    }

    pub fn get_window_f32(&mut self) -> Option<BitmapWindowMut<f32>>{
        let info = self.info().clone();
        let offset = self.offset() as usize;

        self.get_f32_slice().map(|s| {
            BitmapWindowMut{
                slice: &mut s[offset..],
                info,
                is_sub_window: false
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
pub struct SurfaceInfo{
    alpha_premultiplied: bool,
    alpha_meaningful: bool,
    color_space: ColorSpace,
    pixel_layout: PixelLayout,
    compose: BitmapCompositing
}

impl SurfaceInfo{
    pub fn alpha_premultiplied(&self) -> bool{
        self.alpha_premultiplied
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
    pub fn alpha_meaningful(&self) -> bool{
        self.alpha_meaningful
    }
    #[inline]
    pub fn compose(&self) -> &BitmapCompositing{
        &self.compose
    }
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

#[derive(Clone, Debug)]
pub struct BitmapInfo{
    w: u32,
    h: u32,
    /// Row stride
    item_stride: u32,
    info: SurfaceInfo,
}

impl BitmapInfo {
    pub fn calculate_pixel_format(&self) -> Result<PixelFormat, FlowError> {
        self.info.calculate_pixel_format()
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

    pub fn surface_info(&self) -> &SurfaceInfo{
        &self.info
    }

    /// Row stride
    #[inline]
    pub fn item_stride(&self) -> u32{
        self.item_stride
    }
    #[inline]
    pub fn pixel_layout(&self) -> PixelLayout{
        self.info.pixel_layout
    }
    #[inline]
    pub fn color_space(&self) -> ColorSpace{
        self.info.color_space
    }
    #[inline]
    pub fn channels(&self) -> usize{
        self.info.channels()
    }
    #[inline]
    pub fn alpha_premultiplied(&self) -> bool{
        self.info.alpha_premultiplied
    }
    #[inline]
    pub fn alpha_meaningful(&self) -> bool{
        self.info.alpha_meaningful
    }
    #[inline]
    pub fn compose(&self) -> &BitmapCompositing{
        &self.info.compose
    }
}
pub struct Bitmap{
    buffer: BitmapBuffer,
    offset: u32,
    info: BitmapInfo,
    cropped: bool
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
    pub fn size(&self) -> (usize, usize){
        (self.w() as usize, self.h() as usize)
    }
    #[inline]
    pub fn set_alpha_meaningful(&mut self, value: bool){
        self.info.info.alpha_meaningful = value;
    }
    #[inline]
    pub fn set_compositing(&mut self, value: BitmapCompositing){
        self.info.info.compose = value;
    }
    #[inline]
    pub fn w(&self) -> u32{
        self.info.w
    }
    #[inline]
    pub fn h(&self) -> u32{
        self.info.h
    }
    #[inline]
    pub fn is_cropped(&self) -> bool{
        self.cropped
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
                info: SurfaceInfo {
                    alpha_premultiplied,
                    alpha_meaningful,
                    color_space,
                    pixel_layout,
                    compose: BitmapCompositing::BlendWithSelf
                }
            },
            cropped: false
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
                info: SurfaceInfo {
                    alpha_premultiplied,
                    alpha_meaningful,
                    color_space,
                    pixel_layout,
                    compose: compositing_mode.clone()
                }
            },
            cropped: false
        };

        if let BitmapCompositing::BlendWithMatte(c) = compositing_mode{
            let color_val = c.clone();
            if color_val != imageflow_types::Color::Transparent {
                b.get_window_u8().unwrap()
                    .fill_rect(0, 0, w as u32, h as u32, &color_val)
                    .map_err(|e| e.at(here!()))?;
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
        self.cropped = true;
        Ok(())
    }

    pub fn get_pixel_bgra32(&mut self, x: u32, y: u32) -> Option<rgb::alt::BGRA<u8>> {
        let window = self.get_window_u8().unwrap();
        return window.get_pixel_bgra8(x, y);
    }
}

/// Iterator that yields scanlines from a bitmap window
pub struct ScanlineIterMut<'a, T> {
    info: &'a SurfaceInfo,
    remaining_slice: &'a mut [T],
    current_y: usize,
    t_per_pixel: usize,
    t_per_row: usize,
    t_stride: usize,
    w: usize,
    h: usize,
    finished: bool,
}
// Display format for ScanlineIterMut
// ScanlineIterMut({W}x{H}x{t_per_pixel}, y={current_y}) ({t_per_pixel} {T} per px * {w} => {t_per_row} {T} per row + padding {t_stride - t_per_row} => stride {t_stride} * ({h} - {y}) - offset {0} => {required_length}. Slice length {remaining_slice.len()})
impl<'a, T> fmt::Display for ScanlineIterMut<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name_of_t = std::any::type_name::<T>();
        let padding = self.t_stride - self.t_per_row;
        let required_length = self.t_stride * (self.h - self.current_y) - padding;
        let remaining_slice_len = self.remaining_slice.len();
        let t_per_pixel = self.t_per_pixel;
        let w = self.w;
        let t_per_row = self.t_per_row;
        let t_stride = self.t_stride;
        let h = self.h;
        let current_y = self.current_y;
        write!(f, "ScanlineIterMut({w}x{h}x{t_per_pixel}, y={current_y}) ({t_per_pixel} {name_of_t} per px * {w} => {t_per_row} {name_of_t} per row + padding {padding} => stride {t_stride} * ({h} - {current_y}) - padding {padding} => {required_length}. Slice length {remaining_slice_len})")
    }
}

impl<'a, T> ScanlineIterMut<'a, T> {
    pub fn new(slice: &'a mut [T], info: &'a BitmapInfo) -> Option<Self> {
        let t_per_pixel = info.channels();
        let t_per_row = info.width() as usize * t_per_pixel;
        let t_stride = info.item_stride as usize;
        let w = info.width() as usize;
        let h = info.height() as usize;
        let padding = t_stride - t_per_row;
        if slice.len() < t_stride * h - padding {
            return None;
        }
        Some(Self { info: info.surface_info(), remaining_slice: slice, current_y: 0, t_per_pixel, t_per_row, t_stride, w, h, finished: false })
    }
    pub fn try_cast_from<K>(from_slice: &'a mut [K], info: &'a BitmapInfo) -> Result<Self, FlowError>
    where T: rgb::Pod, K: rgb::Pod {
        let w = info.width() as usize;
        let h = info.height() as usize;
        let old_slice_len = from_slice.len();
        let old_stride = info.item_stride as usize;

        match bytemuck::try_cast_slice_mut::<K,T>(from_slice){
            Ok(slice) => {

                let (t_per_pixel, t_stride) = if old_slice_len > slice.len(){
                    let factor = old_slice_len / slice.len();
                    if old_slice_len % slice.len() != 0{
                        panic!("ScanlineIterMut:try_cast: new casted slice length {} not a multiple of old slice length {}: ", slice.len(), old_slice_len);
                    }
                    let t_per_pixel = info.channels() / factor;
                    let t_stride = info.item_stride as usize / factor;
                    (t_per_pixel, t_stride)
                }else if old_slice_len < slice.len(){
                    let inverse_factor = slice.len() / old_slice_len;
                    if slice.len() % old_slice_len != 0{
                        panic!("ScanlineIterMut:try_cast: new casted slice length {} not a multiple of old slice length {}: ", slice.len(), old_slice_len);
                    }
                    let t_per_pixel = info.channels() * inverse_factor;
                    let t_stride = info.item_stride as usize * inverse_factor;
                    (t_per_pixel, t_stride)
                }else{
                    (info.channels(), info.item_stride as usize)
                };

                let t_per_row = info.width() as usize * t_per_pixel;
                let padding = t_stride - t_per_row;
                if slice.len() < t_stride * h - padding {
                    panic!("ScanlineIterMut:try_cast: new slice too short: {} < {} * {} - {}", slice.len(), t_stride, h, padding);
                }

                return Ok(Self { info: info.surface_info(),
                     remaining_slice: slice, current_y: 0, t_per_pixel, t_per_row, t_stride, w, h, finished: false });
            }
            Err(e) => {
                return Err(nerror!(ErrorKind::InvalidArgument, "Failed to cast slice: {}", e));
            }
        }
    }
}

pub struct Scanline<'a, T> {
    y: usize,
    info: &'a SurfaceInfo,
    row: &'a mut [T],
    t_per_pixel: usize,
    w: usize,
    h: usize,
}
impl<'a, T> Scanline<'a, T>{
    pub fn info(&self) -> &SurfaceInfo{
        self.info
    }
    #[inline]
    pub fn width(&self) -> usize{
        self.w
    }
    #[inline]
    pub fn height(&self) -> usize{
        self.h
    }
    #[inline]
    pub fn row_item_per_pixel(&self) -> usize{
        self.t_per_pixel
    }
    #[inline]
    pub fn row(&self) -> &[T]{
        self.row
    }
    #[inline]
    pub fn row_mut(&mut self) -> &mut [T]{
        self.row
    }

    #[inline]
    pub fn y(&self) -> usize{
        self.y
    }
}

impl<'a, T> ExactSizeIterator for ScanlineIterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.h - self.current_y
    }
}

impl<'a, T> Iterator for ScanlineIterMut<'a, T> {
    type Item = Scanline<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        if self.current_y >= self.h {
            self.finished = true;
            return None;
        }

        // Take ownership of the slice temporarily
        let slice = std::mem::replace(&mut self.remaining_slice, &mut []);

        let return_row;
        if  self.current_y == self.h -1 {
            if slice.len() < self.t_per_row {
                panic!("Remaining_slice length {} is less than t_per_row {}, this should never happen. {}: ", slice.len(), self.t_per_row, &self);
            }
            return_row = &mut slice[..self.t_per_row];
            self.finished = true;
        }else{
            if slice.len() < self.t_stride {
                panic!("Remaining_slice length {} is less than t_stride {}, this should never happen. {}: ", slice.len(), self.t_stride, &self);
            }
            // Safe split
            let (row, next_slice) = slice.split_at_mut(self.t_stride);
            self.remaining_slice = next_slice;
            // Only return the actual pixel data, not the stride padding
            return_row = &mut row[..self.t_per_row];
        }

        let y = self.current_y;
        self.current_y += 1;

        Some(Scanline {
            y,
            info: self.info,
            row: return_row,
            t_per_pixel: self.t_per_pixel,
            w: self.w,
            h: self.h,
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            let remaining = self.len();
            (remaining, Some(remaining))
        }
    }
}

impl<'a> BitmapWindowMut<'a, u8> {
    /// Creates an iterator over u8 scanlines
    pub fn scanlines(&mut self) -> ScanlineIterMut<'_, u8> {
        ScanlineIterMut::new(self.slice, &self.info).unwrap()
    }

    /// Creates an iterator over BGRA scanlines
    pub fn scanlines_bgra(&mut self) -> Result<ScanlineIterMut<'_, rgb::alt::BGRA<u8>>, FlowError> {
        if self.info.pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap is not BGRA"));
        }
        return ScanlineIterMut::try_cast_from::<u8>(self.slice, &self.info);
    }

    /// Call normalize_alpha first; this function does not skip unused alpha bytes, only unused whole pixels.
    /// Otherwise Bgr32 may be non-deterministic
    pub fn short_hash_pixels(&mut self) -> u64{
        use std::hash::Hasher;
        let mut hash = ::twox_hash::XxHash64::with_seed(0x8ed1_2ad9_483d_28a0);
        for line in self.scanlines(){
            hash.write(line.row);
        }
        hash.finish()
    }


}

impl<'a> BitmapWindowMut<'a, f32> {
    /// Creates an iterator over f32 scanlines
    pub fn scanlines(&mut self) -> ScanlineIterMut<'_, f32> {
        ScanlineIterMut::new(self.slice, &self.info).unwrap()
    }
}

// Example usage test
#[test]
fn test_scanline_iterator() {
    let mut c = BitmapsContainer::with_capacity(1);
    let b1 = c.create_bitmap_u8(
        10, 10,
        PixelLayout::BGRA,
        false,
        true,
        ColorSpace::StandardRGB,
        BitmapCompositing::ReplaceSelf
    ).unwrap();

    let mut bitmap = c.try_borrow_mut(b1).unwrap();
    let mut window = bitmap.get_window_u8().unwrap();

    // Test u8 scanlines
    for scanline in window.scanlines() {
        assert_eq!(scanline.row.len(), scanline.w as usize * scanline.info.channels());
    }

    // Test BGRA scanlines
    for scanline in window.scanlines_bgra().unwrap() {
        assert_eq!(scanline.row.len(), scanline.w as usize);
        // Each item is one BGRA pixel
    }
}


