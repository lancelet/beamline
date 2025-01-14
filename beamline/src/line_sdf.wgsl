/**
 * NAME:     line_sdf.wgsl
 * PURPOSE:  Vertex and fragment shaders for signed-distance-field lines.
 */

/**** BINDINGS ************************************************************* */

@group(0) @binding(0) var<uniform>       viewport       : Viewport;

@group(1) @binding(0) var<uniform>       shader_options : ShaderOptions;
@group(1) @binding(1) var<storage, read> tile_infos     : array<TileInfo>;
@group(1) @binding(2) var<storage, read> lines          : array<StyledLine>;

/**** STRUCTS ****************************************************************/

/// Viewport
struct Viewport {
    area_width  : u32,
    area_height : u32,
    tile_width  : u32,
    tile_height : u32,
};

/// Shader Options
struct ShaderOptions {
    draw_tiles      : u32,
    tile_background : vec4f,
    tile_edges      : vec4f
};

/// Tile Information
struct TileInfo {
    tile_x      : u32,
    tile_y      : u32,
    start_index : u32,
    n_lines     : u32
};

/// Styled Line
struct StyledLine {
    start   : vec2f,
    end     : vec2f,
    width   : f32,
    cap     : u32,
    color   : vec4f
};

/// Closest Line
struct ClosestLine {
    line_index : u32,
    sdf_value  : f32
};

/// Output from the vertex shader.
struct VertexOutput {
    @location(0)       @interpolate(perspective)  uv             : vec2f,
    @location(2)       @interpolate(flat, either) instance_index : u32,
    @builtin(position)                            position       : vec4f
};

/**** VERTEX SHADER **********************************************************/

@vertex fn vs_main(
    @builtin(vertex_index)   vertex_index    : u32,
    @builtin(instance_index) instance_index  : u32
) -> VertexOutput {
    let vertex_base = tile_base_coords[vertex_index];
    let tile_info   = tile_infos[instance_index];

    // Convert the vertex coordinates to pixel coordinates and then to clip
    // coordinates.
    let vertex_px = tile_vertex_px(
        tile_info.tile_x,
        tile_info.tile_y,
        viewport.tile_width,
        viewport.tile_height,
        vertex_base
    );
    let vertex_clip = pixel_to_clip(vertex_px);

    // uv coordinates are just the original vertex base coordinates.
    let uv = vertex_base;

    return VertexOutput(uv, instance_index, vec4f(vertex_clip, 0.0, 1.0));
}

/**** FRAGMENT SHADER ********************************************************/

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let tile_info = tile_infos[in.instance_index];

    // Convert position from framebuffer coordinates to "beamline" coordinates.
    let p = framebuffer_to_beamline(in.position.xy);

    // Compute the SDF union of all lines in the tile.
    let closest_line = sdf_all_lines(
        tile_info.start_index,
        tile_info.n_lines,
        p
    );
    let line = lines[closest_line.line_index];

    let antialias_width = 3.0; // TODO: Put in shader options.

    let line_amount = line_factor(antialias_width, closest_line.sdf_value);
    var line_color = line.color;
    line_color.w = line_amount;

    let bg_color_temp = vec4f(0.2, 0.2, 0.2, 1.0);

    //return alpha_over(line_color, bg_color_temp);
    /*
    let c = (0.07 * closest_line.sdf_value) % 1.0;
    if (closest_line.sdf_value < 0.0) {
        return vec4f(c, 0.0, 0.0, 1.0);
    } else {
        return vec4f(0.0, c, 0.0, 1.0);
    }
    */
    if (closest_line.sdf_value < 0.0) {
        let c = (0.07 * -closest_line.sdf_value) % 1.0;
        return vec4f(c, 0.0, 0.0, 1.0);
    } else {
        let c = (0.07 * closest_line.sdf_value) % 1.0;
        return vec4f(0.0, c, 0.0, 1.0);
    }
}

/**** FUNCTIONS **************************************************************/

/// Convert a vertex coordinate from `[0.0, 1.0]` range to a pixel coordinate.
///
/// # Parameters
///
/// - `tile_x`: x index of the tile
/// - `tile_y`: y index of the tile
/// - `tile_width`: width of a single tile
/// - `tile_height`: height of a single tile
/// - `vertex`: vertex coordinates in `[0.0, 1.0]`
///
/// # Returns
///
/// `vertex` transformed to pixel coordinates.
fn tile_vertex_px(
    tile_x      : u32,
    tile_y      : u32,
    tile_width  : u32,
    tile_height : u32,
    vertex      : vec2f
) -> vec2f {
    let tile_x_f      = f32(tile_x);
    let tile_y_f      = f32(tile_y);
    let tile_width_f  = f32(tile_width);
    let tile_height_f = f32(tile_height);

    return vec2f(
        (vertex.x + tile_x_f) * tile_width_f,
        (vertex.y + tile_y_f) * tile_height_f
    );
}

/// Convert a pixel coordinate to a clip coordinate.
///
/// # Globals Used
///
/// - `viewport`
///
/// # Parameters
///
/// - `vertex_px`: Vertex in pixel coordinates.
///
/// # Returns
///
/// `vertex_px` transformed from pixel coordinates to clip coordinates.
fn pixel_to_clip(
    vertex_px : vec2f
) -> vec2f {
    let area_width_f  = f32(viewport.area_width);
    let area_height_f = f32(viewport.area_height);

    return vec2f(
        2.0 * vertex_px.x / area_width_f  - 1.0,
        2.0 * vertex_px.y / area_height_f - 1.0
    );
}

