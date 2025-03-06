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
    @builtin(position) frag_coord: vec4<f32>,
    @location(0) uv: vec2<f32>,
};


@fragment
fn lighting_main(input: FragInput) -> @location(0) vec4<f32> {
    // Reconstruct the G-buffer data.
    let albedo: vec3<f32> = textureSample(g_albedo, g_sampler, input.uv).rgb;
    let normal_encoded: vec3<f32> = textureSample(g_normal, g_sampler, input.uv).rgb;
    let normal: vec3<f32> = normalize(normal_encoded * 2.0 - vec3<f32>(1.0));
    let world_pos: vec3<f32> = textureSample(g_position, g_sampler, input.uv).rgb;
    // Sample the depth. Assuming a clear value near 1.0 means background.
    let depth: f32 = textureSample(g_depth, g_sampler, input.uv);

    // If the depth is at the clear value (or very close), consider this pixel background.
    if (depth >= 0.9999) {
         return vec4<f32>(albedo, 1.0);
    }

    // Start with an ambient term.
    var final_color: vec3<f32> = albedo * 0.05;

    // Define a maximum effective distance to ignore very far lights.
    let max_distance: f32 = 15.0;

    // Iterate over all point lights.
    for (var i: u32 = 0u; i < arrayLength(&lights); i = i + 1u) {
        let light = lights[i];
        let light_vec: vec3<f32> = light.position - world_pos;
        let distance: f32 = length(light_vec);
        // Skip lights beyond the maximum effective distance.
        if (distance > max_distance) {
            continue;
        }
        let L: vec3<f32> = normalize(light_vec);
        let NdotL: f32 = max(dot(normal, L), 0.0);
        // Skip if the surface is facing away.
        if (NdotL <= 0.0) {
            continue;
        }
        // Use inverse-square law for attenuation.
        let attenuation: f32 = 1.0 / (distance * distance);
        final_color += albedo * light.color * light.intensity * NdotL * attenuation;
    }

    return vec4<f32>(final_color, 1.0);
}


