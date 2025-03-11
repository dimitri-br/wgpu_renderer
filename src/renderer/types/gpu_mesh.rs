use crate::renderer::types::mesh::Mesh;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::{Buffer, BufferUsages, Device};
use crate::renderer::types::vertex::Vertex;

/// A single submesh in GPU memory
pub struct GpuSubMesh {
    pub vertex_buffer: Arc<Buffer>,
    pub index_buffer: Option<Arc<Buffer>>,
    pub index_count: u32,
    pub vertex_count: u32,
}

/// A GPU mesh that contains multiple submeshes. Each submesh is drawn separately.
pub struct GpuMesh {
    pub submeshes: Vec<GpuSubMesh>,
}

impl GpuMesh {
    /// Builds a GpuMesh from a CPU-side Mesh that may contain multiple SubMesh objects.
    pub fn from_cpu_mesh(device: &Device, cpu_mesh: &Mesh) -> Self {
        let mut gpu_submeshes = Vec::with_capacity(cpu_mesh.submeshes.len());
        for subm in &cpu_mesh.submeshes {
            // Interleave or flatten your vertex data as the shader expects.
            // Example: position, normal, texcoord in each vertex
            let mut vertices = Vec::new();
            let count = subm.positions.len();
            for i in 0..count {
                let pos = subm.positions[i];
                let normal = if subm.normals.is_empty() {
                    [0.0, 0.0, 1.0]
                } else {
                    subm.normals[i]
                };
                let uv = if subm.texcoords.is_empty() {
                    [0.0, 0.0]
                } else {
                    subm.texcoords[i]
                };
                let vertex = Vertex::new(
                    pos.into(),
                    normal.into(),
                    uv.into(),
                );
                vertices.push(vertex);
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GpuSubMesh Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: BufferUsages::VERTEX,
            });

            let (index_buffer, index_count) = if subm.indices.is_empty() {
                (None, 0)
            } else {
                let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("GpuSubMesh Index Buffer"),
                    contents: bytemuck::cast_slice(&subm.indices),
                    usage: BufferUsages::INDEX,
                });
                (Some(Arc::new(ibuf)), subm.indices.len() as u32)
            };

            let gpu_submesh = GpuSubMesh {
                vertex_buffer: Arc::new(vertex_buffer),
                index_buffer,
                index_count,
                vertex_count: subm.positions.len() as u32,
            };
            gpu_submeshes.push(gpu_submesh);
        }

        Self {
            submeshes: gpu_submeshes,
        }
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        for subm in &self.submeshes {
            pass.set_vertex_buffer(0, subm.vertex_buffer.slice(..));
            if let Some(ibuf) = &subm.index_buffer {
                pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..subm.index_count, 0, 0..1);
            } else {
                pass.draw(0..subm.vertex_count, 0..1);
            }
        }
    }
}
