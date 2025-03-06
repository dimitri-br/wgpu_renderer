// --- G-Buffer Vertex Shader ---

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) out_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
};

struct GlobalData {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
};

// A simple light structure. For a point light, 'position' is the light's
// world-space position, 'intensity' scales its brightness, and 'color' is its color.
struct Light {
    position: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    _padding: f32, // Padding for 16-byte alignment
};


@group(0) @binding(0)
var<uniform> global_data: GlobalData;


// Dynamic array of lights (storage buffer).
@group(0) @binding(1)
var<storage, read> lights: array<Light>;

// Push constant containing the model matrix.
struct Transform {
    model: mat4x4<f32>,
};

var<push_constant> uniforms: Transform;

@vertex
fn gb_vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // Compute world-space position.
    let world_position: vec3<f32> = (uniforms.model * vec4<f32>(input.position, 1.0)).xyz;
    // Transform to clip space.
    output.out_position = global_data.view_proj * vec4<f32>(world_position, 1.0);
    output.tex_coords = input.tex_coords;
    // Transform normals (assumes no non-uniform scaling).
    output.normal = normalize((uniforms.model * vec4<f32>(input.normals, 0.0)).xyz);
    output.world_pos = world_position;
    return output;
}

// --- G-Buffer Fragment Shader ---

struct FragmentInput {
    @location(0) tex_coords: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
};

struct FragmentOutput {
    // Render targets for the G-buffer.
    @location(0) color: vec4<f32>,   // Albedo
    @location(1) normal: vec4<f32>,  // Encoded normal
    @location(2) position: vec4<f32>, // World-space position
};

@group(1) @binding(0)
var color_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_sampler: sampler;

@fragment
fn gb_fs_main(input: FragmentInput) -> FragmentOutput {
    var output: FragmentOutput;
    // Sample the albedo (diffuse) color.
    output.color = vec4(1.0);//textureSample(color_texture, color_sampler, input.tex_coords);
    // Encode normal from [-1, 1] to [0, 1] for storage.
    output.normal = vec4<f32>(input.normal * 0.5 + vec3<f32>(0.5), 1.0);
    // Output the world-space position.
    output.position = vec4<f32>(input.world_pos, 1.0);
    return output;
}
