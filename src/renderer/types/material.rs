use crate::renderer::bind_group_cache::{BindGroupCache, BindGroupKey};
use log::{error, warn};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{
    BindGroup, BindGroupEntry, BindingResource, BlendComponent, BlendFactor, BlendOperation, BlendState, DepthBiasState, DepthStencilState, Face, FrontFace, RenderPipeline, StencilState, TextureView
};
// or warn, depending on your preference

use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::shader_reflect::Shader;
use crate::renderer::types::material_resource::MaterialResource;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::uniform::UniformBuffer;

pub const MATERIAL_GROUP_INDEX: u32 = 1;

/// Pipeline-affecting parameters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineParams {
    pub transparent: bool,
    pub cull_mode: Option<Face>,
    pub front_face: FrontFace,
    pub use_depth: bool
}

impl Default for PipelineParams {
    fn default() -> Self {
        Self {
            transparent: false,
            cull_mode: Some(Face::Back),
            front_face: FrontFace::Ccw,
            use_depth: true
        }
    }
}

/// A high-level Material referencing a `Shader` and storing:
/// - Pipeline parameters (blend, cull, etc.)
/// - Resource parameters (textures, samplers, etc.)
///
/// We also cache:
/// - The resulting pipeline
/// - The created bind groups (one per bind group index)
pub struct Material {
    pipeline_manager: Arc<PipelineManager>,
    device: wgpu::Device,

    shader: Arc<Shader>,

    // For pipeline creation:
    pipeline_params: PipelineParams,
    cached_pipeline: Option<Arc<RenderPipeline>>,

    // For resource binding:
    resource_params: HashMap<String, MaterialResource>,

    // We’ll keep the global bind group cache in a field, or store it globally somewhere else.
    bind_group_cache: Arc<BindGroupCache>,

    // The final built bind groups, if any. We can store a vector or map from group index -> Arc<BindGroup>.
    cached_bind_group: Option<Arc<BindGroup>>,
    bind_group_dirty: bool,
}

impl Material {
    pub(crate) fn new(
        shader: Arc<Shader>,
        pipeline_manager: Arc<PipelineManager>,
        device: wgpu::Device,
        bind_group_cache: Arc<BindGroupCache>,
    ) -> Self {
        Self {
            pipeline_manager,
            device,
            shader,
            pipeline_params: PipelineParams::default(),
            cached_pipeline: None,
            resource_params: HashMap::new(),
            bind_group_cache,
            cached_bind_group: None,
            bind_group_dirty: true,
        }
    }

    // -----------------
    // Pipeline Params
    // -----------------
    pub fn set_transparent(&mut self, enable: bool) {
        if self.pipeline_params.transparent != enable {
            self.pipeline_params.transparent = enable;
            self.cached_pipeline = None;
        }
    }

    pub fn set_cull_mode(&mut self, mode: Option<Face>) {
        if self.pipeline_params.cull_mode != mode {
            self.pipeline_params.cull_mode = mode;
            self.cached_pipeline = None;
        }
    }

    pub fn set_front_face(&mut self, face: FrontFace) {
        if self.pipeline_params.front_face != face {
            self.pipeline_params.front_face = face;
            self.cached_pipeline = None;
        }
    }

    pub fn set_depth(&mut self, use_depth: bool){
        if self.pipeline_params.use_depth != use_depth{
            self.pipeline_params.use_depth = use_depth;
            self.cached_pipeline = None;
        }
    }

    pub fn get_shader(&self) -> Arc<Shader> {
        self.shader.clone()
    }

    /// Build or retrieve the pipeline from the pipeline manager.
    /// This is typically called during rendering.
    pub fn get_pipeline(&mut self) -> Arc<RenderPipeline> {
        if let Some(pipe) = &self.cached_pipeline {
            return pipe.clone();
        }

        let pipeline = self.pipeline_manager.create_pipeline_with_config(
            (*self.shader).clone(),
            |desc, color_targets, primitive| {
                // If transparent, set alpha blending on each color target
                if self.pipeline_params.transparent {
                    for tgt_opt in color_targets.iter_mut() {
                        if let Some(cts) = tgt_opt {
                            cts.blend = Some(BlendState {
                                color: BlendComponent {
                                    src_factor: BlendFactor::SrcAlpha,
                                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                                    operation: BlendOperation::Add,
                                },
                                alpha: BlendComponent {
                                    src_factor: BlendFactor::One,
                                    dst_factor: BlendFactor::One,
                                    operation: BlendOperation::Add,
                                },
                            });
                        }
                    }
                }
                // Cull Mode
                primitive.cull_mode = self.pipeline_params.cull_mode;
                // Front Face
                primitive.front_face = self.pipeline_params.front_face;
                desc.depth_stencil = if self.pipeline_params.use_depth {
                    Some(
                        DepthStencilState{
                            format: wgpu::TextureFormat::Depth32Float,
                            depth_write_enabled: true,
                            depth_compare: wgpu::CompareFunction::LessEqual,
                            stencil: StencilState::default(),
                            bias: DepthBiasState::default()
                        }
                    )
                }else{
                    None
                };
            },
        );

        self.cached_pipeline = Some(pipeline.clone());
        pipeline
    }

