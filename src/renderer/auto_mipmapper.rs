use std::sync::Arc;
use shipyard::Unique;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoder, Device, LoadOp, PipelineLayoutDescriptor, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, StoreOp, TextureFormat, TextureViewDescriptor};
use wgpu::util::DeviceExt;
use crate::renderer::types::texture::Texture;

/// AutoMipmapper creates a render pipeline that blits one mip level into the next.
/// It also creates a linear sampler for sampling the source mip level.
#[derive(Unique)]
pub struct AutoMipmapper {
    device: Arc<Device>,
    pipeline: RenderPipeline,
    sampler: Sampler,
}

impl AutoMipmapper {
    /// Creates a new AutoMipmapper.
    ///
    /// * `device` - An Arc to your wgpu::Device.
    /// * `format` - The texture format for the mip levels (for the render target).
    ///
    /// The pipeline is set up with a simple full-screen triangle that samples from
    /// a bound texture and writes its output.
    pub fn new(device: Arc<Device>, format: TextureFormat) -> Self {
        // Create a linear sampler for mipmapping.
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Mipmap Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // WGSL shader source for mipmapping.
        // The vertex shader generates a full-screen triangle.
        // The fragment shader simply samples the source texture.
        let shader_source = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // A full-screen triangle that covers the entire viewport.
    // The positions are defined in clip space.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );
    let pos = positions[vertex_index];
    var output: VertexOutput;
    // Flip the y-coordinate to transform into DirectX clip space.
    output.position = vec4<f32>(pos.x, -pos.y, 0.0, 1.0);
    // Convert from clip space [-1, 1] to texture UV [0, 1]
    output.uv = (pos + vec2<f32>(1.0)) * 0.5;
    return output;
}

@group(0) @binding(0)
var src_tex: texture_2d<f32>;
@group(0) @binding(1)
var src_sampler: sampler;

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    // Sample the source texture.
    let srgb_color = textureSample(src_tex, src_sampler, uv).rgb;
    // Convert from sRGB to linear space.
    let linear_color = pow(srgb_color, vec3<f32>(2.2));
    // For mipmapping, the bilinear filtering occurs in linear space.
    // Convert back to sRGB.
    let final_color = pow(linear_color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(final_color, 1.0);
}
        "#;

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Mipmap Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create a bind group layout that expects a texture and a sampler.
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Mipmap Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create a pipeline layout.
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Mipmap Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create the render pipeline.
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Mipmap Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            device,
            pipeline,
            sampler,
        }
    }

    /// Generates mipmaps for each texture in the provided slice.
    ///
    /// Each texture must have been created with a mip level count > 1.
    /// Note: Since wgpu does not expose the texture descriptor after creation,
    /// you need to store the mip level count separately.
    ///
    /// * `encoder` - A mutable reference to a wgpu::CommandEncoder.
    /// * `textures` - A slice of texture references to generate mipmaps for.
    /// * `mip_levels` - A slice of u32 representing the number of mip levels for each texture.
    pub fn generate_mipmaps(
        &self,
        encoder: &mut CommandEncoder,
        textures: &[Arc<Texture>],
        mip_levels: &[u32],
    ) {
        assert_eq!(textures.len(), mip_levels.len(), "Each texture must have a corresponding mip level count");

        for (texture, &mip_count) in textures.iter().zip(mip_levels.iter()) {
            // For each mip level starting at 1, render a full-screen pass that samples from level (i - 1).
            for level in 1..mip_count {
                let src_view = texture.create_view(&TextureViewDescriptor {
                    label: Some("Mipmap Source View"),
                    base_mip_level: level - 1,
                    mip_level_count: Some(1),
                    ..Default::default()
                });
                let dst_view = texture.create_view(&TextureViewDescriptor {
                    label: Some("Mipmap Destination View"),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                });

                {
                    let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                        label: Some("Mipmap Generation Render Pass"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &dst_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: LoadOp::Clear(Color::BLACK),
                                store: StoreOp::Store
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    rpass.set_pipeline(&self.pipeline);
                    // Create a bind group for this pass.
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Mipmap Bind Group"),
                        layout: &self.pipeline.get_bind_group_layout(0),
                        entries: &[
                            BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&src_view),
                            },
                            BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                        ],
                    });
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.draw(0..3, 0..1);
                }
            }
        }
    }
}
