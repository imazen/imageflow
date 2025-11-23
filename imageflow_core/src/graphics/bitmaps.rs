use crate::ffi::{BitmapCompositingMode, PixelFormat};
use crate::graphics::aligned_buffer::AlignedBuffer;
use crate::ErrorKind::BitmapPointerNull;
use crate::{ErrorKind, FlowError};
use bytemuck::{try_cast_slice, try_cast_slice_mut, Pod};
use imageflow_helpers::colors::Color32;
use imageflow_types::{CompositingMode, PixelBuffer};
use imgref::ImgRef;
use rgb::{Bgra, GrayA, Gray_v09 as Gray, BGR8, BGRA8, RGB8, RGBA8};
use slotmap::*;
use std;
use std::cell::{RefCell, RefMut};
use std::fmt;
use std::ops::DerefMut;
use std::slice;

new_key_type! {
    pub struct BitmapKey;
}

pub struct BitmapsContainer {
    map: ::slotmap::DenseSlotMap<BitmapKey, RefCell<Bitmap>>,
}

// impl Drop for BitmapsContainer {
//     fn drop(&mut self) {
//         eprintln!("Dropping BitmapsContainer with {} bitmaps", self.map.len());
//     }
// }

impl BitmapsContainer {
    pub fn with_capacity(capacity: usize) -> BitmapsContainer {
        BitmapsContainer {
            map: ::slotmap::DenseSlotMap::<BitmapKey, RefCell<Bitmap>>::with_capacity_and_key(
                capacity,
            ),
        }
    }
    pub fn with_default_capacity() -> BitmapsContainer {
        Self::with_capacity(Self::default_capacity())
    }
    pub fn default_capacity() -> usize {
        3
    }
    pub fn get(&self, key: BitmapKey) -> Option<&RefCell<Bitmap>> {
        self.map.get(key)
    }

    pub fn try_borrow_mut(&self, key: BitmapKey) -> Result<RefMut<'_, Bitmap>, FlowError> {
        let lookup = self.get(key);
        if lookup.is_none() {
            // collect all the slotmap keys
            let keys =
                self.map.keys().map(|key| format!("{:?}", key)).collect::<Vec<String>>().join(",");
            return Err(nerror!(
                ErrorKind::BitmapKeyNotFound,
                "Could not find key {:?} in slotmap {:p} of length {:?} and keys {:?}",
                key,
                &self.map,
                self.map.len(),
                keys
            ));
        }
        lookup.unwrap().try_borrow_mut().map_err(|e| nerror!(ErrorKind::FailedBorrow))
    }

    pub fn free(&mut self, key: BitmapKey) -> bool {
        // eprintln!("Freeing {:?} from slotmap {:p}", key, &self.map);
        self.map.remove(key).is_some()
    }

    pub fn create_bitmap_f32(
        &mut self,
        w: u32,
        h: u32,
        pixel_layout: PixelLayout,
        alpha_premultiplied: bool,
        alpha_meaningful: bool,
        color_space: ColorSpace,
    ) -> Result<BitmapKey, FlowError> {
        let key = self.map.insert(RefCell::new(Bitmap::create_float(
            w,
            h,
            pixel_layout,
            alpha_premultiplied,
            alpha_meaningful,
            color_space,
        )?));
        // eprintln!("Creating bitmap {:?} in slotmap {:p}", key, &self.map);
        Ok(key)
    }

    pub fn create_bitmap_u8(
        &mut self,
        w: u32,
        h: u32,
        pixel_layout: PixelLayout,
        alpha_premultiplied: bool,
        alpha_meaningful: bool,
        color_space: ColorSpace,
        compose: BitmapCompositing,
    ) -> Result<BitmapKey, FlowError> {
        let key = self.map.insert(RefCell::new(Bitmap::create_u8(
            w,
            h,
            pixel_layout,
            alpha_premultiplied,
            alpha_meaningful,
            color_space,
            compose,
        )?));
        // eprintln!("Creating bitmap {:?} in slotmap {:p}", key, &self.map);
        Ok(key)
    }
}

#[test]
fn crop_bitmap() {
    let mut c = BitmapsContainer::with_capacity(2);
    let b1 =
        c.create_bitmap_f32(10, 10, PixelLayout::BGRA, false, true, ColorSpace::LinearRGB).unwrap();
    let b2 = c
        .create_bitmap_u8(
            10,
            10,
            PixelLayout::BGRA,
            false,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();

    let mut bitmap = c.get(b1).unwrap().borrow_mut();
    let mut full_window = bitmap.get_window_f32().unwrap();
    let mut window = full_window.window(1, 1, 6, 6).unwrap();
    window.slice_mut()[0] = 3f32;

    bitmap.set_alpha_meaningful(false);

    let _ = c.get(b1).unwrap();
    let _ = c.get(b2).unwrap();
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ColorSpace {
    StandardRGB,
    LinearRGB,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BitmapCompositing {
    ReplaceSelf,
    BlendWithSelf,
    BlendWithMatte(imageflow_types::Color),
}
pub use imageflow_types::PixelLayout;

pub enum BitmapBuffer {
    Floats(AlignedBuffer<f32>),
    Bytes(AlignedBuffer<u8>),
}
impl BitmapBuffer {}

pub struct BitmapWindowMut<'a, T> {
    slice: &'a mut [T],
    info: BitmapInfo,
    is_sub_window: bool,
}

// impl debug for Bitmap
impl<'a, T> fmt::Debug for BitmapWindowMut<'a, T>
where
    T: Pod,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = std::any::type_name::<T>();

        write!(
            f,
            "BitmapWindowMut<{}> {{ slice: {} w: {}, h: {}, t_per_pixel: {}, info: {:?} }}",
            type_name,
            self.slice.len(),
            self.w(),
            self.h(),
            self.t_per_pixel(),
            self.info
        )
    }
}

pub struct RowPointers<T> {
    pub rows: Vec<*mut T>,
    pub items_w: usize,
    pub t_per_pixel: usize,
    pub w: usize,
    pub h: usize,
}

