use futures_util::future::{BoxFuture, FutureExt};
use std::convert::TryFrom;
use std::future::Future;
use std::sync::Arc;
use std::{fmt, time::Duration};

use serde::{Deserialize, Serialize};

use crate::error::WebDriverResult;
use crate::WebElement;

mod sealed {
    use crate::error::{WebDriverError, WebDriverResult};
    use std::borrow::Cow;
    use std::rc::Rc;
    use std::sync::Arc;
    use url::Url;

    pub trait IntoTransfer {
        fn into(self) -> Arc<str>;
    }

    pub trait IntoUrl {
        fn into_url(self) -> WebDriverResult<Url>;
    }

    impl IntoTransfer for &str {
        fn into(self) -> Arc<str> {
            Arc::from(self)
        }
    }

    impl IntoUrl for &str {
        fn into_url(self) -> WebDriverResult<Url> {
            Url::parse(self).map_err(|e| WebDriverError::ParseError(format!("url parse err {e}")))
        }
    }

    macro_rules! deref_impl {
        ($trait: path => $meth:ident -> $ret:ty {on} ($($T: ty),*)) => {$(
            impl $trait for $T {
                #[inline]
                fn $meth(self) -> $ret {
                    <&$T as $trait>::$meth(&self)
                }
            }

            impl $trait for &$T {
                #[inline]
                fn $meth(self) -> $ret {
                    <&str as $trait>::$meth(&self)
                }
            }
        )*};
    }

    deref_impl! {
        IntoTransfer => into -> Arc<str> {on} (String, Box<str>, Rc<str>, Cow<'_, str>)
    }

    deref_impl! {
        IntoUrl => into_url -> WebDriverResult<Url> {on} (String)
    }

    impl IntoTransfer for Arc<str> {
        fn into(self) -> Arc<str> {
            self
        }
    }

    impl IntoTransfer for &Arc<str> {
        fn into(self) -> Arc<str> {
            Arc::clone(self)
        }
    }

    impl IntoUrl for Url {
        fn into_url(self) -> WebDriverResult<Url> {
            Ok(self)
        }
    }

    impl IntoUrl for &Url {
        fn into_url(self) -> WebDriverResult<Url> {
            Ok(self.clone())
        }
    }
}

/// trait for turning a string into a cheaply cloneable and transferable String
pub trait IntoArcStr: sealed::IntoTransfer {}
impl<T: sealed::IntoTransfer> IntoArcStr for T {}

/// A trait to try to convert some type into an `Url`.
pub trait IntoUrl: sealed::IntoUrl {}
impl<T: sealed::IntoUrl> IntoUrl for T {}

/// Rectangle representing the dimensions of an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRect {
    /// The x coordinate of the top-left corner.
    pub x: f64,
    /// The y coordinate of the top-left corner.
    pub y: f64,
    /// The element width.
    pub width: f64,
    /// The element height.
    pub height: f64,
}

impl ElementRect {
    /// The coordinates of the rectangle center point, rounded to integers.
    ///
    /// Returns `(0, 0)` if coordinates are NaN or infinite.
    #[must_use]
    pub fn icenter(&self) -> (i64, i64) {
        let (x, y) = self.center();
        (round_to_i64(x), round_to_i64(y))
    }

    /// The coordinates of the rectangle center point.
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (self.x + (self.width / 2.0), self.y + (self.height / 2.0))
    }
}

fn round_to_i64(value: f64) -> i64 {
    const MAX_SAFE_INT: f64 = 9_007_199_254_740_992.0;
    if value.is_finite() && value.abs() <= MAX_SAFE_INT {
        let r = value.round();
        if r.abs() <= MAX_SAFE_INT {
            // Use ToPrimitive for checked conversion that clippy understands as safe
            return num_traits::ToPrimitive::to_i64(&r).unwrap_or(0);
        }
    }
    0
}

/// Helper to Serialize/Deserialize `ElementRef` from JSON Value.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ElementRef {
    /// Reference for a regular element.
    Element {
        /// Element id, as returned by the webdriver.
        #[serde(rename = "element-6066-11e4-a52e-4f735466cecf")]
        id: String,
    },
    /// Reference for a shadow element.
    ShadowElement {
        /// Element id, as returned by the webdriver.
        #[serde(rename = "shadow-6066-11e4-a52e-4f735466cecf")]
        id: String,
    },
}

impl ElementRef {
    /// The element id, as returned by the webdriver.
    #[must_use]
    pub fn id(&self) -> &str {
        let (ElementRef::Element {
            id,
        }
        | ElementRef::ShadowElement {
            id,
        }) = self;
        id
    }
}