    // -----------------
    // Resource Params
    // -----------------
    pub fn set_texture(&mut self, param_name: &str, view: Arc<TextureView>) {
        self.resource_params
            .insert(param_name.to_string(), MaterialResource::Texture(view));
        self.bind_group_dirty = true;
    }

    pub fn set_sampler(&mut self, param_name: &str, sampler_parameters: SamplerParameters) {
        self.resource_params.insert(
            param_name.to_string(),
            MaterialResource::Sampler(Arc::new(sampler_parameters.create_sampler(&self.device))),
        );
        self.bind_group_dirty = true;
    }

    pub fn set_uniform(&mut self, param_name: &str, buffer: Arc<UniformBuffer>) {
        self.resource_params.insert(
            param_name.to_string(),
            MaterialResource::UniformBuffer(buffer),
        );
        self.bind_group_dirty = true;
    }

    // -----------------
    // Bind Group Caching
    // -----------------

    /// Returns an immutable slice of the cached bind groups.
    /// If `bind_groups_dirty` is true, we rebuild them first.
    pub fn get_bind_groups(&mut self) -> Option<Arc<BindGroup>> {
        if self.bind_group_dirty {
            self.rebuild_bind_group();
        }
        self.cached_bind_group.clone()
    }

    pub fn bind<'a>(&'a mut self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.bind_group_dirty {
            self.rebuild_bind_group();
            warn!("Rebuilding bind group during bind");
        }

        if let Some(bg) = self.get_bind_groups() {
            render_pass.set_bind_group(MATERIAL_GROUP_INDEX, &*bg, &[]);
        } else {
            warn!("No bind group for material");
        }
    }

    fn rebuild_bind_group(&mut self) {
        self.bind_group_dirty = false;
        let layout = match self
            .shader
            .get_bind_group_layout(MATERIAL_GROUP_INDEX as u64)
        {
            Some(layout) => layout,
            None => {
                error!("No bind group layout for group {}", MATERIAL_GROUP_INDEX);
                return;
            }
        };

        let shader_bindings = self.shader.get_bindings();

        // Collect entries
        let mut entries = Vec::new();
        let mut resource_ids = Vec::new();
        let mut missing_resource = false;

        for b in shader_bindings
            .iter()
            .filter(|b| b.group == MATERIAL_GROUP_INDEX)
        {
            if let Some(name) = &b.name {
                if let Some(resource) = self.resource_params.get(name) {
                    // We have a resource for this binding
                    let entry = BindGroupEntry {
                        binding: b.binding,
                        resource: match resource {
                            MaterialResource::Texture(view) => BindingResource::TextureView(view),
                            MaterialResource::Sampler(smp) => BindingResource::Sampler(smp),
                            MaterialResource::UniformBuffer(buf) => {
                                BindingResource::Buffer(buf.get_buffer_binding())
                            }
                        },
                    };
                    entries.push(entry);

                    // For hashing, store pointer addresses or resource IDs
                    // You can store e.g. Arc::as_ptr(...) cast to usize
                    // or store some ID from a resource manager.
                    let ptr_id = match resource {
                        MaterialResource::Texture(view) => {
                            Arc::<wgpu::TextureView>::as_ptr(&view) as usize
                        }
                        MaterialResource::Sampler(smp) => {
                            Arc::<wgpu::Sampler>::as_ptr(&smp) as usize
                        }
                        MaterialResource::UniformBuffer(buf) => {
                            Arc::<UniformBuffer>::as_ptr(&buf) as usize
                        }
                    };
                    resource_ids.push(ptr_id);
                } else {
                    missing_resource = true;
                    error!("Missing resource for binding '{}'", name);
                }
            } else {
                // If no name is provided, skip or handle as you want
                missing_resource = true;
                error!("Unnamed binding in shader");
            }
        }

        if missing_resource || entries.is_empty() {
            // If we can’t build this group, store None
            self.cached_bind_group = None;
        } else {
            // Build the key
            // Sort resource_ids if you want stable ordering, but here we assume
            // the iteration order is stable for these specific bindings.
            // If you like, do resource_ids.sort();
            let key = BindGroupKey::new(layout.as_ref(), resource_ids);

            // Ask the global cache for a bind group
            let bg = self
                .bind_group_cache
                .get_or_create(layout.as_ref(), &entries, key);
            self.cached_bind_group = Some(bg);
        }
    }
}
