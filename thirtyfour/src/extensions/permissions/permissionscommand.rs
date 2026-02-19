use http::Method;
use serde_json::json;

use crate::{common::command::FormatRequestData, RequestData};

/// BiDi Permissions commands for managing browser permissions.
#[derive(Debug)]
pub enum PermissionsCommand {
    /// Set a permission state.
    SetPermission {
        /// The permission descriptor (e.g., {"name": "geolocation"}).
        descriptor: serde_json::Value,
        /// The permission state: "granted", "denied", or "prompt".
        state: String,
        /// The origin for which to set the permission.
        origin: Option<String>,
    },
}

impl FormatRequestData for PermissionsCommand {
    fn format_request(&self, session_id: &crate::SessionId) -> RequestData {
        match &self {
            PermissionsCommand::SetPermission {
                descriptor,
                state,
                origin,
            } => {
                let mut body = json!({
                    "descriptor": descriptor,
                    "state": state,
                });
                if let Some(o) = origin {
                    body["origin"] = json!(o);
                }
                RequestData::new(
                    Method::POST,
                    format!("/session/{}/bidi/permissions/setPermission", session_id),
                )
                .add_body(body)
            }
        }
    }
}