/// Newtype for the session id.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SessionId {
    id: Arc<str>,
}

impl<S> From<S> for SessionId
where
    S: IntoArcStr,
{
    fn from(value: S) -> Self {
        SessionId {
            id: value.into(),
        }
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl SessionId {
    /// Create a placeholder `SessionId` for cases where it's not used.
    ///
    /// E.g., session creation.
    #[must_use]
    pub fn null() -> Self {
        SessionId {
            id: Arc::from(""),
        }
    }
}

/// New-type for the element id.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(transparent)]
pub struct ElementId {
    id: Arc<str>,
}

impl<S> From<S> for ElementId
where
    S: IntoArcStr,
{
    fn from(value: S) -> Self {
        ElementId {
            id: value.into(),
        }
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// New-type for the window handle.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct WindowHandle {
    handle: Arc<str>,
}

impl<S> From<S> for WindowHandle
where
    S: IntoArcStr,
{
    fn from(value: S) -> Self {
        WindowHandle {
            handle: value.into(),
        }
    }
}

impl fmt::Display for WindowHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.handle)
    }
}

/// The window type. The webdriver spec treats tabs and windows as windows.
#[derive(Debug, Clone)]
pub enum WindowType {
    /// A browser tab.
    Tab,
    /// A browser window.
    Window,
}

impl fmt::Display for WindowType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                WindowType::Tab => "tab",
                WindowType::Window => "window",
            }
        )
    }
}

/// Rectangle position and dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rect {
    /// The x coordinate of the top-left corner.
    pub x: i64,
    /// The y coordinate of the top-left corner.
    pub y: i64,
    /// The rectangle width.
    pub width: i64,
    /// The rectangle height.
    pub height: i64,
}

impl Rect {
    /// Create a new `Rect`.
    #[must_use]
    pub fn new(x: i64, y: i64, width: i64, height: i64) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

/// Generic element query function that returns some type T.
pub trait ElementQueryFn<T>: Send + Sync {
    /// the future returned by `ElementQueryFn::query`
    type Fut: Future<Output = WebDriverResult<T>> + Send;

    /// the implementation of the query function
    fn call(&self, arg: WebElement) -> Self::Fut;
}

impl<T, Fut, Fun> ElementQueryFn<T> for Fun
where
    Fun: Fn(WebElement) -> Fut + Send + Sync + ?Sized,
    Fut: Future<Output = WebDriverResult<T>> + Send,
{
    type Fut = Fut;

    fn call(&self, arg: WebElement) -> Fut {
        self(arg)
    }
}

/// element predicates.
pub trait ElementPredicate: ElementQueryFn<bool> {}

impl<Fn: ElementQueryFn<bool> + ?Sized> ElementPredicate for Fn {}

/// a dynamically dispatched query function
pub type DynElementQueryFn<T> = dyn ElementQueryFn<T, Fut = BoxFuture<'static, WebDriverResult<T>>>;

/// a dynamically dispatched element predicate
pub type DynElementPredicate = DynElementQueryFn<bool>;

impl<T: 'static> DynElementQueryFn<T> {
    fn wrap<F: ElementQueryFn<T, Fut: 'static>>(
        fun: F,
    ) -> impl ElementQueryFn<T, Fut = BoxFuture<'static, WebDriverResult<T>>> {
        move |arg: WebElement| fun.call(arg).boxed()
    }

    /// erases the type of `ElementQueryFn`, and dynamically dispatches it using a Box smart pointer
    pub fn boxed<F: ElementQueryFn<T, Fut: 'static> + 'static>(fun: F) -> Box<Self> {
        Box::new(Self::wrap(fun)) as Box<Self>
    }

    /// erases the type of `ElementQueryFn`, and dynamically dispatches it using an Arc smart pointer
    pub fn arc<F: ElementQueryFn<T, Fut: 'static> + 'static>(fun: F) -> Arc<Self> {
        Arc::new(Self::wrap(fun)) as Arc<Self>
    }
}

/// Rect struct with optional fields.
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
pub struct OptionRect {
    /// The x coordinate of the top-left corner.
    pub x: Option<i64>,
    /// The y coordinate of the top-left corner.
    pub y: Option<i64>,
    /// The rectangle width.
    pub width: Option<i64>,
    /// The rectangle height.
    pub height: Option<i64>,
}

impl OptionRect {
    /// Create a new `OptionRect`.
    #[must_use]
    pub fn new() -> Self {
        OptionRect::default()
    }

    /// Set the x coordinate of the top-left corner.
    #[must_use]
    pub fn with_x(mut self, value: i64) -> Self {
        self.x = Some(value);
        self
    }

