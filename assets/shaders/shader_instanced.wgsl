struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) out_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) normal: vec3<f32>,
};

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
    shadow_index: u32,  // Index into shadow_data array.
    shadow_count: u32,  // Number of shadow maps to use.
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

// Push constant (unchanged); still used
struct Transform {
    model: mat4x4<f32>,
    inverse_transpose_model: mat4x4<f32>,
};

// Same push-constant usage
var<push_constant> uniforms: Transform;

@group(2) @binding(0)
var<storage, read> instances: array<Transform>;

fn is_identity(m: mat4x4<f32>) -> bool {
    let epsilon: f32 = 0.0001;
    return  abs(m[0][0] - 1.0) < epsilon &&
            abs(m[0][1] - 0.0) < epsilon &&
            abs(m[0][2] - 0.0) < epsilon &&
            abs(m[0][3] - 0.0) < epsilon &&

            abs(m[1][0] - 0.0) < epsilon &&
            abs(m[1][1] - 1.0) < epsilon &&
            abs(m[1][2] - 0.0) < epsilon &&
            abs(m[1][3] - 0.0) < epsilon &&

            abs(m[2][0] - 0.0) < epsilon &&
            abs(m[2][1] - 0.0) < epsilon &&
            abs(m[2][2] - 1.0) < epsilon &&
            abs(m[2][3] - 0.0) < epsilon &&

            abs(m[3][0] - 0.0) < epsilon &&
            abs(m[3][1] - 0.0) < epsilon &&
            abs(m[3][2] - 0.0) < epsilon &&
            abs(m[3][3] - 1.0) < epsilon;
}


@vertex
fn gb_vs_main(input: VertexInput, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    var output: VertexOutput;

    var instance: Transform;

    // Check if the push constant is the identity matrix
    if (!is_identity(uniforms.model)) {
        instance = uniforms;
    }else{
        instance = instances[instance_index];
    }


    // Compute the world-space position from the model matrix
    let world_pos: vec4<f32> = (instance.model * vec4<f32>(input.position, 1.0));

    // Convert from world-space to clip-space using the view-projection matrix
    output.out_position = global_data.view_proj * world_pos;

    // Pass through the texture coordinates directly
    output.tex_coords = input.tex_coords;

    // Transform and normalize the normals using the model matrix
    // (This assumes uniform scaling; otherwise you'd need an inverse-transpose.)
    output.normal = normalize((instance.inverse_transpose_model * vec4<f32>(input.normals, 0.0)).xyz);

    return output;
}


struct FragmentInput {
    @location(0) tex_coords: vec2<f32>,
    @location(1) normal: vec3<f32>,
};

struct FragmentOutput {
    @location(0) color_rgba16float: vec4<f32>,
    @location(1) normal_rg16snorm: vec4<f32>,
};

@group(1) @binding(0)
var color_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_sampler: sampler;

@fragment
fn gb_fs_main(input: FragmentInput) -> FragmentOutput {
    var output: FragmentOutput;

    // Sample the diffuse/albedo color from a texture
    output.color_rgba16float = textureSample(color_texture, color_sampler, input.tex_coords);

    // Encode the normal from [-1,1] into [0,1]
    // We assume 'input.normal' is already normalized in the vertex shader.
    output.normal_rg16snorm = vec4<f32>(input.normal * 0.5 + vec3<f32>(0.5), 1.0);

    return output;
}
