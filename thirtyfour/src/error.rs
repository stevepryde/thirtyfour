use base64::DecodeError;
use serde::Deserialize;
use std::fmt::{Display, Formatter, Write};
use std::ops::{Deref, DerefMut};

/// Type def for Result<T, `WebDriverError`>.
pub type WebDriverResult<T> = Result<T, WebDriverError>;

fn indent_lines(message: &str, indent: usize) -> String {
    struct IdentLines<'a>(&'a str, usize);

    impl Display for IdentLines<'_> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            for (i, s) in self.0.split('\n').enumerate() {
                if i != 0 {
                    f.write_char('\n')?;
                }
                write!(f, "{:i$}{s}", " ", i = self.1)?;
            }
            Ok(())
        }
    }

    IdentLines(message, indent).to_string()
}

/// Struct representing the error value returned by the `WebDriver` server.
#[derive(Debug, Deserialize, Clone)]
pub struct WebDriverErrorValue {
    /// The `WebDriver` error message.
    pub message: String,
    /// This error is returned from the `WebDriver`.
    pub error: Option<String>,
    /// This stacktrace is returned from the `WebDriver`.
    pub stacktrace: Option<String>,
    /// This data is returned from the `WebDriver`.
    pub data: Option<serde_json::Value>,
}

impl WebDriverErrorValue {
    /// Create a new `WebDriverErrorValue`.
    #[must_use] 
    pub fn new(message: String) -> Self {
        Self {
            message,
            error: None,
            stacktrace: None,
            data: None,
        }
    }
}

impl Display for WebDriverErrorValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let stacktrace = self
            .stacktrace
            .as_ref()
            .map(|x| format!("Stacktrace:\n{}", indent_lines(x, 4)))
            .unwrap_or_default();
        let error = self.error.as_ref().map(|x| format!("Error: {x}")).unwrap_or_default();
        let data = self
            .data
            .as_ref()
            .map(|x| format!("Data:\n{}", indent_lines(&format!("{x:#?}"), 4)))
            .unwrap_or_default();
        let lines: Vec<String> = vec![self.message.clone(), error, data, stacktrace]
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect();
        write!(f, "{}", lines.join("\n"))
    }
}

/// Struct representing the error information returned by the `WebDriver` server.
#[derive(Debug, Deserialize, Clone)]
pub struct WebDriverErrorInfo {
    /// The HTTP status code of the response.
    #[serde(skip)]
    pub status: u16,
    /// The `WebDriver` error state.
    #[serde(default, rename(deserialize = "state"))]
    pub error: String,
    /// The `WebDriver` error value.
    pub value: WebDriverErrorValue,
}

impl WebDriverErrorInfo {
    /// Create a new `WebDriverErrorInfo`.
    #[must_use] 
    pub fn new(message: String) -> Self {
        Self {
            status: 0,
            error: message.clone(),
            value: WebDriverErrorValue::new(message),
        }
    }
}

impl Display for WebDriverErrorInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut msg = String::new();
        if self.status != 0 {
            msg.push_str(&format!("\nStatus: {}\n", self.status));
        }
        if !self.error.is_empty() {
            msg.push_str(&format!("State: {}\n", self.error));
        }
        let additional_info =
            format!("Additional info:\n{}", indent_lines(&self.value.to_string(), 4),);
        msg.push_str(&additional_info);

        write!(f, "{}", indent_lines(&msg, 4))
    }
}

/// `WebDriverError` is the main error type for thirtyfour
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct WebDriverError(Box<WebDriverErrorInner>);

macro_rules! make_enum_variant_func {
    ($enum_name: ident $variant_name: ident()) => {
        #[allow(non_snake_case)]
        #[allow(missing_docs)]
        pub fn $variant_name() -> Self {
            Self::from_inner($enum_name::$variant_name())
        }
    };
    ($enum_name: ident $variant_name: ident($_1: ty)) => {
        #[allow(non_snake_case)]
        #[allow(missing_docs)]
        pub fn $variant_name(a: $_1) -> Self {
            Self::from_inner($enum_name::$variant_name(a))
        }
    };
    ($enum_name: ident $variant_name: ident($_1: ty, $_2: ty)) => {
        #[allow(non_snake_case)]
        #[allow(missing_docs)]
        pub fn $variant_name(a: $_1, b: $_2) -> Self {
            Self::from_inner($enum_name::$variant_name(a, b))
        }
    };

    ($($other:tt)*) => {
        compile_error!(concat!("Unknown variant: ", (stringify!($($other)*))))
    };
}

macro_rules! impl_from_for_variant {
    (#[from] $ty:ty) => {
        impl From<$ty> for WebDriverError {
            fn from(val: $ty) -> Self {
                Self::from_inner((val).into())
            }
        }
    };

    ($($ty:ty),+) => {

    };

    ($($other:tt)*) => {
        compile_error!(concat!("Unknown variant: ", stringify!($($other)*)));
    };
}

