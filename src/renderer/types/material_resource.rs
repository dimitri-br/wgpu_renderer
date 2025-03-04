//! material_resource.rs
//! Enum for material resource parameters (textures, samplers, buffers, etc.)

use std::sync::Arc;
use wgpu::{TextureView, Sampler};
use crate::renderer::types::uniform::UniformBuffer;

#[derive(Clone)]
pub enum MaterialResource {
    Texture(Arc<TextureView>),
    Sampler(Arc<Sampler>),
    UniformBuffer(Arc<UniformBuffer>),
    // Extend with uniform buffers, storage buffers, etc.
}
