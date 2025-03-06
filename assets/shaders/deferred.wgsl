// ============================================================================
// Deferred Lighting Shader
// This shader takes four G-buffer textures:
//  - Albedo (diffuse color)
//  - Normal (encoded in [0, 1] range)
//  - Position (world-space position)
//  - Depth (depth value)
// and computes a final shaded output using multiple point lights passed in
// via a storage buffer.
// ============================================================================

// -----------------------------------------------------------------------------
// Structures and Bindings
// -----------------------------------------------------------------------------

// A simple light structure. For a point light, 'position' is the light's
// world-space position, 'intensity' scales its brightness, and 'color' is its color.
struct Light {
    position: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    _padding: f32, // Padding for 16-byte alignment
};

// Global data uniform. In addition to view-projection, you could include
// a screen-size, time, etc.
struct GlobalData {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
};

// Global resources, stored in group 0.
@group(0) @binding(0)
var<uniform> global_data: GlobalData;

// Dynamic array of lights (storage buffer).
@group(0) @binding(1)
var<storage, read> lights: array<Light>;

// G-buffer textures and sampler are stored in group 1.
@group(1) @binding(0)
var g_albedo: texture_2d<f32>;

@group(1) @binding(1)
var g_normal: texture_2d<f32>;

@group(1) @binding(2)
var g_position: texture_2d<f32>;

@group(1) @binding(3)
var g_depth: texture_depth_2d;

@group(1) @binding(4)
var g_sampler: sampler;

// -----------------------------------------------------------------------------
// Full-Screen Vertex Shader (Triangle Vertex Trick)
// -----------------------------------------------------------------------------

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Hard-coded full-screen triangle positions in clip space.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0,  1.0)
    );
    let pos = positions[vertex_index];
    var output: VertexOutput;
    // For DirectX clip space, flip the y coordinate.
    output.position = vec4<f32>(pos.x, -pos.y, 0.0, 1.0);
    // Compute UVs from the original clip-space positions.
    output.uv = (pos + vec2<f32>(1.0, 1.0)) * 0.5;
    return output;
}

// -----------------------------------------------------------------------------
// Deferred Lighting Fragment Shader
// -----------------------------------------------------------------------------

struct FragInput {
    @location(0) uv: vec2<f32>,
};

@fragment
fn fragment_main(input: FragInput) -> @location(0) vec4<f32> {
    // Sample G-buffer textures.
    let albedo = textureSample(g_albedo, g_sampler, input.uv);
    let normal_encoded = textureSample(g_normal, g_sampler, input.uv).rgb;
    let pos = textureSample(g_position, g_sampler, input.uv).rgb;
    // Depth texture sampling is omitted for this example.

    // Decode the normal from [0, 1] to [-1, 1] and normalize.
    let normal = normalize(normal_encoded * 2.0 - vec3<f32>(1.0));

    // Initialize the accumulated light color.
    var lit_color = vec3<f32>(0.0);

    // Determine the number of lights using WGSL's arrayLength() function.
    let num_lights: u32 = arrayLength(&lights);

    // Loop over each light in the storage buffer.
    // (This works if the storage buffer's array is unsized; otherwise, use a fixed count.)
    for (var i: u32 = 0u; i < num_lights; i = i + 1u) {
        let light = lights[i];

        // Compute the direction from the pixel to the light.
        // For a point light, subtract pixel position from light position.
        let L = normalize(light.position - pos);

        // Fall-off factor based on distance.
        let dist = length(light.position - pos);
        let attenuation = 1.0 / (1.0 + dist * dist);

        // Compute the diffuse term using Lambert's cosine law.
        let NdotL = max(dot(normal, L), 0.0);

        // Accumulate light contribution.
        lit_color += light.color * light.intensity * NdotL;
    }

    // Multiply the computed light with the albedo (diffuse color).
    let final_color = albedo.rgb * lit_color;

    return vec4<f32>(final_color, albedo.a);
}
