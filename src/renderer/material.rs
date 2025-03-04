use std::collections::HashMap;
use std::sync::{Arc};
use wgpu::{
    RenderPipeline, TextureView, Sampler, Face,
    BlendState, ColorTargetState, BlendComponent, BlendFactor, BlendOperation,
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource,
};
use log::error; // or warn, depending on your preference

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
    device: Arc<wgpu::Device>,

    shader: Arc<Shader>,

    // For pipeline creation:
    pipeline_params: PipelineParams,
    cached_pipeline: Option<Arc<RenderPipeline>>,

    // For resource binding:
    resource_params: HashMap<String, MaterialResource>,

    // Cached bind groups, matching the number of bind group layouts in the shader.
    // If a bind group fails to build due to missing resources, store None.
    cached_bind_groups: Vec<Option<Arc<BindGroup>>>,

    // A "dirty" flag that tells us we need to rebuild bind groups.
    bind_groups_dirty: bool,
}

impl Material {
    pub fn new(
        shader: Arc<Shader>,
        pipeline_manager: Arc<PipelineManager>,
        device: Arc<wgpu::Device>,
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

    // And similarly for buffers, etc.

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

    /// Rebuild the bind groups, storing them in `cached_bind_groups`.
    /// If a resource is missing for a required binding, we log an error and store `None`.
    fn rebuild_bind_groups(&mut self) {
        self.bind_groups_dirty = false;

        // Prepare a new vector for each group
        let layouts = self.shader.get_bind_group_layouts();
        let num_groups = layouts.len();
        let mut new_bind_groups: Vec<Option<Arc<BindGroup>>> = Vec::with_capacity(num_groups);

        // We'll iterate over each group index, gather resources, create a bind group if possible.
        for (group_index, layout) in layouts.iter().enumerate() {
            // Collect all wgpu::BindGroupEntry for this group
            let mut entries = Vec::new();
            let mut missing_resource = false;

            // Filter the shader's binding info to those that belong to this group
            for binding_info in self.shader.get_bindings().iter().filter(|b| b.group == group_index as u32) {
                // The reflection might store an optional name for this binding
                if let Some(binding_name) = &binding_info.name {
                    // Look up the resource from `resource_params`
                    if let Some(resource) = self.resource_params.get(binding_name) {
                        // We have a resource for this binding
                        let entry = BindGroupEntry {
                            binding: binding_info.binding,
                            resource: match resource {
                                MaterialResource::Texture(view) => BindingResource::TextureView(view),
                                MaterialResource::Sampler(smp) => BindingResource::Sampler(smp),
                            },
                        };
                        entries.push(entry);
                    } else {
                        // Resource is missing
                        error!(
                            "Material is missing resource for binding '{}'. Group {} binding {} will be invalid.",
                            binding_name, group_index, binding_info.binding
                        );
                        missing_resource = true;
                    }
                } else {
                    // Binding info has no name? Possibly a built-in or something. 
                    // If you expect all to have names, log or skip:
                    error!("Binding in group {} has no name! Skipping...", group_index);
                    missing_resource = true;
                }
            }

            // If we discovered a missing resource, skip creating this bind group
            if missing_resource {
                new_bind_groups.push(None);
            } else {
                // Create the bind group
                let bg = self.device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Material BindGroup"),
                    layout: layout.as_ref(),
                    entries: &entries,
                });
                new_bind_groups.push(Some(Arc::new(bg)));
            }
        }

        // Store in the material
        self.cached_bind_groups = new_bind_groups;
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
}
