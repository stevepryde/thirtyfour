//! WebDriver BiDi bidirectional protocol support.
//!
//! Enable with the `bidi` cargo feature.
//!
//! # Overview
//!
//! BiDi (Bidirectional) protocol enables real-time event streaming from the browser
//! alongside traditional WebDriver commands. This module provides a robust implementation
//! with connection state tracking, configurable timeouts, and multiple event subscription
//! patterns.
//!
//! # Runtime Flexibility
//!
//! The BiDi session spawns a background task to dispatch incoming messages.
//! You control how this task runs:
//!
//! ## Option 1: Spawn the Future (Recommended)
//!
//! ```ignore
//! let bidi = BiDiSessionBuilder::new()
//!     .connect_with_driver(&driver)
//!     .await?;
//!
//! // Spawn the dispatch future on your runtime of choice
//! tokio::spawn(bidi.dispatch_future());
//! // or: async_std::spawn(bidi.dispatch_future());
//! // or: futures::executor::block_on(bidi.dispatch_future());
//! ```
//!
//! ## Option 2: Manual Polling
//!
//! ```ignore
//! let mut bidi = BiDiSessionBuilder::new()
//!     .connect_with_driver(&driver)
//!     .await?;
//!
//! // Poll manually in your own loop
//! loop {
//!     let more = bidi.poll_dispatch().await?;
//!     if !more {
//!         break; // Connection closed
//!     }
//! }
//! ```
//!
//! # Linking to WebDriver Lifecycle
//!
//! To automatically stop the BiDi dispatch when the WebDriver session ends,
//! monitor the connection state:
//!
//! ```ignore
//! let bidi = BiDiSessionBuilder::new()
//!     .connect_with_driver(&driver)
//!     .await?;
//!
//! let bidi_clone = bidi.clone();
//! tokio::spawn(async move {
//!     // This will end when the WebSocket closes
//!     bidi_clone.dispatch_future().await;
//! });
//!
//! // Later, when driver.quit() is called, the WebSocket will close
//! // and dispatch_future() will complete automatically.
//! ```
//!
//! # Timeout Configuration
//!
//! Network conditions can cause BiDi commands to hang. Always configure timeouts
//! for production use:
//!
//! ```ignore
//! use std::time::Duration;
//! use thirtyfour::extensions::bidi::BiDiSessionBuilder;
//!
//! let session = BiDiSessionBuilder::new()
//!     .command_timeout(Duration::from_secs(10))
//!     .event_channel_capacity(512)
//!     .connect_with_driver(&driver)
//!     .await?;
//! ```
//!
//! # Event Subscription Patterns
//!
//! ## 1. Unified Channel (All Events)
//!
//! Subscribe to all events through a single channel:
//!
//! ```ignore
//! let mut rx = session.subscribe_events();
//! while let Ok(event) = rx.recv().await {
//!     match event {
//!         BiDiEvent::Network(e) => { /* handle network event */ }
//!         BiDiEvent::Log(e) => { /* handle log event */ }
//!         BiDiEvent::ConnectionClosed => break,
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## 2. Typed Channels (Domain-Specific)
//!
//! Subscribe to specific event types for cleaner code:
//!
//! ```ignore
//! let mut network_rx = session.network_events();
//! let mut log_rx = session.log_events();
//!
//! // Requires manual subscription to event types
//! session.session().subscribe(&["network.beforeRequestSent"], &[]).await?;
//! ```
//!
//! ## 3. Auto-Subscribe (Convenience)
//!
//! One-call subscribe and get typed receiver:
//!
//! ```ignore
//! let mut rx = session.subscribe_network().await?;
//! while let Ok(event) = rx.recv().await {
//!     // event is NetworkEvent, not BiDiEvent
//! }
//! ```
//!
//! # Connection State
//!
//! Check connection health at any time:
//!
//! ```ignore
//! if session.is_connected() {
//!     session.send_command("some.method", json!({})).await?;
//! }
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use pin_project_lite::pin_project;
use serde_json::{json, Value};
use tokio::sync::{broadcast, oneshot, Mutex as TokioMutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::error::{WebDriverError, WebDriverResult};

pub use browser::Browser;
pub use browsing_context::{BrowsingContext, BrowsingContextEvent};
pub use cdp::Cdp;
pub use console::Console;
pub use emulation::Emulation;
pub use input::Input;
pub use log::Log;
pub use network::{Network, NetworkEvent};
pub use permissions::Permissions;
pub use script::{Script, ScriptEvent};
pub use session::Session;
pub use storage::Storage;
pub use webextension::WebExtension;

/// Type alias for the pending commands map.
type PendingCommands = Arc<StdMutex<HashMap<u64, oneshot::Sender<WebDriverResult<Value>>>>>;

/// Context for the dispatch task, grouping shared state.
struct DispatchContext {
    pending: PendingCommands,
    event_tx: broadcast::Sender<BiDiEvent>,
    connected: Arc<AtomicBool>,
    network_tx: Arc<OnceLock<broadcast::Sender<NetworkEvent>>>,
    log_tx: Arc<OnceLock<broadcast::Sender<log::LogEvent>>>,
    browsing_context_tx: Arc<OnceLock<broadcast::Sender<BrowsingContextEvent>>>,
    script_tx: Arc<OnceLock<broadcast::Sender<ScriptEvent>>>,
}

/// Builder for creating a customized [`BiDiSession`].
///
/// This allows configuring connection parameters before connecting.
///
/// # Example
///
/// ```no_run
/// # use std::time::Duration;
/// # use thirtyfour::prelude::*;
/// # use thirtyfour::BiDiSessionBuilder;
/// # async fn example(driver: &WebDriver) -> WebDriverResult<()> {
/// let bidi = BiDiSessionBuilder::new()
///     .command_timeout(Duration::from_secs(30))
///     .event_channel_capacity(512)
///     .connect_with_driver(driver)
///     .await?;
///
/// // Spawn the dispatch future on your runtime
/// tokio::spawn(bidi.dispatch_future());
/// # Ok(())
/// # }
/// ```
///
/// # TLS Connections (wss://)
///
/// For secure WebSocket connections (`wss://`), you may need to enable
/// the crypto provider installation:
///
/// ```no_run
/// # use std::time::Duration;
/// # use thirtyfour::prelude::*;
/// # use thirtyfour::BiDiSessionBuilder;
/// # async fn example(driver: &WebDriver) -> WebDriverResult<()> {
/// let bidi = BiDiSessionBuilder::new()
///     .install_crypto_provider()
///     .connect_with_driver(driver)
///     .await?;
/// tokio::spawn(bidi.dispatch_future());
/// # Ok(())
/// # }
/// ```
///
/// # HTTP Basic Authentication
///
/// If your `WebDriver` infrastructure requires HTTP Basic Authentication,
/// use the builder to provide credentials:
///
/// ```no_run
/// # use std::time::Duration;
/// # use thirtyfour::prelude::*;
/// # use thirtyfour::BiDiSessionBuilder;
/// # async fn example(driver: &WebDriver) -> WebDriverResult<()> {
/// let bidi = BiDiSessionBuilder::new()
///     .install_crypto_provider()
///     .basic_auth("username", "password")
///     .connect_with_driver(driver)
///     .await?;
/// tokio::spawn(bidi.dispatch_future());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct BiDiSessionBuilder {
    pub(crate) event_channel_capacity: usize,
    pub(crate) command_timeout: Option<Duration>,
    pub(crate) install_crypto_provider: bool,
    pub(crate) use_server_url: bool,
    pub(crate) basic_auth: Option<(String, String)>,
    pub(crate) custom_url_base: Option<String>,
}

