use std::mem::size_of;

use common::{get_wgpu, Entity};
use wgpu_memory::{auto_drop::AutoDropping, simple::SimpleGpuMemory, GpuMemory};

mod common;

#[test]
fn allocations_work() {
    let wgpu = get_wgpu();

    let mut mem = AutoDropping::<Entity, SimpleGpuMemory<Entity>>::new(
        wgpu::BufferUsages::empty(),
        &wgpu.device,
    );

    for _ in 0..100 {
        let index = mem.allocate(1);
        assert_eq!(mem.size(), size_of::<Entity>());

        mem.get(&index)[0] = Entity { param: 1 };
    }

    assert_eq!(mem.size(), 0);
}

#[test]
fn resize_works() {
    let wgpu = get_wgpu();

    let mut mem = AutoDropping::<Entity, SimpleGpuMemory<Entity>>::new(
        wgpu::BufferUsages::empty(),
        &wgpu.device,
    );

    for _ in 0..100 {
        let mut index = mem.allocate(1);
        assert_eq!(mem.size(), size_of::<Entity>());

        // Resize larger
        mem.resize(&mut index, 10);
        assert_eq!(mem.size(), size_of::<Entity>() * 10);

        // Resize smaller
        mem.resize(&mut index, 5);
        assert_eq!(mem.size(), size_of::<Entity>() * 5);

        // Resize equal
        mem.resize(&mut index, 5);
        assert_eq!(mem.size(), size_of::<Entity>() * 5);

        for i in 0..5 {
            mem.get(&index)[i] = Entity { param: i as u32 };
        }
    }

    assert_eq!(mem.size(), 0);
}

#[test]
fn free_works() {
    let wgpu = get_wgpu();

    let mut mem = AutoDropping::<Entity, SimpleGpuMemory<Entity>>::new(
        wgpu::BufferUsages::empty(),
        &wgpu.device,
    );

    for _ in 0..100 {
        {
            let index = mem.allocate(1);
            assert_eq!(mem.size(), size_of::<Entity>());

            mem.get(&index)[0] = Entity { param: 1 };
        }

        assert_eq!(mem.size(), 0);
    }

    assert_eq!(mem.size(), 0);
}
