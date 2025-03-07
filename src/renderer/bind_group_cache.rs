use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Device};

/// A unique key that represents a single bind group configuration:
/// - The layout pointer
/// - A list of resources (pointer addresses or IDs) in stable order
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindGroupKey {
    /// We store the pointer address of the layout for quick equality/hashing.
    /// Real engines often store an ID or a direct Arc<BindGroupLayout> if it’s guaranteed unique.
    layout_ptr: usize,
    /// A stable list of resource “identifiers.”
    /// For textures/samplers in wgpu, you can do pointer addresses or your own ID system.
    resources: Vec<usize>,
}

impl BindGroupKey {
    pub fn new(layout: &BindGroupLayout, resource_ids: Vec<usize>) -> Self {
        Self {
            layout_ptr: layout as *const BindGroupLayout as usize,
            resources: resource_ids,
        }
    }
}

// We implement `Hash` so that we can store BindGroupKey in a HashMap.
impl Hash for BindGroupKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.layout_ptr.hash(state);
        self.resources.hash(state);
    }
}

/// Global or engine-wide bind group cache, keyed by BindGroupKey.
pub struct BindGroupCache {
    device: Arc<Device>,
    cache: RwLock<HashMap<BindGroupKey, Arc<BindGroup>>>,
}

impl BindGroupCache {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Tries to find an existing bind group in the cache, or creates it if not present.
    pub fn get_or_create(
        &self,
        layout: &BindGroupLayout,
        entries: &[BindGroupEntry],
        key: BindGroupKey,
        reuse: bool,
    ) -> Arc<BindGroup> {
        // 1) Check if a bind group already exists for this key
        if reuse {
            let cache = self.cache.read().unwrap();
            if let Some(bg) = cache.get(&key) {
                log::info!("Found cached BindGroup");
                return bg.clone();
            }
        }else{
            // Drop the old bind group
            let mut cache = self.cache.write().unwrap();
            cache.remove(&key);
        }
        log::info!("Creating new BindGroup");

        // 2) Not found, create and insert
        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Cached BindGroup"),
            layout,
            entries,
        });

        let bind_group = Arc::new(bind_group);
        let mut cache = self.cache.write().unwrap();
        cache.insert(key, bind_group.clone());
        bind_group
    }
}
