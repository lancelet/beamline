# Rendering Plan: Lines

This is a plan for how to render lines on a GPU using a signed distance field
approach.

Ideally, we want to run the signed-distance-field computation on a small
number of lines per pixel, and avoid the computation completely in regions
where no lines are present. The renderer uses "tiles" for this. Tiles are a
fixed size (eg. 16x16 pixels), and have associated with them some set of lines
that intersect the tile.

In order to assign lines to tiles, the renderer starts with a compute stage.
Then the tiles are instanced, and the actual SDF line rendering is performed
in a fragment shader.

## Input Data

Lines are submitted to the renderer in a buffer containing a flat array.
The of lines should be a "push constant" for a compute shader.

Each line is something like this (in WGSL):

```
struct StyledLine {
    start   : vec2f,
    end     : vec2f,
    width   : f32,
    cap     : u32,
    color   : vec4f
};
```

## Tiling

### Shader 1: Primitive Intersection Bitmask Image Creation

This is a global binning buffer.

The first shader runs in parallel over the lines (ie. one work item per line).
It produces an array of bitmask images; one per line. Each bit of the bitmask
corresponds to a tile. A `1` means that the line intersects that tile, while
a `0` means that the line does not intersect that tile.

There is also a separate array containing line-counts for each tile. When a
line intersects a tile, it atomically increments the line count for that tile.

### Shader 2: Tile Allocation

This compute shader runs in parallel over the tiles to create an array of line
indices for each tile. This is possible since we know how many lines intersect
each tile.
