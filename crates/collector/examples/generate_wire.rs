fn main() {
    pitwall_collector::wire::generate_contract(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../dashboard/src/generated/wire"),
    )
    .unwrap();
}
