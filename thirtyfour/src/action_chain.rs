use crate::session::handle::SessionHandle;
use crate::{
    common::{
        action::{ActionSource, KeyAction, PointerAction, PointerActionType},
        command::{Actions, Command},
        keys::TypingData,
    },
    error::WebDriverResult,
    WebElement,
};
use std::sync::Arc;
use std::time::Duration;

/// The `ActionChain` struct allows you to perform multiple input actions in
/// a sequence, including drag-and-drop, send keystrokes to an element, and
/// hover the mouse over an element.
///
/// The easiest way to construct an `ActionChain` struct is via the `WebDriver`
/// struct.
///
/// # Example:
/// ```ignore
/// driver.action_chain().drag_and_drop_element(elem_src, elem_target).perform().await?;
/// ```
#[derive(Debug)]
pub struct ActionChain {
    handle: Arc<SessionHandle>,
    key_actions: ActionSource<KeyAction>,
    pointer_actions: ActionSource<PointerAction>,
}

impl ActionChain {
    /// Create a new `ActionChain` struct.
    ///
    /// See [`WebDriver::action_chain()`](../struct.WebDriver.html#method.action_chain)
    /// for more details.
    pub fn new(handle: Arc<SessionHandle>) -> Self {
        ActionChain {
            handle,
            key_actions: ActionSource::<KeyAction>::new("key", None),
            pointer_actions: ActionSource::<PointerAction>::new(
                "pointer",
                PointerActionType::Mouse,
                None,
            ),
        }
    }

    /// Create a new `ActionChain` struct with custom action delays.
    ///
    /// The [`Duration`] is the time before an action is executed in the chain.
    ///
    /// `key_delay` defaults to 0ms, `pointer_delay` defaults to 250ms
    ///
    /// See [`WebDriver::action_chain()`](../struct.WebDriver.html#method.action_chain)
    /// for more details.
    pub fn new_with_delay(
        handle: Arc<SessionHandle>,
        key_delay: Option<Duration>,
        pointer_delay: Option<Duration>,
    ) -> Self {
        ActionChain {
            handle,
            key_actions: ActionSource::<KeyAction>::new("key", key_delay),
            pointer_actions: ActionSource::<PointerAction>::new(
                "pointer",
                PointerActionType::Mouse,
                pointer_delay,
            ),
        }
    }

    /// Reset all actions, reverting all input devices to default states.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// // Hold mouse button down on element.
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().click_and_hold_element(&elem).perform().await?;
    /// let elem_result = driver.find(By::Id("button-result")).await?;
    /// assert_eq!(elem_result.text().await?, "Button 1 down");
    /// // Now reset all actions.
    /// driver.action_chain().reset_actions().await?;
    /// // Mouse button is now released.
    /// assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if the WebDriver command fails.
    pub async fn reset_actions(&self) -> WebDriverResult<()> {
        self.handle.cmd(Command::ReleaseActions).await?;
        Ok(())
    }

    /// Perform the action sequence. No actions are actually performed until
    /// this method is called.
    ///
    /// # Errors
    /// Returns an error if the WebDriver command fails.
    pub async fn perform(&self) -> WebDriverResult<()> {
        let actions = Actions::from(serde_json::json!([self.key_actions, self.pointer_actions]));
        self.handle.cmd(Command::PerformActions(actions)).await?;
        Ok(())
    }

