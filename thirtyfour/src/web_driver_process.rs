use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, sleep};
use std::time::Duration;

use libc::atexit;
use selenium_manager::get_manager_by_browser;

use crate::error::WebDriverResult;
use crate::prelude::WebDriverError;
use crate::CapabilitiesHelper;

#[derive(Debug)]
/// This uses the Selenium Manager to automatically download the appropriate web
/// driver for the specified browser. It then runs the webdriver process. Be
/// sure to call `quit()` to properly shut down the driver before the program
/// exits.
pub struct WebDriverProcess {
    /// A handle to the webdriver process; requires the selenium manager feature.
    pub webdriver_process: Child,
}

/// Define the port used by the webdriver, either in the server url used to access
/// the webdriver or as the port contained in that URL.
#[derive(Debug)]
pub enum WebDriverProcessPort<'a> {
    /// The server's URL. If it contains a port number, the webdriver will run
    /// on that port. Otherwise, it will run on the webdriver's default port.
    ServerUrl(&'a str),
    /// The port which the webdriver process will use.
    Port(u16),
}

/// Define the browser used by the webdriver, either through the capabilities
/// for the select browser or directly as the browser's name.
#[derive(Debug)]
pub enum WebDriverProcessBrowser<'a, C> {
    /// The capabilities struct which describes this browser. The browser's name
    /// will be extract from this.
    Caps(&'a C),
    /// A string containing the name of the browser.
    Name(String),
}

impl WebDriverProcess {
    /// Download then run a webdriver.
    pub fn new<C>(
        web_driver_process_port: WebDriverProcessPort<'_>,
        web_driver_process_browser: WebDriverProcessBrowser<'_, C>,
        allow_offline: bool,
    ) -> WebDriverResult<Self>
    where
        C: CapabilitiesHelper,
    {
        use url::Url;

        // Determine the port from the provided enum.
        let port = match web_driver_process_port {
            WebDriverProcessPort::ServerUrl(server_url) => {
                let url = Url::parse(server_url)
                    .map_err(|e| WebDriverError::ParseError(format!("invalid url: {e}")))?;
                url.port()
            }
            WebDriverProcessPort::Port(port) => Some(port),
        };

        // Determine the browser name from the provided enum.
        let browser_name = match web_driver_process_browser {
            WebDriverProcessBrowser::Caps(capabilities) => capabilities
                ._get("browserName")
                .ok_or(WebDriverError::ParseError(
                    "browserName not found in capabilities".to_string(),
                ))?
                .as_str()
                .ok_or(WebDriverError::ParseError("browserName not a string".to_string()))?
                .to_string(),
            WebDriverProcessBrowser::Name(browser_name) => browser_name,
        };

        // Optionally download the web driver binary, then get a path to it.
        // The Selenium manager starts a Tokio runtime internally, which conflicts
        // with the Tokio runtime in this thread. So, run it in a separate thread.
        let driver_thread = thread::spawn(move || {
            let mut selenium_manager = match get_manager_by_browser(browser_name) {
                Err(err) => return Err(err),
                Ok(s) => s,
            };
            match selenium_manager.setup() {
                Ok(p) => Ok(p),
                Err(err) => {
                    if selenium_manager.is_fallback_driver_from_cache() && allow_offline {
                        if let Some(best_driver_from_cache) =
                            selenium_manager.find_best_driver_from_cache().unwrap()
                        {
                            Ok(best_driver_from_cache)
                        } else {
                            Err(err)
                        }
                    } else {
                        Err(err)
                    }
                }
            }
        });
        let driver_path = driver_thread
            .join()
            .map_err(|e| WebDriverError::FatalError(format!("Thread panicked: {e:?}")))?
            .map_err(|e| WebDriverError::FatalError(e.to_string()))?;

        // Start the web driver process.
        let mut webdriver_command = Command::new(driver_path);
        if let Some(port) = port {
            webdriver_command.args([format!("--port={port}")]);
        }
        let mut webdriver_process = webdriver_command.spawn().map_err(|e| {
            WebDriverError::FatalError(format!("Error running webdriver process: {e}"))
        })?;

        // Wait for it to start up; check for an early exit. Avoid using tokio,
        // since this is placed in a `OnceLock` that may be accessed after the
        // tokio loop shuts down (see `start_webdriver_process_full`).
        sleep(Duration::from_millis(1000));
        if let Some(status) = webdriver_process.try_wait()? {
            return Err(WebDriverError::FatalError(format!(
                "Webdriver process exited with status code: {status}"
            )));
        };

        Ok(WebDriverProcess {
            webdriver_process,
        })
    }

