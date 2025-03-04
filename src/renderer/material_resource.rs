//! material_resource.rs
//! Enum for material resource parameters (textures, samplers, buffers, etc.)

use std::sync::Arc;
use wgpu::{TextureView, Sampler};

#[derive(Clone)]
pub enum MaterialResource {
    Texture(Arc<TextureView>),
    Sampler(Arc<Sampler>),
    UniformBuffer(Arc<wgpu::Buffer>),
    // Extend with uniform buffers, storage buffers, etc.
}
