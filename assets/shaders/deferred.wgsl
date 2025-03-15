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
};

struct ShadowData {
    light_view_proj: mat4x4<f32>,
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,
    bias: f32,
};

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
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0,  1.0)
    );
    let pos = positions[vertex_index];
    return VertexOutput(
        vec4<f32>(pos.x, -pos.y, 0.0, 1.0),
        (pos + vec2<f32>(1.0, 1.0)) * 0.5
    );
}

// Which cube face to sample for a point light, given the direction.
fn pick_point_light_face(direction: vec3<f32>) -> u32 {
    let ad = abs(direction);
    if ad.x > ad.y && ad.x > ad.z {
        if (direction.x < 0.0) { return 0u; } else { return 1u; };
    } else if ad.y > ad.x && ad.y > ad.z {
        if (direction.y < 0.0) { return 2u; } else { return 3u; };
    } else {
        if (direction.z < 0.0) { return 4u; } else { return 5u; };
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

/// Function to decompress depth from perspective projection.
fn decompress_depth(depth: f32, near: f32, far: f32) -> f32 {
    return (2.0 * near) / (far + near - depth * (far - near));
}

@fragment
fn deferred_fs(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the G-buffer.
    let albedo = textureSample(g_albedo, g_sampler, input.uv).rgb;
    let normal_enc = textureSample(g_normal, g_sampler, input.uv).rgb;
    let depth_val = textureSample(g_depth, g_sampler, input.uv);
    let normal = normalize(normal_enc * 2.0 - vec3<f32>(1.0));

    // Reconstruct world position.
    let ndc = vec4<f32>(
        (input.position.x / global_data.screen_size.x) * 2.0 - 1.0,
        1.0 - (input.position.y / global_data.screen_size.y) * 2.0,
        depth_val,
        1.0
    );
    var world_pos = global_data.inv_view_proj * ndc;
    world_pos /= world_pos.w;

    // Calculate the real world units based on the depth
    let depth_dist = length(world_pos.xyz - global_data.view_proj[3].xyz);

    // Start with an ambient term.
    var final_color = albedo * 0.05;

    // Loop over lights.
    for (var i = 0u; i < 4u; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { continue; }

        if (light.light_type == 0u) {
            // Directional light.
            let L = normalize(-light.rotation);
            let NdotL = max(dot(normal, L), 0.0);
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
                //final_color += diffuse;
            }
        } else if (light.light_type == 1u) {
            var diffuse = vec3<f32>(0.0);
            let to_light = light.position - world_pos.xyz;
            let L = normalize(to_light);
            let NdotL = max(dot(normal, L), 0.0);
            let falloff = 1.0 - saturate(length(to_light) / light.range);
            if (NdotL > 0.0) {
                diffuse = albedo * light.color * NdotL * light.intensity * falloff;

            // Point light.
            if (length(to_light) > light.range) { continue; }


            let face_index = pick_point_light_face(to_light);
            let sd = shadow_data[light.shadow_offset + face_index];
            // Compute vector from world position to the light
            let to_light_norm = normalize(light.position - world_pos.xyz);

            // Adjust for
            var shadow_coord = compute_shadow_coord(sd, world_pos.xyz, true);

            ///final_color = shadow_coord.xyz;
            // For point lights, remap Z from [-1,1] to [0,1].
            //var shadow_coord = compute_shadow_coord(sd, world_pos.xyz, true);
            var stored_depth = textureSample(shadow_map, g_sampler, shadow_coord.xy);
            // Convert the stored depth
            stored_depth = decompress_depth(stored_depth, 1.0, light.range);

            var depth_based_shadow_factor = select(1.0, 0.0, shadow_coord.z > stored_depth);

            // Compare the stored depth to the distance of fragment to light.
            // If the fragment is closer to the light, it is in shadow.
            // Multiply stored depth by the range of the light to get the real distance.
            var dist = length(to_light);
            var shadow_dist = dist / light.range;
            // Apply bias
            shadow_dist += sd.bias;

            // Compute the shadow factor.We do this by combining both the depth comparison
            // method and the distance comparison method.
            // The shadow factor is 1.0 if the fragment is in shadow, and 0.0 if it is not.
            var dist_based_shadow_factor = select(1.0, 0.0, shadow_dist > stored_depth);
            var shadow_factor = max(depth_based_shadow_factor, dist_based_shadow_factor);

            diffuse *= shadow_factor;
            final_color += diffuse;
            //final_color = to_light_norm;

            //final_color = vec3<f32>(shadow_coord.z);
            }
        }
        // Spot lights (light_type == 2) can be added similarly.
    }

    //final_color = vec3(depth_dist);

    return vec4<f32>(final_color, 1.0);
}