    /// Set the y coordinate of the top-left corner.
    #[must_use]
    pub fn with_y(mut self, value: i64) -> Self {
        self.y = Some(value);
        self
    }

    /// Set the rectangle width.
    #[must_use]
    pub fn with_width(mut self, value: i64) -> Self {
        self.width = Some(value);
        self
    }

    /// Set the rectangle height.
    #[must_use]
    pub fn with_height(mut self, value: i64) -> Self {
        self.height = Some(value);
        self
    }

    /// Set the rectangle position.
    #[must_use]
    pub fn with_pos(mut self, x: i64, y: i64) -> Self {
        self.x = Some(x);
        self.y = Some(y);
        self
    }

    /// Set the rectangle size.
    #[must_use]
    pub fn with_size(mut self, width: i64, height: i64) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
}

impl From<Rect> for OptionRect {
    fn from(value: Rect) -> Self {
        OptionRect {
            x: Some(value.x),
            y: Some(value.y),
            width: Some(value.width),
            height: Some(value.height),
        }
    }
}

/// The timeout configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeoutConfiguration {
    /// The script timeout.
    #[serde(skip_serializing_if = "Option::is_none")]
    script: Option<u64>,
    /// The page load timeout.
    #[serde(rename = "pageLoad", skip_serializing_if = "Option::is_none")]
    page_load: Option<u64>,
    /// The implicit wait timeout.
    #[serde(skip_serializing_if = "Option::is_none")]
    implicit: Option<u64>,
}

impl Default for TimeoutConfiguration {
    fn default() -> Self {
        TimeoutConfiguration::new(
            Some(Duration::from_secs(60)),
            Some(Duration::from_secs(60)),
            // NOTE: Implicit wait must default to zero to support ElementQuery.
            Some(Duration::from_secs(0)),
        )
    }
}

impl TimeoutConfiguration {
    /// Create a new `TimeoutConfiguration`.
    #[must_use]
    pub fn new(
        script: Option<Duration>,
        page_load: Option<Duration>,
        implicit: Option<Duration>,
    ) -> Self {
        TimeoutConfiguration {
            script: script.map(|x| u64::try_from(x.as_millis()).unwrap_or(u64::MAX)),
            page_load: page_load.map(|x| u64::try_from(x.as_millis()).unwrap_or(u64::MAX)),
            implicit: implicit.map(|x| u64::try_from(x.as_millis()).unwrap_or(u64::MAX)),
        }
    }

    /// Get the script timeout, if set.
    pub fn script(&self) -> Option<Duration> {
        self.script.map(Duration::from_millis)
    }

    /// Set the script timeout.
    pub fn set_script(&mut self, timeout: Option<Duration>) {
        self.script = timeout.and_then(|x| u64::try_from(x.as_millis()).ok());
    }

    /// Get the page load timeout, if set.
    pub fn page_load(&self) -> Option<Duration> {
        self.page_load.map(Duration::from_millis)
    }

    /// Set the page load timeout.
    pub fn set_page_load(&mut self, timeout: Option<Duration>) {
        self.page_load = timeout.and_then(|x| u64::try_from(x.as_millis()).ok());
    }

    /// Get the implicit wait timeout, if set.
    pub fn implicit(&self) -> Option<Duration> {
        self.implicit.map(Duration::from_millis)
    }

    /// Set the implicit wait timeout.
    pub fn set_implicit(&mut self, timeout: Option<Duration>) {
        self.implicit = timeout.and_then(|x| u64::try_from(x.as_millis()).ok());
    }
}

/// The `WebDriver` status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDriverStatus {
    /// Whether the webdriver is ready to accept new sessions.
    pub ready: bool,
    /// The current status message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use serde_json::json;

    #[test]
    fn test_element_ref() {
        let id = "daaea226-43aa-400f-896c-210e5af2ac62";
        let value = json!({ "element-6066-11e4-a52e-4f735466cecf": id });
        let elem_ref: ElementRef = serde_json::from_value(value).unwrap();
        assert_matches!(&elem_ref, ElementRef::Element { id: x} if x == id);
        assert_eq!(elem_ref.id(), id);
    }

    #[test]
    fn test_shadow_element_ref() {
        let id = "daaea226-43aa-400f-896c-210e5af2ac62";
        let value = json!({ "shadow-6066-11e4-a52e-4f735466cecf": id });
        let elem_ref: ElementRef = serde_json::from_value(value).unwrap();
        assert_matches!(&elem_ref, ElementRef::ShadowElement { id: x} if x == id);
        assert_eq!(elem_ref.id(), id);
    }
}