/// Convert coordinates from the fragment shader framebuffer to beamline native
/// coordinates.
///
/// This involves flipping the y axis so that it increases vertically.
fn framebuffer_to_beamline(
    coord_fb : vec2f
) -> vec2f {
    let area_height_f = f32(viewport.area_height);

    return vec2f(
        coord_fb.x,
        area_height_f - coord_fb.y
    );
}

/// Alpha-over composite operation.
///
/// This composites `a` over `b`, where neither has a pre-multiplied alpha.
///
/// # Parameters
///
/// - `a`: Top color to composite (non-premultiplied alpha).
/// - `b`: Bottom color to composite (non-premultiplied alpha).
///
/// # Returns
///
/// `a over b` composite operation.
fn alpha_over(
    a : vec4f,
    b : vec4f
) -> vec4f {
    let a_alpha = a.w;
    let b_alpha = b.w;
    let out_alpha = a_alpha + b_alpha * (1.0 - a_alpha);

    return vec4f(
        alpha_over_channel(a_alpha, b_alpha, out_alpha, a.x, b.x),
        alpha_over_channel(a_alpha, b_alpha, out_alpha, a.y, b.y),
        alpha_over_channel(a_alpha, b_alpha, out_alpha, a.z, b.z),
        out_alpha
    );
}

/// Alpha-over compositing operation for a single channel.
///
/// # Parameters
///
/// - `a_alpha`: The alpha value of the `a` color.
/// - `b_alpha`: The alpha value of the `b` color.
/// - `out_alpha`: Output alpha (`a_alpha + b_alpha * (1.0 - a_alpha)`)
/// - `a_comp`: Channel of the `a` color to operate on.
/// - `b_comp`: Channel of the `b` color to operate on.
///
/// # Returns
///
/// `a over b` for one channel (ie. red, green or blue).
fn alpha_over_channel(
    a_alpha   : f32,
    b_alpha   : f32,
    out_alpha : f32,
    a_comp    : f32,
    b_comp    : f32
) -> f32 {
    return (a_comp * a_alpha + b_comp * b_alpha * (1 - a_alpha)) / out_alpha;
}

/// TODO
fn line_factor(
    antialias_width : f32,
    dist            : f32
) -> f32 {
    return smoothstep(-antialias_width, 0.0, dist);
}

/// Find the SDF union of all lines at the current tile.
///
/// The SDF union is the minimum distance between all SDFs.
fn sdf_all_lines(
    start_index : u32,
    n_lines     : u32,
    p           : vec2f
) -> ClosestLine {
    let end_index: u32 = start_index + n_lines;
    var min_dist: f32 = sdf_styled_line(lines[start_index], p);
    var min_idx: u32 = start_index;
    for (var i: u32 = start_index + 1; i < end_index; i = i + 1) {
        let dist = sdf_styled_line(lines[i], p);
        if (dist < min_dist) {
            min_dist = dist;
            min_idx  = i;
        }
    }
    return ClosestLine(min_idx, min_dist);
}

/// Returns the signed distance function for a styled line.
///
/// This accounts for the end-cap style of the line, which has to form part
/// of the signed distance function. If the end cap style is invalid for any
/// reason, the fallback is to use a rounded line style.
///
/// # Parameters
///
/// - `styled_line`: The line to examine.
/// - `p`: Location.
///
/// # Returns
///
/// The signed distance function evaluated at `p`.
fn sdf_styled_line(
    styled_line : StyledLine,
    p           : vec2f
) -> f32 {
    // Switch operation depending on the end cap.
    if (styled_line.cap == END_CAP_BUTT || styled_line.cap == END_CAP_SQUARE) {
        // TODO
        return 1000.0;
    } else {
        return sdf_rounded_line(
            styled_line.start,
            styled_line.end,
            styled_line.width / 2.0,
            p
        );
    }
}

/// Returns the signed distance function for a rounded line.
///
/// # Parameters
///
/// - `start`: Start coordinate of the line.
/// - `end`: End coordinate of the line.
/// - `radius`: Radius (half-width) of the line.
/// - `p`: Location.
///
/// # Returns
///
/// The signed distance function evaluated at `p`.
fn sdf_rounded_line(
    start  : vec2f,
    end    : vec2f,
    radius : f32,
    p      : vec2f
) -> f32 {
    let pa = p - start;
    let ba = end - start;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    let line_dist = length(pa - ba * h);

    return line_dist - radius;
}

/**** CONSTANTS **************************************************************/

/// Different types of end cap.
const END_CAP_BUTT   : u32 = 1;
const END_CAP_ROUND  : u32 = 2;
const END_CAP_SQUARE : u32 = 3;

/// Basic coordinates for a tile.
///
/// A tile (before transformation in the vertex shader) is a square from
/// (0.0, 0.0) to (1.0, 1.0), composed of two triangles.
const tile_base_coords: array<vec2f, 6> = array<vec2f, 6>(
    // First triangle
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    // Second triangle
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0)
);