impl Default for BiDiSessionBuilder {
    fn default() -> Self {
        Self {
            event_channel_capacity: 256,
            command_timeout: None,
            install_crypto_provider: false,
            use_server_url: false,
            basic_auth: None,
            custom_url_base: None,
        }
    }
}

impl BiDiSessionBuilder {
    /// Create a new builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the capacity for the event broadcast channel.
    ///
    /// Default is 256. Increase this if you expect a high volume of events
    /// and don't want older events to be discarded when the buffer is full.
    #[must_use]
    pub fn event_channel_capacity(mut self, capacity: usize) -> Self {
        self.event_channel_capacity = capacity;
        self
    }

    /// Set a default timeout for all commands sent via this session.
    ///
    /// Without a timeout, commands may hang indefinitely if the browser
    /// becomes unresponsive.
    #[must_use]
    pub fn command_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = Some(timeout);
        self
    }

    /// Install the TLS crypto provider before connecting.
    ///
    /// Required for secure WebSocket connections (`wss://`) when using
    /// the `bidi` feature. Call this method if you encounter a panic
    /// about `CryptoProvider` when connecting.
    ///
    /// This installs the `aws_lc_rs` crypto provider from rustls.
    #[must_use]
    pub fn install_crypto_provider(mut self) -> Self {
        self.install_crypto_provider = true;
        self
    }

    /// Use the server URL to derive the WebSocket URL instead of using
    /// the Hub-provided one.
    ///
    /// When enabled, the BiDi WebSocket URL will be constructed from the
    /// server URL by replacing `http`/`https` with `ws`/`wss` and appending
    /// `/session/{session_id}/bidi`. This is useful when the WebDriver
    /// server does not provide a BiDi connection URL in its response.
    #[must_use]
    pub fn use_server_url(mut self) -> Self {
        self.use_server_url = true;
        self
    }

    /// Set HTTP Basic Authentication credentials.
    ///
    /// Use this if your `WebDriver` infrastructure requires authentication
    /// to access the `BiDi` `WebSocket` endpoint.
    #[must_use]
    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.basic_auth = Some((username.into(), password.into()));
        self
    }

    /// Set a custom base URL for the BiDi WebSocket connection.
    ///
    /// When set, this overrides both the hub-provided WebSocket URL and the
    /// server-derived URL. The library will append `/session/{session_id}/se/bidi`
    /// to construct the full connection URL.
    ///
    /// Use this when connecting to a separate BiDi server or proxy.
    ///
    /// **Note:** When `url_base()` is set, it takes absolute precedence over
    /// `use_server_url()` and `BidiConnectionType` settings.
    ///
    /// # Example
    /// ```ignore
    /// let bidi = driver.bidi_connect_with_builder(
    ///     BiDiSessionBuilder::new()
    ///         .url_base("wss://bidi.grid.example.com:4444")
    /// ).await?;
    /// ```
    #[must_use]
    pub fn url_base(mut self, url: &str) -> Self {
        // Validate that URL starts with ws:// or wss://
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            panic!("BiDi URL base must start with ws:// or wss://");
        }
        self.custom_url_base = Some(url.to_string());
        self
    }

    /// Connect to the `BiDi` WebSocket endpoint with the configured settings.
    ///
    /// **Important:** After connecting, you must run the dispatch loop.
    /// Use either [`BiDiSession::dispatch_future`] or [`BiDiSession::poll_dispatch`].
    ///
    /// # Errors
    ///
    /// Returns `WebDriverError::BiDi` if the WebSocket connection fails.
    pub async fn connect(self, ws_url: &str) -> WebDriverResult<BiDiSession> {
        BiDiSession::connect_with_config(ws_url, self).await
    }

    /// Connect using `WebDriver`'s session or server URL.
    ///
    /// This method respects all builder configuration including:
    /// - `use_server_url()` to derive BiDi URL from the server instead of hub-provided one
    /// - `install_crypto_provider()` for TLS connections
    /// - `basic_auth()` for HTTP Basic Authentication
    /// - `command_timeout()` for command timeouts
    /// - `event_channel_capacity()` for event buffer size
    ///
    /// When `.use_server_url()` is not explicitly called on the builder, this method respects
    /// [`WebDriverConfig::bidi_connection_type`]:
    /// - [`BidiConnectionType::UseHubProvided`] (default): Uses the WebSocket URL from session capabilities
    /// - [`BidiConnectionType::DeriveFromServerUrl`]: Derives BiDi URL from the server URL
    ///
    /// **Important:** After connecting, you must run the dispatch loop.
    /// Use either [`BiDiSession::dispatch_future`] or [`BiDiSession::poll_dispatch`].
    ///
    /// # Errors
    ///
    /// Returns `WebDriverError::BiDi` if:
    /// - No WebSocket URL is available (when not using server URL derivation)
    /// - The browser doesn't support BiDi from the server URL (when use_server_url is set)
    /// - The WebSocket connection fails
    pub async fn connect_with_driver(
        mut self,
        driver: &crate::WebDriver,
    ) -> WebDriverResult<BiDiSession> {
        let ws_url = if let Some(ref base) = self.custom_url_base {
            // Use custom base + session path (takes absolute precedence)
            let sid = driver.handle.session_id();
            format!("{}/session/{}/se/bidi", base.trim_end_matches('/'), sid)
        } else if self.use_server_url {
            // Builder explicitly requested to derive URL from server (overrides config)
            driver.handle.derive_bidi_ws_url()
        } else {
            // Respect the config's bidi_connection_type setting
            match driver.handle.config().bidi_connection_type {
                crate::common::config::BidiConnectionType::DeriveFromServerUrl => {
                    driver.handle.derive_bidi_ws_url()
                }
                crate::common::config::BidiConnectionType::UseHubProvided => {
                    driver.handle.websocket_url.clone().ok_or_else(|| {
                        WebDriverError::BiDi(
                            "No webSocketUrl in session capabilities and unable to derive from server URL. \
                             Enable BiDi in your browser capabilities \
                             (e.g., for Chrome: set 'webSocketUrl: true'), \
                             or configure BidiConnectionType::DeriveFromServerUrl in WebDriverConfig."
                                .to_string(),
                        )
                    })?
                }
            }
        };

        // Clear the use_server_url flag so it's not processed again in connect()
        self.use_server_url = false;
        self.connect(&ws_url).await
    }
}

