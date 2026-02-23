use crate::error::WebDriverResult;
use crate::session::handle::SessionHandle;
use crate::WebElement;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// Helper struct for getting return values from scripts.
///
/// See the examples for [`WebDriver::execute`] and [`WebDriver::execute_async`].
///
/// [`WebDriver::execute`]: crate::session::handle::SessionHandle::execute_async
/// [`WebDriver::execute_async`]: crate::session::handle::SessionHandle::execute_async
#[derive(Debug)]
pub struct ScriptRet {
    handle: Arc<SessionHandle>,
    value: Value,
}

impl ScriptRet {
    /// Create a new `ScriptRet`.
    ///
    /// This is typically done automatically via [`WebDriver::execute`]
    /// or [`WebDriver::execute_async`].
    ///
    /// [`WebDriver::execute`]: crate::session::handle::SessionHandle::execute_async
    /// [`WebDriver::execute_async`]: crate::session::handle::SessionHandle::execute_async
    pub fn new(handle: Arc<SessionHandle>, value: Value) -> Self {
        Self {
            handle,
            value,
        }
    }

    /// Get the raw JSON value.
    #[must_use] 
    pub fn json(&self) -> &Value {
        &self.value
    }

    /// Get the raw JSON value.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to json()")]
    #[must_use] 
    pub fn value(&self) -> &Value {
        self.json()
    }

    /// Convert the JSON value into the a deserializeable type.
    pub fn convert<T>(&self) -> WebDriverResult<T>
    where
        T: DeserializeOwned,
    {
        let v: T = serde_json::from_value(self.value.clone())?;
        Ok(v)
    }

    /// Get a single `WebElement` return value.
    ///
    /// Your script must return only a single element for this to work.
    pub fn element(self) -> WebDriverResult<WebElement> {
        WebElement::from_json(self.value, self.handle)
    }

    /// Get a single `WebElement` return value.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to element()")]
    pub fn get_element(self) -> WebDriverResult<WebElement> {
        self.element()
    }

    /// Get a vec of `WebElements` from the return value.
    ///
    /// Your script must return an array of elements for this to work.
    pub fn elements(self) -> WebDriverResult<Vec<WebElement>> {
        let values: Vec<Value> = serde_json::from_value(self.value)?;
        let handle = self.handle;
        values.into_iter().map(|x| WebElement::from_json(x, handle.clone())).collect()
    }

    /// Get a vec of `WebElements` from the return value.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to elements()")]
    pub fn get_elements(self) -> WebDriverResult<Vec<WebElement>> {
        self.elements()
    }
}
