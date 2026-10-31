//! Cached map files: one `<trackId>-<trackLength>.json` per map, no manifest.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};

use super::{OutlinePoint, TrackKey, outline::Outline};

/// Schema version written to every cached file. Any other version is ignored.
pub(super) const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct File {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    #[serde(rename = "trackId")]
    track_id: i8,
    #[serde(rename = "trackLength")]
    track_length: u16,
    outline: Vec<OutlinePoint>,
    pit: Option<Vec<[f32; 2]>>,
}

fn path(dir: &Path, key: TrackKey) -> std::path::PathBuf {
    dir.join(format!("{}-{}.json", key.track_id, key.track_length))
}

/// Read a cached map. A missing, unreadable, old-schema, mismatched or incomplete file
/// reads as `None`, and the map is relearned.
pub(super) fn read(dir: &Path, key: TrackKey) -> Option<(Outline, Option<Vec<[f32; 2]>>)> {
    let file: File = serde_json::from_slice(&fs::read(path(dir, key)).ok()?).ok()?;
    if file.schema_version != SCHEMA_VERSION
        || TrackKey::new(file.track_id, file.track_length) != key
    {
        return None;
    }
    let outline = Outline::from_points(key.track_length, &file.outline);
    outline.is_complete().then_some((outline, file.pit))
}

/// Write a map through a temporary file so a reader never sees half of one.
pub(super) fn write(
    dir: &Path,
    key: TrackKey,
    outline: &Outline,
    pit: &Option<Vec<[f32; 2]>>,
) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let file = File {
        schema_version: SCHEMA_VERSION,
        track_id: key.track_id,
        track_length: key.track_length,
        outline: outline.points(),
        pit: pit.clone(),
    };
    let target = path(dir, key);
    let temp = target.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec(&file).map_err(io::Error::other)?)?;
    fs::rename(temp, target)
}

/// Delete a map's cached file. Returns whether there was one.
pub(super) fn remove(dir: &Path, key: TrackKey) -> io::Result<bool> {
    match fs::remove_file(path(dir, key)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