/// Browser domain commands.
pub mod browser;
/// Browsing context domain commands and events.
pub mod browsing_context;
/// CDP passthrough domain commands.
pub mod cdp;
/// Console domain (wrapper over log).
pub mod console;
/// Emulation domain commands.
pub mod emulation;
/// Input domain commands.
pub mod input;
/// Log domain commands and events.
pub mod log;
/// Network domain commands and events.
pub mod network;
/// Permissions domain commands.
pub mod permissions;
/// Script domain commands and events.
pub mod script;
/// Session domain commands.
pub mod session;
/// Storage domain commands.
pub mod storage;
/// `WebExtension` domain commands.
pub mod webextension;

/// All `BiDi` events that can be received from the browser.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BiDiEvent {
    /// Network domain events.
    Network(NetworkEvent),
    /// Log domain events.
    Log(log::LogEvent),
    /// Script domain events.
    Script(ScriptEvent),
    /// `BrowsingContext` domain events.
    BrowsingContext(BrowsingContextEvent),
    /// Console domain events (alias for log.entryAdded with console source).
    Console(console::ConsoleEvent),
    /// WebSocket connection closed.
    ConnectionClosed,
    /// An unrecognised event method and its raw params.
    Unknown {
        /// The event method name (e.g., "network.beforeRequestSent").
        method: String,
        /// The event parameters.
        params: Value,
    },
}

pin_project! {
    /// Future that runs the BiDi dispatch loop.
    ///
    /// This future processes incoming WebSocket messages and dispatches
    /// them to the appropriate channels. It completes when the WebSocket
    /// connection closes or an error occurs.
    ///
    /// Spawn this on your preferred async runtime:
    /// ```ignore
    /// tokio::spawn(bidi.dispatch_future());
    /// // or
    /// async_std::spawn(bidi.dispatch_future());
    /// ```
    #[must_use = "dispatch_future does nothing unless spawned or awaited"]
    pub struct DispatchFuture {
        stream: futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        ctx: DispatchContext,
        #[pin]
        span: tracing::Span,
    }
}

