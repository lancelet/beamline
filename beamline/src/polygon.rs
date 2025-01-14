//! 2D polygons.

use super::{types::P2, Line};
use crate::V2;
use crate::{bbox::Bbox, interval::Interval};
use cgmath::InnerSpace;

/// Closed polygon.
///
/// A polygon contains an ordered sequence of points, which are connected by
/// straight lines. The last point of the polygon is also connected to the first
/// point by a straight line.
///
/// To construct a Polygon, use [`Polygon::new`].
pub struct Polygon {
    /// Vertices of the polygon.
    vertices: Vec<P2>,
}
impl Polygon {
    /// Creates a new `Polygon` from the given vertices.
    ///
    /// There must be at least three vertices in the polygon.
    ///
    /// # Parameters
    ///
    /// - `vertices`: The vector of vertices for the polygon. This vector must
    ///   contain at least 3 vertices.
    ///
    /// # Returns
    ///
    /// A new `Polygon`.
    pub fn new(vertices: Vec<P2>) -> Self {
        assert!(vertices.len() >= 3);
        Polygon { vertices }
    }

    /// Construct all edges of the polygon.
    ///
    /// This returns an iterator which will produce all the lines that are the
    /// edges of the polygon, drawn between its pairs of vertices.
    pub fn edges(&self) -> impl Iterator<Item = Line> + use<'_> {
        self.vertices
            .iter()
            .zip(self.vertices.iter().skip(1).chain(self.vertices.first()))
            .map(|(a, b)| Line::new(a.clone(), b.clone()))
    }

    /// Check if a supplied axis is a "separating axis" for two polygons.
    ///
    /// The separating axis test projects both polygons onto a line which is
    /// perpendicular to the supplied axis. Each polygon forms an interval
    /// when projected onto this line. If the intervals are disjoint then the
    /// supplied axis was a "separating axis".
    ///
    /// This test works for all simple, convex polygons.
    ///
    /// # Parameters
    ///
    /// - `other`: Other polygon in the test.
    /// - `axis`: Candidate separating axis.
    /// - `center`: Center to use as a reference for projection.
    ///
    /// # Returns
    ///
    /// `true` if the supplied axis was a separating axis, `false` otherwise.
    pub fn is_separating_axis(&self, other: &Polygon, axis: V2, center: P2) -> bool {
        // Produce a 90-degree rotation of the axis. This is a line onto
        // which we should project for the separating axis test.
        let direction = V2::new(-axis.y, axis.x);

        // Project both polygons onto the line formed by `center` and
        // `direction`.
        let interval_self = project_polygon_to_line(center, direction, self);
        let interval_other = project_polygon_to_line(center, direction, other);

        // If the intervals are disjoint then `axis` was a separating axis
        // for the two polygons.
        interval_self.disjoint(&interval_other)
    }

    /// Returns the axis-aligned bounding box of the polygon.
    pub fn bbox(&self) -> Bbox {
        Bbox::including(self.vertices.iter()).unwrap()
    }
}

/// Project a polygon to a line, producing an interval.
///
/// # Parameters
///
/// - `center`: A point on the line corresponding to the zero point of
///   projections.
/// - `direction`: Vector along the direction of the line corresponding to
///   positive values, and providing a scale.
/// - `polygon`: Polygon to project to the line.
fn project_polygon_to_line(center: P2, direction: V2, polygon: &Polygon) -> Interval {
    let mut p_iter = polygon.vertices.iter();
    let p_first = p_iter.next().unwrap(); // must be at least one point
    let mut interval = Interval::singleton((p_first - center).dot(direction));
    for p in p_iter {
        interval.include((p - center).dot(direction))
    }
    interval
}
