# `wgpu_memory` <!-- omit from toc -->

- [Usage](#usage)
- [Performance](#performance)
- [Memory Efficiency](#memory-efficiency)
- [Quick Reference](#quick-reference)
  - [`SimpleGpuMemory<T>`](#simplegpumemoryt)
    - [Example](#example)
  - [`AutoDropping<T, M: GpuMemory<T>>`](#autodroppingt-m-gpumemoryt)
    - [Example](#example-1)


An abstraction over a `wgpu::Buffer` that supports allocating and freeing memory
like standard library functions, but for memory on the GPU. A quick and dirty
solution for when you have a bunch of constantly changing memory in a buffer
that needs to be uploaded every frame without having to rebuild the buffer every
frame.

# Usage

Create an instance of any struct that implements `GpuMemory<T>`, for example
`SimpleGpuMemory<T>`. Then you can allocate a section, get a mutable reference
to it to write data, resize it or free it.

```rs
use wgpu_memory::{GpuMemory, simple::SimpleGpuMemory};

#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
struct Entity {
    position: [f32; 2],
    size: [f32; 2],
}

fn main() {
    // Setup wgpu instance, adapter, device and queue

    // Create a vertex buffer managed by `wgpu_memory`
    let mut mem = SimpleGpuMemory::new(wgpu::BufferUsages::VERTEX, &device);

    // You can now get what you need to create bind groups
    // with these functions:
    let _ = mem.buffer();
    let _ = mem.buffer_slice();

    // Allocate 1 * size_of::<Entity>() bytes in the buffer
    let address = mem.allocate(1);

    // Write the entity into the buffer
    mem.get(&address)[0] = Entity {
        position: [10.0, 50.0],
        size: [2.0, 2.0],
    };

    // In the render loop

    mem.upload(&queue, &device);

    // Free the memory when the address will no longer be
    // used. When the `GpuMemory` gets dropped, the entire
    // buffer gets deallocated so you don't need to free
    // any leftover allocated space.
    mem.free(address);
}
```

# Performance

Using this library can make writing performant code easy, however
for optimal performance there are some things to note:

- Making edits to already allocated memory (e.g. using
  `buffer.get(address)` and writing to the slice) is an extremely
  cheap operation.
- Allocating memory is also a relatively cheap operation, it will
  reuse previously freed memory and if not enough free memory
  is found, more memory gets allocated at the end of the buffer
- Freeing memory is the most expensive operation of the three,
  when `.upload()` is called, all unallocated memory will have to
  be moved to the end of the buffer so that the buffer that gets
  sent to the gpu is a one continuous sequence of items with no
  holes. This ensures that you do not need to make any changes in
  your shader code. If after you free some memory, you allocate
  the same amount of memory, the slot will just be reused so it
  will be a very cheap operation after all.

In short, just like memory management on the CPU side, allocations
are the costly operations, using the memory is practically free.
Try to keep the amount of calls to `.allocate()` and `.free()`
as low as possible in your update and render loops.

# Memory Efficiency

Deallocating memory (e.g. calling `.free()`) does not actually
deallocate it on the buffer, all it does is mark that section
of the buffer as unallocated, to be reused by following calls
to `.allocate()`.

This of course brings efficiency into the question, so there's
another function to optimize the buffer's efficiency:
`.optimize(strategy)`. This will actually deallocate every bit
of unused memory and resize the gpu buffer to fit only the used
data, nothing more. This also means that subsequent calls to
`.allocate()` will be more expensive as buffer on the cpu will
have to allocate more memory, and the gpu buffer will be resized
and copied. In the case of a game engine, I'd recommend calling
`.optimize()` after loading or unloading a scene, as it can
reduce memory usage in some cases by 75%, and by doing it after
loading or unloading is finished you can be quite sure that there
won't be too many more calls to `.allocate()` and `.free()`.

# Quick Reference

```rs
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
```

There are 2 built-in implementations of this trait:

## `SimpleGpuMemory<T>`

Uses a normal buffer, adding `COPY_DST` to the buffer usages.

### `type Index = struct AddressId` <!-- omit from toc -->

An index to a list of address ranges in the buffer

### `type OptimizationStrategy = enum Strategy` <!-- omit from toc -->

- `Truncate`: delete unused memory and resize the buffer to the smallest
  possible size to fit allocated items
- `SortSizeDescending`: same as `Truncate` but also sorts allocated memory
  regions by their length from longest to shortest
- `SortSizeAscending`: same as `Truncate` but also sorts allocated memory
  regions by their length from shortest to longest

### Example

```rs
let mut mem = SimpleGpuMemory::<Entity>::new(wgpu::BufferUsages::VERTEX, &device);

let index = mem.allocate(1);

mem.get(&index)[0] = Entity {
    position: [10.0, 50.0],
    size: [10.0, 10.0],
};

// Deallocate the memory at `index`
mem.free(index);
```

## `AutoDropping<T, M: GpuMemory<T>>`

A wrapper struct to wrap another `GpuMemory` buffer, any allocations will be
automatically freed when their index goes out of scope. You should not call
`.free()` on this, as it will free the unused memory automatically. This comes
at a slight cost to every operation on the buffer, so consider this a choice
of developer experience over user experience. The performance hit is trivial
if the buffer isn't written to thousands of times every frame.

### `type Index = M::Index` <!-- omit from toc -->

The inner `Index`

### `type OptimizationStrategy = M::OptimizationStrategy` <!-- omit from toc -->

The inner `OptimizationStrategy`

### Example

```rs
let mut mem = AutoDropping::<Entity, SimpleGpuMemory<Entity>>::new(wgpu::BufferUsages::VERTEX, &device);

{
    let index = mem.allocate(1);
    
    mem.get(&index)[0] = Entity {
        position: [10.0, 50.0],
        size: [10.0, 10.0],
    };

    // `index` is now out of scope and will automatically call `.free()`
}
```