use pitwall_collector::{assets::dashboard_asset, paths::AppPaths};

fn main() -> std::io::Result<()> {
    let paths = AppPaths::resolve()?;
    let index = dashboard_asset("index.html").expect("dashboard entry point is embedded");
    println!(
        "pitwall collector scaffold: embedded dashboard entry ({} bytes); data directory: {}",
        index.len(),
        paths.root.display()
    );
    Ok(())
}
