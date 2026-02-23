use crate::{common::command::FormatRequestData, RequestData};

use super::NetworkConditions;
use http::Method;
use serde_json::{json, Value};

/// Extra commands specific to Chrome.
#[derive(Debug)]
pub enum ChromeCommand {
    /// Launch the specified Chrome app.
    LaunchApp(String),
    /// Get the current simulated network conditions.
    GetNetworkConditions,
    /// Set the specified network conditions to simulate.
    SetNetworkConditions(NetworkConditions),
    /// Execute the specified Chrome `DevTools` Protocol command.
    ExecuteCdpCommand(String, Value),
    /// Get the current sinks.
    GetSinks,
    /// Get the issue message.
    GetIssueMessage,
    /// Set the specified sink to use.
    SetSinkToUse(String),
    /// Start tab mirroring.
    StartTabMirroring(String),
    /// Stop casting.
    StopCasting(String),
}

impl FormatRequestData for ChromeCommand {
    fn format_request(&self, session_id: &crate::SessionId) -> RequestData {
        match &self {
            ChromeCommand::LaunchApp(app_id) => {
                RequestData::new(Method::POST, format!("/session/{session_id}/chromium/launch_app"))
                    .add_body(json!({ "id": app_id }))
            }
            ChromeCommand::GetNetworkConditions => RequestData::new(
                Method::GET,
                format!("/session/{session_id}/chromium/network_conditions"),
            ),
            ChromeCommand::SetNetworkConditions(conditions) => RequestData::new(
                Method::POST,
                format!("/session/{session_id}/chromium/network_conditions"),
            )
            .add_body(json!({ "network_conditions": conditions })),
            ChromeCommand::ExecuteCdpCommand(command, params) => {
                RequestData::new(Method::POST, format!("/session/{session_id}/goog/cdp/execute"))
                    .add_body(json!({ "cmd": command, "params": params }))
            }
            ChromeCommand::GetSinks => {
                RequestData::new(Method::GET, format!("/session/{session_id}/goog/cast/get_sinks"))
            }
            ChromeCommand::GetIssueMessage => RequestData::new(
                Method::GET,
                format!("/session/{session_id}/goog/cast/get_issue_message"),
            ),
            ChromeCommand::SetSinkToUse(sink_name) => RequestData::new(
                Method::POST,
                format!("/session/{session_id}/goog/cast/set_sink_to_use"),
            )
            .add_body(json!({ "sinkName": sink_name })),
            ChromeCommand::StartTabMirroring(sink_name) => RequestData::new(
                Method::POST,
                format!("/session/{session_id}/goog/cast/start_tab_mirroring"),
            )
            .add_body(json!({ "sinkName": sink_name })),
            ChromeCommand::StopCasting(sink_name) => RequestData::new(
                Method::POST,
                format!("/session/{session_id}/goog/cast/stop_casting"),
            )
            .add_body(json!({ "sinkName": sink_name })),
        }
    }
}
