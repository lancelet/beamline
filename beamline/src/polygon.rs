//! 2D polygons.

use super::{types::P2, Line};
use crate::V2;
use crate::{bbox::Bbox, interval::Interval};
use cgmath::{EuclideanSpace, InnerSpace};
use kiddo::{KdTree, SquaredEuclidean};

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

    /// Check if this `Polygon` is a simple polygon.
    ///
    /// A simple polygon has no duplicate vertices, and no edges that intersect
    /// one another.
    ///
    /// # Parameters
    ///
    /// - `min_dist` distance below which vertices of the polygon are assumed
    ///   to be coincident.
    ///
    /// # Returns
    ///
    /// - `true` if the `Polygon` is a simple polygon.
    pub fn is_simple(&self, min_dist: f32) -> bool {
        let pt_test = points_coincident(min_dist, self.vertices.iter());
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
    /// - `None`: if the vertices are approximately collinear, and no winding
    ///   direction is defined.
    /// - `Some(direction)`: if the winding direction is defined.
    pub fn winding_direction(&self, vertex: usize) -> Option<WindingDirection> {
        let n = self.vertices.len();
        assert!(vertex < n);

        // Find indices of the previous, current and next vertices. The
        // previous and next indices wrap around the polygon ends.
        let i_prev = if vertex == 0 { n - 1 } else { vertex - 1 };
        let i_curr = vertex;
        let i_next = (vertex + 1) % n;

        // Previous point, current point and next point.
        let p0 = self.vertices[i_prev];
        let p1 = self.vertices[i_curr];
        let p2 = self.vertices[i_next];

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

    /// Checks if this `Polygon` is convex.
    ///
    /// A convex polygon has all angles turning in the same direction. A
    /// convexity test only makes sense for a simple
    /// (ie. non-self-intersecting) polygon, so this function assumes that the
    /// polygon is known to be simple.
    pub fn is_convex(&self) -> bool {
        assert!(
            self.is_simple(f32::EPSILON),
            "Polygon is non-simple, so convexity is not defined."
        );
        all_equal((0..self.vertices.len()).filter_map(|vertex| self.winding_direction(vertex)))
    }

    /// Computes the centroid of a `Polygon`.
    ///
    /// The polygon must be a simple polygon for this equation to be correct.
    pub fn centroid(&self) -> P2 {
        assert!(self.is_simple(f32::EPSILON));

        let n = self.vertices.len();
        let mut sa: f32 = 0.0; // signed area
        let mut cx: f32 = 0.0;
        let mut cy: f32 = 0.0;
        for i in 0..n {
            let j = if i == n - 1 { 0 } else { i + 1 };
            let pi = self.vertices[i];
            let pj = self.vertices[j];

            let z = pi.x * pj.y - pj.x * pi.y;
            sa += z;
            cx += (pi.x + pj.x) * z;
            cy += (pi.y + pj.y) * z;
        }

        sa /= 2.0;
        cx /= 6.0 * sa;
        cy /= 6.0 * sa;

        P2::new(cx, cy)
    }

    /// Checks if this `Polygon` intersects another polygon.
    ///
    /// Both polygons are assumed to be simple and convex. This uses a
    /// separating axis test.
    /*
    pub fn intersects_convex(&self, other: &Polygon) -> bool {
        assert!(self.is_simple(f32::EPSILON) && self.is_convex());
        assert!(other.is_simple(f32::EPSILON) && other.is_convex());

        // Find a point to use as a center for the separating axis test.
        // We choose the average of the centroid of the two polygons.
        let center = (self.centroid() + other.centroid().to_vec()) / 2.0;

        // Test all axes of both polygons to see if they are separating
        // axes.
        let edges = self.edges().chain(other.edges());
        for edge in edges {
            let axis = edge.ab_vec();
            if self.is_separating_axis(other, axis, Some(center)) {
                return false;
            }
        }
        true
    }
    */

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
    /// - `opt_center`: Optional center to use as a reference for projection.
    ///   If this is `None` then the center is chosen as a point which is the
    ///   average of the centroid of the two polygons.
    ///
    /// # Returns
    ///
    /// `true` if the supplied axis was a separating axis, `false` otherwise.
    pub fn is_separating_axis(&self, other: &Polygon, axis: V2, opt_center: Option<P2>) -> bool {
        assert!(self.is_simple(f32::EPSILON) && self.is_convex());
        assert!(other.is_simple(f32::EPSILON) && other.is_convex());

        // The center to use. If `opt_center` is provided, we use that;
        // otherwise we choose a point which is the average of the centroid
        // of the two polygons.
        let center =
            opt_center.unwrap_or_else(|| (self.centroid() + other.centroid().to_vec()) / 2.0);

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
///
/// If the iterator is empty, this returns `true`.
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
    use crate::{assert_close, compare::Tol};
    use proptest::prelude::*;

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

    /// A "bowtie" should not be a simple polygon.
    #[test]
    fn test_bowtie_is_not_simple_polygon() {
        assert!(!bowtie().is_simple(1e-3))
    }

    /// A polygon with coincident points should not be a simple polygon.
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

    /// A square should be convex.
    #[test]
    fn test_square_is_convex() {
        assert!(square().is_convex())
    }

    /// A non-convex polygon should be non-convex.
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

    /*

    /// Test whether two non-intersecting polygons intersect.
    ///
    /// These two polygons have intersecting bounding boxes, but do not
    /// intersect.
    #[test]
    fn test_intersects_convex_example_1() {
        let a = Polygon::new(vec![
            P2::new(4.0, 0.0),
            P2::new(7.0, 0.0),
            P2::new(7.0, 3.0),
            P2::new(4.0, 3.0),
        ]);
        let b = Polygon::new(vec![
            P2::new(1.0, 1.0),
            P2::new(6.0, 6.0),
            P2::new(5.0, 7.0),
            P2::new(0.0, 2.0),
        ]);

        assert!(!a.intersects_convex(&b));
    }

    /// Test whether two intersecting polygons intersect.
    ///
    /// These two polygons do intersect.
    #[test]
    fn test_intersects_convex_example_2() {
        let a = Polygon::new(vec![
            P2::new(4.0, 0.0),
            P2::new(7.0, 0.0),
            P2::new(7.0, 3.0),
            P2::new(4.0, 3.0),
        ]);
        let b = Polygon::new(vec![
            P2::new(3.0, 1.0),
            P2::new(8.0, 6.0),
            P2::new(7.0, 7.0),
            P2::new(2.0, 2.0),
        ]);

        assert!(a.intersects_convex(&b));
    }

    /// Test whether a polygon completely contained within another intersects.
    #[test]
    fn test_intersects_convex_example_3() {
        let a = Polygon::new(vec![
            P2::new(0.0, 0.0),
            P2::new(3.0, 0.0),
            P2::new(3.0, 3.0),
            P2::new(0.0, 3.0),
        ]);
        let b = Polygon::new(vec![
            P2::new(1.0, 1.0),
            P2::new(2.0, 1.0),
            P2::new(2.0, 2.0),
            P2::new(1.0, 2.0),
        ]);

        assert!(a.intersects_convex(&b))
    }

    */

    proptest! {
        /// The centroid of a right triangle is one-third the distance along
        /// its edges from the right-angled corner.
        #[test]
        fn test_right_triangle_centroid(w in 0.5f32..10.0, h in 0.5f32..10.0) {
            let right_triangle = Polygon::new(vec![
                P2::new(0.0, 0.0),
                P2::new(w, 0.0),
                P2::new(0.0, h)
            ]);
            let expected_centroid = P2::new(w/3.0, h/3.0);
            let centroid = right_triangle.centroid();

            let tol = Tol::default().scale(1e1);
            assert_close!(tol, centroid, expected_centroid);
        }

        /// The centroid of a rectangle is half way along its edges.
        #[test]
        fn test_rectangle_centroid(w in 0.5f32..10.0, h in 0.5f32..10.0) {
            let rectangle = Polygon::new(vec![
                P2::new(0.0, 0.0),
                P2::new(w, 0.0),
                P2::new(w, h),
                P2::new(0.0, h)
            ]);
            let expected_centroid = P2::new(w/2.0, h/2.0);
            let centroid = rectangle.centroid();

            let tol = Tol::default().scale(1e1);
            assert_close!(tol, centroid, expected_centroid);
        }
    }
}
