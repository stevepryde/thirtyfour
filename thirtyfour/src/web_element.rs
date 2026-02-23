use serde::ser::{Serialize, Serializer};
use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::common::command::Command;
use crate::error::{WebDriverError, WebDriverErrorInner};
use crate::js::SIMULATE_DRAG_AND_DROP;
use crate::session::handle::SessionHandle;
use crate::support::base64_decode;
use crate::{common::types::ElementRect, error::WebDriverResult, By, ElementRef};
use crate::{support, IntoArcStr};
use crate::{ElementId, TypingData};

/// The `WebElement` struct encapsulates a single element on a page.
///
/// `WebElement` structs are generally not constructed manually, but rather
/// they are returned from a '`find_element()`' operation using a `WebDriver`.
///
/// # Example:
/// ```no_run
/// # use thirtyfour::prelude::*;
/// # use thirtyfour::support::block_on;
/// #
/// # fn main() -> WebDriverResult<()> {
/// #     block_on(async {
/// #         let caps = DesiredCapabilities::chrome();
/// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
/// let elem = driver.find(By::Id("my-element-id")).await?;
/// #         driver.quit().await?;
/// #         Ok(())
/// #     })
/// # }
/// ```
///
/// You can also search for a child element of another element as follows:
/// ```no_run
/// # use thirtyfour::prelude::*;
/// # use thirtyfour::support::block_on;
/// #
/// # fn main() -> WebDriverResult<()> {
/// #     block_on(async {
/// #         let caps = DesiredCapabilities::chrome();
/// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
/// let elem = driver.find(By::Id("my-element-id")).await?;
/// let child_elem = elem.find(By::Tag("button")).await?;
/// #         driver.quit().await?;
/// #         Ok(())
/// #     })
/// # }
/// ```
///
/// Elements can be clicked using the `click()` method, and you can send
/// input to an element using the `send_keys()` method.
///
#[derive(Clone)]
pub struct WebElement {
    /// The element id.
    pub element_id: ElementId,
    /// The underlying session handle.
    pub handle: Arc<SessionHandle>,
}

impl fmt::Debug for WebElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebElement").field("element", &self.element_id).finish()
    }
}

impl PartialEq for WebElement {
    fn eq(&self, other: &Self) -> bool {
        self.element_id() == other.element_id()
    }
}

impl Eq for WebElement {}

impl WebElement {
    /// Create a new `WebElement` struct.
    ///
    /// Typically you would not call this directly. `WebElement` structs are
    /// usually constructed by calling one of the `find_element`*() methods
    /// either on `WebDriver` or another `WebElement`.
    pub(crate) fn new(element_id: ElementId, handle: Arc<SessionHandle>) -> Self {
        Self {
            element_id,
            handle,
        }
    }

    /// Construct a `WebElement` from a JSON response and a session handle.
    ///
    /// The `value` argument should be a JSON object containing the property
    /// `element-6066-11e4-a52e-4f735466cecf` whose value is the element id
    /// assigned by the `WebDriver`.
    ///
    /// You can get the session handle from any existing `WebDriver` or
    /// `WebElement` that is using this session, e.g. `driver.handle`.
    ///
    /// NOTE: if you simply want to convert a script's return value to a
    ///       `WebElement`, use [`ScriptRet::element`] instead.
    ///
    /// [`ScriptRet::element`]: crate::session::scriptret::ScriptRet::element
    pub fn from_json(value: Value, handle: Arc<SessionHandle>) -> WebDriverResult<Self> {
        let element_ref: ElementRef = serde_json::from_value(value)?;
        Ok(Self {
            element_id: ElementId::from(element_ref.id()),
            handle,
        })
    }

    /// Serialize this `WebElement` to JSON.
    ///
    /// This is useful for supplying an element as an argument to a script.
    ///
    /// See the documentation for [`SessionHandle::execute`] for more details.
    pub fn to_json(&self) -> WebDriverResult<Value> {
        Ok(serde_json::to_value(ElementRef::Element {
            id: self.element_id.to_string(),
        })?)
    }