impl<'a, T> BitmapWindowMut<'a, T>
where
    T: Pod,
{
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<&[T]> {
        let t_per_pixel = self.t_per_pixel();
        let index = (y * self.info.t_stride + x * t_per_pixel as u32) as usize;
        self.slice.get(index..index + t_per_pixel)
    }

    pub fn create_row_pointers(&mut self) -> Result<RowPointers<T>, FlowError> {
        let w = self.w() as usize;
        let h = self.h() as usize;
        let t_per_pixel = self.t_per_pixel();

        let w_items = t_per_pixel * w;
        let mut vec = Vec::with_capacity(h);
        vec.try_reserve(h).map_err(|e| {
            nerror!(ErrorKind::InvalidOperation, "Failed to reserve memory for row pointers")
        })?;
        for mut line in self.scanlines() {
            if line.row_mut().len() != w_items {
                return Err(nerror!(ErrorKind::InvalidOperation, "Row length mismatch"));
            }
            vec.push(line.row_mut().as_mut_ptr());
        }
        if vec.len() != h {
            return Err(nerror!(ErrorKind::InvalidOperation, "Row count mismatch"));
        }
        Ok(RowPointers { rows: vec, items_w: w_items, t_per_pixel, w, h })
    }
    #[inline]
    pub fn size_16(&self) -> Result<(u16, u16), FlowError> {
        let w = self.w();
        let h = self.h();
        if h > u16::MAX as u32 || w > u16::MAX as u32 {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Bitmap size {}x{} is too large to fit in a u16",
                w,
                h
            ));
        }
        Ok((w as u16, h as u16))
    }

    #[inline]
    pub fn size(&self) -> (u32, u32) {
        (self.w(), self.h())
    }
    #[inline]
    pub fn size_usize(&self) -> (usize, usize) {
        (self.w() as usize, self.h() as usize)
    }
    #[inline]
    pub fn size_i32(&self) -> (i32, i32) {
        if (self.w() > i32::MAX as u32) || (self.h() > i32::MAX as u32) {
            panic!("Bitmap size {}x{} is too large to fit in a i32", self.w(), self.h());
        }
        (self.w() as i32, self.h() as i32)
    }
    #[inline]
    pub fn is_cropped(&self) -> bool {
        self.is_sub_window
    }

    #[inline]
    pub fn t_stride(&self) -> usize {
        self.info.t_stride as usize
    }

    /// Does not mean this is u8 instead of f32
    #[inline]
    pub fn pixel_format(&self) -> PixelFormat {
        self.info.calculate_pixel_format().unwrap()
    }

    pub fn stride_padding(&self) -> usize {
        self.info.t_stride as usize - self.info.w as usize * self.t_per_pixel()
    }
    pub fn create_contiguous_vec(&mut self) -> Result<Vec<T>, FlowError>
    where
        T: Clone,
    {
        let width_in_t = self.w() as usize * self.t_per_pixel();
        let final_len = self.h() as usize * width_in_t;
        let mut v = Vec::new();
        v.try_reserve(final_len).map_err(|e| {
            nerror!(ErrorKind::InvalidOperation, "Failed to reserve memory for contiguous vec")
        })?;
        for row in self.slice.chunks(self.info.t_stride as usize).take(self.h() as usize) {
            v.extend_from_slice(&row[0..width_in_t]);
        }
        assert_eq!(v.len(), final_len);
        Ok(v)
    }

    #[inline]
    pub fn w(&self) -> u32 {
        self.info.width()
    }
    #[inline]
    pub fn h(&self) -> u32 {
        self.info.height()
    }
    #[inline]
    pub fn w_i32(&self) -> i32 {
        if self.w() > i32::MAX as u32 {
            panic!("Bitmap width {} is too large to fit in a i32", self.w());
        }
        self.w() as i32
    }
    #[inline]
    pub fn h_i32(&self) -> i32 {
        if self.h() > i32::MAX as u32 {
            panic!("Bitmap height {} is too large to fit in a i32", self.h());
        }
        self.h() as i32
    }

    #[inline]
    pub fn t_per_pixel(&self) -> usize {
        self.info.t_per_pixel()
    }

    pub fn row_mut(&mut self, index: usize) -> Option<&mut [T]> {
        if index >= self.info.h as usize {
            None
        } else {
            let start_index = (self.info.t_stride as usize).checked_mul(index).unwrap();
            let end_index = start_index + self.info.w as usize * self.t_per_pixel();
            Some(&mut self.slice[start_index..end_index])
        }
    }
    pub fn row(&self, index: usize) -> Option<&[T]> {
        if index >= self.info.h as usize {
            None
        } else {
            let start_index = (self.info.t_stride as usize).checked_mul(index).unwrap();
            let end_index = start_index + self.info.w as usize * self.t_per_pixel();
            Some(&self.slice[start_index..end_index])
        }
    }
    pub fn try_cast_row<K>(&self, index: usize) -> Option<&[K]>
    where
        K: Pod,
    {
        if index >= self.info.h as usize {
            None
        } else {
            let start_index = (self.info.t_stride as usize).checked_mul(index).unwrap();
            let end_index = start_index + self.info.w as usize * self.t_per_pixel();
            let subslice = &self.slice[start_index..end_index];
            bytemuck::try_cast_slice::<T, K>(subslice).ok()
        }
    }
    pub fn cast_row<K>(&self, index: usize) -> &[K]
    where
        K: Pod,
    {
        if index >= self.info.h as usize {
            panic!("Image row index {} out of bounds (height = {})", index, self.info.h);
        } else {
            let start_index = (self.info.t_stride as usize).checked_mul(index).unwrap();
            let end_index = start_index + self.info.w as usize * self.t_per_pixel();
            let subslice = &self.slice[start_index..end_index];
            bytemuck::cast_slice::<T, K>(subslice)
        }
    }

    pub fn row_window(&mut self, index: u32) -> Option<BitmapWindowMut<'_, T>> {
        let w = self.w();
        self.window(0, index, w, index + 1)
    }

    pub fn underlying_slice_mut(&mut self) -> &mut [T] {
        self.slice
    }

    pub fn slice_mut(&mut self) -> &mut [T] {
        //Exclude padding/alignment/stride after last pixel
        let last_pixel_offset =
            self.info.t_stride() * (self.info.h - 1) + self.info.w * self.t_per_pixel() as u32;
        self.slice[0..last_pixel_offset as usize].as_mut()
    }
    pub fn get_slice(&self) -> &[T] {
        //Exclude padding/alignment/stride after last pixel
        let last_pixel_offset =
            self.info.t_stride() * (self.info.h - 1) + self.info.w * self.t_per_pixel() as u32;
        self.slice[0..last_pixel_offset as usize].as_ref()
    }
    pub fn underlying_slice(&self) -> &[T] {
        self.slice
    }
    pub(crate) fn slice_ptr(&mut self) -> *mut T {
        self.slice.as_mut_ptr()
    }

    pub fn info(&'a self) -> &'a BitmapInfo {
        &self.info
    }
    pub fn clone_mut(&'a mut self) -> BitmapWindowMut<'a, T> {
        BitmapWindowMut {
            info: self.info.clone(),
            slice: self.slice,
            is_sub_window: self.is_sub_window,
        }
    }

    pub fn window(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) -> Option<BitmapWindowMut<'_, T>> {
        if x1 >= x2 || y1 >= y2 || x2 > self.info.width() || y2 > self.info.height() {
            return None; // Err(nerror!(ErrorKind::InvalidArgument, "x1,y1,x2,y2 must be within window bounds"));
        }
        let t_per_pixel = self.t_per_pixel();
        let offset = (x1 * t_per_pixel as u32) + (y1 * self.info.t_stride());
        let (orig_w, orig_h) = (self.w(), self.h());
        Some(BitmapWindowMut {
            slice: &mut self.slice[offset as usize..],
            info: BitmapInfo {
                w: x2 - x1,
                h: y2 - y1,
                t_stride: self.info.t_stride(),
                info: self.info.info.clone(),
                t_per_pixel: t_per_pixel as u32,
            },
            is_sub_window: (x1, y1, x2, y2) != (0, 0, orig_w, orig_h),
        })
    }

    // Split the window into two separate non-overlapping windows at the given y coordinate
    pub fn split_off(&mut self, y: u32) -> Option<BitmapWindowMut<'a, T>> {
        if y >= self.h() {
            return None;
        }
        // create 2 new BitmapInfo, but with different h values
        let mut info2 = self.info.clone();
        info2.h = self.h() - y;

        let s = std::mem::take(&mut self.slice);

        let (top, bottom) = s.split_at_mut(y as usize * self.info.t_stride as usize);
        self.slice = top;
        self.info.h = y;
        Some(BitmapWindowMut { slice: bottom, info: info2, is_sub_window: true })
    }
}

impl Bitmap {
    pub fn get_window_u8(&mut self) -> Option<BitmapWindowMut<'_, u8>> {
        let info = self.info().clone();

