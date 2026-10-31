use pitwall_collector::assets::dashboard_asset;

#[test]
fn embedded_entry_point_and_its_scripts_ship_in_the_collector_binary() {
    let index = dashboard_asset("index.html").expect("built dashboard entry point");
    let html = std::str::from_utf8(index).unwrap();
    assert!(html.contains("<title>pitwall</title>"));

    let binary = std::fs::read(env!("CARGO_BIN_EXE_pitwall")).unwrap();
    assert!(binary.windows(index.len()).any(|bytes| bytes == index));

    let scripts: Vec<_> = html
        .split("src=\"")
        .skip(1)
        .map(|suffix| suffix.split('"').next().unwrap().trim_start_matches('/'))
        .collect();
    assert!(!scripts.is_empty(), "the React entry script must ship");
    for path in scripts {
        let script = dashboard_asset(path).expect("each script is embedded");
        assert!(!script.is_empty());
        assert!(binary.windows(script.len()).any(|bytes| bytes == script));
    }
    assert!(dashboard_asset("missing.js").is_none());
}

#[test]
fn collector_can_start_outside_the_source_directory() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pitwall"))
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("pitwall collector scaffold")
    );
}
