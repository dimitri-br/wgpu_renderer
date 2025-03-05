struct GlobalData{
    view_proj: mat4x4<f32>,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

// 1) Define a uniform containing an MVP matrix.
//    In WGSL you can label it with @group/@binding, matching how you create the bind group on the CPU side.
struct Transform {
    mvp: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> global_data: GlobalData;


var<push_constant> uniforms: Transform;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // Multiply position by the MVP matrix to apply transformations
    output.position = global_data.view_proj * uniforms.mvp * vec4<f32>(input.position, 1.0);

    // Pass tex coords through to the fragment stage
    output.tex_coords = input.tex_coords;
    return output;
}

struct FragmentInput {
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
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
    //output.color = vec4(255.0, 0.0, 0.0, 0.0);
    return output;
}
