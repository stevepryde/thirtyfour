use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename = "lowercase")]
pub enum ConnectionType {
    None,
    Cellular2G,
    Cellular3G,
    Cellular4G,
    Bluetooth,
    Ethernet,
    Wifi,
    Wimax,
    Other,
}

/// The simulated network conditions.
///
/// See <https://chromedevtools.github.io/devtools-protocol/tot/Network/#method-emulateNetworkConditions>.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename = "camelCase")]
pub struct NetworkConditions {
    /// True to simulate the network being offline.
    pub offline: bool,
    /// The latency to add, in milliseconds.
    pub latency: u32,
    /// The download throughput, in bytes/second.
    ///
    /// -1 disables download throttling.
    pub download_throughput: i32,
    /// The upload throughput, in bytes/second.
    ///
    /// -1 disables upload throttling.
    pub upload_throughput: i32,
    /// The connection type, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_type: Option<ConnectionType>,
}

impl Default for NetworkConditions {
    fn default() -> Self {
        Self {
            offline: false,
            latency: 0,
            download_throughput: -1,
            upload_throughput: -1,
            connection_type: None,
        }
    }
}

impl NetworkConditions {
    /// Create a new `NetworkConditions` instance.
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }
}
