// A simple directional light structure.
struct Light {
    direction: vec3<f32>, // Light direction (pointing from the light source)
    color: vec3<f32>,     // Light color/intensity
};

struct GlobalData {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> global_data: GlobalData;

@group(0) @binding(1)
var<storage, read> lights: array<Light>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) out_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) position: vec3<f32>,
}

// 1) Define a uniform containing an MVP matrix.
//    In WGSL you can label it with @group/@binding, matching how you create the bind group on the CPU side.
struct Transform {
    mvp: mat4x4<f32>,
}


var<push_constant> uniforms: Transform;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // Multiply position by the MVP matrix to apply transformations
    output.out_position = global_data.view_proj * uniforms.mvp * vec4<f32>(input.position, 1.0);

    // Pass tex coords through to the fragment stage
    output.tex_coords = input.tex_coords;
    output.normals = input.normals;
    output.position = input.position;
    return output;
}

struct FragmentInput {
    @location(0) tex_coords: vec2<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) position: vec3<f32>,
}

struct FragmentOutput {
    // GBuffer output
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) position: vec4<f32>,
}

// Example texture + sampler for sampling.
@group(1) @binding(0)
var color_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_sampler: sampler;

@fragment
fn fs_main(input: FragmentInput) -> FragmentOutput {
    var output: FragmentOutput;
    output.color = textureSample(color_texture, color_sampler, input.tex_coords);
    // Normal and position are passed through from the vertex shader
    output.normal = vec4<f32>(normalize(input.normals), 0.0);
    output.position = vec4<f32>(input.position, 1.0);
    return output;
}
