/**
 * NAME:    binning.wgsl
 * PURPOSE: Compute shader assigning lines to tiles.
 */

/**
 * TODO:
 *  1. Better documentation.
 *  2. Use separating-axis test.
 */

/**** BINDINGS ****/

var<push_constant> dimensions: Dimensions;
@group(0) @binding(0) var<storage, read> lines: array<StyledLine>;
@group(0) @binding(1) var<storage, read_write> bins: array<u32>;

/**** STRUCTS ****/

struct Dimensions {
    n_lines     : u32,
    n_tiles_x   : u32,
    n_tiles_y   : u32,
    tile_width  : u32,
    tile_height : u32
};

struct StyledLine {
    p0    : vec2f,
    p1    : vec2f,
    width : f32,
    cap   : u32,
    color : vec4f
};

struct AABB {
    min : vec2f,
    max : vec2f
};

alias TileCoord = vec2u;

const N_LINES_PER_CHUNK: u32 = 32;

@compute @workgroup_size(32)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let tile_coord = global_id.xy;
    let line_chunk = global_id.z; 
    
    // For each line in our chunk, check whether it intersects the current
    // tile, and set the bit in the bins array to indicate whether there's
    // an intersection. 
    //
    // There are usually `N_LINES_PER_CHUNK` lines, but at the end there may be
    // fewer.
    var mask: u32 = 0;
    var bit: u32 = 1;
    for (
        var line_i: u32 = line_chunk * N_LINES_PER_CHUNK;
        line_i < max((line_chunk + 1) * N_LINES_PER_CHUNK, dimensions.n_lines);
        line_i = line_i + 1
    ) {
        mask |= bit * u32(line_intersects_tile(tile_coord, lines[line_i]));
        bit <<= 1;
    }
    
    bins[line_chunk] = mask;
}

/**** FUNCTIONS USING GLOBALS ****/

/// Checks if a line intersects a tile.
fn line_intersects_tile(tile: TileCoord, line: StyledLine) -> bool {
    // TODO: Move computation to a single location.
    // TODO: Use separating-axis test.
    
    // For now, we check if the line's bounding box intersects the tile.
    let tile_bb = tile_bound(tile);
    let line_bb = line_bound(line);
    return aabb_intersect(tile_bb, line_bb);
}

/// Computes the bounding box of a tile.
fn tile_bound(tile: TileCoord) -> AABB {
    let tile_sz = vec2u(dimensions.tile_width, dimensions.tile_height);
    let p_min = dot(tile, tile_sz);
    let p_max = p_min + tile_sz;
    return AABB(vec2f(p_min), vec2f(p_max));
}

/**** PURE FUNCTIONS ****/

/// Computes the axis-aligned bounding box of a line.
///
/// TODO: Incorporate end-cap styles into the bounding-box calculation.
fn line_bound(line: StyledLine) -> AABB {
    let rs = abs(line.width) / 2.0;
    let r = vec2f(rs, rs);
    return AABB(min(line.p0, line.p1) - r, max(line.p0, line.p1) + r);
}

/// Checks if two axis-aligned bounding-boxes intersect.
fn aabb_intersect(a: AABB, b: AABB) -> bool {
    let x_isect = interval_intersects(a.min.x, a.max.x, b.min.x, b.max.x);
    let y_isect = interval_intersects(a.min.y, a.max.y, b.min.y, b.max.y);
    return x_isect && y_isect; 
}

/// Checks if two intervals intersect.
fn interval_intersects(a_s: f32, a_e: f32, b_s: f32, b_e: f32) -> bool {
    return !(b_s > a_e || a_s > b_e);
}