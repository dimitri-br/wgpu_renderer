use std::sync::Arc;
use log::{error, info};
use std::mem::size_of;
use wgpu::{Buffer, BufferAddress, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, Device, Queue};
use crate::renderer::types::light::Light;
use crate::renderer::types::shadow_data::ShadowData;

pub(crate) struct ShadowDataStorage {
    device: Arc<Device>,
    queue: Arc<Queue>,
    /// The current list of lights that we intend to upload.
    pub shadow_data: Vec<ShadowData>,
    /// The maximum number of lights the current GPU buffer can store.
    buffer_capacity: usize,
    storage_buffer: Buffer,
    /// Flag indicating that the bind group must be rebuilt (because the buffer was reallocated).
    pub needs_rebuild: bool,
}

impl ShadowDataStorage {
    /// Creates a new LightStorage with an initial capacity.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        const INITIAL_CAPACITY: usize = 1000;
        let storage_buffer = Self::create_buffer(&device, INITIAL_CAPACITY);
        Self {
            device,
            queue,
            shadow_data: Vec::with_capacity(INITIAL_CAPACITY),
            buffer_capacity: INITIAL_CAPACITY,
            storage_buffer,
            needs_rebuild: false,
        }
    }

    /// Helper to create a new storage buffer for a given capacity.
    fn create_buffer(device: &Device, capacity: usize) -> Buffer {
        let buffer_size = (size_of::<Light>() * capacity) as BufferAddress;
        device.create_buffer(&BufferDescriptor {
            label: Some("Shadow Data Storage Buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    pub fn add_shadow_data(&mut self, shadow_data: ShadowData) -> usize {
        self.shadow_data.push(shadow_data);
        self.ensure_capacity();
        self.shadow_data.len() - 1
    }

    pub fn remove_shadow_data(&mut self, index: usize) {
        if index < self.shadow_data.len() {
            self.shadow_data.remove(index);
            self.update_buffer();
        } else {
            error!(
                "Attempted to remove shadow data at index {} but only {} exist",
                index,
                self.shadow_data.len()
            );
        }
    }

    pub fn set_shadow_data(&mut self, shadow_data: Vec<ShadowData>) {
        self.shadow_data = shadow_data;
        self.ensure_capacity();
    }

    /// Replaces all lights and updates the buffer.
    pub fn set_all_shadow_data(&mut self, shadow_data: Vec<ShadowData>) {
        self.shadow_data = shadow_data;
        self.ensure_capacity();
    }

    /// Checks if the current buffer capacity is enough; if not, reallocates the buffer.
    /// If capacity is sufficient, just updates the buffer data.
    fn ensure_capacity(&mut self) {
        let num_lights = self.shadow_data.len();
        if num_lights > self.buffer_capacity {
            info!("Resizing light storage buffer to {}", num_lights);
            self.reallocate_buffer(num_lights);
        } else {
            self.update_buffer();
        }
    }

    /// Reallocates the storage buffer to hold at least `new_capacity` lights.
    fn reallocate_buffer(&mut self, new_capacity: usize) {
        self.needs_rebuild = true;
        self.buffer_capacity = new_capacity;
        self.storage_buffer = Self::create_buffer(&self.device, new_capacity);
        self.update_buffer();
    }

    /// Uploads the current light data to the GPU.
    pub fn update_buffer(&mut self) {
        let data = bytemuck::cast_slice(&self.shadow_data);
        self.queue.write_buffer(&self.storage_buffer, 0, data);
        self.needs_rebuild = false;
    }

    /// Returns a binding to the storage buffer, useful for binding it in a shader.
    pub fn get_buffer_binding(&self) -> BufferBinding {
        BufferBinding {
            buffer: &self.storage_buffer,
            offset: 0,
            size: None,
        }
    }
}
