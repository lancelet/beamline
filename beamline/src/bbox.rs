use crate::{interval::Interval, P2};

/// Axis-aligned bounding box.
struct Bbox {
    x_interval: Interval,
    y_interval: Interval,
}

impl Bbox {
    /// Creates a singleton bounding box containing just one point.
    fn singleton(point: P2) -> Self {
        Bbox {
            x_interval: Interval::singleton(point.x),
            y_interval: Interval::singleton(point.y),
        }
    }

    /// Expands a bounding box, if necessary to include a point.
    fn include(&mut self, point: P2) {
        self.x_interval.include(point.x);
        self.y_interval.include(point.y);
    }

    /// Tests whether this bounding box contains a point.
    fn contains(&self, point: P2) -> bool {
        self.x_interval.contains(point.x) && self.y_interval.contains(point.y)
    }

    /// Tests whether this bounding box overlaps another bounding box.
    fn overlaps(&self, other: &Bbox) -> bool {
        self.x_interval.overlaps(&other.x_interval) && self.y_interval.overlaps(&other.y_interval)
    }

    /// Tests whether this bounding box is disjoint from another bounding box.
    fn disjoint(&self, other: &Bbox) -> bool {
        !self.overlaps(other)
    }
}
