//! material_resource.rs
//! Enum for material resource parameters (textures, samplers, buffers, etc.)

use crate::renderer::types::uniform::UniformBuffer;
use std::sync::Arc;
use wgpu::{Sampler, TextureView};

#[derive(Clone)]
pub enum MaterialResource {
    Texture(Arc<TextureView>),
    Sampler(Arc<Sampler>),
    UniformBuffer(Arc<UniformBuffer>),
    // Extend with uniform buffers, storage buffers, etc.
}
