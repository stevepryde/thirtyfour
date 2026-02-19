#![allow(clippy::missing_errors_doc)]

//! WebDriver BiDi (Bidirectional Protocol) support for real-time event handling.
//!
//! This module provides WebSocket-based communication with WebDriver for
//! capturing and responding to browser events in real-time.
//!
//! # Important: Threading
//!
//! **It is recommended to run the event listener on an independent thread**
//! because the WebSocket listener blocks while waiting for events from CDP
//! (Chrome DevTools Protocol). Running it on a separate thread ensures your
//! main application remains responsive.
//!
//! # Example
//!
//! ```ignore
//! use thirtyfour::prelude::*;
//! use thirtyfour::extensions::bidi::{BiDiWebSocket, BiDiEventListener, events, Network, InterceptPhase};
//!
//! async fn example() -> WebDriverResult<()> {
//!     let caps = DesiredCapabilities::chrome();
//!     let driver = WebDriver::new("http://localhost:4444", caps).await?;
//!     
//!     // Get WebSocket URL from the session
//!     let ws_url = "ws://localhost:4444/session/.../bidi";
//!     
//!     // Create WebSocket connection
//!     let ws = BiDiWebSocket::new(ws_url)?;
//!     let connection = ws.connect().await?;
//!     
//!     // Create event listener
//!     let mut listener = BiDiEventListener::new(connection);
//!     
//!     // Subscribe to network events
//!     listener.subscribe(&[events::BEFORE_REQUEST_SENT, events::RESPONSE_COMPLETED]).await?;
//!     
//!     // Set up network interception via HTTP
//!     let network = Network::new(driver.handle.clone());
//!     network.add_intercept(vec![InterceptPhase::BeforeRequestSent]).await?;
//!     
//!     // Listen for events (run this on a separate thread!)
//!     listener.listen(|event| {
//!         if event.method() == events::BEFORE_REQUEST_SENT {
//!             if let Ok(params) = event.params_as::<events::BeforeRequestSentParams>() {
//!                 println!("Request to: {}", params.request.url);
//!                 // Use network.continue_request() or network.continue_request_modified()
//!                 // to control the request
//!             }
//!         }
//!     }).await;
//!     
//!     Ok(())
//! }
//! ```

mod commands;
mod network;
mod websocket;

pub use commands::{
    domains, events, BiDiCommand, WebSocketCommand, WebSocketEvent, WebSocketResponse,
};
pub use network::{
    Body, CompletedResponse, Header, InterceptPhase, InterceptedRequest, InterceptedResponse,
    Network, NetworkCommand,
};
pub use websocket::{BiDiConnection, BiDiEventListener, BiDiMessage, BiDiWebSocket};
