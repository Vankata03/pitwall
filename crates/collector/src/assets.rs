use include_dir::{Dir, include_dir};

static DASHBOARD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../dashboard/dist");

/// Look up a built dashboard asset by its relative, slash-separated path.
/// Assets are compiled into the collector in both debug and release builds.
pub fn dashboard_asset(path: &str) -> Option<&'static [u8]> {
    DASHBOARD.get_file(path).map(|file| file.contents())
}
