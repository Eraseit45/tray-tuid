use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::{io, path::PathBuf};
use tray_tui::{daemon, protocol::default_socket_path};

#[derive(Debug, Parser)]
#[command(version, about = "Persistent system tray service for tray-tui")]
struct Cli {
    /// Unix socket path (default: $XDG_RUNTIME_DIR/tray-tui.sock)
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
    /// Generate shell completions
    #[arg(long, value_enum)]
    completions: Option<Shell>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();
    if let Some(shell) = cli.completions {
        generate(shell, &mut Cli::command(), "tray-tuid", &mut io::stdout());
        return Ok(());
    }
    SimpleLogger::init(
        if cli.debug {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        },
        Config::default(),
    )
    .map_err(io::Error::other)?;
    let path = cli.socket.map(Ok).unwrap_or_else(default_socket_path)?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = daemon::run(&path) => result,
        result = tokio::signal::ctrl_c() => result,
        _ = terminate.recv() => Ok(()),
    }
}
