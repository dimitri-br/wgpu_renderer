@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // A single big triangle that covers the screen:
    //  (-1, -1) -> bottom-left corner
    //  ( 3, -1) -> far beyond bottom-right
    //  (-1,  3) -> far beyond top-left
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );

    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}


// Fullscreen Fragment Shader
@group(0) @binding(0)
var color_texture: texture_2d<f32>;
@group(0) @binding(1)
var color_sampler: sampler;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // Normalize fragment coordinates to [0, 1]
    let uv = frag_coord.xy / vec2<f32>(textureDimensions(color_texture));
    return textureSample(color_texture, color_sampler, uv);
}
