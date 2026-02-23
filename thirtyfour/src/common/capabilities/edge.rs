use serde::Serialize;
use serde_json::{json, Value};

use crate::common::capabilities::chromium::ChromiumLikeCapabilities;
use crate::{BrowserCapabilitiesHelper, Capabilities, CapabilitiesHelper};

/// Capabilities for Microsoft Edge.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct EdgeCapabilities {
    capabilities: Capabilities,
}

impl Default for EdgeCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgeCapabilities {
    /// Create a new `EdgeCapabilities`.
    #[must_use] 
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.insert("browserName".to_string(), json!("MicrosoftEdge"));
        EdgeCapabilities {
            capabilities,
        }
    }
}

impl From<EdgeCapabilities> for Capabilities {
    fn from(caps: EdgeCapabilities) -> Capabilities {
        caps.capabilities
    }
}

impl CapabilitiesHelper for EdgeCapabilities {
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

impl BrowserCapabilitiesHelper for EdgeCapabilities {
    const KEY: &'static str = "ms:edgeOptions";
}

impl ChromiumLikeCapabilities for EdgeCapabilities {}
