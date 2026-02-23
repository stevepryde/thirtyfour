use serde::Serialize;
use serde_json::{json, Value};

use crate::common::capabilities::chromium::ChromiumLikeCapabilities;
use crate::{BrowserCapabilitiesHelper, Capabilities, CapabilitiesHelper};

/// Capabilities for Chrome.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct ChromeCapabilities {
    capabilities: Capabilities,
}

impl Default for ChromeCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromeCapabilities {
    /// Create a new `ChromeCapabilities` struct.
    #[must_use] 
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.insert("browserName".to_string(), json!("chrome"));
        ChromeCapabilities {
            capabilities,
        }
    }
}

impl CapabilitiesHelper for ChromeCapabilities {
    fn get(&self, key: &str) -> Option<&Value> {
        self.capabilities.get(key)
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.capabilities.get_mut(key)
    }

    fn insert_base_capability(&mut self, key: String, value: Value) {
        self.capabilities.insert_base_capability(key, value);
    }
}

impl BrowserCapabilitiesHelper for ChromeCapabilities {
    const KEY: &'static str = "goog:chromeOptions";
}

impl ChromiumLikeCapabilities for ChromeCapabilities {}

impl From<ChromeCapabilities> for Capabilities {
    fn from(caps: ChromeCapabilities) -> Capabilities {
        caps.capabilities
    }
}
