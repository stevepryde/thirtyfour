use serde::Serialize;
use serde_json::{json, Value};

use crate::{BrowserCapabilitiesHelper, Capabilities, CapabilitiesHelper};

/// Capabilities for Internet Explorer.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct InternetExplorerCapabilities {
    capabilities: Capabilities,
}

impl Default for InternetExplorerCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl InternetExplorerCapabilities {
    /// Create a new `InternetExplorerCapabilities`.
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.insert("browserName".to_string(), json!("internet explorer"));
        InternetExplorerCapabilities {
            capabilities,
        }
    }
}

impl From<InternetExplorerCapabilities> for Capabilities {
    fn from(caps: InternetExplorerCapabilities) -> Capabilities {
        caps.capabilities
    }
}

impl CapabilitiesHelper for InternetExplorerCapabilities {
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

impl BrowserCapabilitiesHelper for InternetExplorerCapabilities {
    const KEY: &'static str = "se:ieOptions";
}
