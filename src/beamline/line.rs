use super::types::P2;
use super::types::V2;

/// Line.
///
/// To construct a line, use [`Line::new`].
pub struct Line {
    /// End point of the line.
    a: P2,
    /// Other end point of the line.
    b: P2,
}
impl Line {
    /// Create a new line from two points.
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
        Line { a, b }
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

        let v2yx = v2.y / v2.x;
        let t1 = ((line.a.y - self.a.y) + v2yx * (self.a.x - line.a.x)) / (v1.y - v1.x * v2yx);
        if t1 < 0.0 || t1 > 1.0 {
            return None;
        }
        let t2 = (self.a.x - line.a.x + t1 * v1.x) / v2.x;
        if t2 < 0.0 || t2 > 1.0 {
            return None;
        }

        Some(self.eval_param(t1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test intersecting two lines where an intersection is known to exist.
    #[test]
    fn test_line_intersection_exists() {
        let line1 = Line::new(P2::new(0.0, 1.0), P2::new(6.0, 5.0));
        let line2 = Line::new(P2::new(2.0, 6.0), P2::new(4.0, 0.0));

        let intersection = line1.intersection(&line2);
        let expected = P2::new(3.0, 3.0);
        assert_close!(intersection, Some(expected));
    }

    /// Test intersecting two lines where there is no intersection.
    #[test]
    fn test_line_intersection_does_not_exist() {
        let line1 = Line::new(P2::new(2.0, 0.0), P2::new(0.0, 6.0));
        let line2 = Line::new(P2::new(2.0, 6.0), P2::new(4.0, 0.0));

        let intersection = line1.intersection(&line2);
        assert_eq!(intersection, None);
    }
}
