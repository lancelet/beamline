use std::collections::HashMap;

use cgmath::{InnerSpace, Point2, Vector2};

/// 2D point type.
type P2 = Point2<f32>;
/// 2D vector type.
type V2 = Vector2<f32>;

pub struct Bucketer {
    /// Width of the screen.
    screen_width: u32,
    /// Height of the screen.
    screen_height: u32,
    /// Width of a single bucket.
    bucket_width: u32,
    /// Height of a single bucket.
    bucket_height: u32,
    /// Map from bucket (x,y) coords to the lines they contain.
    buckets: HashMap<(u32, u32), Vec<Line>>,
}
impl Bucketer {
    /// Create a new Bucketer.
    pub fn new(
        screen_width: u32,
        screen_height: u32,
        bucket_width: u32,
        bucket_height: u32,
    ) -> Self {
        Bucketer {
            screen_width,
            screen_height,
            bucket_width,
            bucket_height,
            buckets: HashMap::new(),
        }
    }

    /// Return all the buckets in the `Bucketer` as an iterator of bucket
    /// coordinate to the lines it contains.
    fn buckets(&self) -> impl Iterator<Item = (&(u32, u32), &Vec<Line>)> {
        self.buckets.iter()
    }

    /// Bucket a line.
    ///
    /// This splits the supplied `line` up into small chunks that are
    /// approximately the size of a cell. Then all chunks which intersect
    /// each cell are added to the buckets.
    ///
    /// # Parameters
    ///
    /// - `line`: Line to add to buckets.
    pub fn add_line(&mut self, line: Line) {
        // These could be pre-computed.
        let max_x = (self.screen_width as f32 / self.bucket_width as f32).ceil() as u32;
        let max_y = (self.screen_height as f32 / self.bucket_height as f32).ceil() as u32;

        let min_edge = self.bucket_width.min(self.bucket_height) as f32;
        for sub_line in line.split(min_edge) {
            let mut intersection = sub_line
                .bound()
                .grid_intersect(self.bucket_width as f32, self.bucket_height as f32);

            if intersection.min_x > max_x || intersection.min_y > max_y {
                continue;
            }
            if intersection.max_x > max_x {
                intersection.max_x = max_x;
            }
            if intersection.max_y > max_y {
                intersection.max_y = max_y;
            }

            for cell_y in intersection.min_y..=intersection.max_y {
                for cell_x in intersection.min_x..=intersection.max_x {
                    self.add_line_to_cell((cell_x, cell_y), sub_line.clone());
                }
            }
        }
    }

    /// Add a line to a cell.
    ///
    /// # Parameters
    ///
    /// - `cell`: Cell to which the line should be added.
    /// - `line`: The line to add to the cell.
    fn add_line_to_cell(&mut self, cell: (u32, u32), line: Line) {
        match self.buckets.get_mut(&cell) {
            None => {
                self.buckets.insert(cell, vec![line]);
            }
            Some(existing_vec) => {
                existing_vec.push(line);
            }
        }
    }
}

/// Describes the intersection of an [`AABB`] with a regular grid.
pub struct GridIntersection {
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
}

/// Axis-aligned bounding box.
pub struct AABB {
    /// Minimum value.
    min: P2,
    /// Maximum value.
    max: P2,
}
impl AABB {
    /// Create a new axis-aligned bounding box to encompass all supplied points.
    ///
    /// # Parameters
    ///
    /// - `pts`: Iterator of points.
    ///
    /// # Returns
    ///
    /// - `None`: if the iterator is empty.
    /// - `Some(_)`: if the iterator contains at least one point.
    pub fn all(mut pts: impl Iterator<Item = P2>) -> Option<AABB> {
        let mut min: P2 = P2::new(0.0, 0.0);
        let mut max: P2 = P2::new(0.0, 0.0);

        match pts.next() {
            None => None,
            Some(p) => {
                min = p;
                max = p;
                for p in pts {
                    if p.x < min.x {
                        min.x = p.x;
                    } else if p.x > max.x {
                        max.x = p.x;
                    }
                    if p.y < min.y {
                        min.y = p.y;
                    } else if p.y > max.y {
                        max.y = p.y;
                    }
                }
                Some(AABB { min, max })
            }
        }
    }

