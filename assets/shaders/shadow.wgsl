// ============================================================================
// Shadow Mapping Shader (Depth-Only) with Dynamic Offset
// ============================================================================

// -----------------------------------------------------------------------------
// Global Uniforms and Structures
// -----------------------------------------------------------------------------

// Global data uniform (for potential use in world-space reconstruction, etc.)
struct GlobalData {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,  // Inverse of the view-projection matrix.
    screen_size: vec2<f32>,
    time: f32,
};

// A generic light structure. For directional lights, we interpret 'position'
// as a normalized direction. However, to allow the shadow map to move with the
// camera, we use the directional light’s position as an offset.
struct Light {
    position: vec3<f32>,  // For directional lights, this is the light's position (set to camera pos).
    range: f32,           // For point/spot lights; ignored for directional lights.
    rotation: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
};

// Bindings in group(0)
@group(0) @binding(0)
var<uniform> global_data: GlobalData;

@group(0) @binding(1)
var<uniform> directional_light: Light;

@group(0) @binding(2)
var<storage, read> lights: array<Light>; // For additional lights if needed

// -----------------------------------------------------------------------------
// Push Constant: Light Transform Data
// -----------------------------------------------------------------------------
// We pass the light's world transform (model matrix) as a push constant.
// The shader computes the view matrix as the inverse of this matrix.
struct Transform {
    model: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
};


var<push_constant> uniforms: Transform;

// -----------------------------------------------------------------------------
// Vertex Shader: Compute Shadow Map Position
// -----------------------------------------------------------------------------
// Input vertex with position, normal, etc.
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.light_view_proj * uniforms.model * vec4<f32>(input.position, 1.0);
    return output;
}

// -----------------------------------------------------------------------------
// Fragment Shader: Depth-Only Output
// -----------------------------------------------------------------------------
// Outputs the computed depth value.
@fragment
fn fs_main(input: VertexOutput) -> @builtin(frag_depth) f32 {
    // Compute normalized depth.
    let depth = input.position.z / input.position.w;
    return depth;
}
