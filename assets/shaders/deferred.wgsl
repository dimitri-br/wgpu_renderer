struct GlobalData {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    time: f32,
};

struct Light {
    position: vec3<f32>,
    range: f32,
    rotation: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    light_type: u32,      // 0=dir, 1=point, 2=spot
    view_proj: mat4x4<f32>,
    shadow_offset: u32,
    shadow_count: u32,
    spot_angle: f32,
};

struct ShadowData {
    light_view_proj: mat4x4<f32>,
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,
    bias: f32,
};

// u32 push constant - light count
var<push_constant> light_count: u32;

@group(0) @binding(0)
var<uniform> global_data: GlobalData;

@group(0) @binding(1)
var<storage, read> lights: array<Light>;

@group(0) @binding(2)
var<storage, read> shadow_data: array<ShadowData>;

// G-buffer + shadow map textures/samplers.
@group(1) @binding(0)
var g_albedo: texture_2d<f32>;
@group(1) @binding(1)
var g_normal: texture_2d<f32>;
@group(1) @binding(2)
var g_depth: texture_depth_2d;
@group(1) @binding(3)
var g_sampler: sampler;

@group(1) @binding(4)
var shadow_map: texture_depth_2d;
@group(1) @binding(5)
var shadow_sampler: sampler_comparison;

// Full-screen triangle.
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    // Original UVs range from 0 to 2, so we scale them to [0,1]
    output.uv = vec2<f32>(
        f32((vertex_index << 1u) & 2u),
        f32(vertex_index & 2u)
    );
    // Convert the scaled UVs to clip space [-1,1]
    output.position = vec4<f32>(output.uv * 2.0 - 1.0, 0.0, 1.0);
    return output;
}

fn calculate_world_position(texture_coordinate: vec2<f32>, depth: f32) -> vec3<f32> {
    // For WebGPU the NDC z-range is already [0,1]; no remapping needed.
    let z_ndc = depth;

    // Convert texture UV to NDC space (-1 to 1 range)
    let x_ndc = texture_coordinate.x * 2.0 - 1.0;
    let y_ndc = (texture_coordinate.y * -2.0) + 1.0; // Flip Y for WebGPU NDC

    // Construct clip-space position
    let clip = vec4<f32>(x_ndc, y_ndc, z_ndc, 1.0);

    // Transform from clip space to world space
    let viewPos = global_data.inv_view_proj * clip;

    // Perspective divide to obtain world-space coordinates
    return viewPos.xyz / viewPos.w;
}

fn pick_point_light_face(direction: vec3<f32>) -> u32 {
    let ad = abs(direction);
    if (ad.x >= ad.y && ad.x >= ad.z) {
        // Invert the result for X axis
        if (direction.x > 0.0) { return 1u; } else { return 0u; }
    } else if (ad.y >= ad.x && ad.y >= ad.z) {
        // Invert the result for Y axis
        if (direction.y > 0.0) { return 3u; } else { return 2u; }
    } else {
        // Invert the result for Z axis
        if (direction.z > 0.0) { return 5u; } else { return 4u; }
    }
}


// Common function to compute shadow coordinates from world position.
fn compute_shadow_coord(sd: ShadowData, world_pos: vec3<f32>, remapZ: bool) -> vec4<f32> {
    // Transform world position into light clip space.
    var coord = sd.light_view_proj * vec4<f32>(world_pos, 1.0);
    coord /= coord.w;
    // Remap XY from [-1,1] to [0,1].
    coord = vec4(coord.xy * 0.5 + vec2<f32>(0.5), coord.z, coord.w);
    // Flip Y if the atlas is top-left origin.
    coord.y = 1.0 - coord.y;
    // Apply atlas UV scale and offset.
    coord = vec4(coord.xy * sd.uv_scale + sd.uv_offset, coord.z, coord.w);

    // Optionally remap Z from [-1,1] to [0,1].
    if (remapZ) {
        coord.z = coord.z * 0.5 + 0.5;
    }
    // Apply bias.
    //let depth_bias = decompress_depth(coord.z, 0.1, sd.light_view_proj[3].z);
    //coord.z -= depth_bias;
    coord.z -= sd.bias;

    // Adjust bias based on the slope of the surface.
    // This is a common technique to reduce shadow acne.
    let slope = 1.0 - dot(world_pos, sd.light_view_proj[2].xyz);
    coord.z += slope * sd.bias;

    // Check if the coord falls outside the atlas.
    // We can do this by checking against the min/max that the
    // UV scale and offset would produce.
    let min_uv = sd.uv_offset;
    let max_uv = sd.uv_offset + sd.uv_scale;
    let outside = any(coord.xy < min_uv) || any(coord.xy > max_uv);
    // If outside, set Z to -1 to indicate that the pixel is in shadow.
    coord.z = select(coord.z, -1.0, outside);

    return coord;
}


struct FragmentOutput {
    @location(0) color_rgba16float: vec4<f32>,
};

