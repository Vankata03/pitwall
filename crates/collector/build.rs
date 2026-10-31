use std::{
    hash::{Hash, Hasher},
    path::Path,
};
fn hash_tree(path: &Path, hash: &mut impl Hasher) {
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for entry in entries {
        entry.to_string_lossy().hash(hash);
        if entry.is_dir() {
            hash_tree(&entry, hash);
        } else {
            std::fs::read(entry).unwrap().hash(hash);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src");
    let dashboard = Path::new("../../dashboard/dist");
    println!("cargo:rerun-if-changed={}", dashboard.display());
    assert!(
        dashboard.join("index.html").is_file(),
        "dashboard build missing: run `pnpm install --frozen-lockfile` and `pnpm build` at the repository root before Cargo"
    );
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    hash_tree(Path::new("src"), &mut hash);
    hash_tree(Path::new("../../dashboard/dist"), &mut hash);
    println!("cargo:rustc-env=PITWALL_BUILD_ID={:016x}", hash.finish());
}
