use pastey::paste;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value, Value};

use crate::error::WebDriverResult;
use crate::CapabilitiesHelper;
use crate::{BrowserCapabilitiesHelper, Capabilities};

/// Capabilities for Firefox.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct FirefoxCapabilities {
    capabilities: Capabilities,
}

impl Default for FirefoxCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! firefox_arg_wrapper {
    ($($fname:ident => $opt:literal),*) => {

        paste! {
            $(
                #[doc = concat!("Set the ", $opt, " option.")]
                pub fn [<set_ $fname>](&mut self) -> WebDriverResult<()> {
                    self.add_arg($opt)
                }

                #[doc = concat!("Unset the ", $opt, " option.")]
                pub fn [<unset_ $fname>](&mut self) -> WebDriverResult<()> {
                    self.remove_arg($opt)
                }

                #[doc = concat!("Return true if the ", $opt, " option is set.")]
                pub fn [<is_ $fname>](&mut self) -> bool {
                    self.has_arg($opt)
                }
            )*
        }
    }
}

impl FirefoxCapabilities {
    /// Create a new `FirefoxCapabilities`.
    pub fn new() -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.insert("browserName".to_string(), json!("firefox"));
        FirefoxCapabilities {
            capabilities,
        }
    }

    /// Set the selenium logging preferences.
    ///
    /// To set the `geckodriver` log level, use `set_log_level()` instead.
    pub fn set_logging_prefs(
        &mut self,
        component: String,
        log_level: LoggingPrefsLogLevel,
    ) -> WebDriverResult<()> {
        self.set_base_capability("loggingPrefs", json!({ component: log_level }))
    }

    /// Get the `geckodriver` log level.
    pub fn log_level(&self) -> WebDriverResult<LogLevel> {
        let level: LogLevel = match self.browser_option("log") {
            Some(Value::Object(x)) => {
                x.get("level").map(|x| from_value(x.clone())).transpose()?.unwrap_or_default()
            }
            _ => LogLevel::default(),
        };
        Ok(level)
    }

    /// Set the `geckodriver` log level.
    pub fn set_log_level(&mut self, log_level: LogLevel) -> WebDriverResult<()> {
        self.insert_browser_option("log", json!({ "level": log_level }))
    }

    /// Set the start command for the firefox binary.
    pub fn set_firefox_binary(&mut self, start_cmd: &str) -> WebDriverResult<()> {
        self.insert_browser_option("binary", start_cmd)
    }

    /// Set the firefox preferences to use.
    pub fn set_preferences(&mut self, preferences: FirefoxPreferences) -> WebDriverResult<()> {
        self.insert_browser_option("prefs", preferences)
    }

    /// Get the firefox profile zip as a base64-encoded string.
    pub fn encoded_profile(&self) -> Option<String> {
        self.browser_option("profile")
    }

    /// Set the firefox profile to use.
    ///
    /// The profile must be a zipped, base64-encoded string of the profile directory.
    pub fn set_encoded_profile(&mut self, profile: &str) -> WebDriverResult<()> {
        self.insert_browser_option("profile", profile)
    }

    /// Add the specified command-line argument to `geckodriver`.
    pub fn add_arg(&mut self, arg: &str) -> WebDriverResult<()> {
        let mut args = self.args();
        let arg_string = arg.to_string();
        if !args.contains(&arg_string) {
            args.push(arg_string);
        }
        self.insert_browser_option("args", to_value(args)?)
    }

    firefox_arg_wrapper! {
        headless => "-headless"
    }
}

impl From<FirefoxCapabilities> for Capabilities {
    fn from(caps: FirefoxCapabilities) -> Capabilities {
        caps.capabilities
    }
}

impl CapabilitiesHelper for FirefoxCapabilities {
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

impl BrowserCapabilitiesHelper for FirefoxCapabilities {
    const KEY: &'static str = "moz:firefoxOptions";
}

/// LogLevel used by `geckodriver`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Trace log level.
    Trace,
    /// Debug log level.
    Debug,
    /// Config log level.
    Config,
    /// Info log level.
    Info,
    /// Warn log level.
    Warn,
    /// Error log level.
    Error,
    /// Fatal log level.
    Fatal,
    /// Default log level.
    #[default]
    Default,
}

