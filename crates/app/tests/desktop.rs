//! Single instance and first run, against a temporary data folder.
use pitwall_core::paths::AppPaths;
use pitwall_native::desktop::{FirstRun, Instance, Launch};

fn temp(name: &str) -> (std::path::PathBuf, AppPaths) {
    let dir = std::env::temp_dir().join(format!("pitwall-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    (dir.clone(), AppPaths::from_data_dir(&dir))
}

#[test]
fn second_launch_requests_the_existing_dashboard_without_taking_the_lock() {
    let (dir, paths) = temp("instance");
    let Launch::Primary(primary) = Instance::acquire(&paths).unwrap() else {
        panic!("first launch owns the data folder");
    };
    assert!(!primary.take_open_request());
    assert!(matches!(
        Instance::acquire(&paths).unwrap(),
        Launch::Existing
    ));
    assert!(
        primary.take_open_request(),
        "second launch asks for a dashboard"
    );
    assert!(
        !primary.take_open_request(),
        "one request opens one dashboard"
    );

    // Another data folder is independent.
    let (other_dir, other) = temp("instance-other");
    assert!(matches!(
        Instance::acquire(&other).unwrap(),
        Launch::Primary(_)
    ));
    drop(primary);
    assert!(matches!(
        Instance::acquire(&paths).unwrap(),
        Launch::Primary(_)
    ));
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(other_dir);
}

#[test]
fn first_run_lasts_until_a_dashboard_opens_successfully() {
    let (dir, paths) = temp("first-run");
    let first = FirstRun::new(&paths);
    assert!(first.pending());
    // A failed open records nothing, so the next start tries again.
    assert!(FirstRun::new(&paths).pending());
    first.opened().unwrap();
    assert!(!first.pending());
    assert!(!FirstRun::new(&paths).pending());
    let _ = std::fs::remove_dir_all(dir);
}
