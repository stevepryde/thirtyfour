use serde::Serialize;
use serde_json::{json, Value};

use crate::{Capabilities, CapabilitiesHelper};

/// Capabilities for Safari.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct SafariCapabilities {
    capabilities: Capabilities,
}

impl Default for SafariCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl SafariCapabilities {
    /// Create a new `SafariCapabilities`.
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.insert("browserName".to_string(), json!("safari"));
        SafariCapabilities {
            capabilities,
        }
    }
}

impl From<SafariCapabilities> for Capabilities {
    fn from(caps: SafariCapabilities) -> Capabilities {
        caps.capabilities
    }
}

impl CapabilitiesHelper for SafariCapabilities {
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
