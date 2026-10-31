use pitwall_collector::{
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
    runtime::run(cli, AppPaths::resolve()?.config)
}
