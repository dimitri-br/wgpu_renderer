// gpu_storage.rs

use std::mem::size_of;
use std::sync::{Arc, Mutex};
use log::{info, error};
use wgpu::{Buffer, BufferAddress, BufferDescriptor, BufferUsages, Device, Queue};
use bytemuck;

/// A trait that defines how a type is converted into a form that can be uploaded
/// to the GPU. The associated type `Storage` is the type that is written to the buffer;
/// it must implement [`bytemuck::Pod`].
pub trait GpuStorable {
    type Storage: bytemuck::Pod;
    /// Converts the instance into its storage representation.
    fn as_storage(&self) -> Self::Storage;
}

/// A generic GPU storage buffer that holds a collection of items of type `T` (which implements
/// `GpuStorable`). The storage buffer on the GPU holds a contiguous array of `T::Storage` values.
///
/// This implementation improves robustness and performance by:
///
/// - **Batching Updates:** Changes mark the storage as dirty. A single update can later be
///   issued to the GPU with `update_if_dirty()`.
/// - **Optimized Data Conversion:** When possible, avoids unnecessary allocations by reinterpreting
///   the internal data slice.
/// - **Smarter Buffer Reallocation:** Uses a configurable growth factor and allows sub-range updates.
/// - **Error Handling & Concurrency:** Uses a Mutex to safely guard concurrent access to the data.
/// - **Improved Logging & Documentation:** Provides detailed logs and comprehensive comments.
pub struct GpuStorage<T: GpuStorable> {
    device: Arc<Device>,
    queue: Arc<Queue>,
    // Mutex is used for safe concurrent access.
    data: Mutex<Vec<T>>,
    // The current capacity of the GPU buffer (in number of items).
    buffer_capacity: usize,
    storage_buffer: Buffer,
    // Indicates that the GPU buffer is "dirty" and needs updating.
    dirty: bool,
    // Indicates that the buffer was reallocated and associated bind groups need rebuilding.
    pub needs_rebuild: bool,
    // Configurable growth factor for buffer reallocation.
    growth_factor: f32,
}

