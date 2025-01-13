use crate::compare::close;
use crate::compare::close_default_tol;
use crate::compare::CloseCmp;
use crate::compare::Tol;
use crate::P2;
use crate::V2;

/// Line.
///
/// To construct a line, use [`Line::new`].
#[derive(Debug, Clone)]
pub struct Line {
    /// Start point of the line.
    a: P2,
    /// End point of the line.
    b: P2,
}
impl Line {
    /// Create a new line from two points.
    ///
    /// The points must not (approximately) lie on top of each other, otherwise
    /// a degenerate line is produced.
    ///
    /// # Parameters
    ///
    /// - `a`: An end-point of the line.
    /// - `b`: The other end-point of the line.
    ///
    /// # Returns
    ///
    /// A new line.
    pub fn new(a: P2, b: P2) -> Line {
        assert!(
            !close_default_tol(&a, &b),
            "Degenerate line: points are too close: ({:?}, {:?})",
            a,
            b
        );
        Line { a, b }
    }

    /// Returns the start point of the line.
    pub fn start(&self) -> P2 {
        self.a
    }

    /// Returns the end point of the line.
    pub fn end(&self) -> P2 {
        self.b
    }

    /// Return the vector along the line.
    ///
    /// The returned vector starts from the beginning of the line, and is the
    /// length of the line, in the direction of the end of the line.
    pub fn ab_vec(&self) -> V2 {
        self.b - self.a
    }

    /// Evaluate the line at parameter value `t`.
    ///
    /// `t` is a parameter which is in the range `[0.0, 1.0]` for the line.
    /// A value of `t=0` corresponds to the point `a` at the start of the line,
    /// while a value of `t=1` corresponds to the point `b` a the end of the
    /// line. `t` can take values outside this range to obtain collinear points
    /// that are on the same line but outside the start-end range.
    ///
    /// # Parameters
    ///
    /// - `t`: Parameter value at which to evaluate the line.
    ///
    /// # Returns
    ///
    /// Point of the line at `t`.
    pub fn eval_param(&self, t: f32) -> P2 {
        self.a + t * self.ab_vec()
    }

    /// Find the intersection point of two lines.
    ///
    /// # Parameters
    ///
    /// - `line`: A second line to try to intersect this one with.
    ///
    /// # Returns
    ///
    /// An intersection point, if one exists.
    pub fn intersection(&self, line: &Line) -> Option<P2> {
        let v1 = self.ab_vec();
        let v2 = line.ab_vec();

        // Finding the intersection point is constructed here as the problem of
        // finding the two linear parameters, t1 and t2, which parameterise all
        // points on the line. We have the restriction that for points on the
        // line:
        //
        // t1 E [0.0, 1.0]
        // t2 E [0.0, 1.0]
        //
        // The problem is constructed as solving a 2x2 linear system.

        // Find the matrix determinant.
        let det = -v1.x * v2.y + v2.x * v1.y;
        let c = 1.0 / det;
        // If the determinant is too small, then c can be NaN.
        if c.is_nan() {
            return None;
        }

        // Linear solution.
        let dx = line.a.x - self.a.x;
        let dy = line.a.y - self.a.y;
        let t1 = c * (-v2.y * dx + v2.x * dy);
        let t2 = c * (-v1.y * dx + v1.x * dy);

        // Check that the parameters are in range.
        if (0.0 <= t1) && (t1 <= 1.0) && (0.0 <= t2) && (t2 <= 1.0) {
            Some(self.eval_param(t1))
        } else {
            None
        }
    }
}

impl CloseCmp for Line {
    type Scalar = f32;
    /// Lines are considered close in a way that ignores the ordering of the
    /// two ends.
    fn close(tol: Tol<Self::Scalar>, a: &Self, b: &Self) -> bool {
        (close(tol, &a.a, &b.a) && close(tol, &a.b, &b.b))
            || (close(tol, &a.a, &b.b) && close(tol, &a.b, &b.a))
    }
}

#[cfg(test)]
mod tests {
    use crate::assert_close;

    use super::*;
    use cgmath::InnerSpace;
    use proptest::prelude::*;

    /// Construct a line from components.
    /// Used in testing for succinctness.
    fn lc(ax: f32, ay: f32, bx: f32, by: f32) -> Line {
        let a = P2::new(ax, ay);
        let b = P2::new(bx, by);
        Line::new(a, b)
    }

    /// Assert than a line intersection exists.
    fn intersection_exists(line1: Line, line2: Line, ix: f32, iy: f32) {
        let expected = P2::new(ix, iy);
        let p_intersect = line1.intersection(&line2);
        assert_close!(p_intersect, Some(expected));
    }

    /// Assert that no line intersection exists.
    fn no_intersection_exists(line1: Line, line2: Line) {
        let p_intersect = line1.intersection(&line2);
        assert_eq!(p_intersect, None);
    }

