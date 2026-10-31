use std::path::Path;

fn main() {
    let dashboard = Path::new("../../dashboard/dist");
    println!("cargo:rerun-if-changed={}", dashboard.display());
    assert!(
        dashboard.join("index.html").is_file(),
        "dashboard build missing: run `pnpm install --frozen-lockfile` and `pnpm build` at the repository root before Cargo"
    );
}
