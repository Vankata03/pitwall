use pitwall_core::paths::AppPaths;
use std::path::PathBuf;

#[test]
fn all_persistent_files_share_the_per_user_pitwall_directory() {
    let data_dir = PathBuf::from("user data");
    let paths = AppPaths::from_data_dir(&data_dir);
    let root = data_dir.join("pitwall");

    assert_eq!(paths.root, root);
    assert_eq!(paths.config, root.join("config.toml"));
    assert_eq!(paths.layouts, root.join("layouts.json"));
    assert_eq!(paths.recordings, root.join("recordings"));
    assert_eq!(paths.tracks, root.join("tracks"));
    assert_eq!(paths.liveries, root.join("liveries"));
    assert_eq!(paths.logs, root.join("logs"));
}

#[test]
fn resolves_the_current_users_platform_directory() {
    let paths = AppPaths::resolve().expect("the current user has a data directory");

    #[cfg(target_os = "macos")]
    let expected = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/pitwall");
    #[cfg(target_os = "windows")]
    let expected = PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("pitwall");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let expected = dirs::data_dir().unwrap().join("pitwall");

    assert_eq!(paths.root, expected);
}