impl Future for DispatchFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _entered = this.span.enter();

        loop {
            match futures_util::StreamExt::poll_next_unpin(this.stream, cx) {
                Poll::Ready(Some(msg)) => {
                    let text = match msg {
                        Ok(Message::Text(t)) => t,
                        Ok(Message::Close(_)) => {
                            this.ctx.connected.store(false, Ordering::Relaxed);
                            let _ = this.ctx.event_tx.send(BiDiEvent::ConnectionClosed);
                            tracing::debug!("BiDi WebSocket connection closed");
                            return Poll::Ready(());
                        }
                        Ok(_) => continue,
                        Err(e) => {
                            tracing::error!("BiDi WebSocket error: {e}");
                            this.ctx.connected.store(false, Ordering::Relaxed);
                            let _ = this.ctx.event_tx.send(BiDiEvent::ConnectionClosed);
                            return Poll::Ready(());
                        }
                    };

                    let v: Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!("BiDi: failed to parse message: {e}");
                            continue;
                        }
                    };

                    match v.get("type").and_then(Value::as_str) {
                        Some("success" | "error") => {
                            BiDiSession::handle_response(&this.ctx.pending, &v);
                        }
                        Some("event") => {
                            let method =
                                v.get("method").and_then(Value::as_str).unwrap_or("").to_string();
                            let params = v.get("params").cloned().unwrap_or(Value::Null);
                            let event = parse_event(&method, params);

                            tracing::trace!(method = %method, "BiDi event received");

                            let _ = this.ctx.event_tx.send(event.clone());
                            BiDiSession::broadcast_typed(
                                &this.ctx.network_tx,
                                &this.ctx.log_tx,
                                &this.ctx.browsing_context_tx,
                                &this.ctx.script_tx,
                                &event,
                            );
                        }
                        _ => {}
                    }
                }
                Poll::Ready(None) => {
                    this.ctx.connected.store(false, Ordering::Relaxed);
                    let _ = this.ctx.event_tx.send(BiDiEvent::ConnectionClosed);
                    tracing::debug!("BiDi WebSocket stream ended");
                    return Poll::Ready(());
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// A live `WebDriver` `BiDi` session over a WebSocket connection.
///
/// Obtain one by calling [`WebDriver::bidi_connect`][crate::WebDriver::bidi_connect].
///
/// # Dispatch Loop
///
/// After creating a `BiDiSession`, you must run the dispatch loop to process
/// incoming messages. Choose one of:
///
/// - [`dispatch_future()`](Self::dispatch_future) - Returns a `Future` to spawn
/// - [`poll_dispatch()`](Self::poll_dispatch) - Manual polling for custom loops
pub struct BiDiSession {
    /// Sends frames to the WebSocket (async mutex for safe await).
    ws_sink: Arc<
        TokioMutex<
            futures_util::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                Message,
            >,
        >,
    >,
    /// WebSocket stream for receiving messages.
    ws_stream: Option<
        futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    >,
    /// Auto-incrementing JSON-RPC command id.
    command_id: Arc<AtomicU64>,
    /// In-flight commands waiting for a response. Never held across `.await`.
    pending: Arc<StdMutex<HashMap<u64, oneshot::Sender<WebDriverResult<Value>>>>>,
    /// Broadcast channel for all incoming events.
    event_tx: broadcast::Sender<BiDiEvent>,
    /// Connection state tracking.
    connected: Arc<AtomicBool>,
    /// Optional session-level command timeout.
    command_timeout: Option<Duration>,
    /// Typed event channels (lazy-initialized).
    network_tx: Arc<OnceLock<broadcast::Sender<NetworkEvent>>>,
    log_tx: Arc<OnceLock<broadcast::Sender<log::LogEvent>>>,
    browsing_context_tx: Arc<OnceLock<broadcast::Sender<BrowsingContextEvent>>>,
    script_tx: Arc<OnceLock<broadcast::Sender<ScriptEvent>>>,
}

impl std::fmt::Debug for BiDiSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BiDiSession")
            .field("connected", &self.connected.load(Ordering::Relaxed))
            .field("command_timeout", &self.command_timeout)
            .field("dispatch_started", &self.ws_stream.is_none())
            .finish_non_exhaustive()
    }
}

impl BiDiSession {
    /// Connect to the `BiDi` WebSocket endpoint with default configuration.
    ///
    /// For timeout and capacity configuration, use [`BiDiSessionBuilder`].
    ///
    /// **Important:** After connecting, you must run the dispatch loop.
    /// Use either [`dispatch_future()`](Self::dispatch_future) or
    /// [`poll_dispatch()`](Self::poll_dispatch).
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket connection fails.
    pub async fn connect(ws_url: &str) -> WebDriverResult<Self> {
        Self::connect_with_config(ws_url, BiDiSessionBuilder::new()).await
    }