        self.get_u8_slice().map(|slice| BitmapWindowMut { slice, info, is_sub_window: false })
    }
    pub fn get_window_bgra32(&mut self) -> Option<BitmapWindowMut<'_, rgb::alt::BGRA<u8, u8>>> {
        let mut info = self.info().clone();
        if !info.t_stride.is_multiple_of(4) {
            return None;
        }
        info.t_stride /= 4;
        info.t_per_pixel = 1;

        self.get_bgra32_slice().map(|slice| BitmapWindowMut { slice, info, is_sub_window: false })
    }

    pub fn get_window_bgra_f32(&mut self) -> Option<BitmapWindowMut<'_, Bgra<f32>>> {
        let mut info = self.info().clone();
        if !info.t_stride.is_multiple_of(4) {
            return None;
        }
        info.t_stride /= 4;
        info.t_per_pixel = 1;

        self.get_bgra_f32_slice().map(|slice| BitmapWindowMut { slice, info, is_sub_window: false })
    }
    pub fn get_window_f32(&mut self) -> Option<BitmapWindowMut<'_, f32>> {
        let info = self.info().clone();
        self.get_f32_slice().map(|slice| BitmapWindowMut { slice, info, is_sub_window: false })
    }

    fn get_u8_slice(&mut self) -> Option<&mut [u8]> {
        let offset = self.offset() as usize;
        if let BitmapBuffer::Bytes(ref mut buf) = &mut (self).buffer {
            Some(&mut buf.as_slice_mut()[offset..])
        } else {
            None
        }
    }
    fn get_bgra32_slice(&mut self) -> Option<&mut [rgb::alt::BGRA<u8, u8>]> {
        if self.info().t_per_pixel() != 4 {
            return None;
        }
        let offset = self.offset() as usize / 4;
        if let BitmapBuffer::Bytes(ref mut buf) = &mut (self).buffer {
            match bytemuck::try_cast_slice_mut::<u8, rgb::alt::BGRA<u8, u8>>(buf.as_slice_mut()) {
                Ok(slice) => Some(&mut slice[offset..]),
                Err(_) => None,
            }
        } else {
            None
        }
    }
    fn get_f32_slice(&mut self) -> Option<&mut [f32]> {
        let offset = self.offset() as usize;
        if let BitmapBuffer::Floats(ref mut buf) = &mut (self).buffer {
            Some(&mut buf.as_slice_mut()[offset..])
        } else {
            None
        }
    }
    fn get_bgra_f32_slice(&mut self) -> Option<&mut [Bgra<f32>]> {
        if self.info().t_per_pixel() != 4 {
            return None;
        }
        let offset = self.offset() as usize / 4;
        if let BitmapBuffer::Floats(ref mut buf) = &mut (self).buffer {
            match bytemuck::try_cast_slice_mut::<f32, Bgra<f32>>(buf.as_slice_mut()) {
                Ok(slice) => Some(&mut slice[offset..]),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    pub fn apply_matte(&mut self, matte: imageflow_types::Color) -> Result<(), FlowError> {
        if self.info().pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(ErrorKind::InvalidState, "Cannot apply matte to non-BGRA bitmap"));
        }
        if self.info().alpha_meaningful() {
            let mut window = self.get_window_bgra32().unwrap();
            crate::graphics::blend::apply_matte(&mut window, matte.clone())
                .map_err(|e| e.at(here!()))?;
            if matte.is_opaque() {
                self.set_alpha_meaningful(false);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SurfaceInfo {
    alpha_premultiplied: bool,
    alpha_meaningful: bool,
    color_space: ColorSpace,
    pixel_layout: PixelLayout,
    compose: BitmapCompositing,
}

impl SurfaceInfo {
    pub fn alpha_premultiplied(&self) -> bool {
        self.alpha_premultiplied
    }
    #[inline]
    pub fn pixel_layout(&self) -> PixelLayout {
        self.pixel_layout
    }
    #[inline]
    pub fn color_space(&self) -> ColorSpace {
        self.color_space
    }

    #[inline]
    pub fn alpha_meaningful(&self) -> bool {
        self.alpha_meaningful
    }
    #[inline]
    pub fn compose(&self) -> &BitmapCompositing {
        &self.compose
    }
    pub(crate) fn calculate_pixel_format(&self) -> Result<PixelFormat, FlowError> {
        Ok(match self.pixel_layout() {
            PixelLayout::BGR => PixelFormat::Bgr24,
            PixelLayout::BGRA if self.alpha_meaningful() => PixelFormat::Bgra32,
            PixelLayout::BGRA if !self.alpha_meaningful() => PixelFormat::Bgr32,
            PixelLayout::Gray => PixelFormat::Gray8,
            _ => {
                return Err(nerror!(ErrorKind::InvalidState));
            }
        })
    }
}

#[derive(Clone, Debug)]
pub struct BitmapInfo {
    w: u32,
    h: u32,
    /// Row stride
    t_stride: u32,
    t_per_pixel: u32,
    info: SurfaceInfo,
}

impl BitmapInfo {
    pub fn calculate_pixel_format(&self) -> Result<PixelFormat, FlowError> {
        self.info.calculate_pixel_format()
    }
}

impl BitmapInfo {
    #[inline]
    pub fn width(&self) -> u32 {
        self.w
    }
    #[inline]
    pub fn height(&self) -> u32 {
        self.h
    }

    pub fn surface_info(&self) -> &SurfaceInfo {
        &self.info
    }

    /// Row stride
    #[inline]
    pub fn t_stride(&self) -> u32 {
        self.t_stride
    }

    #[inline]
    pub fn t_per_pixel(&self) -> usize {
        self.t_per_pixel as usize
    }
    #[inline]
    pub fn pixel_layout(&self) -> PixelLayout {
        self.info.pixel_layout
    }
    #[inline]
    pub fn color_space(&self) -> ColorSpace {
        self.info.color_space
    }

    #[inline]
    pub fn alpha_premultiplied(&self) -> bool {
        self.info.alpha_premultiplied
    }
    #[inline]
    pub fn alpha_meaningful(&self) -> bool {
        self.info.alpha_meaningful
    }
    #[inline]
    pub fn compose(&self) -> &BitmapCompositing {
        &self.info.compose
    }
}
pub struct Bitmap {
    buffer: BitmapBuffer,
    offset: u32,
    info: BitmapInfo,
    cropped: bool,
}
impl Bitmap {
    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset
    }
    #[inline]
    pub fn info(&self) -> &BitmapInfo {
        &self.info
    }
    #[inline]
    pub fn size(&self) -> (usize, usize) {
        (self.w() as usize, self.h() as usize)
    }
    #[inline]
    pub fn set_alpha_meaningful(&mut self, value: bool) {
        self.info.info.alpha_meaningful = value;
    }
    #[inline]
    pub fn set_compositing(&mut self, value: BitmapCompositing) {
        self.info.info.compose = value;
    }
    #[inline]
    pub fn w(&self) -> u32 {
        self.info.w
    }
    #[inline]
    pub fn h(&self) -> u32 {
        self.info.h
    }
    #[inline]
    pub fn is_cropped(&self) -> bool {
        self.cropped
    }
    pub fn frame_info(&self) -> crate::flow::definitions::FrameInfo {
        crate::flow::definitions::FrameInfo {
            w: self.w() as i32,
            h: self.h() as i32,
            fmt: self
                .info()
                .calculate_pixel_format()
                .expect("Only call frame_info() on classic bitmap_bgra formats"),
        }
    }

    pub fn check_dimensions<T>(w: usize, h: usize) -> Result<(), FlowError> {
        if w == 0 || h == 0 {
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap dimensions cannot be zero"));
        };
        if w.saturating_mul(std::mem::size_of::<T>()) >= i32::max_value() as usize / h {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Bitmap dimensions cannot be so large they would cause i32 overflow"
            ));
        }
        Ok(())
    }

    pub fn get_stride<T>(
        w: usize,
        h: usize,
        t_per_pixel: usize,
        alignment_in_bytes: usize,
    ) -> Result<u32, FlowError> {
        Bitmap::check_dimensions::<T>(w, t_per_pixel).map_err(|e| e.at(here!()))?;

        let un_padded_stride = w * t_per_pixel;
        let alignment = alignment_in_bytes / std::mem::size_of::<T>();
        if !alignment_in_bytes.is_multiple_of(std::mem::size_of::<T>()) {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Bitmap alignment must be multiple of type size"
            ));
        }

        let padding = if un_padded_stride.is_multiple_of(alignment) {
            0
        } else {
            alignment - (un_padded_stride % alignment)
        };

        let stride = un_padded_stride.saturating_add(padding);

        Bitmap::check_dimensions::<T>(stride, h).map_err(|e| e.at(here!()))?;

        Ok(stride as u32)
    }

    pub(crate) fn create_float(
        w: u32,
        h: u32,
        pixel_layout: PixelLayout,
        alpha_premultiplied: bool,
        alpha_meaningful: bool,
        color_space: ColorSpace,
    ) -> Result<Bitmap, FlowError> {
        if w > i32::MAX as u32 || h > i32::MAX as u32 {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Bitmap dimensions {}x{} would cause i32 overflow (max i32 is {})",
                w,
                h,
                i32::MAX
            ));
        }
        let t_per_pixel = pixel_layout.channels();
        // Pad rows to 64 bytes (this does not guarantee memory alignment, just stride alignment)
        let stride = Bitmap::get_stride::<f32>(w as usize, h as usize, t_per_pixel, 64)?;

        //TODO: Note that allocs could be aligned to 16 instead of 64 bytes.
        Ok(Bitmap{
            buffer: BitmapBuffer::Floats(AlignedBuffer::new(stride as usize * h as usize, 64)
                .map_err(|e| nerror!(ErrorKind::AllocationFailed, "Failed to allocate {}x{}x{} f32 bitmap ({} bytes). Reduce dimensions or increase RAM.", w, h, t_per_pixel, h as usize * stride as usize * 4))?),
            offset: 0,
            info: BitmapInfo {
                w,
                h,
                t_stride: stride,
                t_per_pixel: t_per_pixel as u32,
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
    pub fn create_u8(
        w: u32,
        h: u32,
        pixel_layout: PixelLayout,
        alpha_premultiplied: bool,
        alpha_meaningful: bool,
        color_space: ColorSpace,
        compositing_mode: BitmapCompositing,
    ) -> Result<Bitmap, FlowError> {
        if w > i32::MAX as u32 || h > i32::MAX as u32 {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Bitmap dimensions {}x{} would cause i32 overflow (max i32 is {})",
                w,
                h,
                i32::MAX
            ));
        }
        let t_per_pixel = pixel_layout.channels();
        // Pad rows to 64 bytes (this does not guarantee memory alignment, just stride alignment)
        let stride = Bitmap::get_stride::<u8>(w as usize, h as usize, t_per_pixel, 64)?;

        //TODO: Note that allocs could be aligned to 16 instead of 64 bytes.
        let mut b = Bitmap{
            buffer: BitmapBuffer::Bytes(AlignedBuffer::new(stride as usize * h as usize, 64)
                .map_err(|e| nerror!(ErrorKind::AllocationFailed,
                 "Failed to allocate {}x{}x{} bitmap ({} bytes). Reduce dimensions or increase RAM.",
                  w, h, t_per_pixel, h as usize * stride as usize))?),
            offset: 0,
            info: BitmapInfo {
                w,
                h,
                t_stride: stride,
                t_per_pixel: t_per_pixel as u32,
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

        if let BitmapCompositing::BlendWithMatte(c) = compositing_mode {
            let color_val = c.clone();
            if color_val != imageflow_types::Color::Transparent {
                b.get_window_u8()
                    .unwrap()
                    .fill_rect(0, 0, w, h, &color_val)
                    .map_err(|e| e.at(here!()))?;
            }
        }
        Ok(b)
    }

    pub fn crop(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) -> Result<(), FlowError> {
        let (w, h) = (self.w(), self.h());
        if x2 <= x1 || y2 <= y1 || x2 > w || y2 > h {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Invalid crop bounds {:?} (image {}x{})",
                ((x1, y1), (x2, y2)),
                w,
                h
            ));
        }

        self.offset =
            self.offset + (self.info.t_stride * y1) + (self.info.t_per_pixel() as u32 * x1);
        self.info.w = x2 - x1;
        self.info.h = y2 - y1;
        self.cropped = true;
        Ok(())
    }

    pub fn get_pixel_bgra32(&mut self, x: u32, y: u32) -> Option<rgb::alt::BGRA<u8>> {
        self.get_window_u8().and_then(|w| w.get_pixel_bgra8(x, y))
    }
}

