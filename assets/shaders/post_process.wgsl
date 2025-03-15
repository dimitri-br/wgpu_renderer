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

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};


@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Hard-coded positions for a full-screen triangle in OpenGL clip space.
    // For DirectX clip space we flip the y component.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, 3.0),
        vec2<f32>( 3.0,  -1.0),
        vec2<f32>(-1.0,  -1.0)
    );
    let pos = positions[vertex_index];
    var output: VertexOutput;
    // Flip the y coordinate to adjust for DirectX clip space.
    //output.position = vec4<f32>(pos.x, -pos.y, 0.0, 1.0);
    output.position = vec4<f32>(pos.x, pos.y, 0.0, 1.0);
    // Compute UVs from the original positions.
    output.uv = (pos + vec2<f32>(1.0, 1.0)) * 0.5;
    return output;
}

@group(1) @binding(0)
var u_texture: texture_2d<f32>;

@group(1) @binding(1)
var u_sampler: sampler;

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    var color = fxaa_main(uv);
    tonemap_aces(color.rgb);
    return color;
}

// -------------------
// FXAA (Fast Approximate Anti-Aliasing) Effect
// Uses neighboring samples and luminance comparisons to smooth edges.
// Requires the inverse screen size uniform.
// -------------------
fn fxaa_main(uv: vec2<f32>) -> vec4<f32> {
    let inverse_screen_size = 1.0 / global_data.screen_size;
    let rgbNW = textureSample(u_texture, u_sampler, uv + vec2(-inverse_screen_size.x, -inverse_screen_size.y)).rgb;
    let rgbNE = textureSample(u_texture, u_sampler, uv + vec2( inverse_screen_size.x, -inverse_screen_size.y)).rgb;
    let rgbSW = textureSample(u_texture, u_sampler, uv + vec2(-inverse_screen_size.x,  inverse_screen_size.y)).rgb;
    let rgbSE = textureSample(u_texture, u_sampler, uv + vec2( inverse_screen_size.x,  inverse_screen_size.y)).rgb;
    let rgbM  = textureSample(u_texture, u_sampler, uv).rgb;

    let lumaNW = dot(rgbNW, vec3<f32>(0.299, 0.587, 0.114));
    let lumaNE = dot(rgbNE, vec3<f32>(0.299, 0.587, 0.114));
    let lumaSW = dot(rgbSW, vec3<f32>(0.299, 0.587, 0.114));
    let lumaSE = dot(rgbSE, vec3<f32>(0.299, 0.587, 0.114));
    let lumaM  = dot(rgbM,  vec3<f32>(0.299, 0.587, 0.114));

    let lumaMin = min(lumaM, min(min(lumaNW, lumaNE), min(lumaSW, lumaSE)));
    let lumaMax = max(lumaM, max(max(lumaNW, lumaNE), max(lumaSW, lumaSE)));

    var dir = vec2<f32>(
        -((lumaNW + lumaNE) - (lumaSW + lumaSE)),
         ((lumaNW + lumaSW) - (lumaNE + lumaSE))
    );

    let dirReduce = max((lumaNW + lumaNE + lumaSW + lumaSE) * (0.25 * 0.5), 0.001);
    let rcpDirMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + dirReduce);
    let dirNormalized = clamp(dir * rcpDirMin, vec2<f32>(-8.0, -8.0), vec2<f32>(8.0, 8.0)) * inverse_screen_size;

    let rgbA = 0.5 * (
        textureSample(u_texture, u_sampler, uv + dirNormalized * (1.0/3.0 - 0.5)).rgb +
        textureSample(u_texture, u_sampler, uv + dirNormalized * (2.0/3.0 - 0.5)).rgb
    );
    let rgbB = rgbA * 0.5 + 0.25 * (
        textureSample(u_texture, u_sampler, uv + dirNormalized * -0.5).rgb +
        textureSample(u_texture, u_sampler, uv + dirNormalized * 0.5).rgb
    );

    let lumaB = dot(rgbB, vec3<f32>(0.299, 0.587, 0.114));
    if (lumaB < lumaMin || lumaB > lumaMax) {
        return vec4<f32>(rgbA, 1.0);
    }
    return vec4<f32>(rgbB, 1.0);
}

// Different tone mapping operators.
fn tonemap_linear(color: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(color, 1.0);
}

fn tonemap_reinhard(color: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(color / (color + vec3<f32>(1.0)), 1.0);
}

fn tonemap_uncharted2(color: vec3<f32>) -> vec4<f32> {
    let A = 0.15;
    let B = 0.50;
    let C = 0.10;
    let D = 0.20;
    let E = 0.02;
    let F = 0.30;
    let mapped = ((color * (A * color + C * B) + D * E) / (color * (A * color + B) + D * F)) - E / F;
    return vec4(mapped, 1.0);
}

fn tonemap_aces(color: vec3<f32>) -> vec4<f32> {
    let A = 2.51;
    let B = 0.03;
    let C = 2.43;
    let D = 0.59;
    let E = 0.14;
    let mapped = (color * (A * color + B)) / (color * (C * color + D) + E);
    return vec4(mapped, 1.0);
}