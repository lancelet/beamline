//! Assign lines to tiles.

use crate::{bbox::Bbox, style::StyledLine, types::v2_rot90_anticlockwise, Line, Polygon, P2, V2};
use cgmath::num_traits::Euclid;
use itertools::Itertools;
use std::ops::Range;

/// Tiler: Assigns lines to a regular grid of tiles.
pub struct Tiler {
    area_width: u32,
    area_height: u32,
    tile_width: u32,
    tile_height: u32,
    n_x_tiles: u32,
    n_y_tiles: u32,
    /// Vector of tuples containing a linear tile index and a styled line
    /// that has been placed in that tile.
    lines: Vec<(usize, StyledLine)>,
}
impl Tiler {
    /// Creates a new `Tiler` for the specified area and tile sizes.
    pub fn new(area_width: u32, area_height: u32, tile_width: u32, tile_height: u32) -> Self {
        assert!(area_width > 0);
        assert!(area_height > 0);
        assert!(tile_width > 0);
        assert!(tile_height > 0);

        // Compute numbers of x and y tiles using a "ceiling" integer divide.
        let n_x_tiles = (area_width + tile_width - 1) / tile_width;
        let n_y_tiles = (area_height + tile_height - 1) / tile_height;
        let n_tiles = n_x_tiles * n_y_tiles;

        Tiler {
            area_width,
            area_height,
            tile_width,
            tile_height,
            n_x_tiles,
            n_y_tiles,
            lines: Vec::new(),
        }
    }

    /// Add a styled line to the tiler.
    ///
    /// This checks the line against the tiles and adds it into a list of
    /// line-tile allocations.
    pub fn add(&mut self, styled_line: StyledLine) {
        // Compute the bounding-polygon and bounding box of the line.
        // These include the line width and end style information.
        let bounding_polygon = styled_line.bounding_polygon();
        let bounding_box = bounding_polygon.bbox();

        // Find the tiles that the line's bounding box intersects.
        let opt_tiles_intersection =
            TilesIntersection::from_bbox(self.tile_width, self.tile_height, &bounding_box)
                .clip_to_area(self.n_x_tiles, self.n_y_tiles);
        let tiles_intersection = match opt_tiles_intersection {
            // If we clip the tiles intersection to the active area and we
            // find there's no intersection, then the line is not visible
            // and we don't have to do anything.
            None => return,
            Some(x) => x,
        };

        // For all tiles in the intersecting area, use a separating axis test
        // to see if each tile intersects the line.
        for tile_y in tiles_intersection.y_tiles() {
            for tile_x in tiles_intersection.x_tiles() {
                if self.tile_intersects_line(tile_x, tile_y, &styled_line.line, &bounding_polygon) {
                    self.lines
                        .push((self.tile_ix(tile_x, tile_y), styled_line.clone()))
                }
            }
        }
    }

    /// Collect all tiles and the lines they contain.
    ///
    /// This consumes the `Tiler`.
    ///
    /// It returns two components:
    ///
    /// 1. A vector of `StyledLine`, which is a list of lines organized
    ///    over the tiles.
    /// 2. A vector of `TileInfo`, which indicates, for each tile location,
    ///    the start index in the `StyledLine` vector and the number of
    ///    lines each tile contains.
    pub fn collect(self) -> (Vec<TileInfo>, Vec<StyledLine>) {
        // Sort the lines according to their linear index.
        let mut lines = self.lines;
        lines.sort_by_key(|(ix, _)| *ix);

        // Process the lines to find the tile offsets.
        let mut start_index: u32 = 0;
        let tile_infos = lines
            .iter()
            .map(|(ix, _)| *ix)
            .chunk_by(|ix| *ix)
            .into_iter()
            .map(|(lindex, chunk)| {
                // Find tile coordinates from linear index.
                let tile_y = (lindex / self.n_x_tiles as usize) as u32;
                let tile_x = (lindex % self.n_x_tiles as usize) as u32;

                let n_lines = chunk.count() as u32;
                let info = TileInfo {
                    tile_x,
                    tile_y,
                    start_index,
                    n_lines,
                };
                start_index += n_lines;

                info
            })
            .collect();

        // Create the vector of styled lines by dropping the linear index.
        let lines_vec: Vec<StyledLine> = lines.into_iter().map(|(_, line)| line).collect();

        (tile_infos, lines_vec)
    }

