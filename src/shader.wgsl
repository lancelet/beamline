/*****************************************************************************
 * Structs
 *****************************************************************************/

/// Camera / View parameters
struct CameraUniform {
    screen_width: u32,   /// Width of the screen.
    screen_height: u32,  /// Height of the screen.
    bucket_width: u32,   /// Width of a bucket.
    bucket_height: u32   /// Height of a bucket.
};

/// Offset information for each tile.
struct InstanceOffsets {
    tile_x: u32,             /// x (horizontal) tile coordinate.
    tile_y: u32,             /// y (vertical) tile coordinate.
    line_start_index: u32,   /// start index of lines for this tile.
    line_count: u32          /// number of lines in this tile.
};

/// Line information.
struct Line {
    x0: f32,          /// Start x coordinate.
    y0: f32,          /// Start y coordinate.
    x1: f32,          /// End x coordinate.
    y1: f32,          /// End y coordinate.
    core_width: f32,  /// Width of the core part of the line.
    glow_width: f32   /// Width of the glow.
};

/// Output from the vertex shader.
struct VertexOutput {
    /// uv coordinates (used to render tile boundaries)
    @location(0) @interpolate(perspective) uv: vec2<f32>,
    /// Instance of the index for the current tile.
    @location(2) @interpolate(flat, either) instance_index: u32,
    /// Position of the vertex.
    @builtin(position) position: vec4<f32>
};

/*****************************************************************************
 * Bindings
 *****************************************************************************/

@group(0) @binding(0) var<uniform>       camera           : CameraUniform;
@group(1) @binding(0) var<storage, read> instance_offsets : array<InstanceOffsets>;
@group(1) @binding(1) var<storage, read> lines            : array<Line>;

/*****************************************************************************
 * Constants
 *****************************************************************************/

/// Basic coordinates for a tile.
///
/// A tile (before transformation in the vertex shader) is a square from
/// (0.0, 0.0) to (1.0, 1.0), composed of two triangles.
const tile_base_coords: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    // First triangle
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    // Second triangle
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0)
);

/// Vertex shader
@vertex
fn vs_main(
    /// The index of the current vertex.
    @builtin(vertex_index)vertex_index: u32,
    /// The index of the current instance.
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    // Vertex in pixel coordinates.
    //
    // This transforms the tile coordinates of the vertex to its coordinates
    // in pixels. The origin is in the bottom left. x increases to the right,
    // y increases upwards.
    let iofs: InstanceOffsets = instance_offsets[instance_index];
    let vtx: vec2<f32> = tile_base_coords[vertex_index];
    let x_px = (vtx.x + f32(iofs.tile_x)) * f32(camera.bucket_width);
    let y_px = (vtx.y + f32(iofs.tile_y)) * f32(camera.bucket_height);

    // Transform vertex from pixel coordinates to clip coordinates
    let x_clip = (2.0 / f32(camera.screen_width)) * x_px - 1.0;
    let y_clip = (2.0 / f32(camera.screen_height)) * y_px - 1.0;

    let clip_position = vec4<f32>(x_clip, y_clip, 0.0, 1.0);
    let uv = vtx;
    return VertexOutput(uv, instance_index, clip_position);
}

const DIST_MAX: f32 = 32767.0;

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert the raster position so that y increases upwards.
    let p = vec2f(in.position.x, f32(camera.screen_height) - in.position.y);

    // fetch instance info
    let iofs = instance_offsets[in.instance_index];

    // find the closest line
    let line_end_index = iofs.line_start_index + iofs.line_count;
    var closest_line_dist: f32 = distance_to_line(lines[iofs.line_start_index], p);
    var closest_line_idx: u32 = 0;
    for (var i: u32 = iofs.line_start_index + 1; i < line_end_index; i = i + 1) {
        let cur_dist = distance_to_line(lines[i], p);
        if (cur_dist < closest_line_dist) {
            closest_line_dist = cur_dist;
            closest_line_idx = i;
        }
    }

    var bg: vec4f = vec4f(0,0,0,0.2);
    if (tile_edge_dist(in.uv) < 0.02) {
        bg = vec4f(1,1,1,0.2);
    }
    // var bg = vec4f(0,0,0,0);

    let line = lines[closest_line_idx];
    let core_width = line.core_width;
    let glow_width = line.glow_width;
    let max_width = max(core_width, glow_width);

    let line_c = vec4f(0.6, 0.8, 1.0, line_amt(closest_line_dist, core_width));
    let glow_c = vec4f(1.0, 1.0, 1.0, glow_amt(closest_line_dist, glow_width));
    let line_o = vec4f(0.9, 0.2, 0.2, 0.0 * line_amt(closest_line_dist, max_width));
    let fg = alpha_over(line_c + 0.15 * glow_c, line_o);

    return alpha_over(fg, bg);
}

/// Find the shortest distance from the current uv coordinates to the edge of
/// the tile (also in uv coordinates).
///
/// # Parameters
///
/// - `uv`: Current uv coordinates.
///
/// # Returns
///
/// Shortest distance from `uv` to any edge of the tile.
fn tile_edge_dist(uv: vec2<f32>) -> f32 {
    var dx = min(uv.x, 1.0 - uv.x);
    var dy = min(uv.y, 1.0 - uv.y);
    return min(dx, dy);
}

/// Alpha-over composite operation.
///
/// This composites `a` over `b`, where neither has a pre-multiplied alpha.
///
/// # Parameters
///
/// - `a`: Top color to composite.
/// - `b`: Bottom color to composite.
fn alpha_over(a: vec4f, b: vec4f) -> vec4f {
    let w = a.w + b.w * (1 - a.w);
    let x = (a.x * a.w + b.x * b.w * (1 - a.w)) / w;
    let y = (a.y * a.w + b.y * b.w * (1 - a.w)) / w;
    let z = (a.z * a.w + b.z * b.w * (1 - a.w)) / w;
    return vec4f(x, y, z, w);
}

/// Find the closest distance to a line.
///
/// # Parameters
///
/// - `line`: The line to examine.
/// - `p`   : Position.
///
/// # Returns
///
/// The closest Euclidean distance from the line to point `p`.
fn distance_to_line(line: Line, p: vec2f) -> f32 {
    // a and b are the points at each end of the line
    let a = vec2f(line.x0, line.y0);
    let b = vec2f(line.x1, line.y1);

    let pa = p - a;   // vector from a to p
    let ba = b - a;   // vector from a to b

    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba*h);
}

fn line_amt(dist: f32, width: f32) -> f32 {
    let w2 = width / 2.0;
    let edge_blend_dist = 1.44;

    if (dist < w2) {
        return 1.0 - smoothstep(w2-edge_blend_dist, w2, dist);
    } else {
        return 0.0;
    }
}

fn glow_amt(dist: f32, width: f32) -> f32 {
    let w2 = width / 2.0;
    if (dist < w2) {
        let t = 1.0 - abs(dist / w2);
        let glow_amt = pow(t, 1.4) + pow(t, 2.2) + pow(t, 3.0);
        return glow_amt;
    } else {
        return 0.0;
    }
}
