pub trait Uniform {
    fn update_uniforms(&self, uniforms: &wgpu::Buffer, queue: &wgpu::Queue);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UniformBuffer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    buffer: wgpu::Buffer,
    size: u64,
}

impl UniformBuffer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, size: u64) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        Self {
            device: device.clone(),
            queue: queue.clone(),
            buffer,
            size,
        }
    }

    pub fn update<T: Uniform>(&self, data: &T) {
        data.update_uniforms(&self.buffer, &self.queue);
    }

    pub fn get_buffer_binding(&self) -> wgpu::BufferBinding {
        wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: None,
        }
    }
}
