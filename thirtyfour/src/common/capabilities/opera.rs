use serde::Serialize;
use serde_json::{json, Value};

use crate::common::capabilities::chromium::ChromiumLikeCapabilities;
use crate::{BrowserCapabilitiesHelper, Capabilities, CapabilitiesHelper};

/// Capabilities for Opera.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct OperaCapabilities {
    capabilities: Capabilities,
}

impl Default for OperaCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl OperaCapabilities {
    /// Create a new `OperaCapabilities`.
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.insert("browserName".to_string(), json!("opera"));
        OperaCapabilities {
            capabilities,
        }
    }
}

impl From<OperaCapabilities> for Capabilities {
    fn from(caps: OperaCapabilities) -> Capabilities {
        caps.capabilities
    }
}

impl CapabilitiesHelper for OperaCapabilities {
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

impl BrowserCapabilitiesHelper for OperaCapabilities {
    const KEY: &'static str = "operaOptions";
}

impl ChromiumLikeCapabilities for OperaCapabilities {}
