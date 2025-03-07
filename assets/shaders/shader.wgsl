struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) out_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
};

struct GlobalData {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
};

struct Light {
    position: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> global_data: GlobalData;

@group(0) @binding(1)
var<uniform> lights: array<Light>;

// Push constant (unchanged); still used
struct Transform {
    model: mat4x4<f32>,
    inverse_transpose_model: mat4x4<f32>,
};

// Same push-constant usage
var<push_constant> uniforms: Transform;

@vertex
fn gb_vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Compute the world-space position from the model matrix
    let world_pos: vec3<f32> = (uniforms.model * vec4<f32>(input.position, 1.0)).xyz;

    // Convert from world-space to clip-space using the view-projection matrix
    output.out_position = global_data.view_proj * vec4<f32>(world_pos, 1.0);

    // Pass through the texture coordinates directly
    output.tex_coords = input.tex_coords;

    // Transform and normalize the normals using the model matrix
    // (This assumes uniform scaling; otherwise you'd need an inverse-transpose.)
    output.normal = normalize((uniforms.inverse_transpose_model * vec4<f32>(input.normals, 0.0)).xyz);

    // Store the world-space position for the G-buffer
    output.world_pos = world_pos;

    return output;
}


struct FragmentInput {
    @location(0) tex_coords: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) position: vec4<f32>,
};

@group(1) @binding(0)
var color_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_sampler: sampler;

@fragment
fn gb_fs_main(input: FragmentInput) -> FragmentOutput {
    var output: FragmentOutput;

    // Sample the diffuse/albedo color from a texture
    output.color = textureSample(color_texture, color_sampler, input.tex_coords);

    // Encode the normal from [-1,1] into [0,1]
    // We assume 'input.normal' is already normalized in the vertex shader.
    output.normal = vec4<f32>(input.normal * 0.5 + vec3<f32>(0.5), 1.0);

    // Write out the world-space position
    output.position = vec4<f32>(input.world_pos, 1.0);

    return output;
}