    /// Computes the linear index of a tile.
    ///
    /// # Parameters
    ///
    /// - `tile_x`: The horizontal tile index.
    /// - `tile_y`: The vertical tile index.
    ///
    /// # Returns
    ///
    /// A linear index of the tile into `self.tiles`.
    fn tile_ix(&self, tile_x: u32, tile_y: u32) -> usize {
        assert!(tile_x < self.n_x_tiles);
        assert!(tile_y < self.n_y_tiles);
        let ix = self.n_x_tiles as usize * tile_y as usize + tile_x as usize;
        ix
    }

    /// Check if a tile intersects a supplied line.
    ///
    /// # Parameters
    ///
    /// - `tile_x`: X coordinate of a tile.
    /// - `tile_y`: Y coordinate of a tile.
    /// - `line`: the line to check.
    /// - `polygon`: the bounding polygon around the line.
    ///
    /// # Returns
    ///
    /// `true` if the tile intersects the line, `false` otherwise.
    fn tile_intersects_line(
        &self,
        tile_x: u32,
        tile_y: u32,
        line: &Line,
        polygon: &Polygon,
    ) -> bool {
        // Compute the test vectors we need for a separating axis test. There
        // are only 4 of them for a line. This means we do half the work of a
        // naive separating axis test.
        let test_axes = vec![
            line.ab_vec(),
            v2_rot90_anticlockwise(line.ab_vec()),
            V2::new(1.0, 0.0),
            V2::new(0.0, 1.0),
        ];
        let center = Some(P2::new(0.0, 0.0));
        let tile = self.tile_polygon(tile_x, tile_y);

        for axis in test_axes {
            if polygon.is_separating_axis(&tile, axis, center) {
                return false;
            }
        }
        true
    }

    /// Returns a polygon representing a tile.
    fn tile_polygon(&self, tile_x: u32, tile_y: u32) -> Polygon {
        let twf = self.tile_width as f32;
        let thf = self.tile_height as f32;
        let min_x = twf * tile_x as f32;
        let max_x = min_x + twf;
        let min_y = thf * tile_y as f32;
        let max_y = min_y + thf;
        Polygon::new(vec![
            P2::new(min_x, min_y),
            P2::new(max_x, min_y),
            P2::new(max_y, max_y),
            P2::new(min_x, max_y),
        ])
    }
}

pub struct TileInfo {
    pub tile_x: u32,
    pub tile_y: u32,
    pub start_index: u32,
    pub n_lines: u32,
}

/// Represents the intersection of something (usually a bounding box) with
/// the tile indices.
struct TilesIntersection {
    min_x_tile: u32,
    max_x_tile: u32,
    min_y_tile: u32,
    max_y_tile: u32,
}
impl TilesIntersection {
    /// Construct a tile intersection with a bounding box.
    pub fn from_bbox(tile_width: u32, tile_height: u32, bbox: &Bbox) -> Self {
        let twf = tile_width as f32;
        let thf = tile_height as f32;

        let min_x_tile = (bbox.min_x() / twf) as u32;
        let max_x_tile = (bbox.max_x() / twf) as u32;
        let min_y_tile = (bbox.min_y() / thf) as u32;
        let max_y_tile = (bbox.max_y() / thf) as u32;

        Self {
            min_x_tile,
            max_x_tile,
            min_y_tile,
            max_y_tile,
        }
    }

    /// Clips a `TilesIntersection` to a area of tiles.
    ///
    /// If the `TilesIntersection` does not intersect the area at all, `None`
    /// is returned.
    pub fn clip_to_area(&self, n_x_tiles: u32, n_y_tiles: u32) -> Option<Self> {
        if self.min_x_tile >= n_x_tiles || self.min_y_tile >= n_y_tiles {
            None
        } else {
            Some(TilesIntersection {
                min_x_tile: self.min_x_tile,
                max_x_tile: self.max_x_tile.min(n_x_tiles),
                min_y_tile: self.min_y_tile,
                max_y_tile: self.max_y_tile.min(n_y_tiles),
            })
        }
    }

    /// Returns a range for the x (horizontal) tiles.
    pub fn x_tiles(&self) -> Range<u32> {
        self.min_x_tile..self.max_x_tile
    }

    /// Returns a range for the y (vertical) tiles.
    pub fn y_tiles(&self) -> Range<u32> {
        self.min_y_tile..self.max_y_tile
    }
}
