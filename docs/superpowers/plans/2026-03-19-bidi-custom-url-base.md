# BiDi Custom URL Base Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add ability to specify a custom base URL for BiDi WebSocket connections that overrides both the hub-provided URL and server-derived URL.

**Architecture:** Add `custom_url_base` field to `BiDiSessionBuilder` with a `url_base()` builder method. Update both `WebDriver::bidi_connect_with_builder()` and `BiDiSessionBuilder::connect_with_driver()` to handle the custom URL, appending `/session/{session_id}/se/bidi` to the base.

**Tech Stack:** Rust, thirtyfour crate

---

## File Mapping

| File | Responsibility |
|------|----------------|
| `thirtyfour/src/extensions/bidi/mod.rs` | Add `custom_url_base` field and `url_base()` method to BiDiSessionBuilder |
| `thirtyfour/src/session/handle.rs` | Make `derive_bidi_ws_url()` and `session_id()` public |
| `thirtyfour/src/web_driver.rs` | Update `bidi_connect_with_builder()` to handle custom URL |

---

## Tasks

### Task 1: Add `custom_url_base` field to BiDiSessionBuilder

**Files:**
- Modify: `thirtyfour/src/extensions/bidi/mod.rs:239-245`

- [ ] **Step 1: Add field to struct**

```rust
#[derive(Debug, Clone)]
pub struct BiDiSessionBuilder {
    pub(crate) event_channel_capacity: usize,
    pub(crate) command_timeout: Option<Duration>,
    pub(crate) install_crypto_provider: bool,
    pub(crate) use_server_url: bool,
    pub(crate) basic_auth: Option<(String, String)>,
    pub(crate) custom_url_base: Option<String>,  // ADD THIS LINE
}
```

- [ ] **Step 2: Add to Default implementation**

```rust
impl Default for BiDiSessionBuilder {
    fn default() -> Self {
        Self {
            event_channel_capacity: 256,
            command_timeout: None,
            install_crypto_provider: false,
            use_server_url: false,
            basic_auth: None,
            custom_url_base: None,  // ADD THIS LINE
        }
    }
}
```

- [ ] **Step 3: Add builder method**

Add this method to the `impl BiDiSessionBuilder` block (after line ~310):

```rust
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
```

- [ ] **Step 4: Commit**

```bash
git add thirtyfour/src/extensions/bidi/mod.rs
git commit -m "feat(bidi): add custom_url_base field and url_base() method to BiDiSessionBuilder"
```

---

### Task 2: Make SessionHandle methods public

**Files:**
- Modify: `thirtyfour/src/session/handle.rs`

- [ ] **Step 1: Make `session_id()` public**

Find the existing `session_id()` method (around line 97) and change visibility:

```rust
/// Get the session ID for this WebDriver session.
pub fn session_id(&self) -> &SessionId {
    &self.session_id
}
```

- [ ] **Step 2: Make `derive_bidi_ws_url()` public**

Find the existing `derive_bidi_ws_url()` method (around line 118) and change visibility:

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

- [ ] **Step 3: Commit**

```bash
git add thirtyfour/src/session/handle.rs
git commit -m "refactor: make session_id() and derive_bidi_ws_url() public for BiDi"
```

---

### Task 3: Update WebDriver::bidi_connect_with_builder()

**Files:**
- Modify: `thirtyfour/src/web_driver.rs:275-304`

- [ ] **Step 1: Update URL resolution logic**

Replace the existing `bidi_connect_with_builder` method's URL resolution:

```rust
pub async fn bidi_connect_with_builder(
    &self,
    builder: crate::extensions::bidi::BiDiSessionBuilder,
) -> crate::error::WebDriverResult<crate::extensions::bidi::BiDiSession> {
    let ws_url = if let Some(ref base) = builder.custom_url_base {
        // Use custom base + session path (takes absolute precedence)
        let sid = self.handle.session_id();
        format!("{}/session/{}/se/bidi", base.trim_end_matches('/'), sid)
    } else if builder.use_server_url {
        self.handle.derive_bidi_ws_url()
    } else {
        match self.handle.config().bidi_connection_type {
            crate::common::config::BidiConnectionType::DeriveFromServerUrl => {
                self.handle.derive_bidi_ws_url()
            }
            crate::common::config::BidiConnectionType::UseHubProvided => {
                self.handle
                    .websocket_url
                    .as_deref()
                    .ok_or_else(|| {
                        crate::prelude::WebDriverError::BiDi(
                            "No webSocketUrl in session capabilities. \
                         Enable BiDi in your browser capabilities \
                         (e.g., for Chrome: set 'webSocketUrl: true')."
                                .to_string(),
                        )
                    })?.to_string()
            }
        }
    };

    builder.connect(&ws_url).await
}
```

- [ ] **Step 2: Commit**

```bash
git add thirtyfour/src/web_driver.rs
git commit -m "feat(bidi): support custom URL base in bidi_connect_with_builder()"
```

---

### Task 4: Update BiDiSessionBuilder::connect_with_driver()

**Files:**
- Modify: `thirtyfour/src/extensions/bidi/mod.rs:357-401`

- [ ] **Step 1: Update URL resolution logic**

Update the URL resolution in `connect_with_driver`:

```rust
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

    self.connect(&ws_url).await
}
```

- [ ] **Step 2: Commit**

```bash
git add thirtyfour/src/extensions/bidi/mod.rs
git commit -m "feat(bidi): support custom URL base in connect_with_driver()"
```

---

### Task 5: Add unit tests

**Files:**
- Test: `thirtyfour/src/extensions/bidi/mod.rs` (add to existing tests)
- Test: `thirtyfour/src/session/handle.rs` (add tests for public methods)

- [ ] **Step 1: Add test for url_base() validation**

Add to `thirtyfour/src/extensions/bidi/mod.rs`:

```rust
#[test]
#[should_panic(expected = "BiDi URL base must start with ws:// or wss://")]
fn test_url_base_invalid_scheme() {
    let _ = BiDiSessionBuilder::new().url_base("http://localhost:4444");
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
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cd thirtyfour && cargo test --lib url_base
```

Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add thirtyfour/src/extensions/bidi/mod.rs thirtyfour/src/session/handle.rs
git commit -m "test(bidi): add unit tests for url_base() validation"
```

---

### Task 6: Verify build and run existing tests

- [ ] **Step 1: Build the project**

```bash
cd thirtyfour && cargo build --features bidi
```

Expected: Build succeeds with no errors

- [ ] **Step 2: Run tests**

```bash
cd thirtyfour && cargo test --features bidi
```

Expected: All tests pass

- [ ] **Step 3: Commit any fixes**

```bash
git add -A && git commit -m "fix: address any build or test issues"
```

---

### Task 7: Final verification

- [ ] **Step 1: Review changes**

```bash
git log --oneline HEAD~6..HEAD
git diff HEAD~6..HEAD --stat
```

- [ ] **Step 2: Verify design spec alignment**

Check that implementation matches `docs/superpowers/specs/2026-03-19-bidi-custom-url-base-design.md`

- [ ] **Step 3: Final commit**

```bash
git tag -a v0.x.x -m "Release version with BiDi custom URL base support"
```

---

**Plan complete.**