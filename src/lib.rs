//! Thirtyfour is a Selenium / WebDriver library for Rust, for automated website UI testing.
//!
//! It supports the full W3C WebDriver spec.
//! Tested with Chrome and Firefox although any W3C-compatible WebDriver
//! should work.
//!
//! Async only (`tokio` and `async-std` runtimes supported via feature flags).
//! For synchronous support, use the [thirtyfour_sync](https://docs.rs/thirtyfour_sync) crate instead.
//!
//! ## Features
//!
//! - All W3C WebDriver and WebElement methods supported
//! - Async / await support (both **tokio** and **async-std** runtimes supported via feature flags)
//! - Synchronous support (use the `thirtyfour_sync` crate)
//! - Create new browser session directly via WebDriver (e.g. chromedriver)
//! - Create new browser session via Selenium Standalone or Grid
//! - Automatically close browser session on drop
//! - Find elements (via all common selectors e.g. Id, Class, CSS, Tag, XPath)
//! - Send keys to elements, including key-combinations
//! - Execute Javascript
//! - Action Chains
//! - Get and set cookies
//! - Switch to frame/window/element/alert
//! - Shadow DOM support
//! - Alert support
//! - Capture / Save screenshot of browser or individual element as PNG
//! - Chrome DevTools Protocol (CDP) support
//!
//! ## Feature Flags
//!
//! Support for **tokio** and **async-std** async runtimes, and support for synchronous http,
//! are provided via feature flags.
//!
//! * `tokio-runtime`: (Default) Use the **tokio** async runtime with the [reqwest](https://docs.rs/reqwest) http client.
//! * `async-std-runtime`: Use the **async-std** runtime with the [surf](https://docs.rs/surf) http client.
//!
//!     **NOTE**: You cannot combine `async-std-runtime` with `tokio-runtime`.
//!
//! There are four `reqwest-*-tls*`-features, which enable the respective features in the `reqwest` dependency:
//! - **reqwest-default-tls** *(enabled by default)*: Provides TLS support to connect over HTTPS.
//! - **reqwest-native-tls**: Enables TLS functionality provided by `native-tls`.
//! - **reqwest-native-tls-vendored**: Enables the `vendored` feature of `native-tls`.
//! - **reqwest-rustls-tls**: Enables TLS functionality provided by `rustls`.
//!
//! ## Examples
//!
//! The following examples assume you have a selenium server running
//! at localhost:4444, and a demo web app running at http://webappdemo
//!
//! You can set these up using docker-compose, as follows:
//!
//! ```ignore
//! docker-compose up -d
//! ```
//!
//! The included web app demo is purely for demonstration / unit testing
//! purposes and is not required in order to use this library in other projects.
//!
//! ### Example (async):
//!
//! ```rust
//! # #[cfg(all(feature = "tokio-runtime", not(feature = "async-std-runtime")))] {
//! use thirtyfour::prelude::*;
//! use tokio;
//!
//! #[tokio::main]
//! async fn main() -> WebDriverResult<()> {
//!     let caps = DesiredCapabilities::chrome();
//!     let driver = WebDriver::new("http://localhost:4444/wd/hub", &caps).await?;
//!
//!     // Navigate to URL.
//!     driver.get("http://webappdemo").await?;
//!
//!     // Navigate to page, by chaining futures together and awaiting the result.
//!     driver.find_element(By::Id("pagetextinput")).await?.click().await?;
//!
//!     // Find element.
//!     let elem_div = driver.find_element(By::Css("div[data-section='section-input']")).await?;
//!
//!     // Find element from element.
//!     let elem_text = elem_div.find_element(By::Name("input1")).await?;
//!
//!     // Type in the search terms.
//!     elem_text.send_keys("selenium").await?;
//!
//!     // Click the button.
//!     let elem_button = elem_div.find_element(By::Tag("button")).await?;
//!     elem_button.click().await?;
//!
//!     // Get text value of element.
//!     let elem_result = driver.find_element(By::Id("input-result")).await?;
//!     assert_eq!(elem_result.text().await?, "selenium");
//!
//!     Ok(())
//! }
//! # }
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::needless_doctest_main)]

pub use alert::Alert;
pub use common::requestdata::{RequestData, RequestMethod};
pub use common::{
    capabilities::{
        chrome::ChromeCapabilities, desiredcapabilities::*, edge::EdgeCapabilities,
        firefox::FirefoxCapabilities, ie::InternetExplorerCapabilities, opera::OperaCapabilities,
        safari::SafariCapabilities,
    },
    command::{By, ExtensionCommand},
    cookie::Cookie,
    keys::{Keys, TypingData},
    scriptargs::ScriptArgs,
    types::*,
};

pub use session::WebDriverSession;
pub use switch_to::SwitchTo;
pub use webdriver::GenericWebDriver;
pub use webdriver::WebDriver;
pub use webdrivercommands::WebDriverCommands;
pub use webelement::WebElement;

#[cfg(feature = "persist")]
pub use webdriver::persist::PersistedSession;

/// Allow importing the common async structs via `use thirtyfour::prelude::*`.
pub mod prelude {
    pub use crate::alert::Alert;
    pub use crate::error::WebDriverResult;
    pub use crate::switch_to::SwitchTo;
    pub use crate::webdriver::WebDriver;
    pub use crate::webdrivercommands::{ScriptRet, WebDriverCommands};
    pub use crate::webelement::WebElement;
    pub use crate::{By, Cookie, DesiredCapabilities, Keys, ScriptArgs, TypingData};
}

/// Action chains allow for more complex user interactions with the keyboard and mouse.
pub mod action_chain;
mod alert;
mod runtime;
mod session;
/// Miscellaneous support functions for `thirtyfour` tests.
pub mod support;
mod switch_to;
mod webdriver;
mod webdrivercommands;
mod webelement;

/// Async HTTP client traits.
pub mod http {
    pub mod connection_async;
    #[cfg(not(any(feature = "tokio-runtime", feature = "async-std-runtime")))]
    pub mod nulldriver_async;
    #[cfg(all(feature = "tokio-runtime", not(feature = "async-std-runtime")))]
    pub mod reqwest_async;
    #[cfg(feature = "async-std-runtime")]
    pub mod surf_async;
}

/// Common types used by both async and sync implementations.
pub mod common {
    pub mod action;
    pub mod capabilities {
        pub mod chrome;
        pub mod desiredcapabilities;
        pub mod edge;
        pub mod firefox;
        pub mod ie;
        pub mod opera;
        pub mod safari;
    }
    pub mod command;
    pub mod config;
    pub mod connection_common;
    pub mod cookie;
    pub mod keys;
    pub mod requestdata;
    pub mod scriptargs;
    pub mod types;
}

/// Extensions for specific browsers.
pub mod extensions {
    /// Extensions for working with Chromium-based browsers.
    pub mod chrome {
        mod chromecommand;
        mod devtools;
        mod networkconditions;

        pub use chromecommand::ChromeCommand;
        pub use devtools::ChromeDevTools;
        pub use networkconditions::NetworkConditions;
    }
}

/// Wrappers for specific component types.
pub mod components {
    /// Wrapper for `<select>` elements.
    pub mod select;
}

/// Error types.
pub mod error;