    /// Intersect an axis-aligned bounding box with a regular grid.
    ///
    /// The grid has lines that pass through the origin and a fixed cell size.
    ///
    /// # Parameters
    ///
    /// - `cell_size_x`: Size of the grid cells along the x direction.
    /// - `cell_size_y`: Size of the grid cells along the y direction.
    ///
    /// # Returns
    ///
    /// Intersection rectangle, describing which cells (inclusive) the
    /// axis-aligned bounding box intersects.
    pub fn grid_intersect(&self, cell_size_x: f32, cell_size_y: f32) -> GridIntersection {
        let min_x = (self.min.x / cell_size_x).floor().max(0.0) as u32;
        let max_x = (self.max.x / cell_size_x).ceil().max(0.0) as u32;
        let min_y = (self.min.y / cell_size_y).floor().max(0.0) as u32;
        let max_y = (self.max.y / cell_size_y).ceil().max(0.0) as u32;
        GridIntersection {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    /// Start coordinate of the line.
    start: P2,
    /// End coordinate of the line.
    end: P2,
    /// Width of the line.
    width: f32,
}
impl Line {
    /// Split a line into segments of a given maximum length.
    ///
    /// # Parameters
    ///
    /// - `length`: the maximum length of a line segment.
    ///
    /// # Returns
    ///
    /// An iterator of lines.
    pub fn split(&self, length: f32) -> impl Iterator<Item = Line> {
        let v = self.end - self.start;
        let line_len = v.magnitude();
        let n_steps = (line_len / length).ceil() as usize;
        let dt = (1.0 / (n_steps as f64)) as f32;
        let dv = v / dt;

        LineSplitter {
            p: self.start,
            t: 0.0,
            dv,
            dt,
            width: self.width,
        }
    }

    pub fn bound(&self) -> AABB {
        // Tangent vector.
        let vt = (self.end - self.start).normalize();
        // Perpendicular vector.
        let vp = V2::new(vt.y, vt.x);
        // Perpendicular vector scaled to half width;
        let vpp = vp * (self.width / 2.0);

        // Expand both ends of the line to include all points at the corners
        // of the rectangular shape it becomes when the width is included.
        AABB::all(
            vec![
                self.start + vpp,
                self.start - vpp,
                self.end + vpp,
                self.end - vpp,
            ]
            .into_iter(),
        )
        .unwrap()
    }
}

/// Iterator that can split a line into sections.
///
/// See [`Line::split`], which produces this iterator.
pub struct LineSplitter {
    /// Current point.
    p: P2,
    /// Current parameter value in the range `[0.0, 1.0]`.
    t: f32,
    /// Vector step along the line direction. This is a vector along the
    /// direction of the line that corresponds to an increment of `dt` in the
    /// line's scalar parameter.
    dv: V2,
    /// Step along the scalar parameter. This is an increment of the line's
    /// parameter that corresponds to a step of `dv` along the line.
    dt: f32,
    /// Width of the line.
    width: f32,
}
impl Iterator for LineSplitter {
    type Item = Line;

    fn next(&mut self) -> Option<Self::Item> {
        if self.t >= 1.0 {
            None
        } else {
            let next_t = self.t + self.dt;
            let next_p = if next_t <= 1.0 {
                // Most of the time we avoid any division, and just increment
                // along the line by a fixed amount.
                self.p + self.dv
            } else {
                // Division may be required in the last segment to avoid going
                // past the end.
                let frac = (1.0 - self.t) / (next_t - 1.0);
                self.p + frac * self.dv
            };

            let line = Line {
                start: self.p,
                end: next_p,
                width: self.width,
            };
            self.t = next_t;
            self.p = next_p;
            Some(line)
        }
    }
}
