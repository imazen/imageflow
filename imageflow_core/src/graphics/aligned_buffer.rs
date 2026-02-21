#[derive(Debug, Clone)]
pub enum AlignedBufferError {
    SizeOverflow,
    InvalidAlignment,
    AllocationFailed,
}

pub struct AlignedBuffer<T: bytemuck::Zeroable + Clone> {
    storage: Vec<T>,
    offset: usize,
    length: usize,
    capacity: usize,
}

impl<T: bytemuck::Zeroable + Clone> AlignedBuffer<T> {
    /// Creates a zero-filled array of type T with the given alignment.
    /// * alignment must be >= 2
    /// * alignment must be a power of two
    pub fn new(size: usize, alignment: usize) -> Result<Self, AlignedBufferError> {
        let element_bytes = std::mem::size_of::<T>();
        let length_bytes = size
            .checked_mul(element_bytes)
            .ok_or(AlignedBufferError::SizeOverflow)?;
        if alignment < 2 || !alignment.is_power_of_two() {
            return Err(AlignedBufferError::InvalidAlignment);
        }
        let capacity_bytes = length_bytes.div_ceil(alignment) * alignment;
        let capacity = capacity_bytes / element_bytes;
        if capacity < size {
            return Err(AlignedBufferError::SizeOverflow);
        }

        // Over-allocate to guarantee we can find an aligned offset.
        // Worst case: base pointer is 1 byte past alignment, so we need
        // (alignment - 1) extra bytes = (alignment - 1) / element_bytes + 1 extra elements.
        // Since element_bytes >= 1, padding_elements <= alignment - 1.
        let padding_elements = alignment.div_ceil(element_bytes);
        let total = capacity
            .checked_add(padding_elements)
            .ok_or(AlignedBufferError::SizeOverflow)?;

        let mut storage = Vec::new();
        storage.try_reserve_exact(total).map_err(|_| AlignedBufferError::AllocationFailed)?;
        storage.resize(total, T::zeroed());

        // Find offset where alignment is satisfied
        let base_addr = storage.as_ptr() as usize;
        let misalignment = base_addr % alignment;
        let byte_offset = if misalignment == 0 { 0 } else { alignment - misalignment };
        // byte_offset must be a multiple of element_bytes for proper T alignment.
        // Since alignment is a power of two and >= element_bytes' natural alignment,
        // and Vec aligns to align_of::<T>(), the byte_offset is always a multiple of element_bytes.
        debug_assert!(byte_offset % element_bytes == 0);
        let offset = byte_offset / element_bytes;

        debug_assert!(offset + capacity <= total);
        debug_assert!((storage.as_ptr() as usize + byte_offset).is_multiple_of(alignment));

        Ok(AlignedBuffer { storage, offset, length: size, capacity })
    }

    pub fn as_slice(&self) -> &[T] {
        &self.storage[self.offset..self.offset + self.length]
    }

    pub fn as_slice_mut(&mut self) -> &mut [T] {
        &mut self.storage[self.offset..self.offset + self.length]
    }
}
