use crate::{
    protocol::{MenuItem, TrayItem, TrayItems, TrayMenu},
    server::{self, Action},
};
use std::{
    io,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use system_tray::{
    client::{ActivateRequest, Client},
    menu,
};
use tokio::{
    net::UnixListener,
    sync::{broadcast, mpsc, watch},
    task::JoinSet,
};

struct SocketGuard {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl SocketGuard {
    fn bind(path: &Path) -> io::Result<(UnixListener, Self)> {
        let listener = UnixListener::bind(path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot bind {}: {error}", path.display()),
            )
        })?;
        let metadata = std::fs::symlink_metadata(path)?;
        let guard = Self {
            path: path.into(),
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok((listener, guard))
    }
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path) {
            if metadata.dev() == self.device && metadata.ino() == self.inode {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

pub async fn run(path: &Path) -> io::Result<()> {
    let (listener, _socket) = SocketGuard::bind(path)?;
    let client = Arc::new(Client::new().await.map_err(io::Error::other)?);
    let mut events = client.subscribe();
    let (state_tx, state_rx) = watch::channel(Arc::new(snapshot(&client)?));
    let (actions_tx, mut actions_rx) = mpsc::channel::<Action>(32);
    let mut workers = JoinSet::new();

    let collector = client.clone();
    workers.spawn(async move {
        loop {
            match events.recv().await {
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {
                    state_tx.send_replace(Arc::new(snapshot(&collector)?));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(io::Error::other("tray event stream closed"))
                }
            }
        }
    });
    workers.spawn(async move {
        while let Some(action) = actions_rx.recv().await {
            let result = match tokio::time::timeout(
                Duration::from_secs(5),
                activate(&client, &action),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err("tray action timed out".into()),
            };
            let _ = action.reply.send(result);
        }
        Err(io::Error::other("tray action queue closed"))
    });

    log::info!("tray-tuid listening on {}", path.display());
    tokio::select! {
        result = server::serve(listener, state_rx, actions_tx) => result,
        result = workers.join_next() => match result {
            Some(Ok(result)) => result,
            Some(Err(error)) => Err(io::Error::other(error)),
            None => Err(io::Error::other("tray workers stopped")),
        },
    }
}

fn snapshot(client: &Client) -> io::Result<TrayItems> {
    let items = client.items();
    let items = items
        .lock()
        .map_err(|_| io::Error::other("tray state lock poisoned"))?;
    Ok(items
        .iter()
        .map(|(key, (item, menu))| {
            let title = item
                .title
                .as_ref()
                .filter(|title| !title.is_empty())
                .or_else(|| item.tool_tip.as_ref().map(|tooltip| &tooltip.title))
                .unwrap_or(&item.id)
                .clone();
            let menu = menu.as_ref().map(|menu| TrayMenu {
                submenus: convert_menu(&menu.submenus),
            });
            (key.clone(), TrayItem { title, menu })
        })
        .collect())
}

fn convert_menu(items: &[menu::MenuItem]) -> Vec<MenuItem> {
    items
        .iter()
        .map(|item| MenuItem {
            id: item.id,
            label: item.label.clone(),
            enabled: item.enabled,
            visible: item.visible,
            submenu: convert_menu(&item.submenu),
        })
        .collect()
}

async fn activate(client: &Client, action: &Action) -> Result<(), String> {
    // Resolve the application's stable menu ID against its current menu, not a UI index.
    let path = {
        let items = client.items();
        let items = items.lock().map_err(|_| "tray state lock poisoned")?;
        let (item, menu) = items.get(&action.key).ok_or("tray item no longer exists")?;
        let menu = menu.as_ref().ok_or("tray item has no menu")?;
        let entry = find_menu_item(&menu.submenus, action.menu_item_id)
            .ok_or("menu item no longer exists")?;
        if !entry.enabled || !entry.visible || !entry.submenu.is_empty() {
            return Err("menu item cannot be activated".into());
        }
        item.menu.clone().ok_or("tray item has no menu path")?
    };
    client
        .activate(ActivateRequest::MenuItem {
            address: action.key.clone(),
            menu_path: path.clone(),
            submenu_id: action.menu_item_id,
        })
        .await
        .map_err(|error| error.to_string())?;
    // Preserve the existing post-action root-menu refresh.
    let _ = client
        .about_to_show_menuitem(action.key.clone(), path, 0)
        .await;
    Ok(())
}

fn find_menu_item(items: &[menu::MenuItem], id: i32) -> Option<&menu::MenuItem> {
    items.iter().find_map(|item| {
        if item.id == id {
            Some(item)
        } else {
            find_menu_item(&item.submenu, id)
        }
    })
}
