//! 2D polygons.

use super::{types::P2, Line};
use crate::V2;
use kiddo::{KdTree, SquaredEuclidean};

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
    /// A new `Polygon`.
    pub fn new(points: Vec<P2>) -> Self {
        assert!(points.len() >= 3);
        Polygon { points }
    }

    /// Construct all edges of the polygon.
    ///
    /// This returns an iterator which will produce all the lines that are the
    /// edges of the polygon, drawn between its pairs of points.
    pub fn edges(&self) -> impl Iterator<Item = Line> + use<'_> {
        self.points
            .iter()
            .zip(self.points.iter().skip(1).chain(self.points.first()))
            .map(|(a, b)| Line::new(a.clone(), b.clone()))
    }

    /// Check if this `Polygon` is a simple polygon.
    ///
    /// A simple polygon has no duplicate vertices, and no edges that intersect
    /// one another.
    ///
    /// # Parameters
    ///
    /// - `min_dist` distance below which vertices of the polygon are assumed
    ///   to self-intersect.
    ///
    /// # Returns
    ///
    /// - `true` if the `Polygon` is a simple polygon.
    pub fn is_simple(&self, min_dist: f32) -> bool {
        let pt_test = points_coincident(min_dist, self.points.iter());
        let edge_test = non_adjacent_edges_intersect(&self);
        !(pt_test || edge_test)
    }

    /// Return the winding direction at a vertex.
    ///
    /// See [`WindingDirection`] for how the direction is defined.
    ///
    /// # Parameters
    ///
    /// - `vertex`: index of the vertex for which to compute the winding
    ///   direction.
    ///
    /// # Returns
    ///
    /// - `None`: if the points are approximately collinear, and no winding
    ///   direction is defined.
    /// - `Some(direction)`: if the winding direction is defined.
    pub fn winding_direction(&self, vertex: usize) -> Option<WindingDirection> {
        let n = self.points.len();
        assert!(vertex < n);

        // Find indices of the previous, current and next vertices. The
        // previous and next indices wrap around the polygon ends.
        let i_prev = if vertex == 0 { n - 1 } else { vertex - 1 };
        let i_curr = vertex;
        let i_next = (vertex + 1) % n;

        // Previous point, current point and next point.
        let p0 = self.points[i_prev];
        let p1 = self.points[i_curr];
        let p2 = self.points[i_next];

        // Compute vectors along the edges.
        let a = p1 - p0;
        let b = p2 - p1;

        // Compute 2D cross product.
        let cross = cross_product(&a, &b);

        // Ignore nearly-zero values, since they are approximately collinear.
        if cross.abs() < f32::EPSILON {
            None
        } else {
            let dir = if cross > 0.0 {
                WindingDirection::Anticlockwise
            } else {
                WindingDirection::Clockwise
            };
            Some(dir)
        }
    }

    /// Check if this `Polygon` is convex.
    ///
    /// A convex polygon has all angles turning in the same direction.
    pub fn is_convex(&self) -> bool {
        assert!(
            self.is_simple(f32::EPSILON),
            "Polygon is non-simple, so convexity is not defined."
        );
        all_equal((0..self.points.len()).filter_map(|vertex| self.winding_direction(vertex)))
    }
}

/// Winding direction.
///
/// Each vertex in the polygon has a winding direction. This is defined as
/// follows:
///
/// 1. Let `i` be the vertex of intered.
/// 2. Define `a` to be the vector from the previous vertex to the current
///    vertex: `a = points[i] - points[i-1]`
/// 3. Define `b` to be the vector from the current vertex to the next
///    vertex: `b = points[i+1] - points[i]`.
///
/// The rotation FROM `a` to get to `b` is either `Clockwise` or
/// `Anticlockwise`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindingDirection {
    /// Clockwise winding at the current vertex.
    Clockwise,
    /// Anticlockwise winding at the current vertex.
    Anticlockwise,
}

/// Return the 2D cross product: `a x b`.
fn cross_product(a: &V2, b: &V2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Check if all items in an iterator are equal.
fn all_equal<A: PartialEq>(mut iter: impl Iterator<Item = A>) -> bool {
    match iter.next() {
        None => true,
        Some(reference_item) => iter.all(|item| item == reference_item),
    }
}

/// Checks if any supplied points are coincident up to a supplied minimum
/// distance.
///
/// This places the points inside a [`KdTree`] to speed up checks.
///
/// # Parameters
///
/// - `min_dist`: the minimum allowable distance between points.
/// - `points`: an iterator of points.
///
/// # Returns
///
/// - `true` if at least two points lie within `min_dist` of each other
/// - `false` if there are no coincident points
fn points_coincident<'a>(min_dist: f32, points: impl Iterator<Item = &'a P2>) -> bool {
    let mut kd_tree: KdTree<f32, 2> = KdTree::new();

    // Check points against the tree.
    for p in points {
        if kd_tree
            .within_unsorted_iter::<SquaredEuclidean>(&[p.x, p.y], min_dist)
            .next()
            .is_none()
        {
            // The current point was not in the tree; place it in the tree.
            kd_tree.add(&[p.x, p.y], 0);
        } else {
            // The current point was in the tree; we found a coincident point.
            return true;
        }
    }

    // We added all the points to the KdTree, but found no coincident points.
    false
}

/// Checks if any non-adjacent edges of a polygon intersect.
fn non_adjacent_edges_intersect(polygon: &Polygon) -> bool {
    // Construct the full vector of edges in order.
    let edges: Vec<Line> = polygon.edges().collect();

    // Compare all pairs of edges, skipping adjacent edges.
    for i in 0..(edges.len() - 2) {
        for j in (i + 2)..edges.len() {
            if i == 0 && j == edges.len() - 1 {
                continue;
            }
            if edges[i].intersection(&edges[j]).is_some() {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square polygon.
    fn square() -> Polygon {
        Polygon::new(vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.0, 1.0),
        ])
    }

    /// A "bowtie" polygon.
    fn bowtie() -> Polygon {
        Polygon::new(vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(0.0, 1.0),
            P2::new(1.0, 1.0),
        ])
    }

    /// A square should be a simple polygon.
    #[test]
    fn test_square_is_simple_polygon() {
        assert!(square().is_simple(1e-3))
    }

    // A "bowtie" should not be a simple polygon.
    #[test]
    fn test_bowtie_is_not_simple_polygon() {
        assert!(!bowtie().is_simple(1e-3))
    }

    // A polygon with coincident points should not be a simple polygon.
    #[test]
    fn test_coincident_points_not_simple_polygon() {
        let coincident = Polygon::new(vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0001, 0.0),
            P2::new(0.0001, 0.0),
        ]);
        assert!(!coincident.is_simple(1e-3))
    }

    // A square should be convex.
    #[test]
    fn test_square_is_convex() {
        assert!(square().is_convex())
    }

    // A non-convex polygon should be non-convex.
    #[test]
    fn test_non_convex_is_not_convex() {
        let non_convex = Polygon::new(vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.5, 0.7),
            P2::new(0.0, 1.0),
        ]);
        assert!(!non_convex.is_convex())
    }
}
