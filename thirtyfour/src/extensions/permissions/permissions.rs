use std::sync::Arc;

use serde_json::json;

use super::PermissionsCommand;
use crate::error::WebDriverResult;
use crate::session::handle::SessionHandle;

/// Permission states for the BiDi Permissions API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    /// Permission is granted.
    Granted,
    /// Permission is denied.
    Denied,
    /// Permission requires user prompt.
    Prompt,
}

impl std::fmt::Display for PermissionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionState::Granted => write!(f, "granted"),
            PermissionState::Denied => write!(f, "denied"),
            PermissionState::Prompt => write!(f, "prompt"),
        }
    }
}

/// Common permission names for extensions.
pub mod extension_permissions {
    /// Clipboard read/write access.
    pub const CLIPBOARD_READ: &str = "clipboard-read";
    /// Clipboard write access.
    pub const CLIPBOARD_WRITE: &str = "clipboard-write";
    /// Geolocation access.
    pub const GEOLOCATION: &str = "geolocation";
    /// Camera access.
    pub const CAMERA: &str = "camera";
    /// Microphone access.
    pub const MICROPHONE: &str = "microphone";
    /// Notifications.
    pub const NOTIFICATIONS: &str = "notifications";
    /// Persistent storage.
    pub const PERSISTENT_STORAGE: &str = "persistent-storage";
    /// All common extension permissions.
    pub const ALL: &[&str] = &[
        CLIPBOARD_READ,
        CLIPBOARD_WRITE,
        GEOLOCATION,
        CAMERA,
        MICROPHONE,
        NOTIFICATIONS,
        PERSISTENT_STORAGE,
    ];
}

/// BiDi Permissions API for managing browser permissions.
///
/// This provides a browser-agnostic way to set permission states
/// using the WebDriver BiDi protocol.
#[derive(Debug, Clone)]
pub struct Permissions {
    handle: Arc<SessionHandle>,
}

impl Permissions {
    /// Create a new Permissions instance.
    pub fn new(handle: Arc<SessionHandle>) -> Self {
        Self {
            handle,
        }
    }

    /// Set a permission state for a given permission name.
    ///
    /// # Arguments
    /// * `name` - The permission name (e.g., "geolocation", "camera")
    /// * `state` - The permission state to set
    /// * `origin` - Optional origin for which to set the permission
    pub async fn set_permission(
        &self,
        name: &str,
        state: PermissionState,
        origin: Option<&str>,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(PermissionsCommand::SetPermission {
                descriptor: json!({ "name": name }),
                state: state.to_string(),
                origin: origin.map(String::from),
            })
            .await?;
        Ok(())
    }

    /// Grant all common extension permissions for the given origin.
    ///
    /// This is useful for headless automation where permission prompts
    /// would block execution.
    pub async fn grant_extension_permissions(&self, origin: &str) -> WebDriverResult<()> {
        for perm in extension_permissions::ALL {
            self.set_permission(perm, PermissionState::Granted, Some(origin)).await?;
        }
        Ok(())
    }

    /// Grant specific permissions for the given origin.
    pub async fn grant_permissions(
        &self,
        permissions: &[&str],
        origin: &str,
    ) -> WebDriverResult<()> {
        for perm in permissions {
            self.set_permission(perm, PermissionState::Granted, Some(origin)).await?;
        }
        Ok(())
    }
}
