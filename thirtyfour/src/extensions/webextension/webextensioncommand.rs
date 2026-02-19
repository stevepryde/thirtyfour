use http::Method;
use serde_json::json;

use crate::{common::command::FormatRequestData, RequestData};

/// WebDriver BiDi WebExtension commands for managing browser extensions.
#[derive(Debug)]
pub enum WebExtensionCommand {
    /// Install a web extension.
    Install {
        /// Path to an extension directory.
        path: Option<String>,
        /// Path to an extension archive file (.crx, .xpi, .zip).
        archive_path: Option<String>,
        /// Base64 encoded string of the extension archive.
        base64_value: Option<String>,
    },
    /// Uninstall a web extension.
    Uninstall {
        /// The ID of the extension to uninstall.
        extension_id: String,
    },
}

impl FormatRequestData for WebExtensionCommand {
    fn format_request(&self, session_id: &crate::SessionId) -> RequestData {
        match &self {
            WebExtensionCommand::Install {
                path,
                archive_path,
                base64_value,
            } => {
                let mut body = json!({});
                if let Some(p) = path {
                    body["path"] = json!(p);
                }
                if let Some(ap) = archive_path {
                    body["archivePath"] = json!(ap);
                }
                if let Some(b64) = base64_value {
                    body["base64Value"] = json!(b64);
                }
                RequestData::new(
                    Method::POST,
                    format!("/session/{}/bidi/webExtension/install", session_id),
                )
                .add_body(body)
            }
            WebExtensionCommand::Uninstall {
                extension_id,
            } => RequestData::new(
                Method::POST,
                format!("/session/{}/bidi/webExtension/uninstall", session_id),
            )
            .add_body(json!({ "extensionId": extension_id })),
        }
    }
}
