use std::sync::Arc;
use shipyard::Unique;
use crate::renderer::types::light::Light;

pub(crate) struct LightStorage {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pub lights: Vec<Light>,
    curr_len: usize,
    storage_buffer: wgpu::Buffer,
    pub delta: bool
}

impl LightStorage {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Storage Buffer"),
            size: (size_of::<Light>() * 1000) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        println!("Size: {:?}", (size_of::<Light>() * 1000) as wgpu::BufferAddress);

        Self {
            device,
            queue,
            lights: Vec::new(),
            curr_len: 1000,
            storage_buffer,
            delta: false
        }
    }

    pub fn add_light(&mut self, light: Light) -> usize {
        self.lights.push(light);
        self.resize();
        self.lights.len() - 1
    }

    pub fn remove_light(&mut self, index: usize) {
        self.lights.remove(index);
        self.resize();
    }

    pub fn resize(&mut self) {
        let new_len = self.lights.len();
        if new_len <= self.curr_len {
            return;
        }
        self.delta = true;

        let new_size = (size_of::<Light>() * new_len) as wgpu::BufferAddress;
        let new_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Storage Buffer"),
            size: new_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.queue.write_buffer(&new_buffer, 0, bytemuck::cast_slice(&self.lights));
        self.storage_buffer = new_buffer;
        self.curr_len = new_len;
    }

    pub fn set_light(&mut self, index: usize, light: Light) {
        self.lights[index] = light;
        self.resize();
    }

    pub fn set_lights(&mut self, lights: Vec<Light>) {
        self.lights = lights;
        self.resize();
    }

    pub fn update(&mut self) {
        if self.lights.is_empty() || !self.delta {
            return;
        }

        self.queue.write_buffer(&self.storage_buffer, 0, bytemuck::cast_slice(&self.lights));
        self.delta = false;
    }

    pub fn get_buffer_binding(&self) -> wgpu::BufferBinding {
        wgpu::BufferBinding {
            buffer: &self.storage_buffer,
            offset: 0,
            size: None,
        }
    }
}