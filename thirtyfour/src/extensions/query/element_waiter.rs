#[allow(unused)]
use super::conditions;
use super::conditions::{collect_arg_slice, handle_errors};
use super::poller::{AnyElementPoller, ElementPoller, ElementPollerWithTimeout, IntoElementPoller};
use crate::error::WebDriverError;
use crate::prelude::WebDriverResult;
use crate::IntoArcStr;
use crate::{DynElementPredicate, ElementPredicate, WebElement};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use stringmatch::Needle;

/// High-level interface for performing explicit waits using the builder pattern.
///
/// # Example:
/// ```no_run
/// # use thirtyfour::prelude::*;
/// # use thirtyfour::support::block_on;
/// #
/// # fn main() -> WebDriverResult<()> {
/// #     block_on(async {
/// #         let caps = DesiredCapabilities::chrome();
/// #         let mut driver = WebDriver::new("http://localhost:4444", caps).await?;
/// let elem = driver.query(By::Id("button1")).first().await?;
/// // Wait until the element is displayed.
/// elem.wait_until().displayed().await?;
/// #         driver.quit().await?;
/// #         Ok(())
/// #     })
/// # }
/// ```
#[derive(Debug)]
pub struct ElementWaiter {
    element: WebElement,
    poller: AnyElementPoller,
    message: String,
    ignore_errors: bool,
}

impl ElementWaiter {
    /// Create a new `ElementWaiter`.
    ///
    /// See `Element::wait_until()` rather than creating this directly.
    #[must_use] 
    pub fn new(element: WebElement, poller: AnyElementPoller) -> Self {
        Self {
            element,
            poller,
            message: String::new(),
            ignore_errors: true,
        }
    }

    /// Use the specified `ElementPoller` for this `ElementWaiter`.
    /// This will not affect the default `ElementPoller` used for other waits.
    pub fn with_poller(mut self, poller: impl IntoElementPoller) -> Self {
        self.poller = poller.start();
        self
    }

    /// Provide a human-readable error message to be returned in the case of timeout.
    #[must_use] 
    pub fn error(mut self, message: &str) -> Self {
        self.message = message.to_string();
        self
    }

    /// By default, a waiter will ignore any errors that occur while polling for the desired
    /// condition(s). However, this behaviour can be modified so that the waiter will return
    /// early if an error is returned from thirtyfour.
    #[must_use] 
    pub fn ignore_errors(mut self, ignore: bool) -> Self {
        self.ignore_errors = ignore;
        self
    }

    /// Force this `ElementWaiter` to wait for the specified timeout, polling once
    /// after each interval. This will override the poller for this
    /// `ElementWaiter` only.
    #[must_use] 
    pub fn wait(self, timeout: Duration, interval: Duration) -> Self {
        self.with_poller(ElementPollerWithTimeout::new(timeout, interval))
    }

    async fn run_poller<'a, F, I, P>(&self, conditions: F) -> WebDriverResult<bool>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = &'a P>,
        P: ElementPredicate + ?Sized + 'a,
    {
        let mut poller = self.poller.clone();
        loop {
            let mut conditions_met = true;
            for f in conditions() {
                if !f.call(self.element.clone()).await? {
                    conditions_met = false;
                    break;
                }
            }

            if conditions_met {
                return Ok(true);
            }

            if !poller.tick().await {
                return Ok(false);
            }
        }
    }

    fn timeout(self) -> WebDriverResult<()> {
        Err(WebDriverError::Timeout(format!("element condition timed out: {}", self.message)))
    }

    /// Wait for the specified condition to be true.
    pub async fn condition(self, f: impl ElementPredicate) -> WebDriverResult<()> {
        if self.run_poller(|| [&f].into_iter()).await? {
            Ok(())
        } else {
            self.timeout()
        }
    }

    /// Wait for the specified conditions to be true.
    pub async fn conditions(
        self,
        conditions: Vec<Box<DynElementPredicate>>,
    ) -> WebDriverResult<()> {
        if self.run_poller(|| conditions.iter().map(Box::deref)).await? {
            Ok(())
        } else {
            self.timeout()
        }
    }

    /// Wait for the element to become stale.
    pub async fn stale(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(move |elem: WebElement| async move {
            handle_errors(elem.is_present().await.map(|x| !x), ignore_errors)
        })
        .await
    }