    /// Connect to the `BiDi` WebSocket endpoint with custom configuration.
    ///
    /// **Important:** After connecting, you must run the dispatch loop.
    /// Use either [`dispatch_future()`](Self::dispatch_future) or
    /// [`poll_dispatch()`](Self::poll_dispatch).
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket connection fails.
    pub async fn connect_with_config(
        ws_url: &str,
        config: BiDiSessionBuilder,
    ) -> WebDriverResult<Self> {
        tracing::debug!(url = %ws_url, "BiDi WebSocket connecting");

        if config.install_crypto_provider {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        }

        let ws_stream = if let Some((username, password)) = &config.basic_auth {
            Self::connect_with_auth(ws_url, username, password).await?
        } else {
            let (stream, _) = connect_async(ws_url)
                .await
                .map_err(|e| WebDriverError::BiDi(format!("WebSocket connect failed: {e}")))?;
            stream
        };

        tracing::debug!(url = %ws_url, "BiDi WebSocket connected");

        let (sink, stream) = ws_stream.split();
        let (event_tx, _) = broadcast::channel::<BiDiEvent>(config.event_channel_capacity);
        let command_id = Arc::new(AtomicU64::new(1));
        let pending: Arc<StdMutex<HashMap<u64, oneshot::Sender<WebDriverResult<Value>>>>> =
            Arc::new(StdMutex::new(HashMap::new()));
        let connected = Arc::new(AtomicBool::new(true));

        let network_tx: Arc<OnceLock<broadcast::Sender<NetworkEvent>>> = Arc::new(OnceLock::new());
        let log_tx: Arc<OnceLock<broadcast::Sender<log::LogEvent>>> = Arc::new(OnceLock::new());
        let browsing_context_tx: Arc<OnceLock<broadcast::Sender<BrowsingContextEvent>>> =
            Arc::new(OnceLock::new());
        let script_tx: Arc<OnceLock<broadcast::Sender<ScriptEvent>>> = Arc::new(OnceLock::new());

        Ok(Self {
            ws_sink: Arc::new(TokioMutex::new(sink)),
            ws_stream: Some(stream),
            command_id,
            pending,
            event_tx,
            connected,
            command_timeout: config.command_timeout,
            network_tx,
            log_tx,
            browsing_context_tx,
            script_tx,
        })
    }

    async fn connect_with_auth(
        ws_url: &str,
        username: &str,
        password: &str,
    ) -> WebDriverResult<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    > {
        use base64::Engine;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let mut request = ws_url.into_client_request().map_err(|e| {
            WebDriverError::BiDi(format!("Failed to create WebSocket request: {e}"))
        })?;

        let credentials =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        let auth_value = format!("Basic {credentials}");
        request.headers_mut().insert(
            tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
            auth_value.parse().map_err(|e| {
                WebDriverError::BiDi(format!("Failed to create auth header value: {e}"))
            })?,
        );

        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| WebDriverError::BiDi(format!("WebSocket connect failed: {e}")))?;

