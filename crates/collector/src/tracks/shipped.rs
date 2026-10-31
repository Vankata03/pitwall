//! The 22 embedded layouts, converted once by `tools/tracks/convert_maps.py`.

use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;

use super::{OutlinePoint, TrackKey};

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "trackId")]
    track_id: i8,
    #[serde(rename = "trackLength")]
    track_length: u16,
    points: Vec<OutlinePoint>,
}

static SHIPPED: OnceLock<HashMap<TrackKey, Vec<OutlinePoint>>> = OnceLock::new();

/// The shipped outline for a layout, if pitwall ships one.
pub(super) fn outline(key: TrackKey) -> Option<&'static [OutlinePoint]> {
    SHIPPED
        .get_or_init(|| {
            let entries: Vec<Entry> =
                serde_json::from_str(include_str!("../../assets/tracks.json"))
                    .expect("embedded track maps are valid");
            entries
                .into_iter()
                .map(|e| (TrackKey::new(e.track_id, e.track_length), e.points))
                .collect()
        })
        .get(&key)
        .map(Vec::as_slice)
}
