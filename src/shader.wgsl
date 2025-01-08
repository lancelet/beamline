// Vertex shader

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,   
};

const r: f32 = 0.5;
const coords: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    // First triangle
    vec2<f32>(-r, -r),
    vec2<f32>(-r, r),
    vec2<f32>(r, -r),
    // Second triangle
    vec2<f32>(r, -r),
    vec2<f32>(-r, r),
    vec2<f32>(r, r)
);

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(coords[in_vertex_index], 0.0, 1.0);
    return out;
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.3, 0.2, 0.1, 1.0);  // sets the color
}