    /// Click and release the left mouse button.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().move_to_element_center(&elem).click().perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn click(mut self) -> Self {
        self.pointer_actions.click();
        // Click = 2 actions (PointerDown + PointerUp).
        self.key_actions.pause();
        self.key_actions.pause();
        self
    }

    /// Click on the specified element using the left mouse button and release.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().click_element(&elem).perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn click_element(self, element: &WebElement) -> Self {
        self.move_to_element_center(element).click()
    }

    /// Click the left mouse button and hold it down.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "None");
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().move_to_element_center(&elem).click_and_hold().perform().await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 down");
    /// #         driver.action_chain().release().perform().await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn click_and_hold(mut self) -> Self {
        self.pointer_actions.click_and_hold();
        self.key_actions.pause();
        self
    }

    /// Click on the specified element using the left mouse button and
    /// hold the button down.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "None");
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().click_and_hold_element(&elem).perform().await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 down");
    /// #         driver.action_chain().release().perform().await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn click_and_hold_element(self, element: &WebElement) -> Self {
        self.move_to_element_center(element).click_and_hold()
    }

    /// Click and release the right mouse button.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().move_to_element_center(&elem).context_click().perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 right-clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn context_click(mut self) -> Self {
        self.pointer_actions.context_click();
        // Click = 2 actions (PointerDown + PointerUp).
        self.key_actions.pause();
        self.key_actions.pause();
        self
    }

    /// Click on the specified element using the right mouse button and release.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().context_click_element(&elem).perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 right-clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn context_click_element(self, element: &WebElement) -> Self {
        self.move_to_element_center(element).context_click()
    }

    /// Double-click the left mouse button.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().move_to_element_center(&elem).double_click().perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 double-clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn double_click(mut self) -> Self {
        self.pointer_actions.double_click();
        // Each click = 2 actions (PointerDown + PointerUp).
        for _ in 0..4 {
            self.key_actions.pause();
        }
        self
    }

    /// Double-click on the specified element.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain().double_click_element(&elem).perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 double-clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn double_click_element(self, element: &WebElement) -> Self {
        self.move_to_element_center(element).double_click()
    }

    /// Drag the mouse cursor from the center of the source element to the
    /// center of the target element.
    #[must_use]
    pub fn drag_and_drop_element(self, source: &WebElement, target: &WebElement) -> Self {
        self.click_and_hold_element(source).release_on_element(target)
    }

    /// Drag the mouse cursor by the specified X and Y offsets.
    #[must_use]
    pub fn drag_and_drop_by_offset(self, x_offset: i64, y_offset: i64) -> Self {
        self.click_and_hold().move_by_offset(x_offset, y_offset)
    }

    /// Drag the mouse cursor by the specified X and Y offsets, starting
    /// from the center of the specified element.
    #[must_use]
    pub fn drag_and_drop_element_by_offset(
        self,
        element: &WebElement,
        x_offset: i64,
        y_offset: i64,
    ) -> Self {
        self.click_and_hold_element(element).move_by_offset(x_offset, y_offset)
    }

    /// Press the specified key down.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         driver.find(By::Id("pagetextinput")).await?.click().await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// #         assert_eq!(elem.value().await?, Some("".to_string()));
    /// driver.action_chain().click_element(&elem).key_down('a').perform().await?;
    /// #         assert_eq!(elem.value().await?, Some("a".to_string()));
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn key_down<T>(mut self, value: T) -> Self
    where
        T: Into<char>,
    {
        self.key_actions.key_down(value.into());
        self.pointer_actions.pause();
        self
    }

    /// Click the specified element and then press the specified key down.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         driver.find(By::Id("pagetextinput")).await?.click().await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// #         assert_eq!(elem.value().await?, Some("".to_string()));
    /// driver.action_chain().key_down_on_element(&elem, 'a').perform().await?;
    /// #         assert_eq!(elem.value().await?, Some("a".to_string()));
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn key_down_on_element<T>(self, element: &WebElement, value: T) -> Self
    where
        T: Into<char>,
    {
        self.click_element(element).key_down(value)
    }

    /// Release the specified key. This usually follows a `key_down()` action.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         driver.find(By::Id("pagetextinput")).await?.click().await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// #         assert_eq!(elem.value().await?, Some("".to_string()));
    /// elem.send_keys("selenium").await?;
    /// assert_eq!(elem.value().await?, Some("selenium".to_string()));
    /// driver.action_chain()
    ///     .key_down_on_element(&elem, Key::Control).key_down('a')
    ///     .key_up(Key::Control).key_up('a')
    ///     .key_down('b')
    ///     .perform().await?;
    /// assert_eq!(elem.value().await?, Some("b".to_string()));
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn key_up<T>(mut self, value: T) -> Self
    where
        T: Into<char>,
    {
        self.key_actions.key_up(value.into());
        self.pointer_actions.pause();
        self
    }

    /// Click the specified element and release the specified key.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         driver.find(By::Id("pagetextinput")).await?.click().await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// #         assert_eq!(elem.value().await?, Some("".to_string()));
    /// elem.send_keys("selenium").await?;
    /// assert_eq!(elem.value().await?, Some("selenium".to_string()));
    /// driver.action_chain()
    ///     .key_down_on_element(&elem, Key::Control).key_down('a')
    ///     .key_up_on_element(&elem, 'a').key_up_on_element(&elem, Key::Control)
    ///     .key_down('b')
    ///     .perform().await?;
    /// assert_eq!(elem.value().await?, Some("b".to_string()));
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn key_up_on_element<T>(self, element: &WebElement, value: T) -> Self
    where
        T: Into<char>,
    {
        self.click_element(element).key_up(value)
    }

    /// Move the mouse cursor to the specified X and Y coordinates.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// let center = elem.rect().await?.icenter();
    /// driver.action_chain()
    ///     .move_to(center.0, center.1)
    ///     .click()
    ///     .perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn move_to(mut self, x: i64, y: i64) -> Self {
        self.pointer_actions.move_to(x, y);
        self.key_actions.pause();
        self
    }

    /// Move the mouse cursor by the specified X and Y offsets.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem1 = driver.find(By::Id("button1")).await?;
    /// let elem2 = driver.find(By::Id("button2")).await?;
    /// // We will calculate the distance between the two center points and then
    /// // use action_chain() to move to the second button before clicking.
    /// let offset = elem2.rect().await?.center().0 as i64 - elem1.rect().await?.center().0 as i64;
    /// driver.action_chain()
    ///     .move_to_element_center(&elem1)
    ///     .move_by_offset(offset, 0)
    ///     .click()
    ///     .perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 2 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn move_by_offset(mut self, x_offset: i64, y_offset: i64) -> Self {
        self.pointer_actions.move_by(x_offset, y_offset);
        self.key_actions.pause();
        self
    }

    /// Move the mouse cursor to the center of the specified element.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// driver.action_chain()
    ///     .move_to_element_center(&elem)
    ///     .click()
    ///     .perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn move_to_element_center(mut self, element: &WebElement) -> Self {
        self.pointer_actions.move_to_element_center(element.element_id.clone());
        self.key_actions.pause();
        self
    }

    /// Move the mouse cursor to the specified offsets relative to the specified
    /// element's center position.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         driver.find(By::Id("button1")).await?.click().await?;
    /// // Select the text in the source element and copy it to the clipboard.
    /// let elem = driver.find(By::Id("button-result")).await?;
    /// let width = elem.rect().await?.width;
    /// driver.action_chain()
    ///     .move_to_element_with_offset(&elem, (-width / 2.0) as i64, 0)
    ///     .drag_and_drop_by_offset(width as i64, 0)
    ///     .key_down(Key::Control)
    ///     .key_down('c').key_up('c')
    ///     .key_up(Key::Control)
    ///     .perform().await?;
    ///
    /// #         driver.find(By::Id("pagetextinput")).await?.click().await?;
    /// // Now paste the text into the input field.
    /// let elem_tgt = driver.find(By::Name("input1")).await?;
    /// elem_tgt.send_keys(Key::Control + "v").await?;
    /// #         assert_eq!(elem_tgt.value().await?, Some("Button 1 clicked".to_string()));
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn move_to_element_with_offset(
        mut self,
        element: &WebElement,
        x_offset: i64,
        y_offset: i64,
    ) -> Self {
        self.pointer_actions.move_to_element(element.element_id.clone(), x_offset, y_offset);
        self.key_actions.pause();
        self
    }

    /// Release the left mouse button.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         let elem = driver.find(By::Id("button1")).await?;
    /// #         driver.action_chain().click_and_hold_element(&elem).perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 down");
    /// driver.action_chain().release().perform().await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn release(mut self) -> Self {
        self.pointer_actions.release();
        self.key_actions.pause();
        self
    }

    /// Move the mouse to the specified element and release the mouse button.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         let elem = driver.find(By::Id("button1")).await?;
    /// #         driver.action_chain().click_and_hold_element(&elem).perform().await?;
    /// #         let elem_result = driver.find(By::Id("button-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 down");
    /// driver.action_chain().release_on_element(&elem).perform().await?;
    /// #         assert_eq!(elem_result.text().await?, "Button 1 clicked");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    #[must_use]
    pub fn release_on_element(self, element: &WebElement) -> Self {
        self.move_to_element_center(element).release()
    }

    /// Send the specified keystrokes to the active element.
    ///
    /// # Example:
    /// ```no_run
    /// use thirtyfour::Key;
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         driver.find(By::Id("pagetextinput")).await?.click().await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// let button = driver.find(By::Id("button-set")).await?;
    /// #         assert_eq!(elem.value().await?, Some("".to_string()));
    /// driver.action_chain()
    ///     .click_element(&elem)
    ///     .send_keys("selenium")
    ///     .click_element(&button)
    ///     .perform().await?;
    /// #         let elem_result = driver.find(By::Id("input-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "selenium");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn send_keys<S>(mut self, text: S) -> Self
    where
        S: Into<TypingData>,
    {
        let typing: TypingData = text.into();
        for c in typing.as_vec() {
            self = self.key_down(c).key_up(c);
        }
        self
    }

    /// Click on the specified element and send the specified keystrokes.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444/wd/hub", caps).await?;
    /// #         driver.get("http://webappdemo").await?;
    /// #         driver.find(By::Id("pagetextinput")).await?.click().await?;
    /// let elem = driver.find(By::Name("input1")).await?;
    /// let button = driver.find(By::Id("button-set")).await?;
    /// #         assert_eq!(elem.value().await?, Some("".to_string()));
    /// driver.action_chain()
    ///     .send_keys_to_element(&elem, "selenium")
    ///     .click_element(&button)
    ///     .perform().await?;
    /// #         let elem_result = driver.find(By::Id("input-result")).await?;
    /// #         assert_eq!(elem_result.text().await?, "selenium");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn send_keys_to_element<S>(self, element: &WebElement, text: S) -> Self
    where
        S: Into<TypingData>,
    {
        self.click_element(element).send_keys(text)
    }
}
