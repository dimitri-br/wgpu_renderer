use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// A single submesh containing positions, normals, texcoords, and indices.
#[derive(Debug)]
pub struct SubMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// A mesh which may contain multiple submeshes if the OBJ file has multiple groups/objects.
#[derive(Debug)]
pub struct Mesh {
    pub submeshes: Vec<SubMesh>,
}

impl Mesh {
    /// Loads an OBJ file using `tobj` and returns a `Mesh` with one `SubMesh` per model/group.
    ///
    /// If you wish to unify them all into one submesh, you could do so by merging.
    /// This function returns an error if the file cannot be loaded or parsed.
    pub fn load_obj<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(File::open(path)?);
        let (models, materials) = tobj::load_obj_buf(
            &mut reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true, // unify indices across positions/normals/UV
                ..Default::default()
            },
            |p| {
                if let Ok(file) = File::open(p) {
                    return tobj::load_mtl_buf(&mut BufReader::new(file));
                }

                tobj::MTLLoadResult::Err(tobj::LoadError::GenericFailure)
            },
        )?;

        if models.is_empty() {
            return Err("No meshes found in OBJ file".into());
        }

        let mut submeshes = Vec::with_capacity(models.len());
        for model in models {
            let mesh_data = model.mesh;

            // Build the positions
            let mut positions = Vec::with_capacity(mesh_data.positions.len() / 3);
            for chunk in mesh_data.positions.chunks_exact(3) {
                positions.push([chunk[0], chunk[1], chunk[2]]);
            }

            // Build the normals (may be empty if not in file)
            let mut normals = Vec::new();
            if !mesh_data.normals.is_empty() {
                normals.reserve_exact(mesh_data.normals.len() / 3);
                for chunk in mesh_data.normals.chunks_exact(3) {
                    normals.push([chunk[0], chunk[1], chunk[2]]);
                }
            }

            // Build the texcoords (may be empty if not in file)
            let mut texcoords = Vec::new();
            if !mesh_data.texcoords.is_empty() {
                texcoords.reserve_exact(mesh_data.texcoords.len() / 2);
                for chunk in mesh_data.texcoords.chunks_exact(2) {
                    texcoords.push([chunk[0], chunk[1]]);
                }
            }

            // The indices
            let indices = mesh_data.indices.clone();

            submeshes.push(SubMesh {
                positions,
                normals,
                texcoords,
                indices,
            });
        }

        Ok(Mesh { submeshes })
    }
}
