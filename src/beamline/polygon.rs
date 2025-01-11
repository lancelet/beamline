//! 2D polygons.

use super::types::P2;

/// Closed polygon.
///
/// A polygon contains an ordered sequence of points, which are connected by
/// straight lines. The last point of the polygon is also connected to the first
/// point by a straight line.
///
/// To construct a Polygon, use [`Polygon::new`].
pub struct Polygon {
    /// Points for the polygon.
    points: Vec<P2>,
}
impl Polygon {
    /// Creates a new `Polygon` from the given points.
    ///
    /// There must be at least three points in the polygon.
    ///
    /// # Parameters
    ///
    /// - `point`: The vector of points for the polygon. This vector must
    ///   contain at least 3 points.
    ///
    /// # Returns
    ///
    /// A new `Polygon`, if the vector of points is valid.
    pub fn new(points: Vec<P2>) -> Option<Self> {
        if (points.len() >= 3) {
            Some(Polygon { points })
        } else {
            None
        }
    }
}
