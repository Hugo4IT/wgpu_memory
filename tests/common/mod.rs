#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Entity {
    pub param: u32,
}

pub struct Wgpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

pub fn get_wgpu() -> Wgpu {
    let instance = wgpu::Instance::new(Default::default());

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Render device"),
            required_features: adapter.features(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))
    .unwrap();

    Wgpu {
        instance,
        adapter,
        device,
        queue,
    }
}
