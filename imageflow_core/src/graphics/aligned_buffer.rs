
#[derive(Debug, Clone)]
pub enum AlignedBufferError{
    SizeOverflow,
    InvalidAlignment,
    AllocationFailed
}

pub struct AlignedBuffer<T:rgb::Zeroable>{
    data: *mut T,
    data_length: usize,
    alignment: usize,
    length: usize,
    capacity: usize,
}

impl<T:rgb::Zeroable> AlignedBuffer<T>{
    /// Creates a zero-filled array of type T. T must be rgb::Zeroable
    /// * alignment must not be zero
    /// * alignment must be a power of two
    pub fn new(size: usize, alignment: usize) -> Result<Self, AlignedBufferError>{
        let element_bytes = std::mem::size_of::<T>();
        let length_bytes =  size * element_bytes;
        if alignment < 2 {
            return Err(AlignedBufferError::InvalidAlignment);
        }
        let capacity_bytes = ((length_bytes + alignment - 1) / alignment) * alignment;
        let capacity = capacity_bytes / element_bytes;
            if length_bytes / element_bytes != size ||
            capacity_bytes / element_bytes < size{
            Err(AlignedBufferError::SizeOverflow)
        }else{
            let l = ::std::alloc::Layout::from_size_align(capacity_bytes, alignment)
                .map_err(|e| AlignedBufferError::InvalidAlignment)?;
            unsafe{
                let ptr = ::std::alloc::alloc_zeroed(l);
                if ptr.is_null(){
                    Err(AlignedBufferError::AllocationFailed)
                }else{
                    Ok(AlignedBuffer{
                        data: ptr as *mut T,
                        data_length: capacity_bytes,
                        alignment,
                        length: size,
                        capacity
                    })
                }
            }
        }
    }

    pub fn as_slice(&self) -> &[T]{
        unsafe {
            std::slice::from_raw_parts(self.data, self.length)
        }
    }

    pub fn as_slice_mut(&mut self) -> &mut [T]{
        unsafe {
            std::slice::from_raw_parts_mut(self.data, self.length)
        }
    }

    pub fn capacity_as_slice(&self) -> &[T]{
        unsafe {
            std::slice::from_raw_parts(self.data, self.capacity)
        }
    }

    pub fn capacity_as_slice_mut(&self) -> &[T]{
        unsafe {
            std::slice::from_raw_parts_mut(self.data, self.capacity)
        }
    }

}

impl<T:rgb::Zeroable> Drop for AlignedBuffer<T>{
    fn drop(&mut self) {
        let l = ::std::alloc::Layout::from_size_align(self.capacity * std::mem::size_of::<T>(), self.alignment)
            .expect("AlignedBuffer<T>.drop() called from_size_align which failed.");
        unsafe{
            ::std::alloc::dealloc(self.data as *mut u8, l)
        }
    }
}