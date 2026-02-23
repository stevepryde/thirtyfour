use http::Method;
use serde_json::{json, Value};

use crate::common::{
    capabilities::desiredcapabilities::make_w3c_caps,
    cookie::Cookie,
    keys::TypingData,
    print::PrintParameters,
    types::{ElementId, OptionRect, SessionId, TimeoutConfiguration, WindowHandle},
};
use crate::IntoArcStr;
use crate::RequestData;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

/// The W3C element identifier key.
pub const MAGIC_ELEMENTID: &str = "element-6066-11e4-a52e-4f735466cecf";

/// Actions.
#[derive(Debug)]
pub struct Actions(Value);

impl From<Value> for Actions {
    fn from(value: Value) -> Self {
        Actions(value)
    }
}

/// Element Selector representation.
#[derive(Debug, Clone)]
pub struct Selector {
    /// Selector name.
    pub name: Arc<str>,
    /// Selector query.
    pub query: Arc<str>,
}

impl Selector {
    /// Create a new Selector.
    pub fn new(name: impl IntoArcStr, query: impl IntoArcStr) -> Self {
        Self {
            name: name.into(),
            query: query.into(),
        }
    }
}

/// Element Selector representation.
#[derive(Debug, Clone)]
pub enum BySelector {
    /// Select an element by id.
    Id(Arc<str>),
    /// Select an element by `XPath`.
    XPath(Arc<str>),
    /// Select an element by link text.
    LinkText(Arc<str>),
    /// Select an element by partial link text.
    PartialLinkText(Arc<str>),
    /// Select element by name.
    Name(Arc<str>),
    /// Select an element by tag.
    Tag(Arc<str>),
    /// Select an element by class.
    ClassName(Arc<str>),
    /// Select an element by CSS.
    Css(Arc<str>),
    /// Select an element by data-testid.
    Testid(Arc<str>),
}

/// Element Selector struct providing a convenient way to specify selectors.
#[derive(Debug, Clone)]
pub struct By {
    selector: BySelector,
}

#[allow(non_snake_case)]
impl By {
    /// Select element by id.
    pub fn Id(id: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::Id(id.into()),
        }
    }

    /// Select element by link text.
    pub fn LinkText(text: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::LinkText(text.into()),
        }
    }

    /// Select element by partial link text.
    pub fn PartialLinkText(text: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::PartialLinkText(text.into()),
        }
    }

    /// Select element by CSS.
    pub fn Css(css: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::Css(css.into()),
        }
    }

    /// Select element by `XPath`.
    pub fn XPath(x: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::XPath(x.into()),
        }
    }

    /// Select element by name.
    pub fn Name(name: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::Css(format!(r#"[name="{}"]"#, name.into()).into()),
        }
    }

    /// Select element by tag.
    pub fn Tag(tag: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::Css(tag.into()),
        }
    }

    /// Select element by class.
    pub fn ClassName(name: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::Css(format!(".{}", name.into()).into()),
        }
    }

    /// Select element by testid.
    pub fn Testid(id: impl IntoArcStr) -> Self {
        Self {
            selector: BySelector::Css(format!("[data-testid=\"{}\"]", id.into()).into()),
        }
    }
}

impl fmt::Display for BySelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BySelector::Id(id) => write!(f, "Id({id})"),
            BySelector::XPath(xpath) => write!(f, "XPath({xpath})"),
            BySelector::LinkText(text) => write!(f, "Link Text({text})"),
            BySelector::PartialLinkText(text) => write!(f, "Partial Link Text({text})"),
            BySelector::Name(name) => write!(f, "Name({name})"),
            BySelector::Tag(tag) => write!(f, "Tag({tag})"),
            BySelector::ClassName(cname) => write!(f, "Class({cname})"),
            BySelector::Css(css) => write!(f, "CSS({css})"),
            BySelector::Testid(id) => write!(f, "Testid({id})"),
        }
    }
}

impl fmt::Display for By {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.selector)
    }
}