struct FlowPixelBufferMut<'a, T> {
    slice: &'a mut [T],
    t_per_pixel: usize,
    t_per_row: usize,
    t_stride: usize,
    w: usize,
    h: usize,
}
struct FlowPixelBuffer<'a, T> {
    slice: &'a [T],
    t_per_pixel: usize,
    t_per_row: usize,
    t_stride: usize,
    w: usize,
    h: usize,
}
impl<'a, T> FlowPixelBuffer<'a, T> {
    pub fn empty() -> Self {
        Self { slice: &mut [], t_per_pixel: 0, t_per_row: 0, t_stride: 0, w: 0, h: 0 }
    }
    pub fn is_empty(&self) -> bool {
        self.slice.is_empty() || self.h == 0 || self.w == 0
    }
    pub fn try_cast_from<K>(
        from_slice: &'a [K],
        info: &'a BitmapInfo,
        require_trailing_padding: bool,
        trim_trailing_padding: bool,
    ) -> Result<Self, FlowError>
    where
        T: rgb::Pod,
        K: rgb::Pod,
    {
        let w = info.width() as usize;
        let h = info.height() as usize;
        let old_slice_len = from_slice.len();
        let old_stride = info.t_stride as usize;
        let old_t_per_pixel = info.t_per_pixel();
        if h == 0 || old_stride == 0 {
            return Ok(Self::empty());
        }
        match bytemuck::try_cast_slice::<K, T>(from_slice) {
            Ok(mut slice) => {
                let (t_stride, t_per_row, t_per_pixel) = calculate_cast_buffer_results(
                    w,
                    h,
                    old_t_per_pixel,
                    old_stride,
                    old_slice_len,
                    slice.len(),
                    require_trailing_padding,
                )
                .map_err(|e| e.at(here!()))?;
                let padding = t_stride - t_per_row;
                if trim_trailing_padding && padding > 0 {
                    slice = &slice[..t_stride * h - padding];
                }
                let r = Self { slice, t_per_pixel, t_per_row, t_stride, w, h };
                //println!("try_cast_from::{}", &r);
                Ok(r)
            }
            Err(e) => Err(nerror!(ErrorKind::InvalidArgument, "Failed to cast slice: {}", e)),
        }
    }
}

impl<'a, T> FlowPixelBufferMut<'a, T> {
    pub fn empty() -> Self {
        Self { slice: &mut [], t_per_pixel: 0, t_per_row: 0, t_stride: 0, w: 0, h: 0 }
    }
    pub fn is_empty(&self) -> bool {
        self.slice.is_empty() || self.h == 0 || self.w == 0
    }

