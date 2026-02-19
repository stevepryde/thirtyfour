use std::path::Path;
use std::sync::Arc;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;

use super::WebExtensionCommand;
use crate::error::WebDriverResult;
use crate::extensions::permissions::Permissions;
use crate::session::handle::SessionHandle;

/// WebDriver BiDi WebExtension API for managing browser extensions.
///
/// This provides a browser-agnostic way to install and uninstall extensions
/// using the WebDriver BiDi protocol.
#[derive(Debug, Clone)]
pub struct WebExtension {
    handle: Arc<SessionHandle>,
}

impl WebExtension {
    /// Create a new WebExtension instance.
    pub fn new(handle: Arc<SessionHandle>) -> Self {
        Self {
            handle,
        }
    }

    /// Install a web extension from a directory path.
    ///
    /// Returns the extension ID on success.
    pub async fn install(&self, path: &str) -> WebDriverResult<String> {
        let r = self
            .handle
            .cmd(WebExtensionCommand::Install {
                path: Some(path.to_string()),
                archive_path: None,
                base64_value: None,
            })
            .await?;
        r.value()
    }

    /// Install a web extension from an archive file (.crx, .xpi, .zip).
    ///
    /// Returns the extension ID on success.
    pub async fn install_archive(&self, archive_path: &str) -> WebDriverResult<String> {
        let r = self
            .handle
            .cmd(WebExtensionCommand::Install {
                path: None,
                archive_path: Some(archive_path.to_string()),
                base64_value: None,
            })
            .await?;
        r.value()
    }

    /// Install a web extension from a base64-encoded string.
    ///
    /// Returns the extension ID on success.
    pub async fn install_base64(&self, base64_value: &str) -> WebDriverResult<String> {
        let r = self
            .handle
            .cmd(WebExtensionCommand::Install {
                path: None,
                archive_path: None,
                base64_value: Some(base64_value.to_string()),
            })
            .await?;
        r.value()
    }

    /// Install a web extension from a file, reading and encoding it as base64.
    ///
    /// Returns the extension ID on success.
    pub async fn install_from_file(&self, path: &Path) -> WebDriverResult<String> {
        let contents = std::fs::read(path)?;
        let base64_value = BASE64_STANDARD.encode(contents);
        self.install_base64(&base64_value).await
    }

    /// Install a web extension as a trusted extension.
    ///
    /// This installs the extension and grants common extension permissions
    /// to prevent permission prompts from blocking headless automation.
    ///
    /// Returns the extension ID on success.
    pub async fn install_trusted(&self, path: &str) -> WebDriverResult<String> {
        let extension_id = self.install(path).await?;
        self.grant_extension_permissions(&extension_id).await?;
        Ok(extension_id)
    }

    /// Install a web extension from an archive file as a trusted extension.
    ///
    /// This installs the extension and grants common extension permissions
    /// to prevent permission prompts from blocking headless automation.
    ///
    /// Returns the extension ID on success.
    pub async fn install_archive_trusted(&self, archive_path: &str) -> WebDriverResult<String> {
        let extension_id = self.install_archive(archive_path).await?;
        self.grant_extension_permissions(&extension_id).await?;
        Ok(extension_id)
    }

    /// Install a web extension from a base64-encoded string as a trusted extension.
    ///
    /// This installs the extension and grants common extension permissions
    /// to prevent permission prompts from blocking headless automation.
    ///
    /// Returns the extension ID on success.
    pub async fn install_base64_trusted(&self, base64_value: &str) -> WebDriverResult<String> {
        let extension_id = self.install_base64(base64_value).await?;
        self.grant_extension_permissions(&extension_id).await?;
        Ok(extension_id)
    }

    /// Install a web extension from a file as a trusted extension.
    ///
    /// This installs the extension and grants common extension permissions
    /// to prevent permission prompts from blocking headless automation.
    /// Supports .crx, .xpi, .zip and other extension formats.
    ///
    /// Returns the extension ID on success.
    pub async fn install_from_file_trusted(&self, path: &Path) -> WebDriverResult<String> {
        let extension_id = self.install_from_file(path).await?;
        self.grant_extension_permissions(&extension_id).await?;
        Ok(extension_id)
    }

    /// Grant common extension permissions for the given extension ID.
    ///
    /// This helps prevent permission prompts from blocking headless automation
    /// by pre-granting common permissions like clipboard, geolocation, etc.
    pub async fn grant_extension_permissions(&self, extension_id: &str) -> WebDriverResult<()> {
        let permissions = Permissions::new(self.handle.clone());

        let chrome_origin = format!("chrome-extension://{}", extension_id);
        let moz_origin = format!("moz-extension://{}", extension_id);

        let _ = permissions.grant_extension_permissions(&chrome_origin).await;
        let _ = permissions.grant_extension_permissions(&moz_origin).await;

        Ok(())
    }

    /// Grant specific permissions for an extension.
    ///
    /// # Arguments
    /// * `extension_id` - The extension ID
    /// * `permissions` - List of permission names to grant
    pub async fn grant_permissions(
        &self,
        extension_id: &str,
        permissions: &[&str],
    ) -> WebDriverResult<()> {
        let perms = Permissions::new(self.handle.clone());

        let chrome_origin = format!("chrome-extension://{}", extension_id);
        let moz_origin = format!("moz-extension://{}", extension_id);

        let _ = perms.grant_permissions(permissions, &chrome_origin).await;
        let _ = perms.grant_permissions(permissions, &moz_origin).await;

        Ok(())
    }

    /// Uninstall a web extension by its ID.
    pub async fn uninstall(&self, extension_id: &str) -> WebDriverResult<()> {
        self.handle
            .cmd(WebExtensionCommand::Uninstall {
                extension_id: extension_id.to_string(),
            })
            .await?;
        Ok(())
    }
}
