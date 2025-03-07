// -----------------------------------------------------------------------------
// Structures and Bindings
// -----------------------------------------------------------------------------

struct Light {
    position: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    range: f32, // <-- NEW: Maximum effective distance of the light
};

struct GlobalData {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> global_data: GlobalData;

@group(0) @binding(1)
var<storage> lights: array<Light>;

// G-buffer textures
@group(1) @binding(0)
var g_albedo:   texture_2d<f32>;
@group(1) @binding(1)
var g_normal:   texture_2d<f32>;
@group(1) @binding(2)
var g_position: texture_2d<f32>;
@group(1) @binding(3)
var g_depth:    texture_depth_2d;
@group(1) @binding(4)
var g_sampler:  sampler;

// -----------------------------------------------------------------------------
// Full-Screen Vertex (full-screen triangle)
// -----------------------------------------------------------------------------
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // The "triangle" that covers the full screen in clip space
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0,  1.0)
    );
    let pos = positions[vertex_index];

    var out: VertexOutput;
    // Depending on your coordinate system, you may flip Y for DirectX
    out.position = vec4<f32>(pos.x, -pos.y, 0.0, 1.0);
    // Convert clip‐space [-1..1] to UV [0..1]
    out.uv = (pos + vec2<f32>(1.0, 1.0)) * 0.5;
    return out;
}

// -----------------------------------------------------------------------------
// Deferred Lighting Fragment
// -----------------------------------------------------------------------------
struct FragmentInput {
    @builtin(position) frag_coord: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@fragment
fn deferred_fs(input: FragmentInput) -> @location(0) vec4<f32> {
    // Reconstruct from G-buffer
    let albedo: vec3<f32>      = textureSample(g_albedo,   g_sampler, input.uv).rgb;
    let normal_enc: vec3<f32>  = textureSample(g_normal,   g_sampler, input.uv).rgb;
    let world_pos: vec3<f32>   = textureSample(g_position, g_sampler, input.uv).rgb;
    let depth: f32             = textureSample(g_depth,    g_sampler, input.uv);

    // Decode normal from [0..1] => [-1..1], then normalize
    let normal = normalize(normal_enc * 2.0 - vec3<f32>(1.0));

    // If depth is near max or the position is zeroed out, treat as background
    if (depth >= 0.9999) {
        // Return the albedo or black, depending on your desired background
        return vec4<f32>(albedo, 1.0);
    }

    // Start with a small ambient term
    var final_color = albedo * vec3<f32>(0.01);

    // Typical attenuation constants
    let attenuation_const = 1.0;
    let attenuation_lin   = 0.09;
    let attenuation_quad  = 0.032;

    let num_lights: u32 = arrayLength(&lights);
    for (var i: u32 = 0u; i < num_lights; i = i + 1u) {
        let light = lights[i];

        // Vector from fragment to the light
        let to_light = light.position - world_pos;
        let dist     = length(to_light);

        // Simple distance cutoff
        if (dist > light.range) {
            continue; // No contribution if outside the light's range
        }

        let L      = normalize(to_light);
        let NdotL  = max(dot(normal, L), 0.0);
        if (NdotL > 0.0) {
            // Quadratic attenuation model
            let attenuation = light.intensity /
                (attenuation_const + attenuation_lin * dist + attenuation_quad * dist * dist);

            // Lambertian diffuse contribution
            final_color += albedo * light.color * NdotL * attenuation;
        }
    }

    return vec4<f32>(final_color, 1.0);
}