    pub fn try_cast_from<K>(
        from_slice: &'a mut [K],
        info: &'a BitmapInfo,
        require_trailing_padding: bool,
        trim_trailing_padding: bool,
    ) -> Result<Self, FlowError>
    where
        T: rgb::Pod,
        K: rgb::Pod,
    {
        let w = info.width() as usize;
        let h = info.height() as usize;
        let old_slice_len = from_slice.len();
        let old_stride = info.t_stride as usize;
        let old_t_per_pixel = info.t_per_pixel();
        if h == 0 || old_stride == 0 {
            return Ok(Self::empty());
        }
        match bytemuck::try_cast_slice_mut::<K, T>(from_slice) {
            Ok(mut slice) => {
                let (t_stride, t_per_row, t_per_pixel) = calculate_cast_buffer_results(
                    w,
                    h,
                    old_t_per_pixel,
                    old_stride,
                    old_slice_len,
                    slice.len(),
                    require_trailing_padding,
                )
                .map_err(|e| e.at(here!()))?;
                let padding = t_stride - t_per_row;
                if trim_trailing_padding && padding > 0 {
                    slice = &mut slice[..t_stride * h - padding];
                }
                let r = Self { slice, t_per_pixel, t_per_row, t_stride, w, h };
                //println!("try_cast_from::{}", &r);
                Ok(r)
            }
            Err(e) => Err(nerror!(ErrorKind::InvalidArgument, "Failed to cast slice: {}", e)),
        }
    }
}

fn calculate_cast_buffer_results(
    old_w: usize,
    old_h: usize,
    old_t_per_pixel: usize,
    old_stride: usize,
    old_slice_length: usize,
    new_slice_length: usize,
    require_trailing_padding: bool,
) -> Result<(usize, usize, usize), FlowError> {
    if !old_slice_length.is_multiple_of(new_slice_length) {
        panic!(
            "::try_cast_from: new casted slice length {} not a multiple of old slice length {}: ",
            new_slice_length, old_slice_length
        );
    }
    let (t_per_pixel, t_stride) = if old_slice_length > new_slice_length {
        let factor = old_slice_length / new_slice_length;
        let t_per_pixel = old_t_per_pixel / factor;
        let t_stride = old_stride / factor;
        (t_per_pixel, t_stride)
    } else if old_slice_length < new_slice_length {
        let inverse_factor = new_slice_length / old_slice_length;
        if !new_slice_length.is_multiple_of(old_slice_length) {
            panic!("::try_cast_from: new casted slice length {} not a multiple of old slice length {}: ", new_slice_length, old_slice_length);
        }
        let t_per_pixel = old_t_per_pixel * inverse_factor;
        let t_stride = old_stride * inverse_factor;
        (t_per_pixel, t_stride)
    } else {
        (old_t_per_pixel, old_stride)
    };

    let t_per_row = old_w * t_per_pixel;
    let padding = t_stride - t_per_row;
    let slice_len = new_slice_length;
    if new_slice_length < t_stride * old_h - padding {
        panic!(
            "::try_cast_from: new slice too short: {} < {} * {} - {}",
            new_slice_length, t_stride, old_h, padding
        );
    }
    if require_trailing_padding && new_slice_length < t_stride * old_h {
        return Err(nerror!(ErrorKind::InvalidArgument, "FlowPixelBuffer::try_cast_from: new slice too short (trailing padding {} required): {} < {} * {}", padding, new_slice_length, t_stride, old_h));
    }
    Ok((t_stride, t_per_row, t_per_pixel))
}

/// Iterator that yields scanlines from a bitmap window
pub struct ScanlineIterMut<'a, T> {
    info: &'a SurfaceInfo,
    // slice should not contain trailing padding
    remaining_slice: &'a mut [T],
    next_y: i32,
    t_per_pixel: usize,
    t_per_row: usize,
    t_stride: usize,
    w: usize,
    h: i32,
    finished: bool,
    reverse: bool,
}
// Display format for ScanlineIterMut
// ScanlineIterMut({W}x{H}x{t_per_pixel}, y={current_y}) ({t_per_pixel} {T} per px * {w} => {t_per_row} {T} per row + padding {t_stride - t_per_row} => stride {t_stride} * ({h} - {y}) - offset {0} => {required_length}. Slice length {remaining_slice.len()})
impl<'a, T> fmt::Display for ScanlineIterMut<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name_of_t = std::any::type_name::<T>();
        let padding = self.t_stride as i32 - self.t_per_row as i32;
        let required_length = (self.t_stride as i32 * self.len() as i32) - padding;
        let remaining_slice_len = self.remaining_slice.len();
        let t_per_pixel = self.t_per_pixel;
        let w = self.w;
        let t_per_row = self.t_per_row;
        let t_stride = self.t_stride;
        let h = self.h;
        let current_y = self.next_y;
        let reverse = if self.reverse { "[reverse]" } else { "" };
        write!(f, "ScanlineIterMut{reverse}({w}x{h}x{t_per_pixel}, y={current_y}) ({t_per_pixel} {name_of_t} per px * {w} => {t_per_row} {name_of_t} per row + padding {padding} => stride {t_stride} * ({h} - {current_y}) - padding {padding} => {required_length}. Slice length {remaining_slice_len})")
    }
}

impl<'a, T> ScanlineIterMut<'a, T> {
    pub fn empty(info: &'a BitmapInfo) -> Self {
        Self {
            info: info.surface_info(),
            remaining_slice: &mut [],
            next_y: 0,
            t_per_pixel: 0,
            t_per_row: 0,
            t_stride: 0,
            w: 0,
            h: 0,
            finished: true,
            reverse: false,
        }
    }
    pub fn new(slice: &'a mut [T], info: &'a BitmapInfo, reverse: bool) -> Option<Self> {
        let t_per_pixel = info.t_per_pixel();
        let t_per_row = info.width() as usize * t_per_pixel;
        let t_stride = info.t_stride as usize;
        let w = info.width() as usize;
        let h = info.height() as usize;
        if h == 0 || t_per_row > t_stride {
            return Some(Self::empty(info));
        }
        if h > i32::MAX as usize {
            panic!("Height {} is too large", h);
        }
        let padding = t_stride - t_per_row;
        if slice.len() < t_stride * h - padding {
            return None;
        }
        let start_y = if reverse { h as i32 - 1 } else { 0 };
        let slice_cropped = &mut slice[..t_stride * h - padding];
        let r = Self {
            info: info.surface_info(),
            remaining_slice: slice_cropped,
            next_y: start_y,
            t_per_pixel,
            t_per_row,
            t_stride,
            w,
            h: h as i32,
            finished: false,
            reverse,
        };
        //println!("new::{}", &r);
        Some(r)
    }
    pub fn try_cast_from<K>(
        from_slice: &'a mut [K],
        info: &'a BitmapInfo,
        reverse: bool,
    ) -> Result<Self, FlowError>
    where
        T: rgb::Pod,
        K: rgb::Pod,
    {
        if info.height() > i32::MAX as u32 {
            panic!("Height {} is too large", info.height());
        }
        let h = info.height() as i32;
        let next_y = if reverse { h - 1 } else { 0 };
        let buffer = FlowPixelBufferMut::try_cast_from(from_slice, info, false, true)
            .map_err(|e| e.at(here!()))?;
        Ok(Self {
            info: info.surface_info(),
            remaining_slice: buffer.slice,
            next_y,
            t_per_pixel: buffer.t_per_pixel,
            t_per_row: buffer.t_per_row,
            t_stride: buffer.t_stride,
            w: buffer.w,
            h,
            finished: false,
            reverse,
        })
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

impl<'a, T> fmt::Display for Scanline<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Scanline<{}[{}]>[y={}]({}x{}x{})",
            std::any::type_name::<T>(),
            self.row.len(),
            self.y,
            self.w,
            self.h,
            self.t_per_pixel
        )
    }
}
impl<'a, T> Scanline<'a, T> {
    pub fn info(&self) -> &SurfaceInfo {
        self.info
    }
    #[inline]
    pub fn width(&self) -> usize {
        self.w
    }
    #[inline]
    pub fn height(&self) -> usize {
        self.h
    }
    #[inline]
    pub fn t_per_pixel(&self) -> usize {
        self.t_per_pixel
    }
    #[inline]
    pub fn row(&self) -> &[T] {
        self.row
    }
    #[inline]
    pub fn row_mut(&mut self) -> &mut [T] {
        self.row
    }

