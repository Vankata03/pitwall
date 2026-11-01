use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=PITWALL_BUILD_ID");
    let dashboard = Path::new("../../dashboard/dist");
    println!("cargo:rerun-if-changed={}", dashboard.display());
    assert!(
        dashboard.join("index.html").is_file(),
        "dashboard build missing: run `pnpm install --frozen-lockfile` and `pnpm build` at the repository root before Cargo"
    );
    let id = std::env::var("PITWALL_BUILD_ID").unwrap_or_else(|_| "dev".into());
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        assert!(
            !id.trim().is_empty() && id != "dev",
            "release requires a real PITWALL_BUILD_ID"
        );
        assert_eq!(
            std::fs::read_to_string(dashboard.join("build-id.txt")).unwrap_or_default(),
            id,
            "dashboard and collector build IDs differ; rebuild dashboard with PITWALL_BUILD_ID"
        );
    }
    println!("cargo:rustc-env=PITWALL_BUILD_ID={id}");
}
