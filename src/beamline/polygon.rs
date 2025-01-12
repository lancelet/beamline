//! 2D polygons.

use kiddo::{KdTree, SquaredEuclidean};

use super::{types::P2, Line};

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
        dbg!(pt_test);
        dbg!(edge_test);

        !(pt_test || edge_test)
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
fn points_coincident<'a>(min_dist: f32, mut points: impl Iterator<Item = &'a P2>) -> bool {
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

    /// A square should be a simple polygon.
    #[test]
    fn test_square_is_simple_polygon() {
        let square = Polygon::new(vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.0, 1.0),
        ]);
        assert!(square.is_simple(1e-3))
    }

    // A "bowtie" should not be a simple polygon.
    #[test]
    fn test_bowtie_is_not_simple_polygon() {
        let bowtie = Polygon::new(vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(0.0, 1.0),
            P2::new(1.0, 1.0),
        ]);
        assert!(!bowtie.is_simple(1e-3))
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
}
