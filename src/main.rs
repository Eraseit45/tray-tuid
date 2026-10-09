use std::{fs::File, io};

use crate::{
    app::{App, AppResult},
    cli::Cli,
    config::Config,
    event::{Event, EventHandler},
    handler::{handle_key_events, handle_mouse_event},
    tui::Tui,
};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use ratatui::{backend::CrosstermBackend, Terminal};
use simplelog::{CombinedLogger, Config as Conf, LevelFilter, WriteLogger};

use tray_tui::{
    client::{Client, Receiver},
    protocol::{default_socket_path, ServerMessage},
};

pub mod app;
pub mod cli;
pub mod config;
pub mod event;
pub mod handler;
pub mod tui;
pub mod ui;
pub mod wrappers;

static CMD: &str = "tray-tui";

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();

    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        let mut out = io::stdout();
        generate(shell, &mut cmd, CMD, &mut out);
        return Ok(());
    }

    if cli.debug {
        CombinedLogger::init(vec![WriteLogger::new(
            LevelFilter::Debug,
            Conf::default(),
            File::create("app.log").unwrap(),
        )])
        .unwrap();
    }

    let config = Config::new(&cli.config_path)?;

    let socket = cli.socket.map(Ok).unwrap_or_else(default_socket_path)?;
    let (client, mut tray_rx, items) = Client::connect(&socket).await?;
    log::info!("Connected to tray-tuid");

    // Create an application.
    let mut app = App::new(client, items, config);
    app.update();

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(app.config.mouse);
    let mut tui = Tui::new(terminal, events);
    tui.init()?;
    log::info!("Initialized TUI");

    let result = run(&mut app, &mut tui, &mut tray_rx).await;
    let exit = tui.exit();
    result?;
    exit?;
    Ok(())
}

async fn run(
    app: &mut App,
    tui: &mut Tui<CrosstermBackend<io::Stdout>>,
    tray_rx: &mut Receiver,
) -> AppResult<()> {
    while app.running {
        tui.draw(app)?;
        tokio::select! {
            message = tray_rx.receive::<ServerMessage>() => {
                let message = message?;
                log::debug!("Daemon message: {message:?}");
                match message {
                    ServerMessage::Upsert { key, item } => { app.items.insert(key, item); }
                    ServerMessage::Remove { key } => { app.items.remove(&key); }
                    ServerMessage::ActionResult { error, .. } => { app.last_error = error; }
                    ServerMessage::Snapshot { .. } => {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected repeated snapshot").into());
                    }
                }
                app.update();
            }

            event = tui.events.next() => {
                let event = event?;
                log::debug!("Key event: {:?}", &event);
                match event {
                    Event::Key(key_event) => handle_key_events(key_event, app).await?,
                    Event::Mouse(mouse_event) => {
                        handle_mouse_event(mouse_event, app).await?
                    },
                    Event::Resize(_, _) => {tui.draw(app)?;}
                    Event::FocusLost => {
                        // doensn't work for some reason
                    }
                }
            }
        };
    }

    log::info!("Exiting application");
    Ok(())
}
