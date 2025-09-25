use std::path::Path;

use base64::{prelude::BASE64_STANDARD, Engine};
use pastey::paste;
use serde::Serialize;
use serde_json::{json, to_value, Value};

use crate::error::WebDriverResult;
use crate::{BrowserCapabilitiesHelper, Capabilities, CapabilitiesHelper};

macro_rules! chromium_arg_wrapper {
    ($($fname:ident => $opt:literal),*) => {
        paste! {
            $(
                #[doc = concat!("Set the ", $opt, " option.")]
                fn [<set_ $fname>](&mut self) -> WebDriverResult<()> {
                    self.add_arg($opt)
                }

                #[doc = concat!("Unset the ", $opt, " option.")]
                fn [<unset_ $fname>](&mut self) -> WebDriverResult<()> {
                    self.remove_arg($opt)
                }

                #[doc = concat!("Return true if the ", $opt, " option is set.")]
                fn [<is_ $fname>](&mut self) -> bool {
                    self.has_arg($opt)
                }
            )*
        }
    }
}

/// Capabilities helper methods for all Chromium-based browsers.
pub trait ChromiumLikeCapabilities: BrowserCapabilitiesHelper {
    /// Get the current list of Chrome extensions as a vec.
    ///
    /// Each item is a base64-encoded string containing the .CRX extension file contents.
    /// Use `add_extension()` to add a new extension file.
    fn extensions(&self) -> Vec<String> {
        self.browser_option("extensions").unwrap_or_default()
    }

    /// Get the path to the chrome binary (if one was previously set).
    fn binary(&self) -> Option<String> {
        self.browser_option("binary")
    }

    /// Set the path to chrome binary to use.
    fn set_binary(&mut self, path: &str) -> WebDriverResult<()> {
        self.insert_browser_option("binary", path)
    }

    /// Unset the chrome binary path.
    fn unset_binary(&mut self) {
        self.remove_browser_option("binary");
    }

    /// Get the current debugger address (if one was previously set).
    fn debugger_address(&self) -> Option<String> {
        self.browser_option("debuggerAddress")
    }

    /// Set the debugger address.
    fn set_debugger_address(&mut self, address: &str) -> WebDriverResult<()> {
        self.insert_browser_option("debuggerAddress", address)
    }

    /// Unset the debugger address.
    fn unset_debugger_address(&mut self) {
        self.remove_browser_option("debuggerAddress");
    }

    /// Add the specified command-line argument to `chromedriver`.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let mut caps = DesiredCapabilities::chrome();
    /// caps.add_arg("--disable-local-storage")?;
    /// ```
    ///
    /// The full list of switches can be found here:
    /// [https://chromium.googlesource.com/chromium/src/+/master/chrome/common/chrome_switches.cc](https://chromium.googlesource.com/chromium/src/+/master/chrome/common/chrome_switches.cc)
    fn add_arg(&mut self, arg: &str) -> WebDriverResult<()> {
        let mut args = self.args();
        let arg_string = arg.to_string();
        if !args.contains(&arg_string) {
            args.push(arg_string);
        }
        self.insert_browser_option("args", to_value(args)?)
    }

    /// Add the specified experimental option.
    ///
    /// ## Example
    /// ```no_run
    /// use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;
    /// use thirtyfour::DesiredCapabilities;
    /// let mut caps = DesiredCapabilities::chrome();
    /// caps.add_experimental_option("excludeSwitches", vec!["--enable-logging"]).unwrap();
    /// ```
    fn add_experimental_option(
        &mut self,
        name: impl Into<String>,
        value: impl Serialize,
    ) -> WebDriverResult<()> {
        self.insert_browser_option(name, value)
    }

    /// Remove the specified experimental option.
    fn remove_experimental_option(&mut self, name: &str) {
        self.remove_browser_option(name);
    }

    /// Add a base64-encoded extension.
    fn add_encoded_extension(&mut self, extension_base64: &str) -> WebDriverResult<()> {
        let mut extensions = self.extensions();
        let ext_string = extension_base64.to_string();
        if !extensions.contains(&ext_string) {
            extensions.push(ext_string);
        }
        self.insert_browser_option("extensions", to_value(extensions)?)
    }

    /// Remove the specified base64-encoded extension if it had been added previously.
    fn remove_encoded_extension(&mut self, extension_base64: &str) -> WebDriverResult<()> {
        let mut extensions = self.extensions();
        if extensions.is_empty() {
            Ok(())
        } else {
            extensions.retain(|v| v != extension_base64);
            self.insert_browser_option("extensions", to_value(extensions)?)
        }
    }

    /// Add Chrome extension file. This will be a file with a .CRX extension.
    fn add_extension(&mut self, crx_file: &Path) -> WebDriverResult<()> {
        let contents = std::fs::read(crx_file)?;
        let b64_contents = BASE64_STANDARD.encode(contents);
        self.add_encoded_extension(&b64_contents)
    }

    /// Remove the specified Chrome extension file if an identical extension had been added
    /// previously.
    fn remove_extension(&mut self, crx_file: &Path) -> WebDriverResult<()> {
        let contents = std::fs::read(crx_file)?;
        let b64_contents = BASE64_STANDARD.encode(contents);
        self.remove_encoded_extension(&b64_contents)
    }

    /// Get the list of exclude switches.
    fn exclude_switches(&self) -> Vec<String> {
        self.browser_option("excludeSwitches").unwrap_or_default()
    }

    /// Add the specified arg to the list of exclude switches.
    fn add_exclude_switch(&mut self, arg: &str) -> WebDriverResult<()> {
        let mut args = self.exclude_switches();
        let arg_string = arg.to_string();
        if !args.contains(&arg_string) {
            args.push(arg_string);
        }
        self.add_experimental_option("excludeSwitches", to_value(args)?)
    }

    /// Remove the specified arg from the list of exclude switches.
    fn remove_exclude_switch(&mut self, arg: &str) -> WebDriverResult<()> {
        let mut args = self.exclude_switches();
        if args.is_empty() {
            Ok(())
        } else {
            args.retain(|v| v != arg);
            self.add_experimental_option("excludeSwitches", to_value(args)?)
        }
    }

    chromium_arg_wrapper! {
        headless => "--headless",
        disable_web_security => "--disable-web-security",
        ignore_certificate_errors => "--ignore-certificate-errors",
        no_sandbox => "--no-sandbox",
        disable_gpu => "--disable-gpu",
        disable_dev_shm_usage => "--disable-dev-shm-usage",
        disable_local_storage => "--disable-local-storage"
    }
}

/// Capabilities for Chromium.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct ChromiumCapabilities {
    capabilities: Capabilities,
}

impl Default for ChromiumCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromiumCapabilities {
    /// Create a new ChromeCapabilities struct.
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.insert("browserName".to_string(), json!("chromium"));
        ChromiumCapabilities {
            capabilities,
        }
    }
}

impl CapabilitiesHelper for ChromiumCapabilities {
    fn _get(&self, key: &str) -> Option<&Value> {
        self.capabilities._get(key)
    }

    fn _get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.capabilities._get_mut(key)
    }

    fn insert_base_capability(&mut self, key: String, value: Value) {
        self.capabilities.insert_base_capability(key, value);
    }
}

impl BrowserCapabilitiesHelper for ChromiumCapabilities {
    const KEY: &'static str = "goog:chromeOptions";
}

impl ChromiumLikeCapabilities for ChromiumCapabilities {}

impl From<ChromiumCapabilities> for Capabilities {
    fn from(caps: ChromiumCapabilities) -> Capabilities {
        caps.capabilities
    }
}
