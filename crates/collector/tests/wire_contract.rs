#[test]
fn generated_wire_contract_is_current() {
    let temp = std::env::temp_dir().join(format!("pitwall-wire-contract-{}", std::process::id()));
    let checked =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dashboard/src/generated/wire");
    pitwall_collector::wire::generate_contract(&temp).unwrap();
    // A Windows checkout with core.autocrlf rewrites the committed LF files as CRLF.
    let read =
        |path: std::path::PathBuf| std::fs::read_to_string(path).map(|s| s.replace("\r\n", "\n"));
    for entry in std::fs::read_dir(&temp).unwrap() {
        let entry = entry.unwrap();
        assert_eq!(
            read(entry.path()).unwrap(),
            read(checked.join(entry.file_name())).expect("run cargo run --example generate_wire"),
            "contract changed: {:?}",
            entry.file_name()
        );
    }
    assert_eq!(
        std::fs::read_dir(&temp).unwrap().count(),
        std::fs::read_dir(&checked).unwrap().count()
    );
    std::fs::remove_dir_all(temp).unwrap();
}
