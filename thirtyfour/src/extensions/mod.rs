/// Extensions for working with Firefox Addons.
pub mod addons;
/// WebDriver BiDi (Bidirectional Protocol) for real-time event handling.
#[cfg(feature = "bidi")]
pub mod bidi;
/// Extensions for Chrome Devtools Protocol
pub mod cdp;
// ElementQuery and ElementWaiter interfaces.
/// BiDi Permissions API for managing browser permissions.
pub mod permissions;
pub mod query;
/// WebDriver BiDi WebExtension API for managing browser extensions.
pub mod webextension;
