use std::mem::size_of;

use common::{get_wgpu, Entity};
use wgpu_memory::{simple::SimpleGpuMemory, GpuMemory};

mod common;

#[test]
fn allocations_work() {
    let wgpu = get_wgpu();

    let mut mem = SimpleGpuMemory::new(wgpu::BufferUsages::empty(), &wgpu.device);

    for _ in 0..100 {
        let index = mem.allocate(1);
        assert_eq!(mem.size(), size_of::<Entity>());

        mem.get(&index)[0] = Entity { param: 1 };
        mem.free(index);
    }

    assert_eq!(mem.size(), 0);
}

#[test]
fn resize_works() {
    let wgpu = get_wgpu();

    let mut mem = SimpleGpuMemory::new(wgpu::BufferUsages::empty(), &wgpu.device);

    for _ in 0..100 {
        let mut index = mem.allocate(1);
        assert_eq!(mem.size(), size_of::<Entity>());

        mem.resize(&mut index, 10);
        assert_eq!(mem.size(), size_of::<Entity>() * 10);

        mem.resize(&mut index, 5);
        assert_eq!(mem.size(), size_of::<Entity>() * 5);

        mem.resize(&mut index, 5);
        assert_eq!(mem.size(), size_of::<Entity>() * 5);

        mem.get(&index)[0] = Entity { param: 1 };
        mem.free(index);
    }

    assert_eq!(mem.size(), 0);
}
