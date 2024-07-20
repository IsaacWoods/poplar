/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use core::ops::Range;

pub trait RangeIntersect: Sized {
    /// Returns `true` if all values in `other` are within `self`.
    fn encompasses(&self, other: Self) -> bool;

    fn intersects(&self, other: Self) -> bool;
    fn intersection(&self, other: Self) -> Option<Self>;

    /// Split `self` into three ranges: the portion before `other`, the intersection, and the portion after `other`.
    fn split(&self, other: Self) -> (Option<Self>, Option<Self>, Option<Self>);
}

impl<T> RangeIntersect for Range<T>
where
    T: Clone + Copy + Ord,
{
    fn encompasses(&self, other: Self) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    fn intersects(&self, other: Self) -> bool {
        self.start < other.end && self.end > other.start
    }

    fn intersection(&self, other: Self) -> Option<Self> {
        use core::cmp::{max, min};
        if self.intersects(other.clone()) {
            Some(max(self.start, other.start)..min(self.end, other.end))
        } else {
            None
        }
    }

    fn split(&self, other: Self) -> (Option<Self>, Option<Self>, Option<Self>) {
        let before = if self.start >= other.start { None } else { Some(self.start..other.start) };
        let middle = self.intersection(other.clone());
        let after = if self.end <= other.end { None } else { Some(other.end..self.end) };

        (before, middle, after)
    }
}

#[cfg(test)]
mod tests {
    use super::RangeIntersect;

    #[test]
    fn intersect() {
        assert!((0..3).intersects(2..6));
        assert!(!(0..4).intersects(4..6));
        assert!(!(4..6).intersects(0..4));
        assert!((4..6).intersects(0..5));
        assert!((5..7).intersects(4..9));
    }

    #[test]
    fn intersection() {
        assert_eq!((0..1000).intersection(100..300), Some(100..300));
        assert_eq!((0..1000).intersection(500..1500), Some(500..1000));
        assert_eq!((500..1500).intersection(0..600), Some(500..600));
        assert_eq!((0..500).intersection(800..1000), None);
    }

    #[test]
    fn split() {
        /*
         * Test normal cases - `other` is fully within `self`, or at the very start or end.
         */
        assert_eq!((0..1000).split(50..100), (Some(0..50), Some(50..100), Some(100..1000)));
        assert_eq!((0..1000).split(0..100), (None, Some(0..100), Some(100..1000)));
        assert_eq!((0..1000).split(900..1000), (Some(0..900), Some(900..1000), None));

        /*
         * Test the case of `other` partially intersecting `self`.
         */
        assert_eq!((100..1000).split(0..300), (None, Some(100..300), Some(300..1000)));

        /*
         * Test the case of `self` being contained within `other`.
         */
        assert_eq!((100..300).split(0..1000), (None, Some(100..300), None));

        /*
         * Test the case of `self` being equal to `other`.
         */
        assert_eq!((100..300).split(100..300), (None, Some(100..300), None));
    }
}
