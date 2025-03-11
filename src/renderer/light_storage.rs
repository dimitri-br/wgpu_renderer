use std::sync::Arc;
use log::{error, info};
use std::mem::size_of;
use wgpu::{Buffer, BufferAddress, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, Device, Queue};
use crate::renderer::types::light::Light;

pub(crate) struct LightStorage {
    device: Arc<Device>,
    queue: Arc<Queue>,
    /// The current list of lights that we intend to upload.
    pub lights: Vec<Light>,
    /// The maximum number of lights the current GPU buffer can store.
    buffer_capacity: usize,
    storage_buffer: Buffer,
    /// Flag indicating that the bind group must be rebuilt (because the buffer was reallocated).
    pub needs_rebuild: bool,
}

impl LightStorage {
    /// Creates a new LightStorage with an initial capacity.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        const INITIAL_CAPACITY: usize = 1000;
        let storage_buffer = Self::create_buffer(&device, INITIAL_CAPACITY);
        Self {
            device,
            queue,
            lights: Vec::with_capacity(INITIAL_CAPACITY),
            buffer_capacity: INITIAL_CAPACITY,
            storage_buffer,
            needs_rebuild: false,
        }
    }

    /// Helper to create a new storage buffer for a given capacity.
    fn create_buffer(device: &Device, capacity: usize) -> Buffer {
        let buffer_size = (size_of::<Light>() * capacity) as BufferAddress;
        device.create_buffer(&BufferDescriptor {
            label: Some("Light Storage Buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Adds a new light and ensures the buffer can hold the new count.
    /// Returns the index of the added light.
    pub fn add_light(&mut self, light: Light) -> usize {
        self.lights.push(light);
        self.ensure_capacity();
        self.lights.len() - 1
    }

    /// Removes a light at the given index and updates the buffer.
    pub fn remove_light(&mut self, index: usize) {
        if index < self.lights.len() {
            self.lights.remove(index);
            // In this example we update the buffer after removal.
            // Depending on your needs you might not shrink the GPU buffer.
            self.update_buffer();
        } else {
            error!(
                "Attempted to remove light at index {} but only {} exist",
                index,
                self.lights.len()
            );
        }
    }

    /// Sets a light at the given index, updating the buffer.
    pub fn set_light(&mut self, index: usize, light: Light) {
        if index < self.lights.len() {
            self.lights[index] = light;
            self.update_buffer();
        } else {
            error!(
                "Attempted to set light at index {} but only {} exist",
                index,
                self.lights.len()
            );
        }
    }

    /// Replaces all lights and updates the buffer.
    pub fn set_lights(&mut self, lights: Vec<Light>) {
        self.lights = lights;
        self.ensure_capacity();
    }

    /// Gets a light at the given index.
    pub fn get_light(&self, index: usize) -> Option<&Light> {
        self.lights.get(index)
    }

    /// Gets a mutable reference to a light at the given index.
    pub fn get_light_mut(&mut self, index: usize) -> Option<&mut Light> {
        self.lights.get_mut(index)
    }

    /// Gets all lights.
    pub fn get_all_lights(&self) -> &Vec<Light> {
        &self.lights
    }

    /// Checks if the current buffer capacity is enough; if not, reallocates the buffer.
    /// If capacity is sufficient, just updates the buffer data.
    fn ensure_capacity(&mut self) {
        let num_lights = self.lights.len();
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
        let data = bytemuck::cast_slice(&self.lights);
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
