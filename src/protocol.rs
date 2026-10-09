//! The local interface between tray-tuid and its frontends.
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io, path::PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines};

pub type TrayItems = HashMap<String, TrayItem>;

/// The data needed to render a tray entry. Raw D-Bus properties stay in the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayItem {
    pub title: String,
    pub menu: Option<TrayMenu>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayMenu {
    pub submenus: Vec<MenuItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuItem {
    pub id: i32,
    pub label: Option<String>,
    pub enabled: bool,
    pub visible: bool,
    pub submenu: Vec<MenuItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Activate {
        request_id: u64,
        key: String,
        menu_item_id: i32,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Snapshot {
        items: TrayItems,
    },
    Upsert {
        key: String,
        item: TrayItem,
    },
    Remove {
        key: String,
    },
    ActionResult {
        request_id: u64,
        error: Option<String>,
    },
}

pub fn default_socket_path() -> io::Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR is not set; specify --socket PATH"))?;
    Ok(PathBuf::from(runtime).join("tray-tui.sock"))
}

pub struct MessageReader<R> {
    lines: Lines<BufReader<R>>,
}

impl<R: AsyncRead + Unpin> MessageReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            lines: BufReader::new(reader).lines(),
        }
    }

    pub async fn receive<T: serde::de::DeserializeOwned>(&mut self) -> io::Result<T> {
        // next_line preserves partial messages when tokio::select! cancels this future.
        let line = self.lines.next_line().await?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "daemon connection closed")
        })?;
        serde_json::from_str(&line)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

pub async fn send_message<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(message).map_err(io::Error::other)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await
}