macro_rules! webdriver_err {
    (
        $(#[$($outer_attr:tt)*])*
        pub enum $enum_name: ident {
            $(
                $(#[$($variant_attr:tt)*])*
                $variant_name: ident($($(#[$($ty_attr:tt)*])* $variant_tys:ty),*)
            ),+
            $(,)?
        }
    ) => {
        $(#[$($outer_attr)*])*
        pub enum $enum_name {
            $(
                $(#[$($variant_attr)*])*
                $variant_name($($(#[$($ty_attr)*])* $variant_tys),*)
            ),+
        }

        impl WebDriverError {
            $(
            make_enum_variant_func! {
                $enum_name $variant_name($($variant_tys),*)
            }
            )+
        }

        $(
            impl_from_for_variant! {
                $($(#[$($ty_attr)*])* $variant_tys),*
            }
        )+
    };
}

webdriver_err! {
    /// Represents all errors Given out by thirtyfour
    #[allow(missing_docs)]
    #[non_exhaustive]
    #[derive(Debug, thiserror::Error)]
    pub enum WebDriverErrorInner {
        #[error("The WebDriver server returned an unrecognised response: {0} :: {1}")]
        UnknownResponse(u16, String),
        #[error("Failed to send request to webdriver: {0}")]
        RequestFailed(String),
        #[error("The requested item '{0}' was not found: {1}")]
        NotFound(String, String),
        #[error("parse error: {0}")]
        ParseError(String),
        #[error("operation timed out: {0}")]
        Timeout(String),
        #[error("Unable to parse JSON: {0}")]
        Json(String),
        #[error("Unable to decode base64: {0}")]
        DecodeError(#[from] DecodeError),
        #[error("IO Error: {0}")]
        IoError(#[from] std::io::Error),
        #[error("The WebDriver request returned an error: {0}")]
        HttpError(String),
        #[error("The WebDriver response does not conform to the W3C WebDriver spec: {0}")]
        NotInSpec(WebDriverErrorInfo),
        #[error("The click event was intercepted by another element: {0}")]
        ElementClickIntercepted(WebDriverErrorInfo),
        #[error("The element is not interactable: {0}")]
        ElementNotInteractable(WebDriverErrorInfo),
        #[error("The certificate is insecure: {0}")]
        InsecureCertificate(WebDriverErrorInfo),
        #[error("An argument passed to the WebDriver server was invalid: {0}")]
        InvalidArgument(WebDriverErrorInfo),
        #[error("An argument passed to the WebDriver server was invalid: {0}")]
        InvalidUrl(url::ParseError),
        #[error("Invalid cookie domain: {0}")]
        InvalidCookieDomain(WebDriverErrorInfo),
        #[error("The element is in an invalid state: {0}")]
        InvalidElementState(WebDriverErrorInfo),
        #[error("The specified element selector is invalid: {0}")]
        InvalidSelector(WebDriverErrorInfo),
        #[error("The WebDriver session id is invalid: {0}")]
        InvalidSessionId(WebDriverErrorInfo),
        #[error("The Javascript code returned an error: {0}")]
        JavascriptError(WebDriverErrorInfo),
        #[error("Unable to scroll the element into the viewport: {0}")]
        MoveTargetOutOfBounds(WebDriverErrorInfo),
        #[error("Alert not found: {0}")]
        NoSuchAlert(WebDriverErrorInfo),
        #[error("Cookie not found: {0}")]
        NoSuchCookie(WebDriverErrorInfo),
        #[error("Element not found: {0}")]
        NoSuchElement(WebDriverErrorInfo),
        #[error("Frame not found: {0}")]
        NoSuchFrame(WebDriverErrorInfo),
        #[error("Window not found: {0}")]
        NoSuchWindow(WebDriverErrorInfo),
        #[error("The Javascript code did not complete within the script timeout (see WebDriver::set_script_timeout()): {0}")]
        ScriptTimeout(WebDriverErrorInfo),
        #[error("Unable to create WebDriver session: {0}")]
        SessionNotCreated(WebDriverErrorInfo),
        #[error("Element is stale: {0}")]
        StaleElementReference(WebDriverErrorInfo),
        #[error("Operation timed out: {0}")]
        WebDriverTimeout(WebDriverErrorInfo),
        #[error("Unable to set cookie: {0}")]
        UnableToSetCookie(WebDriverErrorInfo),
        #[error("Unable to capture screenshot: {0}")]
        UnableToCaptureScreen(WebDriverErrorInfo),
        #[error("An unexpected alert is currently open: {0}")]
        UnexpectedAlertOpen(WebDriverErrorInfo),
        #[error("Unknown command: {0}")]
        UnknownCommand(WebDriverErrorInfo),
        #[error("Unknown error: {0}")]
        UnknownError(WebDriverErrorInfo),
        #[error("Unknown method: {0}")]
        UnknownMethod(WebDriverErrorInfo),
        #[error("Unsupport operation: {0}")]
        UnsupportedOperation(WebDriverErrorInfo),
        #[error("Something caused the session to terminate.")]
        FatalError(String),
        #[error("Failed to receive command: {0}")]
        CommandRecvError(String),
        #[error("The command could not be sent to the session: {0}")]
        CommandSendError(String),
        #[error("Could not create session: {0}")]
        SessionCreateError(String),
    }
}

impl WebDriverError {
    /// Create a new `WebDriverError` by parsing the response from the `WebDriver` server.
    #[must_use] 
    pub fn parse(status: u16, body: String) -> Self {
        let body_json = match serde_json::from_str(&body) {
            Ok(x) => x,
            Err(_) => {
                return Self::from_inner(WebDriverErrorInner::UnknownResponse(status, body));
            }
        };

        let mut payload: WebDriverErrorInfo = match serde_json::from_value(body_json) {
            Ok(x) => x,
            Err(_) => return Self::from_inner(WebDriverErrorInner::UnknownResponse(status, body)),
        };

        payload.status = status;
        let mut error = payload.error.clone();
        if error.is_empty() {
            error = payload.value.error.clone().unwrap_or_default();
            if error.is_empty() {
                return Self::from_inner(WebDriverErrorInner::NotInSpec(payload));
            }
        }

        Self::from_inner(match error.as_str() {
            "element click intercepted" => WebDriverErrorInner::ElementClickIntercepted(payload),
            "element not interactable" => WebDriverErrorInner::ElementNotInteractable(payload),
            "insecure certificate" => WebDriverErrorInner::InsecureCertificate(payload),
            "invalid argument" => WebDriverErrorInner::InvalidArgument(payload),
            "invalid cookie domain" => WebDriverErrorInner::InvalidCookieDomain(payload),
            "invalid element state" => WebDriverErrorInner::InvalidElementState(payload),
            "invalid selector" => WebDriverErrorInner::InvalidSelector(payload),
            "invalid session id" => WebDriverErrorInner::InvalidSessionId(payload),
            "javascript error" => WebDriverErrorInner::JavascriptError(payload),
            "move target out of bounds" => WebDriverErrorInner::MoveTargetOutOfBounds(payload),
            "no such alert" => WebDriverErrorInner::NoSuchAlert(payload),
            "no such cookie" => WebDriverErrorInner::NoSuchCookie(payload),
            "no such element" => WebDriverErrorInner::NoSuchElement(payload),
            "no such frame" => WebDriverErrorInner::NoSuchFrame(payload),
            "no such window" => WebDriverErrorInner::NoSuchWindow(payload),
            "script timeout" => WebDriverErrorInner::ScriptTimeout(payload),
            "session not created" => WebDriverErrorInner::SessionNotCreated(payload),
            "stale element reference" => WebDriverErrorInner::StaleElementReference(payload),
            "timeout" => WebDriverErrorInner::WebDriverTimeout(payload),
            "unable to set cookie" => WebDriverErrorInner::UnableToSetCookie(payload),
            "unable to capture screen" => WebDriverErrorInner::UnableToCaptureScreen(payload),
            "unexpected alert open" => WebDriverErrorInner::UnexpectedAlertOpen(payload),
            "unknown command" => WebDriverErrorInner::UnknownCommand(payload),
            "unknown error" => WebDriverErrorInner::UnknownError(payload),
            "unknown method" => WebDriverErrorInner::UnknownMethod(payload),
            "unsupported operation" => WebDriverErrorInner::UnsupportedOperation(payload),
            _ => WebDriverErrorInner::NotInSpec(payload),
        })
    }

    /// gets a reference to the underlying enum representation of this error
    #[must_use] 
    pub fn as_inner(&self) -> &WebDriverErrorInner {
        self
    }

    /// converts the underlying representation to the main representation
    #[must_use] 
    pub fn from_inner(err: WebDriverErrorInner) -> Self {
        Self(Box::new(err))
    }

    /// converts this error to its underlying representation
    #[must_use] 
    pub fn into_inner(self) -> WebDriverErrorInner {
        *self.0
    }
}

impl From<WebDriverErrorInner> for WebDriverError {
    fn from(value: WebDriverErrorInner) -> Self {
        Self::from_inner(value)
    }
}

impl From<WebDriverError> for WebDriverErrorInner {
    fn from(value: WebDriverError) -> Self {
        value.into_inner()
    }
}

impl Deref for WebDriverError {
    type Target = WebDriverErrorInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WebDriverError {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Convenience function to construct a simulated `NoSuchElement` error.
#[must_use] 
pub fn no_such_element(message: String) -> WebDriverError {
    WebDriverError::from_inner(WebDriverErrorInner::NoSuchElement(WebDriverErrorInfo {
        status: 400,
        error: message.clone(),
        value: WebDriverErrorValue {
            message,
            error: None,
            stacktrace: None,
            data: None,
        },
    }))
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for WebDriverError {
    fn from(err: reqwest::Error) -> Self {
        WebDriverError::HttpError(err.to_string())
    }
}

impl From<serde_json::Error> for WebDriverError {
    fn from(err: serde_json::Error) -> Self {
        WebDriverError::Json(err.to_string())
    }
}