/// Log level for the webdriver server.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LoggingPrefsLogLevel {
    /// Disable logging.
    Off,
    /// Severe log level.
    Severe,
    /// Warning log level.
    Warning,
    /// Info log level.
    Info,
    /// Config log level.
    Config,
    /// Fine log level.
    Fine,
    /// Finer log level.
    Finer,
    /// Finest log level.
    Finest,
    /// All logs.
    All,
}

/// Firefox preferences. See [`FirefoxCapabilities::set_preferences()`] for details.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct FirefoxPreferences {
    preferences: serde_json::Map<String, Value>,
}

impl FirefoxPreferences {
    /// Create a new `FirefoxPreferences` instance.
    pub fn new() -> Self {
        FirefoxPreferences::default()
    }

    /// Sets the specified firefox preference. This is a helper method for the various
    /// specific option methods.
    pub fn set<T>(&mut self, key: &str, value: T) -> WebDriverResult<()>
    where
        T: Serialize,
    {
        self.preferences.insert(key.into(), to_value(value)?);
        Ok(())
    }

    /// Unsets the specified firefox preference. This is a helper method for the various
    /// specific option methods.
    pub fn unset(&mut self, key: &str) -> WebDriverResult<()> {
        self.preferences.remove(key);
        Ok(())
    }

    /// Sets accept untrusted certs
    pub fn set_accept_untrusted_certs(&mut self, value: bool) -> WebDriverResult<()> {
        self.set("webdriver_accept_untrusted_certs", value)
    }

    /// Unsets accept untrusted certs
    pub fn unset_accept_untrusted_certs(&mut self) -> WebDriverResult<()> {
        self.unset("webdriver_accept_untrusted_certs")
    }

    /// Sets assume untrusted issuer
    pub fn set_assume_untrusted_issuer(&mut self, value: bool) -> WebDriverResult<()> {
        self.set("webdriver_assume_untrusted_issuer", value)
    }

    /// Unsets assume untrusted issuer
    pub fn unset_assume_untrusted_issuer(&mut self) -> WebDriverResult<()> {
        self.unset("webdriver_assume_untrusted_issuer")
    }

    /// Sets the log driver
    pub fn set_log_driver(&mut self, value: FirefoxProfileLogDriver) -> WebDriverResult<()> {
        self.set("webdriver.log.driver", value)
    }

    /// Unsets the log driver
    pub fn unset_log_driver(&mut self) -> WebDriverResult<()> {
        self.unset("webdriver.log.driver")
    }

    /// Sets the log file
    pub fn set_log_file(&mut self, value: String) -> WebDriverResult<()> {
        self.set("webdriver.log.file", value)
    }

    /// Unsets the log file
    pub fn unset_log_file(&mut self) -> WebDriverResult<()> {
        self.unset("webdriver.log.file")
    }

    /// Sets the load strategy
    pub fn set_load_strategy(&mut self, value: String) -> WebDriverResult<()> {
        self.set("webdriver.load.strategy", value)
    }

    /// Unsets the load strategy
    pub fn unset_load_strategy(&mut self) -> WebDriverResult<()> {
        self.unset("webdriver.load.strategy")
    }

    /// Sets the webdriver port
    pub fn set_webdriver_port(&mut self, value: u16) -> WebDriverResult<()> {
        self.set("webdriver_firefox_port", value)
    }

    /// Unsets the webdriver port
    pub fn unset_webdriver_port(&mut self) -> WebDriverResult<()> {
        self.unset("webdriver_firefox_port")
    }

    /// Sets the user agent
    pub fn set_user_agent(&mut self, value: String) -> WebDriverResult<()> {
        self.set("general.useragent.override", value)
    }

    /// Unsets the user agent
    pub fn unset_user_agent(&mut self) -> WebDriverResult<()> {
        self.unset("general.useragent.override")
    }
}

/// Log level for Firefox profile.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FirefoxProfileLogDriver {
    /// Debug log level.
    Debug,
    /// Info log level.
    Info,
    /// Warning log level.
    Warning,
    /// Error log level.
    Error,
    /// Disable logging.
    Off,
}