    /// Wait for the element to be displayed.
    pub async fn displayed(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_displayed(ignore_errors)).await
    }

    /// Wait for the element to not be displayed.
    pub async fn not_displayed(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_not_displayed(ignore_errors)).await
    }

    /// Wait for the element to be selected.
    pub async fn selected(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_selected(ignore_errors)).await
    }

    /// Wait for the element to not be selected.
    pub async fn not_selected(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_not_selected(ignore_errors)).await
    }

    /// Wait for the element to be enabled.
    pub async fn enabled(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_enabled(ignore_errors)).await
    }

    /// Wait for the element to not be enabled.
    pub async fn not_enabled(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_not_enabled(ignore_errors)).await
    }

    /// Wait for the element to be clickable.
    pub async fn clickable(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_clickable(ignore_errors)).await
    }

    /// Wait for the element to not be clickable.
    pub async fn not_clickable(self) -> WebDriverResult<()> {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_is_not_clickable(ignore_errors)).await
    }

    /// Wait until the element has the specified class.
    pub async fn has_class<N>(self, class_name: N) -> WebDriverResult<()>
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_class(class_name, ignore_errors)).await
    }

    /// Wait until the element lacks the specified class.
    pub async fn lacks_class<N>(self, class_name: N) -> WebDriverResult<()>
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_class(class_name, ignore_errors)).await
    }

    /// Wait until the element has the specified text.
    pub async fn has_text<N>(self, text: N) -> WebDriverResult<()>
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_text(text, ignore_errors)).await
    }

    /// Wait until the element lacks the specified text.
    pub async fn lacks_text<N>(self, text: N) -> WebDriverResult<()>
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_text(text, ignore_errors)).await
    }

    /// Wait until the element has the specified value.
    pub async fn has_value<N>(self, value: N) -> WebDriverResult<()>
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_value(value, ignore_errors)).await
    }

    /// Wait until the element lacks the specified value.
    pub async fn lacks_value<N>(self, value: N) -> WebDriverResult<()>
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_value(value, ignore_errors)).await
    }

    /// Wait until the element has the specified attribute.
    pub async fn has_attribute<S, N>(self, attribute_name: S, value: N) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_attribute(
            attribute_name.into(),
            value,
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element lacks the specified attribute.
    pub async fn lacks_attribute<S, N>(self, attribute_name: S, value: N) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_attribute(
            attribute_name.into(),
            value,
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element has all the specified attributes.
    pub async fn has_attributes<S, N>(
        self,
        desired_attributes: impl IntoIterator<Item = (S, N)>,
    ) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_attributes(
            collect_arg_slice(desired_attributes),
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element lacks all the specified attributes.
    pub async fn lacks_attributes<S, N>(
        self,
        desired_attributes: impl IntoIterator<Item = (S, N)>,
    ) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_attributes(
            collect_arg_slice(desired_attributes),
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element has the specified property.
    pub async fn has_property<S, N>(self, property_name: S, value: N) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_property(property_name.into(), value, ignore_errors))
            .await
    }

    /// Wait until the element lacks the specified property.
    pub async fn lacks_property<S, N>(self, property_name: S, value: N) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_property(
            property_name.into(),
            value,
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element has all the specified properties.
    pub async fn has_properties<S, N>(
        self,
        desired_properties: impl IntoIterator<Item = (S, N)>,
    ) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let desired_properties: Arc<[(Arc<str>, N)]> =
            desired_properties.into_iter().map(|(a, b)| (a.into(), b)).collect();
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_properties(desired_properties, ignore_errors)).await
    }

    /// Wait until the element lacks all the specified properties.
    pub async fn lacks_properties<S, N>(
        self,
        desired_properties: impl IntoIterator<Item = (S, N)>,
    ) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_properties(
            collect_arg_slice(desired_properties),
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element has the specified CSS property.
    pub async fn has_css_property<S, N>(self, css_property_name: S, value: N) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_css_property(
            css_property_name.into(),
            value,
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element lacks the specified CSS property.
    pub async fn lacks_css_property<S, N>(
        self,
        css_property_name: S,
        value: N,
    ) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_css_property(
            css_property_name.into(),
            value,
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element has all the specified CSS properties.
    pub async fn has_css_properties<S, N>(
        self,
        desired_css_properties: impl IntoIterator<Item = (S, N)>,
    ) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_has_css_properties(
            collect_arg_slice(desired_css_properties),
            ignore_errors,
        ))
        .await
    }

    /// Wait until the element lacks all the specified CSS properties.
    pub async fn lacks_css_properties<S, N>(
        self,
        desired_css_properties: impl IntoIterator<Item = (S, N)>,
    ) -> WebDriverResult<()>
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.ignore_errors;
        self.condition(conditions::element_lacks_css_properties(
            collect_arg_slice(desired_css_properties),
            ignore_errors,
        ))
        .await
    }
}

/// Trait for enabling the `ElementWaiter` interface.
pub trait ElementWaitable {
    /// Wait until the element meets one or more conditions.
    fn wait_until(&self) -> ElementWaiter;
}

impl ElementWaitable for WebElement {
    /// Return an `ElementWaiter` instance for more executing powerful explicit waits.
    ///
    /// This uses the builder pattern to construct explicit waits using one of the
    /// provided predicates. Or you can provide your own custom predicate if desired.
    ///
    /// See [`ElementWaiter`] for more documentation.
    fn wait_until(&self) -> ElementWaiter {
        ElementWaiter::new(self.clone(), self.handle.config().poller.clone())
    }
}

#[cfg(test)]
/// This function checks if the public async methods implement Send. It is not intended to be executed.
async fn _test_is_send() -> WebDriverResult<()> {
    use crate::prelude::*;

    // Helper methods
    fn is_send<T: Send>() {}
    fn is_send_val<T: Send>(_val: &T) {}

    // Pre values
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:4444", caps).await?;
    let elem = driver.find(By::Css(r#"div"#)).await?;

    // ElementWaitCondition
    is_send_val(&elem.wait_until().stale());
    is_send_val(&elem.wait_until().displayed());
    is_send_val(&elem.wait_until().selected());
    is_send_val(&elem.wait_until().enabled());
    is_send_val(
        &elem.wait_until().condition(move |elem: WebElement| async move {
            elem.is_enabled().await.or(Ok(false))
        }),
    );

    Ok(())
}
