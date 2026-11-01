//! Isolated production host for the browser harness. No user data is accessed.
use pitwall_collector::{
    ingest::{config::Cli, runtime},
    paths::AppPaths,
};
fn main() -> std::io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let root = std::path::PathBuf::from(args.next().expect("data directory"));
    let paths = AppPaths::from_data_dir(&root);
    let cli = Cli::parse(args).map_err(std::io::Error::other)?;
    runtime::run_with_paths(cli, paths.config.clone(), paths)
}
