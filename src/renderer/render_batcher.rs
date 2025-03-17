use std::sync::Arc;
use shipyard::Unique;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::material::Material;
use crate::renderer::types::transform::Transform;

pub enum RenderCommand {
    Instanced {
        mesh: Arc<GpuMesh>,
        material: Arc<Material>,
        /// Multiple transforms for a given mesh/material pair.
        transforms: Vec<Transform>,
    },
    Single {
        mesh: Arc<GpuMesh>,
        material: Arc<Material>,
        transform: Transform,
    },
}

#[derive(Unique)]
pub struct RenderBatcher {
    pub commands: Vec<RenderCommand>,
}

impl RenderBatcher {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }

    /// Adds a render command. If the material supports instancing,
    /// group by mesh/material pair.
    pub fn add(&mut self, mesh: Arc<GpuMesh>, material: Arc<Material>, transform: Transform) {
        if material.is_instanced() {
            if let Some(RenderCommand::Instanced { mesh: m, material: mat, transforms, .. }) =
                self.commands.iter_mut().find(|cmd| match cmd {
                    RenderCommand::Instanced { mesh: m, material: mat, .. } => {
                        Arc::ptr_eq(m, &mesh) && Arc::ptr_eq(mat, &material)
                    }
                    _ => false,
                })
            {
                transforms.push(transform);
            } else {
                self.commands.push(RenderCommand::Instanced {
                    mesh,
                    material,
                    transforms: vec![transform],
                });
            }
        } else {
            self.commands.push(RenderCommand::Single {
                mesh,
                material,
                transform,
            });
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }
}