//! WebSocket connection for `BiDi` real-time events.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use url::Url;

use super::commands::{WebSocketCommand, WebSocketEvent, WebSocketResponse};
use crate::error::{WebDriverError, WebDriverResult};

/// WebSocket connection factory for `BiDi`.
#[derive(Debug, Clone)]
pub struct BiDiWebSocket {
    ws_url: Url,
    command_id: Arc<AtomicU64>,
}

impl BiDiWebSocket {
    /// Create a new WebSocket connection factory.
    pub fn new(ws_url: &str) -> WebDriverResult<Self> {
        let url = Url::parse(ws_url)
            .map_err(|e| WebDriverError::ParseError(format!("Invalid WebSocket URL: {e}")))?;
        Ok(Self {
            ws_url: url,
            command_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Connect to the WebSocket and return a connection handle.
    pub async fn connect(&self) -> WebDriverResult<BiDiConnection> {
        let (ws_stream, _) = connect_async(self.ws_url.as_str()).await.map_err(|e| {
            WebDriverError::ConnectionFailed(format!("WebSocket connection failed: {e}"))
        })?;

        let (write, read) = ws_stream.split();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        let write = Arc::new(tokio::sync::Mutex::new(write));
        let write_clone = write.clone();

        tokio::spawn(async move {
            let mut read = read;
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        let text_str = text.to_string();
                        if let Ok(event) = serde_json::from_str::<WebSocketEvent>(&text_str) {
                            let _ = event_tx.send(BiDiMessage::Event(event));
                        } else if let Ok(response) =
                            serde_json::from_str::<WebSocketResponse>(&text_str)
                        {
                            let _ = response_tx.send(BiDiMessage::Response(response));
                        }
                    }
                    Ok(WsMessage::Ping(data)) => {
                        let mut writer = write_clone.lock().await;
                        let _ = writer.send(WsMessage::Pong(data)).await;
                    }
                    Ok(WsMessage::Close(_)) => {
                        break;
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        });

        Ok(BiDiConnection {
            write,
            event_rx,
            response_rx,
            command_id: self.command_id.clone(),
        })
    }
}

/// A message received from the WebSocket.
#[derive(Debug)]
pub enum BiDiMessage {
    /// An event from `BiDi`.
    Event(WebSocketEvent),
    /// A response to a command.
    Response(WebSocketResponse),
}

/// Active WebSocket connection to `BiDi`.
pub struct BiDiConnection {
    write: Arc<
        tokio::sync::Mutex<
            futures_util::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                WsMessage,
            >,
        >,
    >,
    event_rx: mpsc::UnboundedReceiver<BiDiMessage>,
    response_rx: mpsc::UnboundedReceiver<BiDiMessage>,
    command_id: Arc<AtomicU64>,
}

impl fmt::Debug for BiDiConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BiDiConnection")
            .field("command_id", &self.command_id)
            .finish_non_exhaustive()
    }
}

impl BiDiConnection {
    fn next_command_id(&self) -> u64 {
        self.command_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Send a command to `BiDi`.
    pub async fn send_command(
        &self,
        method: impl Into<String>,
        params: serde_json::Value,
    ) -> WebDriverResult<u64> {
        let id = self.next_command_id();
        let command = WebSocketCommand::new(id, method, params);
        let json = command.to_json()?;

        let mut writer = self.write.lock().await;
        writer.send(WsMessage::Text(json.into())).await.map_err(|e| {
            WebDriverError::ConnectionFailed(format!("Failed to send command: {e}"))
        })?;

        Ok(id)
    }

    /// Subscribe to events.
    pub async fn subscribe(&self, events: &[&str]) -> WebDriverResult<u64> {
        let id = self.next_command_id();
        let command = WebSocketCommand::subscribe(id, events);
        let json = command.to_json()?;

        let mut writer = self.write.lock().await;
        writer
            .send(WsMessage::Text(json.into()))
            .await
            .map_err(|e| WebDriverError::ConnectionFailed(format!("Failed to subscribe: {e}")))?;

        Ok(id)
    }

    /// Unsubscribe from events.
    pub async fn unsubscribe(&self, events: &[&str]) -> WebDriverResult<u64> {
        let id = self.next_command_id();
        let command = WebSocketCommand::unsubscribe(id, events);
        let json = command.to_json()?;

        let mut writer = self.write.lock().await;
        writer
            .send(WsMessage::Text(json.into()))
            .await
            .map_err(|e| WebDriverError::ConnectionFailed(format!("Failed to unsubscribe: {e}")))?;

        Ok(id)
    }

    /// Receive the next event (blocks until an event arrives).
    pub async fn recv_event(&mut self) -> Option<WebSocketEvent> {
        loop {
            tokio::select! {
                msg = self.event_rx.recv() => {
                    if let Some(BiDiMessage::Event(event)) = msg {
                        return Some(event);
                    }
                }
                _ = self.response_rx.recv() => {}
            }
        }
    }

    /// Receive any message (event or response).
    pub async fn recv(&mut self) -> Option<BiDiMessage> {
        tokio::select! {
            msg = self.event_rx.recv() => msg,
            msg = self.response_rx.recv() => msg,
        }
    }

    /// Get a sender for external message injection.
    pub fn event_channel(&self) -> mpsc::UnboundedSender<BiDiMessage> {
        let (tx, _) = mpsc::unbounded_channel();
        tx
    }
}

/// Event listener wrapper for `BiDi` connections.
///
/// **Important:** The listener blocks while waiting for events.
/// Run on a separate thread to avoid blocking your main application.
#[derive(Debug)]
pub struct BiDiEventListener {
    connection: BiDiConnection,
}

impl BiDiEventListener {
    /// Create a new event listener from a connection.
    pub fn new(connection: BiDiConnection) -> Self {
        Self {
            connection,
        }
    }

    /// Subscribe to events.
    pub async fn subscribe(&self, events: &[&str]) -> WebDriverResult<u64> {
        self.connection.subscribe(events).await
    }

    /// Unsubscribe from events.
    pub async fn unsubscribe(&self, events: &[&str]) -> WebDriverResult<u64> {
        self.connection.unsubscribe(events).await
    }

    /// Send a command to `BiDi`.
    pub async fn send_command(
        &self,
        method: impl Into<String>,
        params: serde_json::Value,
    ) -> WebDriverResult<u64> {
        self.connection.send_command(method, params).await
    }

    /// Get the next event (blocks until an event arrives).
    pub async fn next_event(&mut self) -> Option<WebSocketEvent> {
        self.connection.recv_event().await
    }

    /// Listen for events and call the handler for each one.
    ///
    /// This method blocks indefinitely. Run on a separate thread.
    pub async fn listen<F>(&mut self, mut handler: F)
    where
        F: FnMut(WebSocketEvent),
    {
        while let Some(event) = self.next_event().await {
            handler(event);
        }
    }
}