    #[inline]
    pub fn y(&self) -> usize {
        self.y
    }
}

impl<'a, T> ExactSizeIterator for ScanlineIterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.length()
    }
}

impl<'a, T> ScanlineIterMut<'a, T> {
    #[inline]
    pub fn length(&self) -> usize {
        //eprintln!("length::{} (h={}, y={}, finished={})", len, self.h, self.next_y, self.finished);
        {
            if self.finished {
                return 0;
            }
            if self.reverse {
                (self.next_y + 1) as usize
            } else {
                (self.h - self.next_y) as usize
            }
        }
    }
}

impl<'a, T> Iterator for ScanlineIterMut<'a, T> {
    type Item = Scanline<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.length() == 0 {
            //eprintln!("next::none{}", &self);
            return None;
        }
        self.finished = self.length() <= 1;
        // Take ownership of the slice temporarily
        let slice = std::mem::take(&mut self.remaining_slice);

        let chop_len = if self.next_y + 1 == self.h { self.t_per_row } else { self.t_stride };
        if slice.len() < chop_len {
            panic!("Remaining_slice length {} is less than chop_len {}, this should never happen. \n{}: ", slice.len(), chop_len, &self);
        }

        // Safe split
        let (a, b) = if self.reverse {
            slice.split_at_mut(slice.len() - chop_len)
        } else {
            slice.split_at_mut(chop_len)
        };

        let return_row = if self.reverse {
            self.remaining_slice = a;
            &mut b[..self.t_per_row]
        } else {
            self.remaining_slice = b;
            &mut a[..self.t_per_row]
        };
        let y = self.next_y;

        if self.reverse {
            self.next_y -= 1;
        } else {
            self.next_y += 1;
        }

        //eprintln!("next::Some{}", &r.as_ref().unwrap());
        //eprintln!("{}", &self);
        Some(Scanline {
            y: y as usize,
            info: self.info,
            row: return_row,
            t_per_pixel: self.t_per_pixel,
            w: self.w,
            h: self.h as usize,
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            let remaining = self.length();
            (remaining, Some(remaining))
        }
    }
}

impl<'a> BitmapWindowMut<'a, BGRA8> {
    pub fn to_vec_rgba(&mut self) -> Result<(Vec<rgb::RGBA8>, usize, usize), FlowError> {
        let w = self.w() as usize;
        let h = self.h() as usize;

        let mut v = Vec::new();
        v.try_reserve(w * h).map_err(|e| {
            nerror!(ErrorKind::InvalidOperation, "Failed to reserve memory for contiguous vec")
        })?;

        let mut pixels_present = 0;
        for line in self.scanlines() {
            pixels_present += line.row.len();
            v.extend(line.row.iter().map(|pix| rgb::RGBA8 {
                r: pix.r,
                g: pix.g,
                b: pix.b,
                a: pix.a,
            }));
        }
        if v.len() != w * h {
            return Err(nerror!(
                ErrorKind::InvalidOperation,
                "to_vec_rgba produced {} pixels from {} pixels present, expected {} ({}x{})",
                v.len(),
                pixels_present,
                w * h,
                w,
                h
            ));
        }

        Ok((v, w, h))
    }

