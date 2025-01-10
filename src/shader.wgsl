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
    y1: f32,
    width: f32,
};
@group(1) @binding(1)
var<storage, read> lines: array<Line>;

// NB: @location(n) values are assigned on 4-byte (32-bit) boundaries.
struct VertexOutput {
    @location(0) @interpolate(perspective) uv: vec2<f32>,
    @location(2) @interpolate(flat, either) instance_index: u32,
    @builtin(position) clip_position: vec4<f32>
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
    let vtx: vec2<f32> = coords[in_vertex_index];
    let iofs: InstanceOffsets = instance_offsets[in_instance_index];

    // Vertex in pixel coordinates
    let x_px = (vtx.x + f32(iofs.tile_x)) * f32(camera.bucket_width);
    let y_px = (vtx.y + f32(iofs.tile_y)) * f32(camera.bucket_height);

    // Convert vertex in pixel coordinates to clip coordinates
    let m_x = 2.0 / f32(camera.width);
    let m_y = 2.0 / f32(camera.height);
    let c_x = -1.0;
    let c_y = -1.0;
    let x_clip = m_x * x_px + c_x;
    let y_clip = m_y * y_px + c_y;

    out.clip_position = vec4<f32>(x_clip, y_clip, 0.0, 1.0);
    out.instance_index = in_instance_index;
    out.uv = vtx;
    return out;
}

const DIST_MAX: f32 = 32767.0;

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // convert clip position to our coordinate system
    let p = vec2f(in.clip_position.x, f32(camera.height) - in.clip_position.y);

    // fetch instance info
    let iofs = instance_offsets[in.instance_index];

    // for all lines in the cell, find the closest
    let line_end_index = iofs.line_start_index + iofs.line_count;
    var dist: f32 = DIST_MAX;
    var closest_width: f32 = 0.0;
    for (var i: u32 = iofs.line_start_index; i < line_end_index; i = i + 1) {
        let line = lines[i];
        let a = vec2f(line.x0, line.y0);
        let b = vec2f(line.x1, line.y1);
        let pa = p - a;
        let ba = b - a;
        let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
        let cur_dist = length(pa - ba*h);
        if (cur_dist < dist) {
            dist = cur_dist;
            closest_width = line.width;
        }
    }

    var bg: vec4f = vec4f(0,0,0,0.2);
    if (edge_dist(in.uv) < 0.02) {
        bg = vec4f(1,1,1,0.2);
    }

    let edge_blend_dist = 1.44;
    let thresh = closest_width / 2.0;
    if (dist < thresh) {
        let alpha = 1.0 - smoothstep(thresh - edge_blend_dist, thresh, dist);
        let fg = vec4f(0.8, 0.2, 0.2, alpha);
        return alpha_over(fg, bg);
    } else {
        return bg;
    }
}

fn edge_dist(uv: vec2<f32>) -> f32 {
    var dx = min(uv.x, 1.0 - uv.x);
    var dy = min(uv.y, 1.0 - uv.y);
    return min(dx, dy);
}

// Blend a over b (non-premultiplied).
fn alpha_over(a: vec4f, b: vec4f) -> vec4f {
    let w = a.w + b.w * (1 - a.w);
    let x = (a.x * a.w + b.x * b.w * (1 - a.w)) / w;
    let y = (a.y * a.w + b.y * b.w * (1 - a.w)) / w;
    let z = (a.z * a.w + b.z * b.w * (1 - a.w)) / w;
    return vec4f(x, y, z, w);
}
