use log::error;
use naga::{ScalarKind, ShaderStage, StorageFormat, TypeInner, VectorSize, front::wgsl};
use std::hash::{Hash, Hasher};
use std::{collections::BTreeMap, num::NonZeroU32, sync::Arc};
use wgpu::{
    BindGroupLayout, BindingType, BufferBindingType, ColorTargetState, Device, PushConstantRange,
    RenderPipeline, ShaderStages, StorageTextureAccess, TextureFormat, VertexBufferLayout,
    VertexFormat,
};

/// Represents a single binding in a shader.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) struct Binding {
    pub name: Option<String>, // Optional name of the binding (e.g., "ubo", "sampler")
    pub group: u32,           // Bind group index
    pub binding: u32,         // Binding index within the group
    pub ty: BindingType,      // Type of the binding (e.g., Buffer, Texture, Sampler)
    pub binding_size: Option<u32>, // Size of the binding in bytes
    pub count: Option<NonZeroU32>, // Optional count for array bindings
}

/// Represents a shader with its bindings, layouts, entry points, and configurations.
#[derive(Debug, Clone)]
pub(crate) struct Shader {
    device: Arc<Device>,
    bindings: Vec<Binding>, // All bindings extracted from the shader
    bind_group_layouts: BTreeMap<u64, Arc<BindGroupLayout>>, // Bind group layouts indexed by group ID
    vertex_entry_point: Option<String>, // Entry point name for the vertex shader
    fragment_entry_point: Option<String>, // Entry point name for the fragment shader
    color_targets: Vec<Option<ColorTargetState>>, // Color target configurations for the fragment shader
    vertex_buffer_layouts: BTreeMap<u64, Vec<wgpu::VertexAttribute>>, // Vertex buffer layouts indexed by buffer ID
    push_constant_ranges: Vec<PushConstantRange>, // Push constant ranges for the shader
    shader_source: String,                        // Original WGSL shader source code
}

impl Shader {
    /// Creates a new `Shader` instance with the provided WGSL source code.
    ///
    /// # Arguments
    ///
    /// * `shader_source` - WGSL shader source code as a string.
    ///
    /// # Returns
    ///
    /// A new instance of `Shader`.
    pub fn new<T: Into<String>>(device: Arc<Device>, shader_source: T) -> Self {
        Self {
            device,
            bindings: Vec::new(),
            bind_group_layouts: BTreeMap::new(),
            vertex_entry_point: None,
            fragment_entry_point: None,
            color_targets: Vec::new(),
            vertex_buffer_layouts: BTreeMap::new(),
            push_constant_ranges: Vec::new(),
            shader_source: shader_source.into(),
        }
    }

    /// Analyzes the shader source to extract bindings, layouts, and configurations.
    ///
    /// This method performs the following steps:
    /// 1. Compiles the shader source into a WGPU shader module.
    /// 2. Parses the shader using Naga to extract the abstract syntax tree (AST).
    /// 3. Extracts entry points (vertex and fragment).
    /// 4. Extracts resource bindings (uniforms, samplers, textures).
    /// 5. Creates bind group layouts based on the extracted bindings.
    /// 6. Extracts color target configurations from the fragment shader.
    /// 7. Extracts vertex buffer layouts from the vertex shader.
    ///
    /// # Arguments
    ///
    /// * `device` - Reference to the WGPU device used to create bind group layouts.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or containing a `RendererError` on failure.
    pub fn analyze(&mut self) -> Result<(), &'static str> {
        // Step 1: Compile the shader source to ensure it's valid WGSL.
        self.compile(&self.device.clone())?;

        // Step 2: Parse the shader source using Naga to obtain the shader module (AST).
        let module =
            wgsl::parse_str(&self.shader_source).map_err(|_| "Failed to parse shader source")?;

        // Step 3: Extract entry points (vertex and fragment shader entry points).
        self.extract_entry_points(&module);

        // Step 4: Extract resource bindings (uniform buffers, textures, samplers).
        self.extract_bindings(&module)?;

        // Step 5: Create bind group layouts based on the extracted bindings.
        self.create_bind_group_layouts(&self.device.clone());

        // Step 6: Extract color target configurations from the fragment shader.
        self.color_targets = self.extract_color_targets(&module)?;