    pub fn to_vec_rgb(&mut self) -> Result<(Vec<rgb::RGB8>, usize, usize), FlowError> {
        let w = self.w() as usize;
        let h = self.h() as usize;

        let mut v = Vec::new();
        v.try_reserve(w * h).map_err(|e| {
            nerror!(ErrorKind::InvalidOperation, "Failed to reserve memory for contiguous vec")
        })?;

        let mut pixels_present = 0;
        for line in self.scanlines() {
            pixels_present += line.row.len();
            v.extend(line.row.iter().map(|pix| rgb::RGB8 {
                r: pix.r,
                g: pix.g,
                b: pix.b,
            }));
        }
        if v.len() != w * h {
            return Err(nerror!(
                ErrorKind::InvalidOperation,
                "to_vec_rgb produced {} pixels from {} pixels present, expected {} ({}x{})",
                v.len(),
                pixels_present,
                w * h,
                w,
                h
            ));
        }

        Ok((v, w, h))
    }
    pub fn get_pixel_buffer(&self) -> Result<PixelBuffer<'_>, FlowError> {
        if self.info.pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap is not BGRA"));
        }
        let imgref = self.try_cast_imgref::<BGRA8>().map_err(|e| e.at(here!()))?;
        Ok(PixelBuffer::Bgra32(imgref))
    }

    pub fn to_window_u8(&mut self) -> Result<BitmapWindowMut<'_, u8>, FlowError> {
        let buffer = FlowPixelBufferMut::try_cast_from(self.slice, &self.info, false, false)
            .map_err(|e| e.at(here!()))?;

        let mut info = self.info.clone();
        info.t_per_pixel = buffer.t_per_pixel as u32;
        info.t_stride = buffer.t_stride as u32;

        Ok(BitmapWindowMut { slice: buffer.slice, info, is_sub_window: self.is_sub_window })
    }

    pub fn apply_matte(&mut self, matte: imageflow_types::Color) -> Result<(), FlowError> {
        crate::graphics::blend::apply_matte(self, matte)
    }

    pub fn fill_rectangle(
        &mut self,
        color: imageflow_helpers::colors::Color32,
        x: u32,
        y: u32,
        x2: u32,
        y2: u32,
    ) -> Result<(), FlowError> {
        if let BitmapCompositing::BlendWithMatte(_) = self.info().compose() {
            if self.is_sub_window || (x, y, x2, y2) != (0, 0, self.w(), self.h()) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "Cannot draw a rectangle on a sub-rectangle of a bitmap in BlendWithMatte mode"
                ));
            }
        }
        if y2 == y || x2 == x {
            return Ok(());
        } // Don't fail on zero width rect
        if y2 <= y || x2 <= x || x2 > self.w() || y2 > self.h() {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Coordinates {},{} {},{} must be within image dimensions {}x{}",
                x,
                y,
                x2,
                y2,
                self.w(),
                self.h()
            ));
        }
        if self.info().pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Only BGRA supported for fill_rectangle"
            ));
        }
        let bgra = color.to_bgra8();

        let mut top = self.window(x, y, x2, y2).unwrap();
        for line in top.scanlines() {
            line.row.fill(bgra);
        }
        Ok(())
    }
}
impl<'a> BitmapWindowMut<'a, u8> {
    pub fn to_window_bgra32(
        &mut self,
    ) -> Result<BitmapWindowMut<'_, rgb::alt::BGRA<u8>>, FlowError> {
        if self.info.pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap is not BGRA"));
        }

        let buffer = FlowPixelBufferMut::try_cast_from(self.slice, &self.info, false, false)
            .map_err(|e| e.at(here!()))?;

        let mut info = self.info.clone();
        info.t_per_pixel = buffer.t_per_pixel as u32;
        info.t_stride = buffer.t_stride as u32;

        Ok(BitmapWindowMut { slice: buffer.slice, info, is_sub_window: self.is_sub_window })
    }

    /// Creates an iterator over BGRA scanlines. Stride padding is not included.
    pub fn scanlines_bgra(&mut self) -> Result<ScanlineIterMut<'_, rgb::alt::BGRA<u8>>, FlowError> {
        if self.info.pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap is not BGRA"));
        }
        ScanlineIterMut::try_cast_from::<u8>(self.slice, &self.info, false)
    }

    /// Creates an iterator over BGRA scanlines in reverse order. Stride padding is not included.
    pub fn scanlines_bgra_reverse(
        &mut self,
    ) -> Result<ScanlineIterMut<'_, rgb::alt::BGRA<u8>>, FlowError> {
        ScanlineIterMut::try_cast_from::<u8>(self.slice, &self.info, true)
    }

    /// Creates an iterator over BGRA scanlines
    pub fn scanlines_u32(&mut self) -> Result<ScanlineIterMut<'_, u32>, FlowError> {
        if self.info.pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(ErrorKind::InvalidArgument, "Bitmap is not BGRA"));
        }
        ScanlineIterMut::try_cast_from::<u8>(self.slice, &self.info, false)
    }

    /// Creates an iterator over u32 scanlines in reverse order
    pub fn scanlines_u32_reverse(&mut self) -> Result<ScanlineIterMut<'_, u32>, FlowError> {
        ScanlineIterMut::try_cast_from::<u8>(self.slice, &self.info, true)
    }

    /// Call normalize_alpha first; this function does not skip unused alpha bytes, only unused whole pixels.
    /// Otherwise Bgr32 may be non-deterministic
    pub fn short_hash_pixels(&mut self) -> u64 {
        use std::hash::Hasher;
        let mut hash = ::twox_hash::XxHash64::with_seed(0x8ed1_2ad9_483d_28a0);
        for line in self.scanlines() {
            hash.write(line.row);
        }
        hash.finish()
    }

    pub fn get_pixel_buffer(&self) -> Result<PixelBuffer<'_>, FlowError> {
        Ok(match self.pixel_format() {
            PixelFormat::Bgra32 => {
                let imgref = self.try_cast_imgref::<BGRA8>().map_err(|e| e.at(here!()))?;
                PixelBuffer::Bgra32(imgref)
            }
            PixelFormat::Bgr32 => {
                let imgref = self.try_cast_imgref::<BGRA8>().map_err(|e| e.at(here!()))?;
                PixelBuffer::Bgr32(imgref)
            }
            PixelFormat::Bgr24 => {
                let imgref = self.try_cast_imgref::<BGR8>().map_err(|e| e.at(here!()))?;
                PixelBuffer::Bgr24(imgref)
            }
            PixelFormat::Gray8 => {
                let imgref = self.try_cast_imgref::<rgb::Gray<u8>>().map_err(|e| e.at(here!()))?;
                PixelBuffer::Gray8(imgref)
            }
        })
    }

    pub fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        x2: u32,
        y2: u32,
        color: &imageflow_types::Color,
    ) -> Result<(), FlowError> {
        let color_srgb_argb = color.to_color_32()?;
        self.fill_rectangle(color_srgb_argb, x, y, x2, y2).map_err(|e| e.at(here!()))
    }
    pub fn fill_rectangle(
        &mut self,
        color: imageflow_helpers::colors::Color32,
        x: u32,
        y: u32,
        x2: u32,
        y2: u32,
    ) -> Result<(), FlowError> {
        if let BitmapCompositing::BlendWithMatte(_) = self.info().compose() {
            if self.is_sub_window || (x, y, x2, y2) != (0, 0, self.w(), self.h()) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "Cannot draw a rectangle on a sub-rectangle of a bitmap in BlendWithMatte mode"
                ));
            }
        }
        if y2 == y || x2 == x {
            return Ok(());
        } // Don't fail on zero width rect
        if y2 <= y || x2 <= x || x2 > self.w() || y2 > self.h() {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Coordinates {},{} {},{} must be within image dimensions {}x{}",
                x,
                y,
                x2,
                y2,
                self.w(),
                self.h()
            ));
        }
        if self.info().pixel_layout() != PixelLayout::BGRA {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Only BGRA supported for fill_rectangle"
            ));
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
        for line in top.scanlines_bgra().unwrap() {
            line.row.fill(bgra);
        }
        //}

        Ok(())
    }

    pub fn set_alpha_to_255(&mut self) -> Result<(), FlowError> {
        for line in self.scanlines_bgra()? {
            for pix in line.row {
                pix.a = 255;
            }
        }
        Ok(())
    }
    pub fn normalize_unused_alpha(&mut self) -> Result<(), FlowError> {
        if self.info().alpha_meaningful() {
            return Ok(());
        }
        self.set_alpha_to_255()?;
        Ok(())
    }

    pub fn slice_of_pixels_first_row(&mut self) -> Option<&mut [rgb::alt::BGRA8]> {
        if self.info().t_per_pixel() != 4 || !self.slice.len().is_multiple_of(4) {
            return None;
        }
        unsafe {
            Some(core::slice::from_raw_parts_mut(
                self.slice.as_mut_ptr() as *mut rgb::alt::BGRA8,
                (self.slice.len() / 4).min(self.info.w as usize),
            ))
        }
    }

    pub fn get_pixel_bgra8(&self, x: u32, y: u32) -> Option<rgb::alt::BGRA<u8>> {
        if self.t_per_pixel() != 4 || !self.slice.len().is_multiple_of(4) {
            return None;
        }
        let index = (y * self.info.t_stride + x * 4) as usize;
        let pixel = bytemuck::cast_slice::<u8, rgb::alt::BGRA8>(&self.slice[index..index + 4]);
        Some(pixel[0])
    }
}
impl<'a, T> BitmapWindowMut<'a, T>
where
    T: Pod,
{
    /// Creates an iterator over f32 scanlines. Padding is not included.
    pub fn scanlines(&mut self) -> ScanlineIterMut<'_, T> {
        ScanlineIterMut::new(self.slice, &self.info, false).unwrap()
    }

    /// Creates an iterator over f32 scanlines in reverse order. Padding is not included.
    pub fn scanlines_reverse(&mut self) -> ScanlineIterMut<'_, T> {
        ScanlineIterMut::new(self.slice, &self.info, true).unwrap()
    }

    pub fn try_cast_imgref<K: Copy>(&self) -> Result<ImgRef<'_, K>, FlowError>
    where
        K: rgb::Pod,
    {
        let buffer = FlowPixelBuffer::try_cast_from(self.slice, &self.info, false, false)
            .map_err(|e| e.at(here!()))?;
        Ok(ImgRef::new_stride(buffer.slice, buffer.w, buffer.h, buffer.t_stride))
    }

    pub fn row_mut_bgra(&mut self, index: u32) -> Option<&mut [rgb::Bgra<T>]> {
        if self.info.pixel_layout() != PixelLayout::BGRA {
            return None;
        }
        self.row_mut(index as usize).map(|r| bytemuck::cast_slice_mut::<T, rgb::Bgra<T>>(r))
    }
}

