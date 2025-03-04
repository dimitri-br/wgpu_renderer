use std::collections::HashMap;
use std::sync::{Arc};
use wgpu::{
    RenderPipeline, TextureView, Sampler, Face,
    BlendState, ColorTargetState, BlendComponent, BlendFactor, BlendOperation,
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource,
};
use log::error;
use crate::renderer::bind_group_cache::{BindGroupCache, BindGroupKey};
// or warn, depending on your preference

use crate::renderer::material_resource::MaterialResource;
use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::shader_reflect::{Shader, Binding};

/// Pipeline-affecting parameters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineParams {
    pub transparent: bool,
    pub cull_mode: Option<Face>,
}

impl Default for PipelineParams {
    fn default() -> Self {
        Self {
            transparent: false,
            cull_mode: Some(Face::Back),
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
    cached_bind_groups: Vec<Option<Arc<BindGroup>>>,
    bind_groups_dirty: bool,
}

impl Material {
    pub(crate) fn new(
        shader: Arc<Shader>,
        pipeline_manager: Arc<PipelineManager>,
        device: wgpu::Device,
        bind_group_cache: Arc<BindGroupCache>,
    ) -> Self {
        // Prepare a vector of empty bind group slots matching the number
        // of bind group layouts in the shader.
        let num_bind_groups = shader.get_bind_group_layouts().len();
        Self {
            pipeline_manager,
            device,
            shader,
            pipeline_params: PipelineParams::default(),
            cached_pipeline: None,
            resource_params: HashMap::new(),
            bind_group_cache,
            cached_bind_groups: vec![None; num_bind_groups],
            bind_groups_dirty: true,
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
            },
        );

        self.cached_pipeline = Some(pipeline.clone());
        pipeline
    }

    // -----------------
    // Resource Params
    // -----------------
    pub fn set_texture(&mut self, param_name: &str, view: Arc<TextureView>) {
        self.resource_params.insert(param_name.to_string(), MaterialResource::Texture(view));
        self.bind_groups_dirty = true;
    }

    pub fn set_sampler(&mut self, param_name: &str, sampler: Arc<Sampler>) {
        self.resource_params.insert(param_name.to_string(), MaterialResource::Sampler(sampler));
        self.bind_groups_dirty = true;
    }

    pub fn set_uniform(&mut self, param_name: &str, buffer: Arc<wgpu::Buffer>) {
        self.resource_params.insert(param_name.to_string(), MaterialResource::UniformBuffer(buffer));
        self.bind_groups_dirty = true;
    }

    // -----------------
    // Bind Group Caching
    // -----------------

    /// Returns an immutable slice of the cached bind groups.
    /// If `bind_groups_dirty` is true, we rebuild them first.
    pub fn get_bind_groups(&mut self) -> &[Option<Arc<BindGroup>>] {
        if self.bind_groups_dirty {
            self.rebuild_bind_groups();
        }
        &self.cached_bind_groups
    }

    pub fn bind<'a>(&'a mut self, render_pass: &mut wgpu::RenderPass<'a>) {
        // Get the bind groups
        let bind_groups = self.get_bind_groups();

        // Iterate over each bind group, binding it to the render pass
        for (index, bg) in bind_groups.iter().enumerate() {
            if let Some(bg) = bg {
                render_pass.set_bind_group(index as u32, &**bg, &[]);
            }
        }
    }

    fn rebuild_bind_groups(&mut self) {
        self.bind_groups_dirty = false;
        let layouts = self.shader.get_bind_group_layouts();
        let num_groups = layouts.len();

        let shader_bindings = self.shader.get_bindings();

        for (group_index, layout) in layouts.iter().enumerate() {
            // Collect entries
            let mut entries = Vec::new();
            let mut resource_ids = Vec::new();
            let mut missing_resource = false;

            for b in shader_bindings.iter().filter(|b| b.group == group_index as u32) {
                if let Some(name) = &b.name {
                    if let Some(resource) = self.resource_params.get(name) {
                        // We have a resource for this binding
                        let entry = BindGroupEntry {
                            binding: b.binding,
                            resource: match resource {
                                MaterialResource::Texture(view) => BindingResource::TextureView(view),
                                MaterialResource::Sampler(smp) => BindingResource::Sampler(smp),
                                MaterialResource::UniformBuffer(buf) => BindingResource::Buffer(buf.as_entire_buffer_binding()),
                            },
                        };
                        entries.push(entry);

                        // For hashing, store pointer addresses or resource IDs
                        // You can store e.g. Arc::as_ptr(...) cast to usize
                        // or store some ID from a resource manager.
                        let ptr_id = match resource {
                            MaterialResource::Texture(view) => Arc::<wgpu::TextureView>::as_ptr(&view) as usize,
                            MaterialResource::Sampler(smp) => Arc::<wgpu::Sampler>::as_ptr(&smp) as usize,
                            MaterialResource::UniformBuffer(buf) => Arc::<wgpu::Buffer>::as_ptr(&buf) as usize,
                        };
                        resource_ids.push(ptr_id);
                    } else {
                        missing_resource = true;
                        log::error!("Missing resource for binding '{}' in group {}", name, group_index);
                    }
                } else {
                    // If no name is provided, skip or handle as you want
                    missing_resource = true;
                    log::error!("Binding in group {} has no name, skipping...", group_index);
                }
            }

            if missing_resource || entries.is_empty() {
                // If we can’t build this group, store None
                self.cached_bind_groups[group_index] = None;
            } else {
                // Build the key
                // Sort resource_ids if you want stable ordering, but here we assume
                // the iteration order is stable for these specific bindings.
                // If you like, do resource_ids.sort();
                let key = BindGroupKey::new(layout.as_ref(), resource_ids);

                // Ask the global cache for a bind group
                let bg = self.bind_group_cache.get_or_create(
                    layout.as_ref(),
                    &entries,
                    key,
                );
                self.cached_bind_groups[group_index] = Some(bg);
            }
        }
    }
}