        // Step 7: Extract vertex buffer layouts from the vertex shader.
        self.vertex_buffer_layouts = self.extract_vertex_inputs(&module)?;

        // Step 8: Extract push constant ranges from the shader.
        self.push_constant_ranges = self.extract_push_constant_ranges(&module)?;

        Ok(())
    }

    /// Compiles the shader source into a WGPU shader module.
    ///
    /// # Arguments
    ///
    /// * `device` - Reference to the WGPU device used to create the shader module.
    ///
    /// # Returns
    ///
    /// A `Result` containing the compiled `renderer::ShaderModule` on success or a `RendererError` on failure.
    pub fn compile(&self, device: &Device) -> Result<wgpu::ShaderModule, &'static str> {
        Ok(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compiled Shader Module"),
            source: wgpu::ShaderSource::Wgsl(self.shader_source.clone().into()),
        }))
    }

    /// Retrieves a specific bind group layout by its group identifier.
    ///
    /// # Arguments
    ///
    /// * `group` - The bind group index.
    ///
    /// # Returns
    ///
    /// An optional reference to the `BindGroupLayout` if found.
    pub fn get_bind_group_layout(&self, group: u64) -> Option<&Arc<BindGroupLayout>> {
        self.bind_group_layouts.get(&group)
    }

    /// Retrieves all bind group layouts associated with the shader.
    ///
    /// # Returns
    ///
    /// A vector of `Arc<BindGroupLayout>` references.
    pub fn get_bind_group_layouts(&self) -> Vec<Arc<BindGroupLayout>> {
        self.bind_group_layouts.values().cloned().collect()
    }

    /// Retrieves the color target states extracted from the shader.
    ///
    /// These configurations define how the fragment shader outputs are blended and written to render targets.
    ///
    /// # Returns
    ///
    /// A vector of optional `ColorTargetState` configurations.
    pub fn get_color_targets(&self) -> Vec<Option<ColorTargetState>> {
        self.color_targets.clone()
    }

    /// Retrieves the vertex buffer layouts extracted from the shader.
    ///
    /// These layouts define how vertex data is structured and interpreted by the GPU.
    ///
    /// # Returns
    ///
    /// A vector of `VertexBufferLayout` configurations.
    pub fn get_vertex_buffer_layouts<'a>(&'a self) -> Vec<VertexBufferLayout<'a>> {
        self.vertex_buffer_layouts
            .values()
            .map(|attributes| VertexBufferLayout {
                array_stride: attributes.iter().map(|a| a.format.size()).sum(),
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes,
            })
            .collect()
    }

    /// Retrieves the push constant ranges extracted from the shader.
    ///
    /// These ranges define the size and stages of the push constants.
    ///
    /// # Returns
    ///
    /// A vector of `PushConstantRange` configurations.
    pub fn get_push_constant_ranges(&self) -> Vec<PushConstantRange> {
        self.push_constant_ranges.clone()
    }

    /// Retrieves a binding entry by its name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the binding to retrieve.
    ///
    /// # Returns
    ///
    /// An optional `Binding` if a binding with the specified name exists.
    pub fn get_binding_by_name(&self, name: &str) -> Option<Binding> {
        self.bindings
            .iter()
            .find(|b| b.name.as_deref() == Some(name))
            .cloned()
    }

    /// Retrieves all binding entries.
    ///
    /// # Returns
    ///
    /// A vector of all `Binding` entries extracted from the shader.
    pub fn get_bindings(&self) -> Vec<Binding> {
        self.bindings.clone()
    }

    /// Retrieves the vertex entry point of the shader.
    ///
    /// # Returns
    ///
    /// An optional `String` representing the vertex shader's entry point name.
    pub fn get_vertex_entry_point(&self) -> Option<String> {
        self.vertex_entry_point.clone()
    }

    /// Retrieves the fragment entry point of the shader.
    ///
    /// # Returns
    ///
    /// An optional `String` representing the fragment shader's entry point name.
    pub fn get_fragment_entry_point(&self) -> Option<String> {
        self.fragment_entry_point.clone()
    }

    /// Extracts the names of the vertex and fragment entry points from the shader module.
    ///
    /// This function scans through the shader's entry points and assigns the names
    /// to the respective fields in the `Shader` struct.
    ///
    /// # Arguments
    ///
    /// * `module` - Reference to the parsed Naga shader module.
    fn extract_entry_points(&mut self, module: &naga::Module) {
        for entry in &module.entry_points {
            match entry.stage {
                ShaderStage::Vertex => self.vertex_entry_point = Some(entry.name.clone()),
                ShaderStage::Fragment => self.fragment_entry_point = Some(entry.name.clone()),
                _ => (), // Ignore other shader stages (e.g., Compute)
            }
        }
    }

    /// Extracts resource bindings from the shader module.
    ///
    /// This function iterates through all global variables in the shader, identifies those
    /// with binding information, and constructs corresponding `Binding` structs.
    ///
    /// # Arguments
    ///
    /// * `module` - Reference to the parsed Naga shader module.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or containing a `RendererError` on failure.
    fn extract_bindings(&mut self, module: &naga::Module) -> Result<(), &'static str> {
        for (_, global) in module.global_variables.iter() {
            // Check if the global variable has binding information.
            if let Some(binding) = &global.binding {
                // Retrieve the type of the global variable.
                let ty = module
                    .types
                    .get_handle(global.ty)
                    .map_err(|_| "Invalid type handle")?;

                // Initialize array count to 1 by default.
                let mut array_count = None;

                // Determine the binding type based on the address space and type information.
                let binding_type = match global.space {
                    naga::AddressSpace::Uniform => BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    naga::AddressSpace::Storage { access } => BindingType::Buffer {
                        ty: BufferBindingType::Storage {
                            read_only: !access.contains(naga::StorageAccess::STORE),
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    // For other address spaces (e.g., PushConstant), map accordingly.
                    _ => self.map_binding_type(module, ty, &mut array_count)?,
                };

                let size = self.map_binding_to_size(module, ty);

                // Create and store the binding information.
                self.bindings.push(Binding {
                    name: global.name.clone(),
                    binding: binding.binding,
                    group: binding.group,
                    ty: binding_type,
                    binding_size: size,
                    count: array_count,
                });
            }
        }
        Ok(())
    }

    /// Creates bind group layouts based on the extracted bindings.
    ///
    /// This function groups bindings by their bind group index and creates corresponding
    /// `BindGroupLayout` instances using the WGPU device.
    ///
    /// # Arguments
    ///
    /// * `device` - Reference to the WGPU device used to create bind group layouts.
    fn create_bind_group_layouts(&mut self, device: &Device) {
        // Temporary map to collect bind group entries grouped by their group ID.
        let mut layouts_map: BTreeMap<u32, Vec<wgpu::BindGroupLayoutEntry>> = BTreeMap::new();

        // Iterate through all bindings and organize them by their bind group index.
        for binding in &self.bindings {
            layouts_map
                .entry(binding.group)
                .or_default()
                .push(wgpu::BindGroupLayoutEntry {
                    binding: binding.binding,
                    visibility: ShaderStages::all(), // Make the binding visible to all shader stages.
                    ty: binding.ty.clone(),
                    count: binding.count,
                });
        }

        // Check if we have each group from 0 - n, where n is the largest group index.
        // If we're missing any, we need to insert empty layouts to avoid bind group layout creation errors.
        let max_group = layouts_map.keys().max().cloned().unwrap_or(0);
        for group in 0..=max_group {
            layouts_map.entry(group).or_default();
        }

        // Create a bind group layout for each group and store it in the layouts map.
        for (group, entries) in layouts_map {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(&format!("Bind Group Layout {}", group)),
                entries: &entries,
            });
            self.bind_group_layouts
                .insert(group as u64, Arc::new(layout));
        }
    }

    /// Extracts color outputs from the fragment shader stage.
    ///
    /// This function identifies how the fragment shader writes to color targets, including
    /// blend modes and write masks.
    ///
    /// # Arguments
    ///
    /// * `module` - Reference to the parsed Naga shader module.
    ///
    /// # Returns
    ///
    /// A `Result` containing a vector of optional `ColorTargetState` configurations on success
    /// or a `RendererError` on failure.
    fn extract_color_targets(
        &self,
        module: &naga::Module,
    ) -> Result<Vec<Option<ColorTargetState>>, &'static str> {
        // Find the fragment shader entry point.
        let fragment = module
            .entry_points
            .iter()
            .find(|e| e.stage == ShaderStage::Fragment);
        let mut color_targets = Vec::new();

        if let Some(fragment) = fragment {
            // Check if the fragment shader has a result type (i.e., color outputs).
            if let Some(output) = &fragment.function.result {
                let ty = &module.types[output.ty];
                let name = ty.name.clone().unwrap_or("bgra8unorm".to_string());
                match &ty.inner {
                    // Handle single vector outputs (e.g., vec4<f32>).
                    TypeInner::Vector { size, scalar, .. } => {
                        let format = self.map_vector_to_texture_format(&name, *size, scalar)?;
                        color_targets.push(Some(ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::REPLACE), // Default blend state.
                            write_mask: wgpu::ColorWrites::ALL, // Enable writing to all color channels.
                        }));
                    }
                    // Handle struct outputs (e.g., multiple color outputs).
                    TypeInner::Struct { members, .. } => {
                        for member in members {
                            let ty =  &module.types[member.ty];
                            let name = member.name.clone().unwrap_or("bgra8unorm".to_string());
                            if let TypeInner::Vector { size, scalar, .. } =
                                &ty.inner
                            {
                                let format = self.map_vector_to_texture_format(&name, *size, scalar)?;
                                color_targets.push(Some(ColorTargetState {
                                    format,
                                    blend: Some(wgpu::BlendState::REPLACE),
                                    write_mask: wgpu::ColorWrites::ALL,
                                }));
                            }
                        }
                    }
                    // Return an error for unsupported fragment shader output types.
                    _ => {
                        error!("Unsupported fragment shader output type: {:?}", output.ty);
                    }
                }
            }
        }

        Ok(color_targets)
    }

    /// Extracts vertex inputs from the vertex shader stage.
    ///
    /// This function identifies the structure of vertex inputs, including attribute formats and locations.
    ///
    /// # Arguments
    ///
    /// * `module` - Reference to the parsed Naga shader module.
    ///
    /// # Returns
    ///
    /// A `Result` containing a map of buffer indices to their corresponding vertex attributes on success
    /// or a `RendererError` on failure.
    fn extract_vertex_inputs(
        &self,
        module: &naga::Module,
    ) -> Result<BTreeMap<u64, Vec<wgpu::VertexAttribute>>, &'static str> {
        // Find the vertex shader entry point.
        let vertex = module
            .entry_points
            .iter()
            .find(|e| e.stage == ShaderStage::Vertex);
        let mut vertex_inputs = BTreeMap::new();

        if let Some(vertex) = vertex {
            let mut attributes = Vec::new();
            let mut offset = 0;
            let mut location = 0;

            for param in &vertex.function.arguments {
                // Skip built-in attributes (e.g., position, normal).
                if matches!(param.binding, Some(naga::Binding::BuiltIn(_))) {
                    continue;
                }

                // Retrieve the type of the vertex input parameter.
                let ty = &module.types[param.ty];
                match &ty.inner {
                    // Handle vector types (e.g., vec3<f32>).
                    TypeInner::Vector { size, scalar, .. } => {
                        let format = self.map_vector_to_vertex_format(*size, scalar)?;
                        attributes.push(wgpu::VertexAttribute {
                            format,
                            offset,
                            shader_location: location,
                        });
                        offset += format.size();
                        location += 1;
                    }
                    // Handle struct types (e.g., multiple attributes in a struct).
                    TypeInner::Struct { members, .. } => {
                        for member in members {
                            if let TypeInner::Vector { size, scalar, .. } =
                                &module.types[member.ty].inner
                            {
                                let format = self.map_vector_to_vertex_format(*size, scalar)?;
                                attributes.push(wgpu::VertexAttribute {
                                    format,
                                    offset,
                                    shader_location: location,
                                });
                                offset += format.size();
                                location += 1;
                            }
                        }
                    }
                    // Handle scalar types (e.g., single float).
                    TypeInner::Scalar(scalar) => {
                        let size = match scalar.width {
                            2 => VectorSize::Bi,
                            3 => VectorSize::Tri,
                            4 => VectorSize::Quad,
                            _ => return Err("Unsupported scalar width"),
                        };
                        let format = self.map_vector_to_vertex_format(size, scalar)?;
                        attributes.push(wgpu::VertexAttribute {
                            format,
                            offset,
                            shader_location: location,
                        });
                        offset += format.size();
                        location += 1;
                    }
                    // Return an error for unsupported vertex shader input types.
                    _ => {
                        error!("Unsupported vertex shader input type: {:?}", ty.inner);
                        return Err("Unsupported vertex shader input type");
                    }
                }
            }

            if !attributes.is_empty() {
                // Assign the collected attributes to buffer index 0.
                vertex_inputs.insert(0, attributes);
            }
        }

        Ok(vertex_inputs)
    }

    fn extract_push_constant_ranges(
        &self,
        module: &naga::Module,
    ) -> Result<Vec<PushConstantRange>, &'static str> {
        let mut push_constant_ranges = Vec::new();
        let mut current_offset = 0;
        for (_, global) in module.global_variables.iter() {
            if global.space == naga::AddressSpace::PushConstant {
                let ty = module
                    .types
                    .get_handle(global.ty)
                    .map_err(|_| "Invalid type handle")?;
                // We need to figure out the range of the push constant
                // This is a bit tricky because we need to know the size of the type
                let size = self.map_binding_to_size(module, ty);
                if let Some(size) = size {
                    let push_constant_range = PushConstantRange {
                        stages: ShaderStages::VERTEX_FRAGMENT,
                        range: current_offset..current_offset + size,
                    };
                    push_constant_ranges.push(push_constant_range);
                    current_offset += size;
                } else {
                    error!("Failed to determine size of push constant range");
                    return Err("Failed to determine size of push constant range");
                }
            }
        }
        Ok(push_constant_ranges)
    }

    /// Maps a Naga type to a WGPU binding type.
    ///
    /// This function translates Naga's type representations to WGPU's `BindingType`,
    /// handling various resource types such as textures, samplers, and storage buffers.
    ///
    /// # Arguments
    ///
    /// * `module` - Reference to the parsed Naga shader module.
    /// * `binding_type` - Reference to the Naga type representing the binding.
    /// * `array_count` - Mutable reference to track array sizes for bindings.
    ///
    /// # Returns
    ///
    /// A `Result` containing the corresponding `BindingType` on success or a `RendererError` on failure.
    fn map_binding_type(
        &self,
        module: &naga::Module,
        binding_type: &naga::Type,
        array_count: &mut Option<NonZeroU32>,
    ) -> Result<BindingType, &'static str> {
        match &binding_type.inner {
            // Handle image bindings (textures).
            TypeInner::Image {
                dim,
                arrayed,
                class,
            } => {
                let view_dimension = match dim {
                    naga::ImageDimension::D1 => wgpu::TextureViewDimension::D1,
                    naga::ImageDimension::D2 => {
                        if *arrayed {
                            wgpu::TextureViewDimension::D2Array
                        } else {
                            wgpu::TextureViewDimension::D2
                        }
                    }
                    naga::ImageDimension::D3 => wgpu::TextureViewDimension::D3,
                    naga::ImageDimension::Cube => {
                        if *arrayed {
                            wgpu::TextureViewDimension::CubeArray
                        } else {
                            wgpu::TextureViewDimension::Cube
                        }
                    }
                };

                // Determine the binding type based on the image class (Sampled, Depth, Storage).
                let binding_type = match class {
                    naga::ImageClass::Sampled { kind, multi } => {
                        let sample_type = match kind {
                            ScalarKind::Float => {
                                wgpu::TextureSampleType::Float { filterable: true }
                            }
                            ScalarKind::Sint => wgpu::TextureSampleType::Sint,
                            ScalarKind::Uint => wgpu::TextureSampleType::Uint,
                            _ => return Err("Unsupported texture sample type"),
                        };
                        BindingType::Texture {
                            sample_type,
                            view_dimension,
                            multisampled: *multi,
                        }
                    }
                    naga::ImageClass::Depth { .. } => BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension,
                        multisampled: false,
                    },
                    naga::ImageClass::Storage { format, access } => {
                        let access = if access.contains(naga::StorageAccess::STORE) {
                            StorageTextureAccess::WriteOnly
                        } else if access.contains(naga::StorageAccess::LOAD) {
                            StorageTextureAccess::ReadOnly
                        } else {
                            StorageTextureAccess::ReadWrite
                        };

                        let format = match format {
                            StorageFormat::R8Unorm => TextureFormat::R8Unorm,
                            // Add additional storage format mappings as needed.
                            StorageFormat::Bgra8Unorm => TextureFormat::Bgra8UnormSrgb,
                            _ => return Err("Unsupported storage format: {:?}"),
                        };

                        BindingType::StorageTexture {
                            access,
                            format,
                            view_dimension,
                        }
                    }
                };
                Ok(binding_type)
            }
            // Handle sampler bindings.
            TypeInner::Sampler { comparison } => {
                let sampler_type = if *comparison {
                    wgpu::SamplerBindingType::Comparison
                } else {
                    wgpu::SamplerBindingType::Filtering
                };
                Ok(BindingType::Sampler(sampler_type))
            }
            // Handle array types by recursively mapping the base type and updating the array count.
            TypeInner::Array { base, size, .. } => {
                if let naga::ArraySize::Constant(size) = size {
                    *array_count = Some(*size);
                } else {
                    *array_count = None;
                }
                self.map_binding_type(module, &module.types[*base], array_count)
            }
            // Handle vector bindings as uniform buffers.
            TypeInner::Vector { .. } => Ok(BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            }),
            // Handle matrix bindings as uniform buffers.
            TypeInner::Matrix { .. } => Ok(BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            }),
            // Return an error for unsupported binding types.
            _ => {
                error!("Unsupported binding type: {:?}", binding_type.inner);
                Err("Unsupported binding type")
            }
        }
    }

    fn map_binding_to_size(&self, module: &naga::Module, binding_type: &naga::Type) -> Option<u32> {
        match &binding_type.inner {
            TypeInner::Scalar(scalar) => match scalar.width {
                2 => Some(2),
                3 => Some(3),
                4 => Some(4),
                _ => {
                    error!("Unsupported scalar width: {:?}", scalar.width);
                    None
                }
            },
            TypeInner::Vector { size, .. } => match size {
                VectorSize::Bi => Some(2),
                VectorSize::Tri => Some(3),
                VectorSize::Quad => Some(4),
            },
            TypeInner::Matrix {
                columns,
                rows,
                scalar,
            } => {
                let scalar_width = scalar.width;
                let columns = match columns {
                    VectorSize::Bi => 2,
                    VectorSize::Tri => 3,
                    VectorSize::Quad => 4,
                };
                let rows = match rows {
                    VectorSize::Bi => 2,
                    VectorSize::Tri => 3,
                    VectorSize::Quad => 4,
                };

                Some((scalar_width * columns * rows) as u32)
            }
            TypeInner::Struct { members, .. } => {
                let mut size = 0;
                for member in members {
                    let member_ty = module
                        .types
                        .get_handle(member.ty)
                        .expect("Invalid type handle");
                    let member_size = self.map_binding_to_size(module, member_ty)?;
                    size += member_size;
                }
                Some(size)
            }
            TypeInner::Array { base, size, stride } => {
                let base_ty = module.types.get_handle(*base).expect("Invalid type handle");
                let base_size = self.map_binding_to_size(module, base_ty)?;
                let size = match size {
                    naga::ArraySize::Constant(size) => *size,
                    _ => {
                        //error!("Unsupported array size");
                        return None;
                    }
                };
                Some(base_size * size.get())
            }
            TypeInner::Image { .. } => Some(1),
            TypeInner::Sampler { .. } => Some(1),
            _ => {
                error!("Unsupported binding type: {:?}", binding_type.inner);
                None
            }
        }
    }

    /// Maps a Naga vector type to a WGPU texture format.
    ///
    /// This function translates Naga's vector types to appropriate WGPU `TextureFormat` values.
    ///
    /// # Arguments
    ///
    /// * `size` - The size of the vector (e.g., Bi, Tri, Quad).
    /// * `scalar` - Reference to the scalar kind (e.g., Float, Sint).
    ///
    /// # Returns
    ///
    /// A `Result` containing the corresponding `TextureFormat` on success or a `RendererError` on failure.
    fn map_vector_to_texture_format(
        &self,
        name: &str,
        size: VectorSize,
        scalar: &naga::Scalar,
    ) -> Result<TextureFormat, &'static str> {
        match (size, scalar.kind) {
            // Example mapping: a float vector maps to a specific texture format.
            (VectorSize::Bi | VectorSize::Tri | VectorSize::Quad, ScalarKind::Float) => {
                // We check for as string within the name to determine the format
                // For example xxx_r8unorm would be a R8Unorm format
                // or r8unorm_xxx would also be a R8Unorm format
                let format = match name {
                    name if name.contains("r8unorm") => TextureFormat::R8Unorm,
                    name if name.contains("r8snorm") => TextureFormat::R8Snorm,
                    name if name.contains("r8uint") => TextureFormat::R8Uint,
                    name if name.contains("r8sint") => TextureFormat::R8Sint,
                    name if name.contains("r16uint") => TextureFormat::R16Uint,
                    name if name.contains("r16sint") => TextureFormat::R16Sint,
                    name if name.contains("r16float") => TextureFormat::R16Float,
                    name if name.contains("rg8unorm") => TextureFormat::Rg8Unorm,
                    name if name.contains("rg8snorm") => TextureFormat::Rg8Snorm,
                    name if name.contains("rg8uint") => TextureFormat::Rg8Uint,
                    name if name.contains("rg8sint") => TextureFormat::Rg8Sint,
                    name if name.contains("r32uint") => TextureFormat::R32Uint,
                    name if name.contains("r32sint") => TextureFormat::R32Sint,
                    name if name.contains("r32float") => TextureFormat::R32Float,
                    name if name.contains("rg16uint") => TextureFormat::Rg16Uint,
                    name if name.contains("rg16sint") => TextureFormat::Rg16Sint,
                    name if name.contains("rg16float") => TextureFormat::Rg16Float,
                    name if name.contains("rgba8unorm_srgb") => TextureFormat::Rgba8UnormSrgb,
                    name if name.contains("rgba8unorm") => TextureFormat::Rgba8Unorm,
                    name if name.contains("rgba8snorm") => TextureFormat::Rgba8Snorm,
                    name if name.contains("rgba8uint") => TextureFormat::Rgba8Uint,
                    name if name.contains("rgba8sint") => TextureFormat::Rgba8Sint,
                    name if name.contains("bgra8unorm_srgb") => TextureFormat::Bgra8UnormSrgb,
                    name if name.contains("bgra8unorm") => TextureFormat::Bgra8Unorm,
                    name if name.contains("rgb10a2unorm") => TextureFormat::Rgb10a2Unorm,
                    name if name.contains("rg32uint") => TextureFormat::Rg32Uint,
                    name if name.contains("rg32sint") => TextureFormat::Rg32Sint,
                    name if name.contains("rg32float") => TextureFormat::Rg32Float,
                    name if name.contains("rgba16uint") => TextureFormat::Rgba16Uint,
                    name if name.contains("rgba16sint") => TextureFormat::Rgba16Sint,
                    name if name.contains("rgba16float") => TextureFormat::Rgba16Float,
                    name if name.contains("rgba32uint") => TextureFormat::Rgba32Uint,
                    name if name.contains("rgba32sint") => TextureFormat::Rgba32Sint,
                    name if name.contains("rgba32float") => TextureFormat::Rgba32Float,
                    _ => {
                        error!("Unsupported texture format: {:?}", name);
                        return Err("Unsupported texture format");
                    }
                };
                Ok(format)
            }
            // Extend this mapping based on your shader's output requirements.
            _ => {
                error!("Unsupported texture format: {:?}, {:?}", size, scalar.kind);
                Err("Unsupported texture format")
            }
        }
    }

    /// Maps a Naga vector type to a WGPU vertex format.
    ///
    /// This function translates Naga's vector types to appropriate WGPU `VertexFormat` values.
    ///
    /// # Arguments
    ///
    /// * `size` - The size of the vector (e.g., Bi, Tri, Quad).
    /// * `scalar` - Reference to the scalar kind (e.g., Float, Sint).
    ///
    /// # Returns
    ///
    /// A `Result` containing the corresponding `VertexFormat` on success or a `RendererError` on failure.
    fn map_vector_to_vertex_format(
        &self,
        size: VectorSize,
        scalar: &naga::Scalar,
    ) -> Result<VertexFormat, &'static str> {
        match (size, scalar.kind) {
            (VectorSize::Bi, ScalarKind::Float) => Ok(VertexFormat::Float32x2),
            (VectorSize::Tri, ScalarKind::Float) => Ok(VertexFormat::Float32x3),
            (VectorSize::Quad, ScalarKind::Float) => Ok(VertexFormat::Float32x4),
            (VectorSize::Bi, ScalarKind::Sint) => Ok(VertexFormat::Sint32x2),
            (VectorSize::Tri, ScalarKind::Sint) => Ok(VertexFormat::Sint32x3),
            (VectorSize::Quad, ScalarKind::Sint) => Ok(VertexFormat::Sint32x4),
            (VectorSize::Bi, ScalarKind::Uint) => Ok(VertexFormat::Uint32x2),
            (VectorSize::Tri, ScalarKind::Uint) => Ok(VertexFormat::Uint32x3),
            (VectorSize::Quad, ScalarKind::Uint) => Ok(VertexFormat::Uint32x4),
            // Extend this mapping based on your vertex input requirements.
            _ => {
                error!("Unsupported vertex format: {:?}, {:?}", size, scalar.kind);
                Err("Unsupported vertex format")
            }
        }
    }

    pub fn hash_to_string(&self) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