    /// You must **always** call this method before exiting the program. It
    /// shuts down the web driver process. It does not use tokio to wait,
    /// because it may be called after the tokio loop has exited.
    pub fn quit(&mut self) -> WebDriverResult<()> {
        self.webdriver_process.kill()?;
        self.webdriver_process.wait().map_err(WebDriverError::IoError)?;
        Ok(())
    }
}

/// A single instance of the web driver process. TODO: add one per type of
/// support driver (chrome, firefox, safari, etc.) so we could have all
/// drivers running at the same time.
static WEB_DRIVER_PROCESS: OnceLock<WebDriverResult<Mutex<WebDriverProcess>>> = OnceLock::new();

/// Start a web driver process by downloading the appropriate driver if necessary,
/// then starting the process. When this application exits, automatically stop
/// the web driver process. This only starts the process once, regardless of
/// how often it is called.
pub fn start_webdriver_process_full<'a, C>(
    web_driver_process_port: WebDriverProcessPort,
    web_driver_process_browser: WebDriverProcessBrowser<C>,
    allow_offline: bool,
) -> Result<(), &'a WebDriverError>
where
    C: CapabilitiesHelper,
{
    match WEB_DRIVER_PROCESS.get_or_init(|| {
        if unsafe { atexit(on_exit_handler) } != 0 {
            return Err(WebDriverError::FatalError(
                "Unable to register atexit handler.".to_string(),
            ));
        }
        let webdriver_process = WebDriverProcess::new(
            web_driver_process_port,
            web_driver_process_browser,
            allow_offline,
        )?;
        Ok(Mutex::new(webdriver_process))
    }) {
        Err(e) => Err(e),
        Ok(_) => Ok(()),
    }
}

/// The most common case: call `start_webdriver_process_full` with the provided
/// parameters.
pub fn start_webdriver_process<'a, C>(
    server_url: &str,
    capabilities: &C,
    allow_offline: bool,
) -> Result<(), &'a WebDriverError>
where
    C: CapabilitiesHelper,
{
    start_webdriver_process_full(
        WebDriverProcessPort::ServerUrl(server_url),
        WebDriverProcessBrowser::Caps(capabilities),
        allow_offline,
    )
}

/// This function is called by the C `atexit()` handler when the program exits.
/// It attempts to shut down the web driver process.
extern "C" fn on_exit_handler() {
    // Per the C spec, this function must not call `quit()`. Therefore, avoid
    // using `unwrap()`, `expect()`, etc. Instead, report any errors to stderr
    // then exit without shutting down the web driver process.
    let Some(web_driver_process_result) = WEB_DRIVER_PROCESS.get() else {
        eprintln!("Web driver process not initialized.");
        return;
    };
    // If there was an error running the process, then don't do any shutdown.
    let Ok(web_driver_process_mutex) = web_driver_process_result else {
        return;
    };
    let mut web_driver_process = match web_driver_process_mutex.lock() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Unable to acquire lock for WebDriverProcess: {err}");
            return;
        }
    };
    if let Err(err) = web_driver_process.quit() {
        eprintln!("Unable to shut down web driver process: {err}");
    };
}
