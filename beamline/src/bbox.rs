use crate::{interval::Interval, P2};

/// Axis-aligned bounding box.
pub struct Bbox {
    x_interval: Interval,
    y_interval: Interval,
}

impl Bbox {
    /// Creates a bounding box containing all points from an iterator.
    ///
    /// If the iterator is empty, `None` is returned.
    pub fn including<'a>(mut points: impl Iterator<Item = &'a P2>) -> Option<Self> {
        match points.next() {
            None => None,
            Some(p0) => {
                let mut bbox = Bbox::singleton(*p0);
                for p in points {
                    bbox.include(*p)
                }
                Some(bbox)
            }
        }
    }

    /// Creates a singleton bounding box containing just one point.
    pub fn singleton(point: P2) -> Self {
        Bbox {
            x_interval: Interval::singleton(point.x),
            y_interval: Interval::singleton(point.y),
        }
    }

    /// Expands a bounding box, if necessary to include a point.
    pub fn include(&mut self, point: P2) {
        self.x_interval.include(point.x);
        self.y_interval.include(point.y);
    }

    /// Tests whether this bounding box contains a point.
    pub fn contains(&self, point: P2) -> bool {
        self.x_interval.contains(point.x) && self.y_interval.contains(point.y)
    }

    /// Tests whether this bounding box overlaps another bounding box.
    pub fn overlaps(&self, other: &Bbox) -> bool {
        self.x_interval.overlaps(&other.x_interval) && self.y_interval.overlaps(&other.y_interval)
    }

    /// Tests whether this bounding box is disjoint from another bounding box.
    pub fn disjoint(&self, other: &Bbox) -> bool {
        !self.overlaps(other)
    }

    /// Returns the minimum x value of the bounding box.
    pub fn min_x(&self) -> f32 {
        self.x_interval.min()
    }

    /// Returns the maximum x value of the bounding box.
    pub fn max_x(&self) -> f32 {
        self.x_interval.max()
    }

    /// Returns the minimum y value of the bounding box.
    pub fn min_y(&self) -> f32 {
        self.y_interval.min()
    }

    /// Returns the maximum y value of the bounding box.
    pub fn max_y(&self) -> f32 {
        self.y_interval.max()
    }
}