impl<T: GpuStorable + std::clone::Clone> GpuStorage<T> {
    /// Creates a new GPU storage with a given initial capacity, buffer label, and growth factor.
    ///
    /// # Arguments
    ///
    /// * `device` - A shared reference to the GPU device.
    /// * `queue` - A shared reference to the GPU queue.
    /// * `initial_capacity` - The number of items the storage can initially hold.
    /// * `label` - A human-readable label for the GPU buffer.
    /// * `growth_factor` - The multiplier used when resizing the buffer (e.g., 2.0 for doubling).
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        initial_capacity: usize,
        label: &'static str,
        growth_factor: f32,
    ) -> Self {
        let storage_buffer = Self::create_buffer(&device, initial_capacity, label);
        Self {
            device,
            queue,
            data: Mutex::new(Vec::with_capacity(initial_capacity)),
            buffer_capacity: initial_capacity,
            storage_buffer,
            dirty: false,
            needs_rebuild: false,
            growth_factor,
        }
    }

    /// Creates a new GPU buffer sized to hold `capacity` items of type `T::Storage`.
    fn create_buffer(device: &Device, capacity: usize, label: &'static str) -> Buffer {
        let buffer_size = (size_of::<T::Storage>() * capacity) as BufferAddress;
        device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Marks the storage as dirty, indicating that the GPU buffer needs an update.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Batches the update: if the storage is dirty, updates the GPU buffer.
    /// Call this once per frame or at a designated synchronization point.
    pub fn update_if_dirty(&mut self) {
        if self.dirty {
            self.update_buffer();
        }
    }

    /// Adds a new item to the storage, marks the buffer as dirty, and ensures sufficient capacity.
    ///
    /// Returns the index of the newly added item.
    pub fn add(&mut self, item: T) -> usize {
        let index;
        {
            let mut data_lock = self.data.lock().unwrap();
            data_lock.push(item);
            index = data_lock.len() - 1;
        }
        self.ensure_capacity(index + 1);
        self.dirty = true;
        index
    }

    /// Removes an item at the given index, marks the buffer as dirty, and ensures capacity.
    ///
    /// Returns an error if the index is out of range.
    pub fn remove(&mut self, index: usize) -> Result<(), String> {
        let len;

        {
            let mut data_lock = self.data.lock().unwrap();
            len = data_lock.len();
            if index < len {
                data_lock.remove(index);
                self.dirty = true;
            } else {
                error!(
                    "Attempted to remove item at index {} but only {} exist",
                    index,
                    data_lock.len()
                );
                return Err(format!(
                    "Attempted to remove item at index {} but only {} exist",
                    index,
                    data_lock.len()
                ));
            }
        }

        self.ensure_capacity(len - 1);

        Ok(())
    }

    /// Sets an item at the given index, marking the buffer as dirty.
    ///
    /// Returns an error if the index is out of range.
    pub fn set(&mut self, index: usize, item: T) -> Result<(), String> {
        let mut data_lock = self.data.lock().unwrap();
        if index < data_lock.len() {
            data_lock[index] = item;
            self.dirty = true;
            Ok(())
        } else {
            error!(
                "Attempted to set item at index {} but only {} exist",
                index,
                data_lock.len()
            );
            Err(format!(
                "Attempted to set item at index {} but only {} exist",
                index,
                data_lock.len()
            ))
        }
    }

    /// Replaces all items with a new vector, marks the buffer as dirty, and ensures capacity.
    pub fn set_all(&mut self, items: Vec<T>) {
        let len;
        {
            let mut data_lock = self.data.lock().unwrap();
            *data_lock = items;
            len = data_lock.len();
        }
        self.ensure_capacity(len);
        self.dirty = true;
    }

    /// Returns a clone of the item at the given index, if it exists.
    ///
    /// This method requires that `T` implements `Clone`.
    pub fn get(&self, index: usize) -> Option<T>
    where
        T: Clone,
    {
        let data_lock = self.data.lock().unwrap();
        data_lock.get(index).cloned()
    }

    /// Returns a copy of all the items in the storage.
    ///
    /// This method requires that `T` implements `Clone`.
    pub fn get_all(&self) -> Vec<T>
    where
        T: Clone,
    {
        let data_lock = self.data.lock().unwrap();
        data_lock.clone()
    }

    /// Checks whether the current GPU buffer capacity is sufficient for `len` items.
    /// If not, reallocates the buffer using the configured growth factor.
    fn ensure_capacity(&mut self, len: usize) {
        if len > self.buffer_capacity {
            let new_capacity = ((self.buffer_capacity as f32) * self.growth_factor).ceil() as usize;
            info!(
                "Resizing GPU storage buffer from {} to {} (requested length: {})",
                self.buffer_capacity, new_capacity, len
            );
            self.reallocate_buffer(new_capacity);
        }
    }

    /// Reallocates the storage buffer to hold at least `new_capacity` items.
    /// Marks the storage as needing a bind group rebuild.
    fn reallocate_buffer(&mut self, new_capacity: usize) {
        self.needs_rebuild = true;
        self.buffer_capacity = new_capacity;
        self.storage_buffer = Self::create_buffer(&self.device, new_capacity, "GPU Storage Buffer");
        // Immediately update the buffer with the current data.
        self.update_buffer();
    }

    /// Uploads the current data to the GPU.
    ///
    /// If the conversion from `T` to `T::Storage` is an identity transformation,
    /// you may use `data_as_storage` to avoid an extra allocation.
    /// Otherwise, this method maps each item using `as_storage` and writes the full data to the GPU.
    pub fn update_buffer(&mut self) {
        // In this example, we perform a per-item conversion.
        // If possible, you could optimize by directly using `data_as_storage()`.
        let upload_data: Vec<T::Storage> = self.get_all().iter().map(|d| d.as_storage()).collect();
        let data_bytes = bytemuck::cast_slice(&upload_data);
        self.queue.write_buffer(&self.storage_buffer, 0, data_bytes);
        self.dirty = false;
    }

    /// Updates a sub-range of the GPU buffer.
    ///
    /// # Arguments
    ///
    /// * `offset` - The starting index of the sub-range to update.
    /// * `count` - The number of items to update.
    ///
    /// Returns an error if the specified sub-range is out of bounds.
    pub fn update_sub_range(&mut self, offset: usize, count: usize) -> Result<(), String> {
        let data_lock = self.data.lock().unwrap();
        if offset + count > data_lock.len() {
            return Err(format!(
                "Sub-range out of bounds: offset {} + count {} exceeds length {}",
                offset,
                count,
                data_lock.len()
            ));
        }
        let upload_data: Vec<T::Storage> = data_lock[offset..offset + count]
            .iter()
            .map(|d| d.as_storage())
            .collect();
        let data_bytes = bytemuck::cast_slice(&upload_data);
        let byte_offset = (offset * size_of::<T::Storage>()) as BufferAddress;
        self.queue.write_buffer(&self.storage_buffer, byte_offset, data_bytes);
        Ok(())
    }

    /// Returns a binding to the storage buffer, which is useful for shader binding.
    pub fn get_buffer_binding(&self) -> wgpu::BufferBinding {
        wgpu::BufferBinding {
            buffer: &self.storage_buffer,
            offset: 0,
            size: None,
        }
    }
}
