use crate::protocol::{send_message, ClientMessage, MessageReader, ServerMessage, TrayItems};
use std::{io, sync::Arc};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{mpsc, oneshot, watch},
    task::JoinSet,
};

pub struct Action {
    pub key: String,
    pub menu_item_id: i32,
    pub reply: oneshot::Sender<Result<(), String>>,
}

/// Serve independent frontends without tying collection to their lifetimes.
pub async fn serve(
    listener: UnixListener,
    state: watch::Receiver<Arc<TrayItems>>,
    actions: mpsc::Sender<Action>,
) -> io::Result<()> {
    let mut clients = JoinSet::new();
    loop {
        tokio::select! {
            connection = listener.accept() => {
                let (stream, _) = connection?;
                let state = state.clone();
                let actions = actions.clone();
                clients.spawn(async move { serve_client(stream, state, actions).await });
            }
            Some(result) = clients.join_next(), if !clients.is_empty() => {
                if let Err(error) = result {
                    log::warn!("frontend task failed: {error}");
                } else if let Ok(Err(error)) = result {
                    log::debug!("frontend disconnected: {error}");
                }
            }
        }
    }
}

async fn serve_client(
    stream: UnixStream,
    mut state: watch::Receiver<Arc<TrayItems>>,
    actions: mpsc::Sender<Action>,
) -> io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = MessageReader::new(reader);
    // Subscribe before reading the snapshot, so changes during its write remain pending.
    let mut previous = state.borrow_and_update().clone();
    send_message(
        &mut writer,
        &ServerMessage::Snapshot {
            items: (*previous).clone(),
        },
    )
    .await?;
    loop {
        tokio::select! {
            changed = state.changed() => {
                changed.map_err(|_| io::Error::other("tray state publisher stopped"))?;
                let current = state.borrow_and_update().clone();
                for key in previous.keys().filter(|key| !current.contains_key(*key)) {
                    send_message(&mut writer, &ServerMessage::Remove { key: key.clone() }).await?;
                }
                for (key, item) in current.iter() {
                    if previous.get(key) != Some(item) {
                        send_message(&mut writer, &ServerMessage::Upsert { key: key.clone(), item: item.clone() }).await?;
                    }
                }
                previous = current;
            }
            message = reader.receive::<ClientMessage>() => {
                let ClientMessage::Activate { request_id, key, menu_item_id } = message?;
                let (reply, result) = oneshot::channel();
                actions.send(Action { key, menu_item_id, reply }).await
                    .map_err(|_| io::Error::other("tray action handler stopped"))?;
                let error = result.await
                    .map_err(|_| io::Error::other("tray action handler stopped"))?.err();
                send_message(&mut writer, &ServerMessage::ActionResult { request_id, error }).await?;
            }
        }
    }
}
