use crate::graphics::aligned_buffer::{AlignedBuffer, AlignedBufferError};

pub struct AllocationContainer{
    allocations: Vec<AlignedBuffer<u8>>
}
impl AllocationContainer{

    pub fn new() -> AllocationContainer{
        AllocationContainer{
            allocations: Vec::new()
        }
    }
    /// Allocates the specified number of bytes with the given alignment and returns a pointer
    pub fn allocate(&mut self, bytes: usize, alignment: usize) -> Result<*mut u8, AlignedBufferError>{
        let mut buffer = AlignedBuffer::new(bytes, alignment)?;
        let ptr = buffer.as_slice_mut().as_mut_ptr();
        self.allocations.push(buffer);
        Ok(ptr)
    }
    /// Returns true if the pointer was found and freed.
    pub fn free(&mut self, pointer: *const u8) -> bool{
        match self.allocations.iter()
            .position(|a| a.as_slice().as_ptr() == pointer) {
            Some(index) => {
                self.allocations.remove(index);
                true
            },
            None => false
        }
    }
}