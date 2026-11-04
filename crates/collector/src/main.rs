#![cfg_attr(windows, windows_subsystem = "windows")]
use pitwall_collector::{
    assets::dashboard_asset,
    ingest::config::{Cli, HELP},
    paths::AppPaths,
};

fn main() -> std::io::Result<()> {
    let cli = Cli::parse(std::env::args_os().skip(1)).map_err(std::io::Error::other)?;
    #[cfg(windows)]
    if cli.console || cli.help {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AttachConsole(process: u32) -> i32;
            fn AllocConsole() -> i32;
        }
        unsafe {
            if AttachConsole(u32::MAX) == 0 {
                AllocConsole();
            }
        }
    }
    if cli.help {
        println!("{HELP}");
        return Ok(());
    }
    let index = dashboard_asset("index.html").expect("dashboard entry point is embedded");
    if cli.console {
        eprintln!("Embedded dashboard entry: {} bytes", index.len());
    }
    let paths = AppPaths::resolve_or(cli.data_dir.as_deref())?;
    pitwall_collector::app::run(cli, paths)
}
