# BiDi Custom URL Base Design

**Date:** 2026-03-19  
**Status:** Draft

## Summary

Add the ability to specify a custom base URL for BiDi WebSocket connections that overrides both the hub-provided URL and the server-derived URL. The library will append `/session/{session_id}/se/bidi` automatically.

## Motivation

Users connecting to Selenium Grid or other WebDriver infrastructures may have BiDi available at a different URL than:
1. What the hub provides in the `newSession` response
2. What can be derived from the server URL (host:port)

This feature enables connecting to a separate BiDi server or proxy while maintaining all existing functionality.

## Design

### 1. BiDiSessionBuilder Changes

**File:** `thirtyfour/src/extensions/bidi/mod.rs`

Add new field to `BiDiSessionBuilder` struct:

```rust
pub(crate) custom_url_base: Option<String>,
```

Add new builder method:

```rust
/// Set a custom base URL for the BiDi WebSocket connection.
///
/// When set, this overrides both the hub-provided WebSocket URL and the
/// server-derived URL. The library will append `/session/{session_id}/se/bidi`
/// to construct the full connection URL.
///
/// Use this when connecting to a separate BiDi server or proxy.
///
/// # Example
/// ```ignore
/// let bidi = driver.bidi_connect_with_builder()
///     .url_base("wss://bidi.grid.example.com:4444")
///     .await?;
/// ```
#[must_use]
pub fn url_base(mut self, url: &str) -> Self {
    self.custom_url_base = Some(url.to_string());
    self
}
```

### 2. SessionHandle Changes

**File:** `thirtyfour/src/session/handle.rs`

Make `derive_bidi_ws_url` public and optionally accept a session ID:

```rust
/// Derive the BiDi WebSocket URL from the server URL.
///
/// Uses only the host (and port) portion of the server URL, not any path.
/// This is because BiDi uses a different route to connect to the node.
///
/// Converts http:// to ws:// and https:// to wss://.
pub fn derive_bidi_ws_url(&self) -> String {
    let url = self.server_url.as_ref();
    
    let host_with_port = if let Some(port) = url.port() {
        format!("{}:{}", url.host_str().unwrap_or("localhost"), port)
    } else {
        url.host_str().unwrap_or("localhost").to_string()
    };
    
    match url.scheme() {
        "https" => format!("wss://{}/", host_with_port),
        _ => format!("ws://{}/", host_with_port),
    }
}
```

### 3. BiDiSessionBuilder::connect_with_driver() Changes

**File:** `thirtyfour/src/extensions/bidi/mod.rs`

Update the URL resolution logic in `connect_with_driver`:

```rust
let ws_url = if let Some(ref base) = self.custom_url_base {
    // Use custom base + session path
    let sid = driver.handle.session_id();
    format!("{}/session/{}/se/bidi", base.trim_end_matches('/'), sid)
} else if self.use_server_url {
    // Builder explicitly requested to derive URL from server (overrides config)
    Some(driver.handle.derive_bidi_ws_url())
} else {
    // Respect the config's bidi_connection_type setting
    match driver.handle.config().bidi_connection_type {
        crate::common::config::BidiConnectionType::DeriveFromServerUrl => {
            Some(driver.handle.derive_bidi_ws_url())
        }
        crate::common::config::BidiConnectionType::UseHubProvided => {
            driver.handle.websocket_url.clone()
        }
    }
};
```

### 4. SessionHandle Session ID Access

**File:** `thirtyfour/src/session/handle.rs`

Add a public method to get the session ID:

```rust
/// Get the session ID for this WebDriver session.
pub fn session_id(&self) -> &SessionId {
    &self.session_id
}
```

## Usage Examples

### Example 1: Basic custom URL base

```rust
let bidi = driver.bidi_connect_with_builder()
    .url_base("wss://bidi.grid.example.com:4444")
    .await?;
```

### Example 2: Custom URL with authentication

```rust
let bidi = driver.bidi_connect_with_builder()
    .url_base("wss://bidi.grid.example.com:4444")
    .basic_auth("user", "pass")
    .await?;
```

### Example 3: Custom URL with other options

```rust
let bidi = driver.bidi_connect_with_builder()
    .url_base("wss://bidi.grid.example.com:4444")
    .command_timeout(Duration::from_secs(30))
    .event_channel_capacity(512)
    .await?;
```

## Backward Compatibility

- Default behavior unchanged when `url_base()` is not called
- Existing `BidiConnectionType` options continue to work as before
- No changes to WebDriverConfig required

## Testing

1. Unit test: Verify `url_base()` sets the field correctly
2. Unit test: Verify URL construction with custom base includes session path
3. Integration test: Connect using custom URL base to a mock/test server