        Ok(ws_stream)
    }

    /// Returns a future that runs the BiDi dispatch loop.
    ///
    /// This future processes incoming WebSocket messages and dispatches
    /// them to the appropriate channels. It completes when the WebSocket
    /// connection closes or an error occurs.
    ///
    /// **Note:** Can only be called once per session. Returns `None` if
    /// the dispatch loop has already been started.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bidi = BiDiSessionBuilder::new()
    ///     .connect_with_driver(&driver)
    ///     .await?;
    ///
    /// // Spawn on your preferred runtime
    /// tokio::spawn(bidi.dispatch_future());
    /// // or: async_std::spawn(bidi.dispatch_future());
    /// ```
    #[must_use = "dispatch_future must be spawned or awaited to process messages"]
    pub fn dispatch_future(&mut self) -> Option<DispatchFuture> {
        let stream = self.ws_stream.take()?;
        let ctx = DispatchContext {
            pending: Arc::clone(&self.pending),
            event_tx: self.event_tx.clone(),
            connected: Arc::clone(&self.connected),
            network_tx: Arc::clone(&self.network_tx),
            log_tx: Arc::clone(&self.log_tx),
            browsing_context_tx: Arc::clone(&self.browsing_context_tx),
            script_tx: Arc::clone(&self.script_tx),
        };
        let span = tracing::debug_span!("bidi_dispatch");

        Some(DispatchFuture {
            stream,
            ctx,
            span,
        })
    }

    /// Manually poll for a single incoming message.
    ///
    /// Returns `Ok(true)` if a message was processed, `Ok(false)` if the
    /// connection has closed, or an error on WebSocket failure.
    ///
    /// Use this for custom event loops where you need more control:
    ///
    /// ```ignore
    /// loop {
    ///     let more = bidi.poll_dispatch().await?;
    ///     if !more {
    ///         break; // Connection closed
    ///     }
    /// }
    /// ```
    ///
    /// **Note:** Can only be called if `dispatch_future()` has not been called.
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket receives an error.
    pub async fn poll_dispatch(&mut self) -> WebDriverResult<bool> {
        let stream = self.ws_stream.as_mut().ok_or_else(|| {
            WebDriverError::BiDi("dispatch loop already started via dispatch_future()".to_string())
        })?;

        let msg = stream.next().await;

        match msg {
            Some(msg) => {
                let text = match msg {
                    Ok(Message::Text(t)) => t,
                    Ok(Message::Close(_)) => {
                        self.connected.store(false, Ordering::Relaxed);
                        let _ = self.event_tx.send(BiDiEvent::ConnectionClosed);
                        tracing::debug!("BiDi WebSocket connection closed");
                        return Ok(false);
                    }
                    Ok(_) => return Ok(true),
                    Err(e) => {
                        tracing::error!("BiDi WebSocket error: {e}");
                        self.connected.store(false, Ordering::Relaxed);
                        let _ = self.event_tx.send(BiDiEvent::ConnectionClosed);
                        return Err(WebDriverError::BiDi(format!("WebSocket error: {e}")));
                    }
                };

                let v: Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("BiDi: failed to parse message: {e}");
                        return Ok(true);
                    }
                };

                match v.get("type").and_then(Value::as_str) {
                    Some("success" | "error") => {
                        Self::handle_response(&self.pending, &v);
                    }
                    Some("event") => {
                        let method =
                            v.get("method").and_then(Value::as_str).unwrap_or("").to_string();
                        let params = v.get("params").cloned().unwrap_or(Value::Null);
                        let event = parse_event(&method, params);

                        tracing::trace!(method = %method, "BiDi event received");

                        let _ = self.event_tx.send(event.clone());
                        Self::broadcast_typed(
                            &self.network_tx,
                            &self.log_tx,
                            &self.browsing_context_tx,
                            &self.script_tx,
                            &event,
                        );
                    }
                    _ => {}
                }

                Ok(true)
            }
            None => {
                self.connected.store(false, Ordering::Relaxed);
                let _ = self.event_tx.send(BiDiEvent::ConnectionClosed);
                tracing::debug!("BiDi WebSocket stream ended");
                Ok(false)
            }
        }
    }

    fn handle_response(pending: &PendingCommands, v: &Value) {
        if let Some(id) = v.get("id").and_then(Value::as_u64) {
            let sender = {
                match pending.lock() {
                    Ok(mut map) => map.remove(&id),
                    Err(poisoned) => {
                        tracing::error!("BiDi pending commands mutex poisoned");
                        poisoned.into_inner().remove(&id)
                    }
                }
            };
            if let Some(tx) = sender {
                let result = if v.get("type").and_then(Value::as_str) == Some("success") {
                    Ok(v.get("result").cloned().unwrap_or(Value::Null))
                } else {
                    let msg = v
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown BiDi error")
                        .to_string();
                    Err(WebDriverError::BiDi(msg))
                };
                let _ = tx.send(result);
            }
        }
    }

    fn broadcast_typed(
        network_tx: &Arc<OnceLock<broadcast::Sender<NetworkEvent>>>,
        log_tx: &Arc<OnceLock<broadcast::Sender<log::LogEvent>>>,
        browsing_context_tx: &Arc<OnceLock<broadcast::Sender<BrowsingContextEvent>>>,
        script_tx: &Arc<OnceLock<broadcast::Sender<ScriptEvent>>>,
        event: &BiDiEvent,
    ) {
        match event {
            BiDiEvent::Network(e) => {
                if let Some(tx) = network_tx.get() {
                    let _ = tx.send(e.clone());
                }
            }
            BiDiEvent::Log(e) => {
                if let Some(tx) = log_tx.get() {
                    let _ = tx.send(e.clone());
                }
            }
            BiDiEvent::BrowsingContext(e) => {
                if let Some(tx) = browsing_context_tx.get() {
                    let _ = tx.send(e.clone());
                }
            }
            BiDiEvent::Script(e) => {
                if let Some(tx) = script_tx.get() {
                    let _ = tx.send(e.clone());
                }
            }
            _ => {}
        }
    }

    /// Check if the WebSocket connection is still alive.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Check if the dispatch loop has been started.
    #[must_use]
    pub fn is_dispatch_started(&self) -> bool {
        self.ws_stream.is_none()
    }

    /// Send a `BiDi` command and await the response with a custom timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails, the WebSocket send fails,
    /// the response channel closes, or the timeout elapses.
    pub async fn send_command_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> WebDriverResult<Value> {
        let id = self.command_id.fetch_add(1, Ordering::SeqCst);
        let msg = json!({ "id": id, "method": method, "params": params });
        let text = serde_json::to_string(&msg)
            .map_err(|e| WebDriverError::BiDi(format!("serialise error: {e}")))?;

        let (tx, rx) = oneshot::channel();
        {
            match self.pending.lock() {
                Ok(mut map) => {
                    map.insert(id, tx);
                }
                Err(poisoned) => {
                    tracing::error!("BiDi pending commands mutex poisoned");
                    poisoned.into_inner().insert(id, tx);
                }
            }
        }

        tracing::trace!(method = %method, id = %id, timeout = ?timeout, "Sending BiDi command with timeout");

        self.ws_sink
            .lock()
            .await
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| WebDriverError::BiDi(format!("WebSocket send failed: {e}")))?;

        tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| {
                WebDriverError::BiDi(format!("command '{method}' timed out after {timeout:?}"))
            })?
            .map_err(|_| WebDriverError::BiDi("response channel closed".to_string()))?
    }

    /// Send a `BiDi` command and await the response.
    ///
    /// Uses the session-level timeout if configured via [`BiDiSessionBuilder::command_timeout`].
    ///
    /// `method` is e.g. `"network.addIntercept"`.
    /// `params` is the JSON params object.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails, the WebSocket send fails,
    /// the response channel closes, or the configured timeout elapses.
    pub async fn send_command(&self, method: &str, params: Value) -> WebDriverResult<Value> {
        if let Some(timeout) = self.command_timeout {
            self.send_command_with_timeout(method, params, timeout).await
        } else {
            let id = self.command_id.fetch_add(1, Ordering::SeqCst);
            let msg = json!({ "id": id, "method": method, "params": params });
            let text = serde_json::to_string(&msg)
                .map_err(|e| WebDriverError::BiDi(format!("serialise error: {e}")))?;

            let (tx, rx) = oneshot::channel();
            {
                match self.pending.lock() {
                    Ok(mut map) => {
                        map.insert(id, tx);
                    }
                    Err(poisoned) => {
                        tracing::error!("BiDi pending commands mutex poisoned");
                        poisoned.into_inner().insert(id, tx);
                    }
                }
            }

            self.ws_sink
                .lock()
                .await
                .send(Message::Text(text.into()))
                .await
                .map_err(|e| WebDriverError::BiDi(format!("WebSocket send failed: {e}")))?;

            rx.await.map_err(|_| WebDriverError::BiDi("response channel closed".to_string()))?
        }
    }

    /// Subscribe to all `BiDi` events.
    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<BiDiEvent> {
        self.event_tx.subscribe()
    }

    /// Subscribe to network domain events only.
    ///
    /// The channel is lazily initialized on first call.
    /// This does NOT automatically subscribe to events in the browser -
    /// call [`Self::subscribe_network`] for that.
    #[must_use]
    pub fn network_events(&self) -> broadcast::Receiver<NetworkEvent> {
        self.network_tx.get_or_init(|| broadcast::channel(256).0).subscribe()
    }

    /// Subscribe to log domain events only.
    ///
    /// The channel is lazily initialized on first call.
    /// This does NOT automatically subscribe to events in the browser -
    /// call [`Self::subscribe_log`] for that.
    #[must_use]
    pub fn log_events(&self) -> broadcast::Receiver<log::LogEvent> {
        self.log_tx.get_or_init(|| broadcast::channel(256).0).subscribe()
    }

    /// Subscribe to browsing context domain events only.
    ///
    /// The channel is lazily initialized on first call.
    /// This does NOT automatically subscribe to events in the browser -
    /// call [`Self::subscribe_browsing_context`] for that.
    #[must_use]
    pub fn browsing_context_events(&self) -> broadcast::Receiver<BrowsingContextEvent> {
        self.browsing_context_tx.get_or_init(|| broadcast::channel(256).0).subscribe()
    }

    /// Subscribe to script domain events only.
    ///
    /// The channel is lazily initialized on first call.
    /// This does NOT automatically subscribe to events in the browser -
    /// call [`Self::subscribe_script`] for that.
    #[must_use]
    pub fn script_events(&self) -> broadcast::Receiver<ScriptEvent> {
        self.script_tx.get_or_init(|| broadcast::channel(256).0).subscribe()
    }

    /// Subscribe to all network events and return a typed receiver.
    ///
    /// This is a convenience method that calls `session.subscribe(["network.*"])`
    /// and returns a typed receiver.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscribe command fails.
    pub async fn subscribe_network(&self) -> WebDriverResult<broadcast::Receiver<NetworkEvent>> {
        self.session().subscribe(&["network.*"], &[]).await?;
        Ok(self.network_events())
    }

    /// Subscribe to all log events and return a typed receiver.
    ///
    /// This is a convenience method that calls `session.subscribe(["log.*"])`
    /// and returns a typed receiver.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscribe command fails.
    pub async fn subscribe_log(&self) -> WebDriverResult<broadcast::Receiver<log::LogEvent>> {
        self.session().subscribe(&["log.*"], &[]).await?;
        Ok(self.log_events())
    }

    /// Subscribe to all browsing context events and return a typed receiver.
    ///
    /// This is a convenience method that calls `session.subscribe(["browsingContext.*"])`
    /// and returns a typed receiver.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscribe command fails.
    pub async fn subscribe_browsing_context(
        &self,
    ) -> WebDriverResult<broadcast::Receiver<BrowsingContextEvent>> {
        self.session().subscribe(&["browsingContext.*"], &[]).await?;
        Ok(self.browsing_context_events())
    }

    /// Subscribe to all script events and return a typed receiver.
    ///
    /// This is a convenience method that calls `session.subscribe(["script.*"])`
    /// and returns a typed receiver.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscribe command fails.
    pub async fn subscribe_script(&self) -> WebDriverResult<broadcast::Receiver<ScriptEvent>> {
        self.session().subscribe(&["script.*"], &[]).await?;
        Ok(self.script_events())
    }

    // --- Domain accessors ---

    /// Access the `session` domain.
    #[must_use]
    pub fn session(&self) -> Session<'_> {
        Session::new(self)
    }

    /// Access the `log` domain.
    #[must_use]
    pub fn log(&self) -> Log<'_> {
        Log::new(self)
    }

    /// Access the `network` domain.
    #[must_use]
    pub fn network(&self) -> Network<'_> {
        Network::new(self)
    }

    /// Access the `browsingContext` domain.
    #[must_use]
    pub fn browsing_context(&self) -> BrowsingContext<'_> {
        BrowsingContext::new(self)
    }

    /// Access the `script` domain.
    #[must_use]
    pub fn script(&self) -> Script<'_> {
        Script::new(self)
    }

    /// Access the `browser` domain.
    #[must_use]
    pub fn browser(&self) -> Browser<'_> {
        Browser::new(self)
    }

    /// Access the `console` domain (thin wrapper over log).
    #[must_use]
    pub fn console(&self) -> Console<'_> {
        Console::new(self)
    }

    /// Access the `input` domain.
    #[must_use]
    pub fn input(&self) -> Input<'_> {
        Input::new(self)
    }

    /// Access the `permissions` domain.
    #[must_use]
    pub fn permissions(&self) -> Permissions<'_> {
        Permissions::new(self)
    }

    /// Access the `storage` domain.
    #[must_use]
    pub fn storage(&self) -> Storage<'_> {
        Storage::new(self)
    }

    /// Access the `webExtension` domain.
    #[must_use]
    pub fn webextension(&self) -> WebExtension<'_> {
        WebExtension::new(self)
    }

    /// Access the `emulation` domain.
    #[must_use]
    pub fn emulation(&self) -> Emulation<'_> {
        Emulation::new(self)
    }

    /// Access the `BiDi` CDP passthrough domain.
    #[must_use]
    pub fn cdp(&self) -> Cdp<'_> {
        Cdp::new(self)
    }
}