    /// Get the internal element id for this element.
    ///
    /// NOTE: If you want the `id` property of an element,
    ///       use [`WebElement::id`] instead.
    #[must_use] 
    pub fn element_id(&self) -> ElementId {
        self.element_id.clone()
    }

    /// Get the bounding rectangle for this `WebElement`.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// let r = elem.rect().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn rect(&self) -> WebDriverResult<ElementRect> {
        let r = self.handle.cmd(Command::GetElementRect(self.element_id.clone())).await?;
        r.value()
    }

    /// Alias for [`WebElement::rect()`].
    #[deprecated(since = "0.32.0", note = "Use rect() instead")]
    pub async fn rectangle(&self) -> WebDriverResult<ElementRect> {
        self.rect().await
    }

    /// Get the tag name for this `WebElement`.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// assert_eq!(elem.tag_name().await?, "button");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn tag_name(&self) -> WebDriverResult<String> {
        self.handle.cmd(Command::GetElementTagName(self.element_id.clone())).await?.value()
    }

    /// Get the class name for this `WebElement`.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// let class_name: Option<String> = elem.class_name().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn class_name(&self) -> WebDriverResult<Option<String>> {
        self.attr("class").await
    }

    /// Get the id for this `WebElement`.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// let id: Option<String> = elem.id().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn id(&self) -> WebDriverResult<Option<String>> {
        self.attr("id").await
    }

    /// Get the text contents for this `WebElement`.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// let text = elem.text().await?;
    /// assert_eq!(text, "Click Me");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn text(&self) -> WebDriverResult<String> {
        self.handle.cmd(Command::GetElementText(self.element_id.clone())).await?.value()
    }

    /// Convenience method for getting the (optional) value property of this element.
    pub async fn value(&self) -> WebDriverResult<Option<String>> {
        self.prop("value").await
    }

    /// Click the `WebElement`.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// elem.click().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn click(&self) -> WebDriverResult<()> {
        self.handle.cmd(Command::ElementClick(self.element_id.clone())).await?;
        Ok(())
    }

    /// Clear the `WebElement` contents.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Css("input[type='text']")).await?;
    /// elem.clear().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn clear(&self) -> WebDriverResult<()> {
        self.handle.cmd(Command::ElementClear(self.element_id.clone())).await?;
        Ok(())
    }

    /// Get the specified property.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Css("input[type='checkbox']")).await?;
    /// let property_value: Option<String> = elem.prop("checked").await?;
    /// assert_eq!(property_value.unwrap(), "true");
    ///
    /// // If a property is not found, None is returned.
    /// assert_eq!(elem.prop("invalid-property").await?, None);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn prop(&self, name: impl IntoArcStr) -> WebDriverResult<Option<String>> {
        let resp = self
            .handle
            .cmd(Command::GetElementProperty(self.element_id.clone(), name.into()))
            .await?;
        match resp.value()? {
            Value::String(v) => Ok(Some(v)),
            Value::Bool(b) => Ok(Some(b.to_string())),
            Value::Number(number) => Ok(Some(number.to_string())),
            Value::Null => Ok(None),
            v => Err(WebDriverError::Json(format!("Unexpected value for property: {v:?}"))),
        }
    }

    /// Get the specified property.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to prop()")]
    pub async fn get_property(&self, name: impl IntoArcStr) -> WebDriverResult<Option<String>> {
        self.prop(name).await
    }

    /// Get the specified attribute.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Name("input2")).await?;
    /// let attribute: Option<String> = elem.attr("name").await?;
    /// assert_eq!(attribute.unwrap(), "input2");
    ///
    /// // If the attribute does not exist, None is returned.
    /// assert_eq!(elem.attr("invalid-attribute").await?, None);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn attr(&self, name: impl IntoArcStr) -> WebDriverResult<Option<String>> {
        self.handle
            .cmd(Command::GetElementAttribute(self.element_id.clone(), name.into()))
            .await?
            .value()
    }

    /// Get the specified attribute.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to attr()")]
    pub async fn get_attribute(&self, name: impl IntoArcStr) -> WebDriverResult<Option<String>> {
        self.attr(name).await
    }

    /// Get the specified CSS property.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("my-element-id")).await?;
    /// let css_color = elem.css_value("color").await?;
    /// assert_eq!(css_color, "rgba(0, 0, 0, 1)");
    ///
    /// // If an invalid CSS property is specified, a blank string is returned.
    /// assert_eq!(elem.css_value("invalid-css-property").await?, "");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn css_value(&self, name: impl IntoArcStr) -> WebDriverResult<String> {
        self.handle
            .cmd(Command::GetElementCssValue(self.element_id.clone(), name.into()))
            .await?
            .value()
    }

    /// Get the specified CSS property.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to css_value()")]
    pub async fn get_css_property(&self, name: impl IntoArcStr) -> WebDriverResult<String> {
        self.css_value(name).await
    }

    /// Return true if the `WebElement` is currently selected, otherwise false.
    pub async fn is_selected(&self) -> WebDriverResult<bool> {
        self.handle.cmd(Command::IsElementSelected(self.element_id.clone())).await?.value()
    }

    /// Return true if the `WebElement` is currently displayed, otherwise false.
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// assert!(elem.is_displayed().await?);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn is_displayed(&self) -> WebDriverResult<bool> {
        self.handle.cmd(Command::IsElementDisplayed(self.element_id.clone())).await?.value()
    }

    /// Return true if the `WebElement` is currently enabled, otherwise false.
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// assert!(elem.is_enabled().await?);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn is_enabled(&self) -> WebDriverResult<bool> {
        self.handle.cmd(Command::IsElementEnabled(self.element_id.clone())).await?.value()
    }

    /// Return true if the `WebElement` is currently clickable (visible and enabled),
    /// otherwise false.
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// assert!(elem.is_clickable().await?);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn is_clickable(&self) -> WebDriverResult<bool> {
        Ok(self.is_displayed().await? && self.is_enabled().await?)
    }

    /// Return true if the `WebElement` is currently (still) present
    /// and not stale.
    ///
    /// NOTE: This method simply queries the tag name to
    ///       determine whether the element is still present.
    ///
    /// IMPORTANT:
    /// If an element is re-rendered, it may be considered stale even
    /// though to the user it looks like it is still there.
    ///
    /// The recommended way to check for the presence of an element is
    /// to simply search for the element again.
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// assert!(elem.is_present().await?);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn is_present(&self) -> WebDriverResult<bool> {
        let present = match self.tag_name().await {
            Ok(..) => true,
            Err(e) if matches!(*e, WebDriverErrorInner::StaleElementReference(..)) => false,
            Err(e) => return Err(e),
        };
        Ok(present)
    }

    /// Search for a child element of this `WebElement` using the specified selector.
    ///
    /// **NOTE**: For more powerful element queries including polling and filters, see the
    ///  [`WebElement::query`] method instead.
    ///
    /// [`WebElement::query`]: crate::extensions::query::ElementQueryable::query
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("my-element-id")).await?;
    /// let child_elem = elem.find(By::Tag("button")).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn find(&self, by: By) -> WebDriverResult<WebElement> {
        let r = self
            .handle
            .cmd(Command::FindElementFromElement(self.element_id.clone(), by.into()))
            .await?;
        r.element(self.handle.clone())
    }

    /// Search for a child element of this `WebElement` using the specified selector.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to find()")]
    pub async fn find_element(&self, by: By) -> WebDriverResult<WebElement> {
        self.find(by).await
    }

    /// Search for all child elements of this `WebElement` that match the specified selector.
    ///
    /// **NOTE**: For more powerful element queries including polling and filters, see the
    /// [`WebElement::query`] method instead.
    ///
    /// [`WebElement::query`]: crate::extensions::query::ElementQueryable::query
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("my-element-id")).await?;
    /// let child_elems = elem.find_all(By::Tag("button")).await?;
    /// for child_elem in child_elems {
    ///     assert_eq!(child_elem.tag_name().await?, "button");
    /// }
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn find_all(&self, by: By) -> WebDriverResult<Vec<WebElement>> {
        let r = self
            .handle
            .cmd(Command::FindElementsFromElement(self.element_id.clone(), by.into()))
            .await?;
        r.elements(self.handle.clone())
    }

    /// Search for all child elements of this `WebElement` that match the specified selector.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to find_all()")]
    pub async fn find_elements(&self, by: By) -> WebDriverResult<Vec<WebElement>> {
        self.find_all(by).await
    }

    /// Send the specified input.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// elem.send_keys("thirtyfour").await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// You can also send special key combinations like this:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// elem.send_keys("selenium").await?;
    /// elem.send_keys(Key::Control + "a").await?;
    /// elem.send_keys("thirtyfour" + Key::Enter).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn send_keys(&self, key: impl Into<TypingData>) -> WebDriverResult<()> {
        self.handle.cmd(Command::ElementSendKeys(self.element_id.clone(), key.into())).await?;
        Ok(())
    }

    /// Take a screenshot of this `WebElement` and return it as PNG, base64 encoded.
    pub async fn screenshot_as_png_base64(&self) -> WebDriverResult<String> {
        self.handle.cmd(Command::TakeElementScreenshot(self.element_id.clone())).await?.value()
    }

    /// Take a screenshot of this `WebElement` and return it as PNG bytes.
    pub async fn screenshot_as_png(&self) -> WebDriverResult<Vec<u8>> {
        base64_decode(&self.screenshot_as_png_base64().await?)
    }

    /// Take a screenshot of this `WebElement` and write it to the specified filename.
    pub async fn screenshot(&self, path: &Path) -> WebDriverResult<()> {
        let png = self.screenshot_as_png().await?;
        support::write_file(path, png).await?;
        Ok(())
    }

    /// Focus this `WebElement` using JavaScript.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// elem.focus().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn focus(&self) -> WebDriverResult<()> {
        self.handle.execute(r"arguments[0].focus();", vec![self.to_json()?]).await?;
        Ok(())
    }

    /// Scroll this element into view using JavaScript.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// elem.scroll_into_view().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn scroll_into_view(&self) -> WebDriverResult<()> {
        self.handle
            .execute(
                r#"arguments[0].scrollIntoView({block: "center", inline: "center"});"#,
                vec![self.to_json()?],
            )
            .await?;
        Ok(())
    }

    /// Get the innerHtml property of this element.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("my-element-id")).await?;
    /// let html = elem.inner_html().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn inner_html(&self) -> WebDriverResult<String> {
        self.prop("innerHTML").await.map(Option::unwrap_or_default)
    }

    /// Get the outerHtml property of this element.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("my-element-id")).await?;
    /// let html = elem.outer_html().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn outer_html(&self) -> WebDriverResult<String> {
        self.prop("outerHTML").await.map(Option::unwrap_or_default)
    }

    /// Get the shadowRoot property of the current element.
    ///
    /// Call this method on the element containing the `#shadowRoot` node.
    /// You can then use the returned `WebElement` to query elements within the shadowRoot node.
    pub async fn get_shadow_root(&self) -> WebDriverResult<WebElement> {
        let ret =
            self.handle.execute("return arguments[0].shadowRoot", vec![self.to_json()?]).await?;
        ret.element()
    }

    /// Switch to the specified iframe element.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem_iframe = driver.find(By::Id("iframeid1")).await?;
    /// elem_iframe.enter_frame().await?;
    /// // We can now search for elements within the iframe.
    /// let elem = driver.find(By::Id("button1")).await?;
    /// elem.click().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn enter_frame(self) -> WebDriverResult<()> {
        self.handle.cmd(Command::SwitchToFrameElement(self.element_id.clone())).await?;
        Ok(())
    }

    /// Drag the element to a target element using JavaScript.
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("draggable")).await?;
    /// let target = driver.find(By::Id("target")).await?;
    /// elem.js_drag_to(&target).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn js_drag_to(&self, target: &Self) -> WebDriverResult<()> {
        self.handle
            .execute(SIMULATE_DRAG_AND_DROP, vec![self.to_json()?, target.to_json()?])
            .await?;
        Ok(())
    }

    /// Get the parent of the `WebElement`.
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("child")).await?;
    /// let parent = elem.parent().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn parent(&self) -> WebDriverResult<Self> {
        self.find(By::XPath("./..")).await
    }
}

impl fmt::Display for WebElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.element_id)
    }
}

impl Serialize for WebElement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.element_id.serialize(serializer)
    }
}
