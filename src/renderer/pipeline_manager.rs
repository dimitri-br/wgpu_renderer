//! pipeline_manager.rs
//! Manages pipeline layouts and pipelines, caching them based on reflection data.

use crate::renderer::shader_reflect::Shader;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, RwLock},
};
use wgpu::{Device, PipelineLayout, PushConstantRange, RenderPipeline};

// The closure type now takes three mut references:
//   1) The pipeline descriptor
//   2) The color_targets slice
//   3) The primitive state
type PipelineConfigFn = dyn FnOnce(
    &mut wgpu::RenderPipelineDescriptor,
    &mut [Option<wgpu::ColorTargetState>],
    &mut wgpu::PrimitiveState,
);

// -----------------------------------------------------------------------------
// PipelineLayoutKey
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineLayoutKey {
    // Because the user’s code used pointer addresses for BindGroupLayouts,
    // let’s do something similar or simpler. For demonstration, store a key string.
    bind_group_layout_ids: Vec<String>,
    push_constant_ranges: Vec<PushConstantRange>,
}

impl PipelineLayoutKey {
    pub fn new(shader: &Shader) -> Self {
        let bgl_ids = shader
            .get_bind_group_layouts()
            .iter()
            .map(|bgl| {
                // a pseudo-unique ID
                format!("{:p}", bgl.as_ref())
            })
            .collect();
        let push_constants = shader.get_push_constant_ranges();
        Self {
            bind_group_layout_ids: bgl_ids,
            push_constant_ranges: push_constants,
        }
    }
}

impl Hash for PipelineLayoutKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bind_group_layout_ids.hash(state);
        self.push_constant_ranges.hash(state);
    }
}

// -----------------------------------------------------------------------------
// PipelineLayoutManager
// -----------------------------------------------------------------------------

pub struct PipelineLayoutManager {
    device: Arc<Device>,
    layouts: RwLock<HashMap<PipelineLayoutKey, Arc<PipelineLayout>>>,
}

impl PipelineLayoutManager {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            layouts: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_layout(&self, shader: &Shader) -> Arc<PipelineLayout> {
        let key = PipelineLayoutKey::new(shader);

        // Try read
        {
            let layouts = self.layouts.read().unwrap();
            if let Some(layout) = layouts.get(&key) {
                return layout.clone();
            }
        }

        // Not found, build it
        let bgl: Vec<_> = shader.get_bind_group_layouts();
        let bgl_refs = bgl.iter().map(|bgl| bgl.as_ref()).collect::<Vec<_>>();

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PipelineLayout"),
                bind_group_layouts: &bgl_refs,
                push_constant_ranges: &key.push_constant_ranges,
            });
        let layout_arc = Arc::new(pipeline_layout);

        // Insert
        let mut layouts = self.layouts.write().unwrap();
        layouts.insert(key, layout_arc.clone());
        layout_arc
    }
}

// -----------------------------------------------------------------------------
// PipelineManager
// -----------------------------------------------------------------------------

pub struct PipelineManager {
    device: Arc<Device>,
    layout_manager: Arc<PipelineLayoutManager>,

    // Optional pipeline cache
    pipelines: RwLock<HashMap<String, Arc<RenderPipeline>>>,
}

impl PipelineManager {
    pub fn new(device: Arc<Device>) -> Self {
        let layout_manager = Arc::new(PipelineLayoutManager::new(device.clone()));
        Self {
            device,
            layout_manager,
            pipelines: RwLock::new(HashMap::new()),
        }
    }

    // Creates a pipeline with a custom config function that can modify the descriptor,
    // the color targets, and the primitive state in one scope.
    pub fn create_pipeline_with_config<F>(
        &self,
        shader: Shader,
        config_fn: F,
    ) -> Arc<RenderPipeline>
    where
        F: FnOnce(
            &mut wgpu::RenderPipelineDescriptor,
            &mut [Option<wgpu::ColorTargetState>],
            &mut wgpu::PrimitiveState,
        ),
    {
        // 1) Ensure the shader is analyzed
        if shader.get_bind_group_layouts().is_empty() {
            log::error!("No bind group layouts");
            panic!("Shader must be analyzed before creating a pipeline");
        }

        // 2) Compile the shader and gather reflection data
        let module = shader
            .compile(&self.device)
            .expect("Shader compilation failed");
        let vertex_entry = shader
            .get_vertex_entry_point()
            .unwrap_or_else(|| "main".to_string());
        let fragment_entry = shader
            .get_fragment_entry_point()
            .unwrap_or_else(|| "main".to_string());
        let color_targets = shader.get_color_targets();

        let layout = (*self.layout_manager.get_layout(&shader)).clone();
        // 3) Build the pipeline descriptor. Notice that color_targets is a local Vec, so we
        //    store a reference to it in the descriptor.
        let mut descriptor = wgpu::RenderPipelineDescriptor {
            label: Some("Custom Render Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(&vertex_entry),
                compilation_options: Default::default(),
                buffers: &shader.get_vertex_buffer_layouts(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(&fragment_entry),
                compilation_options: Default::default(),
                targets: &color_targets, // reference into our local Vec
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        };

        // 4) Let the caller modify the pipeline descriptor, the color_targets,
        //    and the primitive state in one closure call.

        let mut updated_color_targets = color_targets.clone();
        let mut updated_descriptor = descriptor.clone();
        config_fn(
            &mut descriptor,
            &mut updated_color_targets,
            &mut updated_descriptor.primitive,
        );

        // 5) Now that everything is set, reassign the possibly modified color_targets
        if let Some(fragment) = &mut descriptor.fragment {
            fragment.targets = &updated_color_targets;
        }

        // 6) Now reassign the possibly modified descriptor
        descriptor.primitive = updated_descriptor.primitive;

        // 7) Finally, create the pipeline. The references in descriptor (including color_targets)
        //    only need to live until we exit this function, which is valid.
        let pipeline = self.device.create_render_pipeline(&descriptor);
        Arc::new(pipeline)
    }

    /// Simple get_pipeline that returns a pipeline from a default config function (empty).
    /// If you want to cache by the pipeline layout key, etc., store or generate a key here.
    pub fn get_pipeline(&self, shader: &Shader) -> Arc<RenderPipeline> {
        let key = format!("{:?}-default", shader.hash_to_string());
        {
            let read_map = self.pipelines.read().unwrap();
            if let Some(pipe) = read_map.get(&key) {
                return pipe.clone();
            }
        }
        let pipeline = self.create_pipeline_with_config(shader.clone(), |_, _, _| {});
        let mut write_map = self.pipelines.write().unwrap();
        write_map.insert(key, pipeline.clone());
        pipeline
    }

    pub fn layout_manager(&self) -> &Arc<PipelineLayoutManager> {
        &self.layout_manager
    }
}