/// Parse an incoming event message into a `BiDiEvent`.
fn parse_event(method: &str, params: Value) -> BiDiEvent {
    match method {
        "network.beforeRequestSent"
        | "network.responseStarted"
        | "network.responseCompleted"
        | "network.fetchError"
        | "network.authRequired" => {
            match serde_json::from_value::<NetworkEvent>(
                json!({ "method": method, "params": params }),
            ) {
                Ok(e) => BiDiEvent::Network(e),
                Err(_) => BiDiEvent::Unknown {
                    method: method.to_string(),
                    params,
                },
            }
        }
        "log.entryAdded" => match serde_json::from_value::<log::LogEvent>(params.clone()) {
            Ok(e) => BiDiEvent::Log(e),
            Err(_) => BiDiEvent::Unknown {
                method: method.to_string(),
                params,
            },
        },
        "script.realmCreated" | "script.realmDestroyed" => {
            match serde_json::from_value::<ScriptEvent>(
                json!({ "method": method, "params": params }),
            ) {
                Ok(e) => BiDiEvent::Script(e),
                Err(_) => BiDiEvent::Unknown {
                    method: method.to_string(),
                    params,
                },
            }
        }
        "browsingContext.contextCreated"
        | "browsingContext.contextDestroyed"
        | "browsingContext.navigationStarted"
        | "browsingContext.navigationAborted"
        | "browsingContext.navigationFailed"
        | "browsingContext.domContentLoaded"
        | "browsingContext.load"
        | "browsingContext.download" => {
            match serde_json::from_value::<BrowsingContextEvent>(
                json!({ "method": method, "params": params }),
            ) {
                Ok(e) => BiDiEvent::BrowsingContext(e),
                Err(_) => BiDiEvent::Unknown {
                    method: method.to_string(),
                    params,
                },
            }
        }
        _ => BiDiEvent::Unknown {
            method: method.to_string(),
            params,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_builder_defaults() {
        let builder = BiDiSessionBuilder::new();
        assert_eq!(builder.event_channel_capacity, 256);
        assert_eq!(builder.command_timeout, None);
    }

    #[test]
    fn test_builder_configuration() {
        let builder = BiDiSessionBuilder::new()
            .command_timeout(Duration::from_secs(10))
            .event_channel_capacity(512);
        assert_eq!(builder.command_timeout, Some(Duration::from_secs(10)));
        assert_eq!(builder.event_channel_capacity, 512);
    }

    #[test]
    fn test_parse_event_unknown() {
        let event = parse_event("some.unknownEvent", json!({"foo": "bar"}));
        matches!(event, BiDiEvent::Unknown { .. });
    }

    #[test]
    fn test_builder_default_trait() {
        let builder = BiDiSessionBuilder::default();
        assert_eq!(builder.event_channel_capacity, 256);
        assert_eq!(builder.command_timeout, None);
    }

    #[test]
    fn test_url_base_valid_ws() {
        let builder = BiDiSessionBuilder::new().url_base("ws://localhost:4444");
        assert_eq!(builder.custom_url_base, Some("ws://localhost:4444".to_string()));
    }

    #[test]
    fn test_url_base_valid_wss() {
        let builder = BiDiSessionBuilder::new().url_base("wss://localhost:4444");
        assert_eq!(builder.custom_url_base, Some("wss://localhost:4444".to_string()));
    }

    #[test]
    #[should_panic(expected = "BiDi URL base must start with ws:// or wss://")]
    fn test_url_base_invalid_scheme() {
        let _ = BiDiSessionBuilder::new().url_base("http://localhost:4444");
    }

    #[test]
    #[should_panic(expected = "BiDi URL base must start with ws:// or wss://")]
    fn test_url_base_invalid_scheme_https() {
        let _ = BiDiSessionBuilder::new().url_base("https://localhost:4444");
    }

    #[test]
    fn test_url_base_with_trailing_slash() {
        // Test that trailing slash is handled correctly
        let builder = BiDiSessionBuilder::new().url_base("wss://localhost:4444/");
        assert_eq!(builder.custom_url_base, Some("wss://localhost:4444/".to_string()));
    }

    #[test]
    fn test_url_base_empty_is_valid() {
        // Test that minimal valid URL (scheme only) passes validation
        // Connection will fail at runtime due to missing host, but validation passes
        let builder = BiDiSessionBuilder::new().url_base("ws://");
        assert_eq!(builder.custom_url_base, Some("ws://".to_string()));
    }
}
