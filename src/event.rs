use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::app::AppResult;

/// Terminal events.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// Key press.
    Key(KeyEvent),
    /// Mouse click/scroll.
    Mouse(MouseEvent),
    /// Terminal resize.
    Resize(u16, u16),
    /// Loosing focus, doesnt' happen however :(
    FocusLost,
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
    /// Event handler thread.
    handler: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    pub fn new(use_mouse: bool) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let handler = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            loop {
                let event = tokio::select! {
                    _ = sender.closed() => break,
                    event = reader.next() => event,
                };
                let Some(Ok(event)) = event else {
                    break;
                };
                let event = match event {
                    CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Event::Key(key))
                    }
                    CrosstermEvent::Mouse(mouse) if use_mouse => Some(Event::Mouse(mouse)),
                    CrosstermEvent::Resize(x, y) => Some(Event::Resize(x, y)),
                    CrosstermEvent::FocusLost => Some(Event::FocusLost),
                    _ => None,
                };
                if let Some(event) = event {
                    if sender.send(event).is_err() {
                        break;
                    }
                }
            }
        });
        Self { receiver, handler }
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub async fn next(&mut self) -> AppResult<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| std::io::Error::other("terminal event stream closed").into())
    }
}

impl Drop for EventHandler {
    fn drop(&mut self) {
        self.handler.abort();
    }
}
