use crate::protocol::{send_message, ClientMessage, MessageReader, ServerMessage, TrayItems};
use std::{io, path::Path};
use tokio::net::{unix::OwnedReadHalf, unix::OwnedWriteHalf, UnixStream};

pub type Receiver = MessageReader<OwnedReadHalf>;

#[derive(Debug)]
pub struct Client {
    writer: OwnedWriteHalf,
    next_request_id: u64,
}

impl Client {
    pub async fn connect(path: &Path) -> io::Result<(Self, Receiver, TrayItems)> {
        let stream = UnixStream::connect(path).await.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "cannot connect to tray-tuid at {}: {error}; start tray-tuid first",
                    path.display()
                ),
            )
        })?;
        let (reader, writer) = stream.into_split();
        let mut receiver = Receiver::new(reader);
        let ServerMessage::Snapshot { items } = receiver.receive().await? else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected an initial tray snapshot",
            ));
        };
        Ok((
            Self {
                writer,
                next_request_id: 1,
            },
            receiver,
            items,
        ))
    }

    pub async fn activate(&mut self, key: String, menu_item_id: i32) -> io::Result<()> {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        send_message(
            &mut self.writer,
            &ClientMessage::Activate {
                request_id,
                key,
                menu_item_id,
            },
        )
        .await
    }
}