impl<'a> BitmapWindowMut<'a, u8> {
    pub unsafe fn to_vec_rgba(&self) -> Result<(Vec<rgb::RGBA8>, usize, usize), FlowError> {
        let w = self.w() as usize;
        let h = self.h() as usize;

        match &self.info().compose() {
            BitmapCompositing::ReplaceSelf | BitmapCompositing::BlendWithSelf => {
                let mut v = vec![rgb::RGBA8::new(0, 0, 0, 255); w * h];

                if self.info().t_per_pixel() != 4 || !self.slice.len().is_multiple_of(4) {
                    return Err(unimpl!("Only Bgr(a)32 supported"));
                }

                // TODO: if alpha might be random, we should clear it if self.info.alpha_meaningful(){

                let mut y = 0;
                for stride_row in self.slice.chunks(self.info().t_stride() as usize) {
                    for x in 0..w {
                        v[y * w + x].b = stride_row[x * 4];
                        v[y * w + x].g = stride_row[x * 4 + 1];
                        v[y * w + x].r = stride_row[x * 4 + 2];
                        v[y * w + x].a = stride_row[x * 4 + 3];
                    }
                    y += 1;
                }

                Ok((v, w, h))
            }
            BitmapCompositing::BlendWithMatte(c) => {
                let matte = c.clone().to_color_32().unwrap().to_rgba8();
                Ok((vec![matte; w * h], w, h))
            }
        }
    }
}
pub trait BitmapRowAccess {
    fn row_bgra8(&self, row_ix: usize, stride: usize) -> Option<&[BGRA8]>;
    fn row_rgba8(&self, row_ix: usize, stride: usize) -> Option<&[RGBA8]>;
    fn row_bgr8(&self, row_ix: usize, stride: usize) -> Option<&[BGR8]>;
    fn row_rgb8(&self, row_ix: usize, stride: usize) -> Option<&[RGB8]>;
    fn row_gray8(&self, row_ix: usize, stride: usize) -> Option<&[Gray<u8>]>;
    fn row_grayalpha8(&self, row_ix: usize, stride: usize) -> Option<&[GrayA<u8>]>;

    fn row_mut_bgra8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [BGRA8]>;
    fn row_mut_rgba8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [RGBA8]>;
    fn row_mut_bgr8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [BGR8]>;
    fn row_mut_rgb8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [RGB8]>;
    fn row_mut_gray8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [Gray<u8>]>;
    fn row_mut_grayalpha8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [GrayA<u8>]>;
}

impl BitmapRowAccess for Vec<u8> {
    fn row_bgra8(&self, row_ix: usize, stride: usize) -> Option<&[BGRA8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get(start..start.checked_add(stride)?)?;
        try_cast_slice(row).ok()
    }

    fn row_rgba8(&self, row_ix: usize, stride: usize) -> Option<&[RGBA8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get(start..start.checked_add(stride)?)?;
        try_cast_slice(row).ok()
    }

    fn row_bgr8(&self, row_ix: usize, stride: usize) -> Option<&[BGR8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get(start..start.checked_add(stride)?)?;
        try_cast_slice(row).ok()
    }

    fn row_rgb8(&self, row_ix: usize, stride: usize) -> Option<&[RGB8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get(start..start.checked_add(stride)?)?;
        try_cast_slice(row).ok()
    }

    fn row_gray8(&self, row_ix: usize, stride: usize) -> Option<&[Gray<u8>]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get(start..start.checked_add(stride)?)?;
        try_cast_slice(row).ok()
    }

    fn row_grayalpha8(&self, row_ix: usize, stride: usize) -> Option<&[GrayA<u8>]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get(start..start.checked_add(stride)?)?;
        try_cast_slice(row).ok()
    }

    fn row_mut_bgra8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [BGRA8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get_mut(start..start.checked_add(stride)?)?;
        try_cast_slice_mut(row).ok()
    }

    fn row_mut_rgba8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [RGBA8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get_mut(start..start.checked_add(stride)?)?;
        try_cast_slice_mut(row).ok()
    }

    fn row_mut_bgr8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [BGR8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get_mut(start..start.checked_add(stride)?)?;
        try_cast_slice_mut(row).ok()
    }

    fn row_mut_rgb8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [RGB8]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get_mut(start..start.checked_add(stride)?)?;
        try_cast_slice_mut(row).ok()
    }

    fn row_mut_gray8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [Gray<u8>]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get_mut(start..start.checked_add(stride)?)?;
        try_cast_slice_mut(row).ok()
    }

    fn row_mut_grayalpha8(&mut self, row_ix: usize, stride: usize) -> Option<&mut [GrayA<u8>]> {
        let start = row_ix.checked_mul(stride)?;
        let row = self.get_mut(start..start.checked_add(stride)?)?;
        try_cast_slice_mut(row).ok()
    }
}
#[test]
fn test_scanline_for_1x1() {
    let mut c = BitmapsContainer::with_capacity(1);
    let b1 = c
        .create_bitmap_u8(
            1,
            1,
            PixelLayout::BGRA,
            false,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();
    let mut bitmap = c.try_borrow_mut(b1).unwrap();
    let mut window = bitmap.get_window_u8().unwrap();
    window.fill_rectangle(Color32(0xFF0000FF), 0, 0, 1, 1).unwrap();
    let mut row_count = 0;
    for scanline in window.scanlines() {
        assert_eq!(scanline.row.len(), scanline.w as usize * scanline.t_per_pixel() as usize);
        eprintln!("{}\n{:?}", &scanline, &scanline.row);
        assert_eq!(scanline.row[0], 0xFF);
        row_count += 1;
    }
    assert_eq!(row_count, window.info().height() as usize);
}

// Example usage test
#[test]
fn test_scanline_iterator_bgra32() {
    let mut c = BitmapsContainer::with_capacity(1);
    let b1 = c
        .create_bitmap_u8(
            5,
            5,
            PixelLayout::BGRA,
            false,
            true,
            ColorSpace::StandardRGB,
            BitmapCompositing::ReplaceSelf,
        )
        .unwrap();

    let mut bitmap = c.try_borrow_mut(b1).unwrap();
    let mut window = bitmap.get_window_u8().unwrap();
    for y in 0..window.info().height() {
        for x in 0..window.info().width() {
            let color = Color32::from_rgba(x as u8, y as u8, 0, 255);
            window.fill_rectangle(color, x, y, x + 1, y + 1).unwrap();
        }
    }
    // for (i, pixel) in window.slice_mut().iter_mut().enumerate(){
    //     *pixel = i as u8;
    // }
    // Test u8 scanlines
    for scanline in window.scanlines() {
        assert_eq!(scanline.row.len(), scanline.w as usize * scanline.t_per_pixel() as usize);
        println!("{}\n{:?}", &scanline, &scanline.row);
        for x in (0..scanline.w as usize).step_by(4) {
            assert_eq!(scanline.row[x..x + 4], [0, scanline.y as u8, (x / 4) as u8, 255]);
        }
    }
    let mut row_count = 0;
    for scanline in window.scanlines_bgra().unwrap() {
        assert_eq!(scanline.row.len(), scanline.w as usize);
        row_count += 1;
    }
    assert_eq!(row_count, window.info().height() as usize);
    // Test BGRA scanlines
    for scanline in window.scanlines_bgra().unwrap() {
        assert_eq!(scanline.row.len(), scanline.w as usize);
        // Each item is one BGRA pixel
        println!("{}\n{:?}", &scanline, &scanline.row);
        for x in 0..scanline.w as usize {
            print!("{} ", scanline.row[x]);
            assert_eq!(
                scanline.row[x],
                rgb::alt::BGRA8::new_bgra(0x00, scanline.y as u8, x as u8, 0xFF)
            );
        }
    }
}

#[test]
fn test_scanline_iterator_f32_reverse() {
    let mut c = BitmapsContainer::with_capacity(1);
    let b1 = c
        .create_bitmap_f32(10, 10, PixelLayout::BGRA, false, true, ColorSpace::StandardRGB)
        .unwrap();

    let mut bitmap = c.try_borrow_mut(b1).unwrap();
    let mut window = bitmap.get_window_f32().unwrap();

    for scanline in window.scanlines_reverse() {
        assert_eq!(scanline.row.len(), scanline.w as usize * scanline.t_per_pixel() as usize);
    }
    for scanline in window.scanlines() {
        assert_eq!(scanline.row.len(), scanline.w as usize * scanline.t_per_pixel() as usize);
    }
}