    /// Test intersecting pairs of lines where an intersection is known to
    /// exist.
    #[test]
    fn test_line_intersections_exist() {
        intersection_exists(lc(0.0, 1.0, 6.0, 5.0), lc(2.0, 6.0, 4.0, 0.0), 3.0, 3.0);
        intersection_exists(lc(1.0, 0.0, 0.0, 0.0), lc(0.0, 0.0, 0.0, -1.0), 0.0, 0.0);
        intersection_exists(lc(0.0, 1.0, 0.0, -1.0), lc(-1.0, 0.0, 0.0, 0.0), 0.0, 0.0);
        intersection_exists(
            lc(0.0, 26774.988, 0.0, -50091.824),
            lc(-48912.94, 0.0, 0.0, 0.0),
            0.0,
            0.0,
        );
    }

    /// Test intersecting pairs of lines where there is no intersection.
    #[test]
    fn test_line_intersection_does_not_exist_1() {
        no_intersection_exists(lc(2.0, 0.0, 0.0, 6.0), lc(2.0, 6.0, 4.0, 0.0));
        no_intersection_exists(lc(1.0, 0.0, 1.0, 1.0), lc(0.0, 1.0, 0.0, 0.0));
    }

    /// Intersecting pair of lines.
    ///
    /// This should be constructed such that the lines are intersecting.
    #[derive(Debug)]
    struct IntersectingLinePair {
        intersection: P2,
        line1: Line,
        line2: Line,
    }
    impl IntersectingLinePair {
        /// Construct a new intersecting line pair.
        ///
        /// # Parameters
        ///
        /// - `intersection`: the guaranteed point of intersection
        /// - `v1`: vector along the direction of the first line
        /// - `c11`: positive value scaling `v1` from the point of intersection
        /// - `c12`: positive value scaling `v1` from the point of intersection
        /// - `c21`: positive value scaling `v2` from the point of intersection
        /// - `c22`: positive value scaling `v2` from the point of intersection
        fn new(
            intersection: P2,
            v1: V2,
            c11: f32,
            c12: f32,
            v2: V2,
            c21: f32,
            c22: f32,
        ) -> Option<Self> {
            const MAG2_LIMIT: f32 = 0.2; // Min v1, v2 length.
            const PARALLEL_LIMIT: f32 = 0.95; // Limit of |dot(|v1|, |v2|)|.
            const MIN_LEN: f32 = 0.2; // Min line length.
            const MIN_OFS: f32 = 0.01; // Min cij length.

            // Check that offset lengths are OK.
            if c11 < MIN_OFS || c12 < MIN_OFS || c21 < MIN_OFS || c22 < MIN_OFS {
                return None;
            }

            // Check that the two vectors are non-zero length.
            if v1.magnitude2() < MAG2_LIMIT || v2.magnitude2() < MAG2_LIMIT {
                return None;
            }

            let v1n = v1.normalize();
            let v2n = v2.normalize();
            // Check that the two vectors are not parallel.
            if v1n.dot(v2n).abs() > PARALLEL_LIMIT {
                return None;
            }

            let a1 = intersection - c11 * v1n;
            let b1 = intersection + c12 * v1n;
            // Check that line1 points are not coincident.
            if (a1 - b1).magnitude() < MIN_LEN {
                return None;
            }

            let a2 = intersection - c21 * v2n;
            let b2 = intersection + c22 * v2n;
            // Check that line2 points are not coincident.
            if (a2 - b2).magnitude() < MIN_LEN {
                return None;
            }

            let line1 = Line::new(a1, b1);
            let line2 = Line::new(a2, b2);

            Some(IntersectingLinePair {
                intersection,
                line1,
                line2,
            })
        }
    }
    impl Arbitrary for IntersectingLinePair {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;
        fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
            let r: f32 = 10.0;
            let q = -r..=r;
            let c = 0.01..r;
            (
                q.clone(), // ix
                q.clone(), // iy
                q.clone(), // v1x
                q.clone(), // v1y
                c.clone(), // c11
                c.clone(), // c12
                q.clone(), // v2x
                q.clone(), // v2y
                c.clone(), // c21
                c.clone(), // c22
            )
                .prop_filter_map(
                    "avoid degenerate cases",
                    |(ix, iy, v1x, v1y, c11, c12, v2x, v2y, c21, c22)| {
                        let px = P2::new(ix, iy);
                        let v1 = V2::new(v1x, v1y);
                        let v2 = V2::new(v2x, v2y);

                        IntersectingLinePair::new(px, v1, c11, c12, v2, c21, c22)
                    },
                )
                .boxed()
        }
    }

    proptest! {
        #[test]
        fn test_intersecting_lines_intersect(ilp: IntersectingLinePair) {
            let tol = Tol::default().scale(1e2);
            let opt_intersection = ilp.line1.intersection(&ilp.line2);
            assert_close!(tol, opt_intersection, Some(ilp.intersection));
        }
    }
}