@fragment
fn fragment_main(input: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;
    var uv = input.position.xy / global_data.screen_size;
    uv.y = 1.0 - uv.y; // Flip Y for WebGPU

    // Sample G-buffer textures
    let albedo = textureSample(g_albedo, g_sampler, uv).xyz;
    let encodedNormal = textureSample(g_normal, g_sampler, uv).xyz;

    // Decode the normal from [0,1] to [-1,1]
    let N_decoded = normalize(encodedNormal * 2.0 - 1.0);

    // Sample non-linear depth from the depth buffer
    let depthSample = textureSample(g_depth, g_sampler, uv);

    // Compute world-space position
    var world_pos = calculate_world_position(uv, depthSample);

    // Start with an ambient term.
    var final_color = albedo * 0.05;

    // Loop over lights.
    for (var i = 0u; i < light_count; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { continue; }

        if (light.light_type == 0u) {
            // Directional light.
            let L = normalize(-light.rotation);
            let NdotL = max(dot(N_decoded, L), 0.0);
            if (NdotL > 0.0) {
                var diffuse = albedo * light.color * NdotL * light.intensity;
                let sd = shadow_data[light.shadow_offset];
                var shadow_coord = compute_shadow_coord(sd, world_pos.xyz, false);

                // 3x3 PCF.
                let offsets = array<vec2<f32>, 9>(
                    vec2<f32>(-1.0, -1.0), vec2<f32>( 0.0, -1.0), vec2<f32>( 1.0, -1.0),
                    vec2<f32>(-1.0,  0.0), vec2<f32>( 0.0,  0.0), vec2<f32>( 1.0,  0.0),
                    vec2<f32>(-1.0,  1.0), vec2<f32>( 0.0,  1.0), vec2<f32>( 1.0,  1.0)
                );
                let kernel_radius = 0.0001;
                var pcf_sum = 0.0;
                for (var j = 0u; j < 9u; j = j + 1u) {
                    let offset = offsets[j] * kernel_radius;
                    pcf_sum += textureSampleCompare(shadow_map, shadow_sampler, shadow_coord.xy + offset, shadow_coord.z);
                }
                let shadow_factor = pcf_sum / 9.0;
                diffuse *= shadow_factor;
                final_color += diffuse;
            }
        } else if (light.light_type == 1u) {
            var diffuse = vec3<f32>(0.0);
            let to_light = light.position - world_pos.xyz;
            let L = normalize(to_light);
            let NdotL = max(dot(N_decoded, L), 0.0);
            let falloff = 1.0 - saturate(length(to_light) / light.range);
            if (NdotL > 0.0) {
                diffuse = albedo.xyz * light.color * NdotL * light.intensity * falloff;

                // Point light.
                if (length(to_light) > light.range) { continue; }


                let face_index = pick_point_light_face(to_light);
                let sd = shadow_data[light.shadow_offset + face_index];

                // Adjust shadow coord to be in the light's view space.
                var pos = world_pos.xyz;

                var shadow_coord = compute_shadow_coord(sd, pos, false);

                // Sample the shadow map.
                let offsets = array<vec2<f32>, 9>(
                    vec2<f32>(-1.0, -1.0), vec2<f32>( 0.0, -1.0), vec2<f32>( 1.0, -1.0),
                    vec2<f32>(-1.0,  0.0), vec2<f32>( 0.0,  0.0), vec2<f32>( 1.0,  0.0),
                    vec2<f32>(-1.0,  1.0), vec2<f32>( 0.0,  1.0), vec2<f32>( 1.0,  1.0)
                );
                let kernel_radius = 0.0002;
                var pcf_sum = 0.0;
                for (var j = 0u; j < 9u; j = j + 1u) {
                    let offset = offsets[j] * kernel_radius;
                    pcf_sum += textureSampleCompare(shadow_map, shadow_sampler, shadow_coord.xy + offset, shadow_coord.z);
                }
                let shadow_factor = pcf_sum / 9.0;
                diffuse *= shadow_factor;
                final_color += diffuse;
            }
        }
        // Spot light.
        else if (light.light_type == 2u) {
            let to_light = light.position - world_pos.xyz;
            let L = normalize(to_light);
            let NdotL = max(dot(N_decoded, L), 0.0);
            let falloff = 1.0 - saturate(length(to_light) / light.range);

            // Compute spotlight effect.
            // Assume spotlight direction is given by the normalized negative rotation.
            let spot_dir = normalize(-light.rotation);
            let cos_angle = dot(L, spot_dir);
            let cutoff = cos(light.spot_angle); // cutoff cosine value.
            if (NdotL > 0.0 && cos_angle > cutoff) {
                // Smooth the edge of the spotlight.
                let spot_factor = smoothstep(cutoff, cutoff + 0.1, cos_angle);
                var diffuse = albedo * light.color * NdotL * light.intensity * falloff * spot_factor;

                // Shadow mapping for spotlight.
                let sd = shadow_data[light.shadow_offset];
                var shadow_coord = compute_shadow_coord(sd, world_pos.xyz, false);

                let offsets = array<vec2<f32>, 9>(
                    vec2<f32>(-1.0, -1.0), vec2<f32>( 0.0, -1.0), vec2<f32>( 1.0, -1.0),
                    vec2<f32>(-1.0,  0.0), vec2<f32>( 0.0,  0.0), vec2<f32>( 1.0,  0.0),
                    vec2<f32>(-1.0,  1.0), vec2<f32>( 0.0,  1.0), vec2<f32>( 1.0,  1.0)
                );
                let kernel_radius = 0.0001;
                var pcf_sum = 0.0;
                for (var j = 0u; j < 9u; j = j + 1u) {
                    let offset = offsets[j] * kernel_radius;
                    pcf_sum += textureSampleCompare(shadow_map, shadow_sampler, shadow_coord.xy + offset, shadow_coord.z);
                }
                let shadow_factor = pcf_sum / 9.0;
                diffuse *= shadow_factor;
                final_color += diffuse;
            }
        }
    }

    output.color_rgba16float = vec4<f32>(final_color, 1.0);
    //output = vec4<f32>(world_pos, 1.0);
    return output;
}
