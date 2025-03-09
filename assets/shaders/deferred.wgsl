// ============================================================================
// Deferred Lighting Shader with Shadow Mapping (PCF Example)
// ============================================================================

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
};

@group(0) @binding(0)
var<uniform> global_data: GlobalData;

@group(0) @binding(1)
var<uniform> directional_light: Light;

@group(0) @binding(2)
var<storage, read> lights: array<Light>; // Additional (point) lights.

// -----------------------------------------------------------------------------
// G-buffer Bindings (group 1)
// -----------------------------------------------------------------------------

@group(1) @binding(0)
var g_albedo: texture_2d<f32>;
@group(1) @binding(1)
var g_normal: texture_2d<f32>;
@group(1) @binding(2)
var g_depth: texture_depth_2d;
@group(1) @binding(3)
var g_sampler: sampler;

// -----------------------------------------------------------------------------
// Shadow Map Data (group 1, extra bindings)
// -----------------------------------------------------------------------------

struct ShadowData {
    light_view_proj: mat4x4<f32>, // Precomputed shadow pass matrix.
    uv_offset: vec2<f32>,         // If using an atlas, set to tile offset.
    uv_scale: vec2<f32>,          // If using an atlas, set to tile scale.
    bias: f32,                    // Depth bias.
};

@group(1) @binding(4)
var<uniform> shadow_data: ShadowData;

@group(1) @binding(5)
var shadow_map: texture_depth_2d;

@group(1) @binding(6)
var shadow_sampler: sampler_comparison;

// -----------------------------------------------------------------------------
// Full-Screen Vertex Shader (Triangle Trick)
// -----------------------------------------------------------------------------

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0,  1.0)
    );
    let pos = positions[vertex_index];
    var out: VertexOutput;
    out.position = vec4<f32>(pos.x, -pos.y, 0.0, 1.0);
    out.uv = (pos + vec2<f32>(1.0, 1.0)) * 0.5;
    return out;
}

// -----------------------------------------------------------------------------
// Deferred Fragment Shader with PCF Shadow Sampling
// -----------------------------------------------------------------------------

struct FragmentInput {
    @builtin(position) frag_coord: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@fragment
fn deferred_fs(input: FragmentInput) -> @location(0) vec4<f32> {
    // 1) Sample the G-buffer.
    let albedo = textureSample(g_albedo, g_sampler, input.uv).rgb;
    let normal_enc = textureSample(g_normal, g_sampler, input.uv).rgb;
    let depth = textureSample(g_depth, g_sampler, input.uv);

    // 2) Decode normal.
    let normal = normalize(normal_enc * 2.0 - vec3<f32>(1.0));
    if (depth >= 0.9999) {
        // Background or far plane
        return vec4<f32>(albedo, 1.0);
    }

    // 3) Reconstruct world position.
    let ndc_x = (input.frag_coord.x / global_data.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (input.frag_coord.y / global_data.screen_size.y) * 2.0;
    let ndc = vec4<f32>(ndc_x, ndc_y, depth, 1.0);
    var world_pos = global_data.inv_view_proj * ndc;
    world_pos = world_pos / world_pos.w;

    // 4) Base ambient lighting.
    var final_color = albedo * 0.05;

    // 5) Directional light (no shadow yet).
    let dir_vec = -directional_light.rotation;  // Light direction
    let NdotL   = max(dot(normal, dir_vec), 0.0);
    final_color += albedo * directional_light.color * NdotL * directional_light.intensity;

    // 6) PCF Shadow sampling.
    // Transform world_pos into light clip space:
    var shadow_coord = shadow_data.light_view_proj * vec4<f32>(world_pos.xyz, 1.0);
    shadow_coord /= shadow_coord.w; // For perspective or if w != 1

    // Map [-1..1] to [0..1] in X and Y:
    shadow_coord.x = shadow_coord.x * 0.5 + 0.5;
    shadow_coord.y = shadow_coord.y * 0.5 + 0.5;
    // Flip Y if needed for your texture space:
    shadow_coord.y = 1.0 - shadow_coord.y;

    // Atlas offset/scale if used:
    shadow_coord.x = shadow_coord.x * shadow_data.uv_scale.x + shadow_data.uv_offset.x;
    shadow_coord.y = shadow_coord.y * shadow_data.uv_scale.y + shadow_data.uv_offset.y;

    // Subtract bias:
    shadow_coord.z = shadow_coord.z - shadow_data.bias;

    // ---- PCF 3x3 sample pattern ----
    let offsets = array<vec2<f32>, 9>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 0.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  0.0),
        vec2<f32>( 0.0,  0.0),
        vec2<f32>( 1.0,  0.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 0.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );
    let kernel_radius = 0.0001; // Tweak for softness
    var pcf_sum = 0.0;

    for (var i = 0u; i < 9u; i = i + 1u) {
        let uv_offset = offsets[i] * kernel_radius;
        pcf_sum += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            shadow_coord.xy + uv_offset,
            shadow_coord.z
        );
    }
    var shadow_factor = pcf_sum / 9.0;

    // If we're outside the uv range, don't shadow.
    if (shadow_coord.x < 0.0 || shadow_coord.x > 1.0 ||
        shadow_coord.y < 0.0 || shadow_coord.y > 1.0) {
        shadow_factor = 1.0;
    }
    
    // Optionally darken if the factor is below some threshold (your code):
    if (shadow_factor < 0.5) {
        final_color *= 0.5;
    }

    // Apply final shadow factor.
    final_color *= shadow_factor;

    // 7) Additional point lights (if any).
    let num_lights: u32 = arrayLength(&lights);
    for (var i: u32 = 0u; i < num_lights; i = i + 1u) {
        let light = lights[i];
        let to_light = light.position - world_pos.xyz;
        let dist = length(to_light);
        if (dist > light.range) { continue; }
        let L = normalize(to_light);
        let NdotL_point = max(dot(normal, L), 0.0);
        if (NdotL_point > 0.0) {
            let attenuation_const = 1.0;
            let attenuation_lin   = 0.09;
            let attenuation_quad  = 0.032;
            let attenuation = light.intensity /
                (attenuation_const + attenuation_lin * dist + attenuation_quad * dist * dist);
            final_color += albedo * light.color * NdotL_point * attenuation;
        }
    }

    return vec4<f32>(final_color, 1.0);
}
