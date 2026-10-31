use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=PITWALL_BUILD_ID");
    let dashboard = Path::new("../../dashboard/dist");
    println!("cargo:rerun-if-changed={}", dashboard.display());
    assert!(
        dashboard.join("index.html").is_file(),
        "dashboard build missing: run `pnpm install --frozen-lockfile` and `pnpm build` at the repository root before Cargo"
    );
    println!(
        "cargo:rustc-env=PITWALL_BUILD_ID={}",
        std::env::var("PITWALL_BUILD_ID").unwrap_or_else(|_| "dev".into())
    );
}
