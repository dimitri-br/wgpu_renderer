// shadow_data_storage.rs

use std::sync::Arc;
use wgpu::{Device, Queue};
use crate::renderer::ecs::components::ShadowMapComponent;
use crate::renderer::gpu_storage::GpuStorage;

/// A wrapper around `GpuStorage<ShadowMapComponent>` that mimics your original `ShadowDataStorage`.
pub struct ShadowDataStorage {
    pub storage: GpuStorage<ShadowMapComponent>,
}

impl ShadowDataStorage {
    /// Creates a new ShadowDataStorage.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        // Again, initial capacity of 1000 with a growth factor of 2.0.
        let storage = GpuStorage::new(device, queue, 1000, "Shadow Data Storage Buffer", 2.0);
        Self { storage }
    }

    /// Adds new shadow data and returns its index.
    pub fn add_shadow_data(&mut self, shadow_data: ShadowMapComponent) -> usize {
        self.storage.add(shadow_data)
    }

    /// Removes the shadow data at the given index.
    pub fn remove_shadow_data(&mut self, index: usize) -> Result<(), String> {
        self.storage.remove(index)
    }

    /// Replaces the shadow data at the given index.
    pub fn set_shadow_data(&mut self, index: usize, shadow_data: ShadowMapComponent) -> Result<(), String> {
        self.storage.set(index, shadow_data)
    }

    /// Gets a clone of the shadow data at the given index.
    pub fn get_shadow_data(&self, index: usize) -> Option<ShadowMapComponent>
    where
        ShadowMapComponent: Clone,
    {
        self.storage.get(index)
    }

    /// Returns all the shadow data.
    pub fn get_all_shadow_data(&self) -> Vec<ShadowMapComponent>
    where
        ShadowMapComponent: Clone,
    {
        self.storage.get_all()
    }

    /// Updates the GPU buffer if any changes have been made.
    pub fn update(&mut self) {
        self.storage.update_if_dirty();
    }
}
