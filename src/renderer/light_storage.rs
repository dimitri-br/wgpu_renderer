// light_storage.rs

use std::sync::Arc;
use wgpu::{Device, Queue};
use crate::renderer::gpu_storage::GpuStorage;
use crate::renderer::types::light::Light;

/// A wrapper around `GpuStorage<Light>` that provides an API similar to your original `LightStorage`.
pub struct LightStorage {
    pub storage: GpuStorage<Light>,
}

impl LightStorage {
    /// Creates a new LightStorage.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        // Create the generic storage with an initial capacity of 1000 and a doubling growth factor.
        let storage = GpuStorage::new(device, queue, 1000, "Light Storage Buffer", 2.0);
        Self { storage }
    }

    /// Adds a new light and returns its index.
    pub fn add_light(&mut self, light: Light) -> usize {
        self.storage.add(light)
    }

    /// Removes the light at the given index.
    pub fn remove_light(&mut self, index: usize) -> Result<(), String> {
        self.storage.remove(index)
    }

    /// Sets the light at the given index.
    pub fn set_light(&mut self, index: usize, light: Light) -> Result<(), String> {
        self.storage.set(index, light)
    }

    /// Sets all the lights.
    pub fn set_all_lights(&mut self, lights: Vec<Light>) {
        self.storage.set_all(lights);
    }

    /// Gets a clone of the light at the given index.
    pub fn get_light(&self, index: usize) -> Option<Light>
    where
        Light: Clone,
    {
        self.storage.get(index)
    }

    /// Returns all the lights.
    pub fn get_all_lights(&self) -> Vec<Light>
    where
        Light: Clone,
    {
        self.storage.get_all()
    }

    /// Should be called at the end of the frame to update the GPU buffer if any changes occurred.
    pub fn update(&mut self) {
        self.storage.update_if_dirty();
    }
}