impl std::hash::Hash for Shader {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.shader_source.hash(state);
    }
}

impl std::fmt::Display for Shader {
    /// Formats the shader information for display.
    ///
    /// This implementation provides a detailed overview of the shader's bindings,
    /// bind group layouts, entry points, vertex buffer layouts, and color targets.
    ///
    /// # Arguments
    ///
    /// * `f` - The formatter.
    ///
    /// # Returns
    ///
    /// A `std::fmt::Result` indicating success or failure.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display all bindings.
        writeln!(f, "Bindings:")?;
        for binding in &self.bindings {
            writeln!(
                f,
                "  Name: {:?}, Group: {}, Binding: {}, Type: {:?}, Count: {:?}",
                binding.name, binding.group, binding.binding, binding.ty, binding.count
            )?;
        }

        // Display all bind group layouts.
        writeln!(f, "\nBind Group Layouts:")?;
        for (group, layout) in &self.bind_group_layouts {
            writeln!(f, "  Group: {}, Layout: {:?}", group, layout)?;
        }

        // Display vertex and fragment entry points.
        writeln!(f, "\nVertex Entry Point: {:?}", self.vertex_entry_point)?;
        writeln!(f, "Fragment Entry Point: {:?}", self.fragment_entry_point)?;

        // Display vertex buffer layouts.
        writeln!(f, "\nVertex Buffer Layouts:")?;
        for (i, layout) in self.vertex_buffer_layouts.iter().enumerate() {
            writeln!(f, "  Layout {}: {:?}", i, layout)?;
        }

