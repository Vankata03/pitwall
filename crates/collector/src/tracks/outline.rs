//! Bucketed outline: one X/Z point per 8 m of lap distance.

use super::{BUCKET_METRES, OutlinePoint};

/// The largest uncovered stretch a complete map may have, in metres.
pub(super) const MAX_GAP_METRES: f32 = 40.0;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Outline {
    buckets: Vec<Option<[f32; 2]>>,
}

impl Outline {
    pub(super) fn empty(track_length: u16) -> Self {
        let count = (f32::from(track_length) / BUCKET_METRES).ceil() as usize;
        Self {
            buckets: vec![None; count],
        }
    }

    pub(super) fn from_points(track_length: u16, points: &[OutlinePoint]) -> Self {
        let mut outline = Self::empty(track_length);
        for point in points {
            let bucket = point.bucket as usize;
            if bucket >= outline.buckets.len() {
                outline.buckets.resize(bucket + 1, None);
            }
            outline.buckets[bucket] = Some([point.x, point.z]);
        }
        outline
    }

    pub(super) fn points(&self) -> Vec<OutlinePoint> {
        self.buckets
            .iter()
            .enumerate()
            .filter_map(|(bucket, at)| {
                at.map(|[x, z]| OutlinePoint {
                    bucket: bucket as u32,
                    x,
                    z,
                })
            })
            .collect()
    }

    pub(super) fn bucket_of(&self, lap_distance: f32) -> Option<usize> {
        let bucket = (lap_distance / BUCKET_METRES).floor() as usize;
        (lap_distance >= 0.0 && bucket < self.buckets.len()).then_some(bucket)
    }

    /// Record a sample's position if its bucket is empty. Returns whether it was new.
    pub(super) fn fill(&mut self, bucket: usize, at: [f32; 2]) -> bool {
        let slot = &mut self.buckets[bucket];
        if slot.is_some() {
            return false;
        }
        *slot = Some(at);
        true
    }

    /// Every stretch of the lap is covered: no two neighbouring filled buckets, counting
    /// across the start line, are more than 40 m apart.
    pub(super) fn is_complete(&self) -> bool {
        let filled: Vec<usize> = self
            .buckets
            .iter()
            .enumerate()
            .filter_map(|(bucket, at)| at.map(|_| bucket))
            .collect();
        let (Some(&first), Some(&last)) = (filled.first(), filled.last()) else {
            return false;
        };
        let widest = filled
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .chain([first + self.buckets.len() - last])
            .max()
            .unwrap_or(0);
        widest as f32 * BUCKET_METRES <= MAX_GAP_METRES
    }

    /// Distance from `at` to the stored line near `bucket`, or `None` when nothing is
    /// stored there. The line is read across the nearby filled buckets, wrapping at the start.
    pub(super) fn distance_to_line(&self, bucket: usize, at: [f32; 2]) -> Option<f32> {
        const WINDOW: isize = 6;
        let count = self.buckets.len() as isize;
        let near: Vec<[f32; 2]> = (-WINDOW..=WINDOW)
            .filter_map(|offset| {
                self.buckets[(bucket as isize + offset).rem_euclid(count) as usize]
            })
            .collect();
        match near.as_slice() {
            [] => None,
            [only] => Some(point_distance(*only, at)),
            points => points
                .windows(2)
                .map(|pair| segment_distance(pair[0], pair[1], at))
                .reduce(f32::min),
        }
    }
}

fn point_distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn segment_distance(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length_squared = dx * dx + dz * dz;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length_squared).clamp(0.0, 1.0)
    };
    point_distance([a[0] + t * dx, a[1] + t * dz], p)
}
