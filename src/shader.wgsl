struct CameraUniform {
    width: u32,
    height: u32,
    bucket_width: u32,
    bucket_height: u32
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct InstanceOffsets {
    tile_x: u32,
    tile_y: u32,
    line_start_index: u32,
    line_count: u32
};
@group(1) @binding(0)
var<storage, read> instance_offsets: array<InstanceOffsets>;

struct Line {
    x0: f32,
    y0: f32,
    x1: f32,
    y2: f32
};
@group(1) @binding(1)
var<storage, read> lines: array<Line>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @interpolate(flat) @location(0) line_start_index: u32,
    @interpolate(flat) @location(1) line_count: u32
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

// Vertex shader
@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    @builtin(instance_index) in_instance_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    var vtx: vec2<f32> = coords[in_vertex_index];
    var iofs: InstanceOffsets = instance_offsets[in_instance_index];

    // Vertex in pixel coordinates
    var x_px = (vtx.x + f32(iofs.tile_x)) * f32(camera.bucket_width);
    var y_px = (vtx.y + f32(iofs.tile_y)) * f32(camera.bucket_height);

    // Convert vertex in pixel coordinates to clip coordinates
    var m_x = 2.0 / f32(camera.width);
    var m_y = 2.0 / f32(camera.height);
    var c_x = -1.0;
    var c_y = -1.0;
    var x_clip = m_x * x_px + c_x;
    var y_clip = m_y * y_px + c_y;

    out.clip_position = vec4<f32>(x_clip, y_clip, 0.0, 1.0);
    out.line_start_index = iofs.line_start_index;
    out.line_count = iofs.line_count;
    return out;
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.3, 0.8, 0.3, 1.0);  // sets the color
}
