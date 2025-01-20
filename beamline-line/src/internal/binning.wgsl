/**
 * NAME:    binning.wgsl
 * PURPOSE: Compute shader to bin lines into tiles.
 */

/**** CONSTANTS ****/

const MAX_U32: u32 = 0xffffffffu;

/**** BINDINGS ****/

/// Dimensions.
var<push_constant> dimensions: Dimensions;

/// Lines to bin.
///
/// This is an array of lines which will be binned by this compute shader.
@group(0) @binding(0) var<storage, read> lines: array<StyledLine>;

/// Bins indicating whether a tile is occupied by a line.
///
/// Bins is a bit array, containing one bit per line per tile. It is indexed
/// as `[line, tile_y, tile_x]`, where the first element is the
/// slowest-varying.
///
/// It should have length `ceil(n_lines * n_tiles_x * n_tiles_y / 32)`.
@group(0) @binding(1) var<storage, read_write> bins: array<atomic<u32>>;

/**** STRUCTS ****/

struct Dimensions {
    n_lines     : u32,
    n_tiles_x   : u32,
    n_tiles_y   : u32,
    tile_width  : u32,
    tile_height : u32
};

struct Strides {
    line_stride   : u32,
    tile_x_stride : u32,
    tile_y_stride : u32
};

struct BinCoord {
    line   : u32,
    tile_x : u32,
    tile_y : u32
};

struct StyledLine {
    start  : vec2f,
    end    : vec2f,
    width  : f32,
    cap    : u32,
    color  : vec4f
};

struct AABB {
    min : vec2f,
    max : vec2f
};

struct TileBounds {
    min : vec2u,
    max : vec2u
};

/**** COMPUTE SHADER ****/

@compute @workgroup_size(32)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let strides = dimensions_to_strides(dimensions);
    let line_idx = global_id[0];
    let line = lines[line_idx];

    // Find the tile bounding rectangle. This represents the tiles that the
    // line's bounding box intersects.
    let tile_bounds = bounds_to_bin(dimensions, bound_line(line));

    // TODO: For the tile bounding box, use a more precise separating-axis
    // test to assign tiles.

    for (
        var tile_y: u32 = tile_bounds.min.y;
        tile_y <= tile_bounds.max.y;
        tile_y = tile_y + 1
    ) {
        for (
            var tile_x: u32 = tile_bounds.min.x;
            tile_x <= tile_bounds.max.x;
            tile_x = tile_x + 1
        ) {
            let coord = BinCoord(line_idx, tile_x, tile_y);
            set_bin(strides, coord);
        }
    }
}

/**** FUNCTIONS USING GLOBALS ****/

/// Sets a bin as active.
///
/// # Parameters
///
/// - `line_stride`: Number of bits between successive lines in the `bins`
///   array.
/// - `tile_x_stride`: Number of bits between successive tiles in the
///   x-direction in the `bins` array.
/// - `tile_y_stride`: Number of bits between successive tiles in the
///   y-direction in the `bins` array.
/// - `line_index`: Index of the line.
/// - `tile_x`: x-index of the tile.
/// - `tile_y`: y-index of the tile.
fn set_bin(
    strides : Strides,
    coord   : BinCoord
) {
    let lindex = bin_lindex(strides, coord);
    atomicOr(
        &bins[lindex / 32],
        u32(1) << (lindex % 32)
    );
}

/**** PURE FUNCTIONS ****/

/// Computes strides of the bins bit array from the dimensions.
fn dimensions_to_strides(
    dims : Dimensions
) -> Strides {
    let tile_x_stride = 1u;
    let tile_y_stride = tile_x_stride * dims.n_tiles_x;
    let line_stride   = tile_y_stride * dims.n_tiles_y;
    return Strides(line_stride, tile_x_stride, tile_y_stride);
}

/// Computes the linear index of a bin from its coordinate index.
fn bin_lindex(
    strides : Strides,
    coord   : BinCoord
) -> u32 {
    return
        coord.line   * strides.line_stride   +
        coord.tile_x * strides.tile_x_stride +
        coord.tile_y * strides.tile_y_stride;
}

/// Computes the bounding-box of a line.
///
/// TODO: Handle specific end-cap styles in the bounding box.
fn bound_line(
    line : StyledLine
) -> AABB {
    let r = abs(line.width) / 2.0;
    return AABB(
        vec2f(
            min(line.start.x, line.end.x) - r,
            min(line.start.y, line.end.y) - r
        ),
        vec2f(
            max(line.start.x, line.end.x) + r,
            max(line.start.y, line.end.y) + r
        )
    );
}

/// Convert floating-point bounding box to a tile bounding box using the
/// tile width and height.
fn bounds_to_bin(
    dims : Dimensions,
    aabb : AABB,
) -> TileBounds {
    let tile_wh = vec2f(f32(dims.tile_width), f32(dims.tile_height));
    let tile_min = vec2u(0, 0);
    let tile_max = vec2u(dims.n_tiles_x - 1, dims.n_tiles_y - 1);

    return TileBounds(
        clamp(vec2u(aabb.min / tile_wh), tile_min, tile_max),
        clamp(vec2u(aabb.max / tile_wh), tile_min, tile_max)
    );
}
