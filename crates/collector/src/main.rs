use pitwall_collector::{
    assets::dashboard_asset,
    ingest::config::{Cli, HELP},
    ingest::runtime,
    paths::AppPaths,
};

fn main() -> std::io::Result<()> {
    let cli = Cli::parse(std::env::args_os().skip(1)).map_err(std::io::Error::other)?;
    if cli.help {
        println!("{HELP}");
        return Ok(());
    }
    let index = dashboard_asset("index.html").expect("dashboard entry point is embedded");
    if cli.console {
        eprintln!("Embedded dashboard entry: {} bytes", index.len());
    }
    runtime::run(cli, AppPaths::resolve()?.config)
}
