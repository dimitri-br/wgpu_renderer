// ============================================================================
// Shadow Mapping Shader (Depth-Only) with Dynamic Offset
// ============================================================================

// -----------------------------------------------------------------------------
// Global Uniforms and Structures
// -----------------------------------------------------------------------------

struct GlobalData {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    time: f32,
};

struct Light {
    position: vec3<f32>,  // For directional lights, typically not used for attenuation.
    range: f32,
    rotation: vec3<f32>,  // Here, assumed to encode the light's direction (in radians).
    intensity: f32,
    color: vec3<f32>,
    light_type: u32,  // 0 = directional, 1 = point, 2 = spot
    view_proj: mat4x4<f32>,  // Precomputed shadow pass matrix.
};


struct ShadowData {
    light_view_proj: mat4x4<f32>, // Precomputed shadow pass matrix.
    uv_offset: vec2<f32>,         // If using an atlas, set to tile offset.
    uv_scale: vec2<f32>,          // If using an atlas, set to tile scale.
    bias: f32,                    // Depth bias.
};


@group(0) @binding(0)
var<uniform> global_data: GlobalData;

@group(0) @binding(1)
var<storage, read> lights: array<Light>; // lights.

@group(0) @binding(2)
var<storage, read> shadow_data: array<ShadowData>;

// -----------------------------------------------------------------------------
// Push Constant: Light Transform Data
// -----------------------------------------------------------------------------
// We pass the light's world transform (model matrix) as a push constant.
// The shader computes the view matrix as the inverse of this matrix.
struct Transform {
    model: mat4x4<f32>,
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

    // First, loop through lights to see if we have a directional light.
    // If we do, we'll use it for the main directional light.
    let num_lights: u32 = arrayLength(&lights);
    var found_directional: bool = false;
    var directional_index: u32 = 0u;
    for (var i: u32 = 0u; i < num_lights; i = i + 1u) {
        let light = lights[i];
        if (light.light_type == 0u) {
            directional_index = i;
            found_directional = true;
            break;
        }
    }

    // 5) Directional light (no shadow yet).
    // Check if we have a directional light.
    if (found_directional) {
        var directional_light: Light = lights[directional_index];
        var directional_shadow_data: ShadowData = shadow_data[directional_index];

        output.position = directional_light.view_proj * uniforms.model * vec4<f32>(input.position, 1.0);
    } else {
        output.position = vec4<f32>(input.position, 1.0);
    }
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