impl From<BySelector> for Selector {
    fn from(by: BySelector) -> Self {
        match by {
            BySelector::Id(x) => Selector::new("css selector", format!("[id=\"{x}\"]")),
            BySelector::XPath(x) => Selector::new("xpath", x),
            BySelector::LinkText(x) => Selector::new("link text", x),
            BySelector::PartialLinkText(x) => Selector::new("partial link text", x),
            BySelector::Name(x) => Selector::new("css selector", format!("[name=\"{x}\"]")),
            BySelector::Tag(x) => Selector::new("css selector", x),
            BySelector::ClassName(x) => Selector::new("css selector", format!(".{x}")),
            BySelector::Css(x) => Selector::new("css selector", x),
            BySelector::Testid(x) => {
                Selector::new("testid selector", format!("[data-testid=\"{x}\"]"))
            }
        }
    }
}

impl From<By> for Selector {
    fn from(by: By) -> Self {
        by.selector.into()
    }
}

/// Extension Command trait.
pub trait ExtensionCommand: Debug {
    /// Request Body
    fn parameters_json(&self) -> Option<Value>;

    /// HTTP method accepting by the webdriver
    fn method(&self) -> Method;

    /// Endpoint URL without `session/{sessionId}` prefix
    ///
    /// Example:- `/moz/addon/install`
    fn endpoint(&self) -> Arc<str>;
}

/// All the standard `WebDriver` commands.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Command {
    NewSession(Value),
    DeleteSession,
    Status,
    GetTimeouts,
    SetTimeouts(TimeoutConfiguration),
    NavigateTo(Arc<str>),
    GetCurrentUrl,
    Back,
    Forward,
    Refresh,
    GetTitle,
    GetWindowHandle,
    CloseWindow,
    SwitchToWindow(WindowHandle),
    GetWindowHandles,
    NewWindow,
    NewTab,
    SwitchToFrameDefault,
    SwitchToFrameNumber(u16),
    SwitchToFrameElement(ElementId),
    SwitchToParentFrame,
    GetWindowRect,
    SetWindowRect(OptionRect),
    MaximizeWindow,
    MinimizeWindow,
    FullscreenWindow,
    GetActiveElement,
    FindElement(Selector),
    FindElements(Selector),
    FindElementFromElement(ElementId, Selector),
    FindElementsFromElement(ElementId, Selector),
    IsElementSelected(ElementId),
    IsElementDisplayed(ElementId),
    GetElementAttribute(ElementId, Arc<str>),
    GetElementProperty(ElementId, Arc<str>),
    GetElementCssValue(ElementId, Arc<str>),
    GetElementText(ElementId),
    GetElementTagName(ElementId),
    GetElementRect(ElementId),
    IsElementEnabled(ElementId),
    ElementClick(ElementId),
    ElementClear(ElementId),
    ElementSendKeys(ElementId, TypingData),
    GetPageSource,
    ExecuteScript(Arc<str>, Arc<[Value]>),
    ExecuteAsyncScript(Arc<str>, Arc<[Value]>),
    GetAllCookies,
    GetNamedCookie(Arc<str>),
    AddCookie(Cookie),
    DeleteCookie(Arc<str>),
    DeleteAllCookies,
    PerformActions(Actions),
    ReleaseActions,
    DismissAlert,
    AcceptAlert,
    GetAlertText,
    SendAlertText(TypingData),
    PrintPage(PrintParameters),
    TakeScreenshot,
    TakeElementScreenshot(ElementId),
    ExtensionCommand(Box<dyn ExtensionCommand + Send + Sync>),
}

/// Trait for formatting a `WebDriver` command into a `RequestData` struct.
pub trait FormatRequestData: Debug {
    /// Format the command into a `RequestData` struct.
    fn format_request(&self, session_id: &SessionId) -> RequestData;
}