        // Display color target configurations.
        writeln!(f, "\nColor Targets:")?;
        for (i, target) in self.color_targets.iter().enumerate() {
            writeln!(f, "  Target {}: {:?}", i, target)?;
        }

        Ok(())
    }
}

impl Into<RenderPipeline> for Shader {
    fn into(self) -> RenderPipeline {
        let bind_groups = self.get_bind_group_layouts();
        let bind_groups: Vec<&BindGroupLayout> = bind_groups.iter().map(|b| b.as_ref()).collect();
        let vertex_buffers = self.get_vertex_buffer_layouts();
        let color_targets = self.get_color_targets();
        let vertex_entry_point = self.get_vertex_entry_point().unwrap();
        let fragment_entry_point = self.get_fragment_entry_point().unwrap();
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: &bind_groups,
                push_constant_ranges: &self.get_push_constant_ranges(),
            });

        let compiled_shader_module = self.compile(&self.device).unwrap();

        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &compiled_shader_module,
                    entry_point: Some(&vertex_entry_point),
                    compilation_options: Default::default(),
                    buffers: &vertex_buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &compiled_shader_module,
                    entry_point: Some(&fragment_entry_point),
                    compilation_options: Default::default(),
                    targets: &color_targets,
                }),
                multiview: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: Default::default(),
                cache: None,
            })
    }
}
