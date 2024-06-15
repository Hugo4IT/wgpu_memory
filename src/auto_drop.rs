use std::{
    marker::PhantomData,
    sync::{Arc, Weak},
};

use parking_lot::RwLock;

use crate::GpuMemory;

/// A wrapper struct to wrap another `GpuMemory` buffer, any allocations will be
/// automatically freed when their index goes out of scope. You should not call
/// `.free()` on this, as it will free the unused memory automatically. This comes
/// at a slight cost to every operation on the buffer, so consider this a choice
/// of developer experience over user experience. The performance hit is trivial
/// if the buffer isn't written to thousands of times every frame.
#[derive(Debug)]
pub struct AutoDropping<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern, M: GpuMemory<T>> {
    inner: Arc<RwLock<M>>,
    _phantom: PhantomData<T>,
}

#[derive(Debug)]
pub struct AutoDroppingAddressId<
    T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern,
    M: GpuMemory<T>,
> {
    inner: M::Index,
    parent: Weak<RwLock<M>>,
    refcount: Arc<()>,
    _phantom: PhantomData<T>,
}

impl<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern, M: GpuMemory<T>> Clone
    for AutoDroppingAddressId<T, M>
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            parent: Weak::clone(&self.parent),
            refcount: Arc::new(()),
            _phantom: Default::default(),
        }
    }
}

impl<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern, M: GpuMemory<T>> Drop
    for AutoDroppingAddressId<T, M>
{
    fn drop(&mut self) {
        // If there's only one strong reference, that means this is the only one
        if Arc::strong_count(&self.refcount) == 1 {
            if let Some(parent) = self.parent.upgrade() {
                let mut parent = parent.write();

                parent.free(self.inner.to_owned());
            }
        }
    }
}

impl<T: Copy + bytemuck::NoUninit + bytemuck::AnyBitPattern, M: GpuMemory<T>> GpuMemory<T>
    for AutoDropping<T, M>
{
    /// The inner `Index`
    type Index = AutoDroppingAddressId<T, M>;
    /// The inner `OptimizationStrategy`
    type OptimizationStrategy = M::OptimizationStrategy;

    fn is_empty(&self) -> bool {
        let inner = self.inner.read();

        inner.is_empty()
    }

    fn size(&self) -> usize {
        let inner = self.inner.read();

        inner.size()
    }

    fn new(usages: wgpu::BufferUsages, device: &wgpu::Device) -> Self {
        let inner = Arc::new(RwLock::new(M::new(usages, device)));

        Self {
            inner,
            _phantom: Default::default(),
        }
    }

    fn mutated(&self) -> bool {
        let inner = self.inner.read();

        inner.mutated()
    }

    fn allocate(&mut self, count: usize) -> Self::Index {
        let mut inner = self.inner.write();

        let id = inner.allocate(count);

        AutoDroppingAddressId {
            inner: id,
            parent: Arc::downgrade(&self.inner),
            refcount: Arc::new(()),
            _phantom: Default::default(),
        }
    }

    fn get(&mut self, index: &Self::Index) -> &mut [T] {
        let mut inner = self.inner.write();

        unsafe { (inner.get(&index.inner) as *mut [T]).as_mut().unwrap() }
    }

    fn len(&self) -> usize {
        let inner = self.inner.read();

        inner.len()
    }

    fn len_of(&self, index: &Self::Index) -> usize {
        let inner = self.inner.read();

        inner.len_of(&index.inner)
    }

    fn resize(&mut self, index: &mut Self::Index, len: usize) {
        let mut inner = self.inner.write();

        inner.resize(&mut index.inner, len)
    }

    fn free(&mut self, index: Self::Index) {
        // Self::Index::drop() does the freeing
        let _ = index;

        log::warn!("Attempted to free an AutoDropping memory block");
    }

    fn upload(&mut self, queue: &wgpu::Queue, device: &wgpu::Device) {
        let mut inner = self.inner.write();

        inner.upload(queue, device)
    }

    fn optimize(
        &mut self,
        strategy: Self::OptimizationStrategy,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
    ) {
        let mut inner = self.inner.write();

        inner.optimize(strategy, queue, device)
    }

    fn buffer(&self) -> &wgpu::Buffer {
        let inner = self.inner.read();

        unsafe { (inner.buffer() as *const wgpu::Buffer).as_ref().unwrap() }
    }

    fn buffer_slice(&self) -> wgpu::BufferSlice {
        self.buffer().slice(..(self.size() as u64))
    }
}
