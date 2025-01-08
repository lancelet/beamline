// Vertex shader

struct CameraUniform {
    width: u32,
    height: u32,
    bucket_width: u32,
    bucket_height: u32
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,   
};

const coords: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    // First triangle
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    // Second triangle
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0)
);

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    @builtin(instance_index) in_instance_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    var vtx: vec2<f32> = coords[in_vertex_index];
    
    var dx = f32(camera.bucket_width) / f32(camera.width);
    var dy = f32(camera.bucket_height) / f32(camera.height);

    vtx.x = vtx.x * dx - 1.0;
    vtx.y = vtx.y * dy - 1.0;
    
    vtx.x += dx * f32(in_instance_index) * 2;
    vtx.y += dy * f32(in_instance_index) * 2;

    out.clip_position = vec4<f32>(vtx, 0.0, 1.0);
    return out;
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.3, 0.8, 0.3, 1.0);  // sets the color
}