impl FormatRequestData for Command {
    fn format_request(&self, session_id: &SessionId) -> RequestData {
        match self {
            Command::NewSession(caps) => {
                let w3c_caps = make_w3c_caps(caps);
                RequestData::new(Method::POST, "session").add_body(json!({
                    "capabilities": w3c_caps,
                    "desiredCapabilities": caps
                }))
            }
            Command::DeleteSession => {
                RequestData::new(Method::DELETE, format!("session/{session_id}"))
            }
            Command::Status => RequestData::new(Method::GET, "/status"),
            Command::GetTimeouts => {
                RequestData::new(Method::GET, format!("session/{session_id}/timeouts"))
            }
            Command::SetTimeouts(timeout_configuration) => {
                RequestData::new(Method::POST, format!("session/{session_id}/timeouts"))
                    .add_body(json!(timeout_configuration))
            }
            Command::NavigateTo(url) => {
                RequestData::new(Method::POST, format!("session/{session_id}/url"))
                    .add_body(json!({ "url": url }))
            }
            Command::GetCurrentUrl => {
                RequestData::new(Method::GET, format!("session/{session_id}/url"))
            }
            Command::Back => RequestData::new(Method::POST, format!("session/{session_id}/back"))
                .add_body(json!({})),
            Command::Forward => {
                RequestData::new(Method::POST, format!("session/{session_id}/forward"))
                    .add_body(json!({}))
            }
            Command::Refresh => {
                RequestData::new(Method::POST, format!("session/{session_id}/refresh"))
                    .add_body(json!({}))
            }
            Command::GetTitle => {
                RequestData::new(Method::GET, format!("session/{session_id}/title"))
            }
            Command::GetWindowHandle => {
                RequestData::new(Method::GET, format!("session/{session_id}/window"))
            }
            Command::CloseWindow => {
                RequestData::new(Method::DELETE, format!("session/{session_id}/window"))
            }
            Command::SwitchToWindow(window_handle) => {
                RequestData::new(Method::POST, format!("session/{session_id}/window"))
                    .add_body(json!({ "handle": window_handle.to_string() }))
            }
            Command::GetWindowHandles => {
                RequestData::new(Method::GET, format!("session/{session_id}/window/handles"))
            }
            Command::NewWindow => {
                RequestData::new(Method::POST, format!("session/{session_id}/window/new"))
                    .add_body(json!({"type": "window"}))
            }
            Command::NewTab => {
                RequestData::new(Method::POST, format!("session/{session_id}/window/new"))
                    .add_body(json!({"type": "tab"}))
            }
            Command::SwitchToFrameDefault => {
                RequestData::new(Method::POST, format!("session/{session_id}/frame"))
                    .add_body(json!({ "id": serde_json::Value::Null }))
            }
            Command::SwitchToFrameNumber(frame_number) => {
                RequestData::new(Method::POST, format!("session/{session_id}/frame"))
                    .add_body(json!({ "id": frame_number }))
            }
            Command::SwitchToFrameElement(element_id) => {
                RequestData::new(Method::POST, format!("session/{session_id}/frame")).add_body(
                    json!({"id": {
                        "ELEMENT": element_id.to_string(),
                        MAGIC_ELEMENTID: element_id.to_string()
                    }}),
                )
            }
            Command::SwitchToParentFrame => {
                RequestData::new(Method::POST, format!("session/{session_id}/frame/parent"))
                    .add_body(json!({}))
            }
            Command::GetWindowRect => {
                RequestData::new(Method::GET, format!("session/{session_id}/window/rect"))
            }
            Command::SetWindowRect(option_rect) => {
                RequestData::new(Method::POST, format!("session/{session_id}/window/rect"))
                    .add_body(json!(option_rect))
            }
            Command::MaximizeWindow => {
                RequestData::new(Method::POST, format!("session/{session_id}/window/maximize"))
                    .add_body(json!({}))
            }
            Command::MinimizeWindow => {
                RequestData::new(Method::POST, format!("session/{session_id}/window/minimize"))
                    .add_body(json!({}))
            }
            Command::FullscreenWindow => {
                RequestData::new(Method::POST, format!("session/{session_id}/window/fullscreen"))
                    .add_body(json!({}))
            }
            Command::GetActiveElement => {
                RequestData::new(Method::GET, format!("session/{session_id}/element/active"))
            }
            Command::FindElement(selector) => {
                RequestData::new(Method::POST, format!("session/{session_id}/element"))
                    .add_body(json!({"using": selector.name, "value": selector.query}))
            }
            Command::FindElements(selector) => {
                RequestData::new(Method::POST, format!("session/{session_id}/elements"))
                    .add_body(json!({"using": selector.name, "value": selector.query}))
            }
            Command::FindElementFromElement(element_id, selector) => RequestData::new(
                Method::POST,
                format!("session/{session_id}/element/{element_id}/element"),
            )
            .add_body(json!({"using": selector.name, "value": selector.query})),
            Command::FindElementsFromElement(element_id, selector) => RequestData::new(
                Method::POST,
                format!("session/{session_id}/element/{element_id}/elements"),
            )
            .add_body(json!({"using": selector.name, "value": selector.query})),
            Command::IsElementSelected(element_id) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/selected"),
            ),
            Command::IsElementDisplayed(element_id) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/displayed"),
            ),
            Command::GetElementAttribute(element_id, attribute_name) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/attribute/{attribute_name}"),
            ),
            Command::GetElementProperty(element_id, property_name) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/property/{property_name}"),
            ),
            Command::GetElementCssValue(element_id, property_name) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/css/{property_name}"),
            ),
            Command::GetElementText(element_id) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/text"),
            ),
            Command::GetElementTagName(element_id) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/name"),
            ),
            Command::GetElementRect(element_id) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/rect"),
            ),
            Command::IsElementEnabled(element_id) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/enabled"),
            ),
            Command::ElementClick(element_id) => RequestData::new(
                Method::POST,
                format!("session/{session_id}/element/{element_id}/click"),
            )
            .add_body(json!({})),
            Command::ElementClear(element_id) => RequestData::new(
                Method::POST,
                format!("session/{session_id}/element/{element_id}/clear"),
            )
            .add_body(json!({})),
            Command::ElementSendKeys(element_id, typing_data) => RequestData::new(
                Method::POST,
                format!("session/{session_id}/element/{element_id}/value"),
            )
            .add_body(json!({"text": typing_data.to_string(), "value": typing_data.as_vec() })),
            Command::GetPageSource => {
                RequestData::new(Method::GET, format!("session/{session_id}/source"))
            }
            Command::ExecuteScript(script, args) => {
                RequestData::new(Method::POST, format!("session/{session_id}/execute/sync"))
                    .add_body(json!({"script": script, "args": args}))
            }
            Command::ExecuteAsyncScript(script, args) => {
                RequestData::new(Method::POST, format!("session/{session_id}/execute/async"))
                    .add_body(json!({"script": script, "args": args}))
            }
            Command::GetAllCookies => {
                RequestData::new(Method::GET, format!("session/{session_id}/cookie"))
            }
            Command::GetNamedCookie(cookie_name) => {
                RequestData::new(Method::GET, format!("session/{session_id}/cookie/{cookie_name}"))
            }
            Command::AddCookie(cookie) => {
                RequestData::new(Method::POST, format!("session/{session_id}/cookie"))
                    .add_body(json!({ "cookie": cookie }))
            }
            Command::DeleteCookie(cookie_name) => RequestData::new(
                Method::DELETE,
                format!("session/{session_id}/cookie/{cookie_name}"),
            ),
            Command::DeleteAllCookies => {
                RequestData::new(Method::DELETE, format!("session/{session_id}/cookie"))
            }
            Command::PerformActions(actions) => {
                RequestData::new(Method::POST, format!("session/{session_id}/actions"))
                    .add_body(json!({"actions": actions.0}))
            }
            Command::ReleaseActions => {
                RequestData::new(Method::DELETE, format!("session/{session_id}/actions"))
            }
            Command::DismissAlert => {
                RequestData::new(Method::POST, format!("session/{session_id}/alert/dismiss"))
                    .add_body(json!({}))
            }
            Command::AcceptAlert => {
                RequestData::new(Method::POST, format!("session/{session_id}/alert/accept"))
                    .add_body(json!({}))
            }
            Command::GetAlertText => {
                RequestData::new(Method::GET, format!("session/{session_id}/alert/text"))
            }
            Command::SendAlertText(typing_data) => {
                RequestData::new(Method::POST, format!("session/{session_id}/alert/text")).add_body(
                    json!({
                        "value": typing_data.as_vec(), "text": typing_data.to_string()
                    }),
                )
            }
            Command::PrintPage(params) => {
                RequestData::new(Method::POST, format!("/session/{session_id}/print")).add_body(
                    serde_json::to_value(params)
                        .expect("Fail to parse Print Page Parameters to json"),
                )
            }
            Command::TakeScreenshot => {
                RequestData::new(Method::GET, format!("session/{session_id}/screenshot"))
            }
            Command::TakeElementScreenshot(element_id) => RequestData::new(
                Method::GET,
                format!("session/{session_id}/element/{element_id}/screenshot"),
            ),
            Command::ExtensionCommand(command) => {
                let request_data = RequestData::new(
                    command.method(),
                    format!("session/{session_id}{}", command.endpoint()),
                );
                match command.parameters_json() {
                    Some(param) => request_data.add_body(param),
                    None => request_data,
                }
            }
        }
    }
}
