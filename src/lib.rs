//! An abstraction over a `wgpu::Buffer` that supports allocating and freeing memory
//! like standard library functions, but for memory on the GPU. A quick and dirty
//! solution for when you have a bunch of constantly changing memory in a buffer
//! that needs to be uploaded every frame without having to rebuild the buffer every
//! frame.

pub mod auto_drop;
pub mod simple;

pub trait GpuMemory<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern> {
    /// The index type to be used to access the memory
    type Index: Clone;

    /// The manner in which the buffer gets optimized
    type OptimizationStrategy: Default + Clone + Copy;

    /// Create a new managed buffer
    fn new(usages: wgpu::BufferUsages, device: &wgpu::Device) -> Self;

    /// Has the buffer been changed since its last upload
    fn mutated(&self) -> bool;

    /// Allocate `count * size_of::<T>()` bytes in the buffer
    fn allocate(&mut self, count: usize) -> Self::Index;

    /// Get a mutable slice to the allocated memory at `index`
    ///
    /// # Safety
    ///
    /// You may only use this function with addresses given by .allocate() and
    /// may not be used after .free()
    fn get(&mut self, index: &Self::Index) -> &mut [T];

    /// The amount of items allocated in the buffer
    fn len(&self) -> usize;

    /// The amount of items allocated in the buffer at this `index`
    fn len_of(&self, index: &Self::Index) -> usize;

    /// Resize the amount of allocated memory at `index`
    fn resize(&mut self, index: &mut Self::Index, len: usize);

    /// Deallocate the memory at `index`
    fn free(&mut self, index: Self::Index);

    /// Upload all allocated memory to the gpu
    fn upload(&mut self, queue: &wgpu::Queue, device: &wgpu::Device);

    /// Optimize the memory usage of the buffer using the strategy given in
    /// `strategy`
    fn optimize(
        &mut self,
        strategy: Self::OptimizationStrategy,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    );

    /// Returns the wgpu::Buffer for use in creating a bind group
    fn buffer(&self) -> &wgpu::Buffer;

    /// Returns a slice of the buffer containing exactly all the elements in it
    fn buffer_slice(&self) -> wgpu::BufferSlice;

    /// Is the buffer empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `self.len() * size_of::<T>()`
    fn size(&self) -> usize {
        self.len() * core::mem::size_of::<T>()
    }
}

pub fn upload_or_resize(
    queue: &wgpu::Queue,
    device: &wgpu::Device,
    buffer: &mut wgpu::Buffer,
    data: &[u8],
) {
    use wgpu::util::DeviceExt;

    if buffer.size() >= data.len() as u64 {
        queue.write_buffer(buffer, 0, data);
    } else {
        *buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wgpu_text Resized Buffer"),
            usage: buffer.usage(),
            contents: data,
        })
    }
}
