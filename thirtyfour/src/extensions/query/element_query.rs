use super::conditions::{collect_arg_slice, handle_errors, negate};
use super::{conditions, ElementPollerNoWait, ElementPollerWithTimeout, IntoElementPoller};
use crate::error::{WebDriverError, WebDriverErrorInner};
use crate::prelude::WebDriverResult;
use crate::session::handle::SessionHandle;
use crate::IntoArcStr;
use crate::{By, DynElementPredicate, ElementPredicate, WebElement};
use indexmap::IndexMap;
use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter, Write};
use std::sync::Arc;
use std::time::Duration;
use stringmatch::Needle;

/// Get String containing comma-separated list of selectors used.
fn get_selector_summary(selectors: &[ElementSelector]) -> String {
    struct Criteria<'a>(&'a [ElementSelector]);

    impl Display for Criteria<'_> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            for (i, by) in self.0.iter().map(|s| &s.by).enumerate() {
                if i != 0 {
                    f.write_char(',')?
                }
                Display::fmt(by, f)?;
            }
            Ok(())
        }
    }

    format!("[{}]", Criteria(selectors))
}

fn get_elements_description(len: Option<usize>, description: &str) -> Cow<'_, str> {
    let suffix = match len {
        Some(1) => "element",
        Some(_) => "elements",
        None => "element(s)",
    };

    match description.trim() {
        "" => Cow::Borrowed(suffix),
        _ => Cow::Owned(format!("'{}' {suffix}", description.escape_default())),
    }
}

/// Helper function to return the NoSuchElement error struct.
fn no_such_element(selectors: &[ElementSelector], description: &str) -> WebDriverError {
    let element_description = get_elements_description(None, description);

    crate::error::no_such_element(format!(
        "no such element: {element_description} not found using selectors: {}",
        get_selector_summary(selectors)
    ))
}

/// Filter the specified elements using the specified filters.
pub async fn filter_elements<I, P, Ref>(
    mut elements: Vec<WebElement>,
    filters: I,
) -> WebDriverResult<Vec<WebElement>>
where
    I: IntoIterator<Item = Ref>,
    Ref: AsRef<P>,
    P: ElementPredicate + ?Sized,
{
    for func in filters {
        let tmp_elements = std::mem::take(&mut elements);
        for element in tmp_elements {
            if func.as_ref().call(element.clone()).await? {
                elements.push(element);
            }
        }

        if elements.is_empty() {
            break;
        }
    }

    Ok(elements)
}

/// An ElementSelector contains a selector method (By) as well as zero or more filters.
/// The filters will be applied to any elements matched by the selector.
/// Selectors and filters all run in full on every poll iteration.
pub struct ElementSelector {
    /// The selector to use.
    pub by: By,
    /// The filters for this element selector.
    pub filters: Vec<Box<DynElementPredicate>>,
}

impl Debug for ElementSelector {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElementSelector").field("by", &self.by).finish()
    }
}

impl ElementSelector {
    /// Create a new `ElementSelector`.
    pub fn new(by: By) -> Self {
        Self {
            by,
            filters: Vec::new(),
        }
    }

    /// Add the specified filter to the list of filters for this selector.
    pub fn add_filter(&mut self, f: impl ElementPredicate + 'static) {
        self.add_box_filter(DynElementPredicate::boxed(f));
    }

    /// Add the specified filter to the list of filters for this selector.
    pub fn add_box_filter(&mut self, f: Box<DynElementPredicate>) {
        self.filters.push(f);
    }
}

/// Elements can be queried from either a WebDriver or from a WebElement.
/// The command issued to the webdriver will differ depending on the source,
/// i.e. FindElement vs FindElementFromElement etc. but the ElementQuery
/// interface is the same for both.
#[derive(Debug)]
pub enum ElementQuerySource {
    /// Execute a query from the `WebDriver` instance.
    Driver(Arc<SessionHandle>),
    /// Execute a query using the specified `WebElement` as the base.
    Element(WebElement),
}

/// Options for wait characteristics for an element query.
#[derive(Debug, Clone)]
pub enum ElementQueryWaitOptions {
    /// Use the default poller.
    WaitDefault,
    /// Use a poller with the specified timeout and interval.
    Wait {
        /// The timeout for this poller.
        timeout: Duration,
        /// The minimum interval between attempts.
        interval: Duration,
    },
    /// Do not wait. This uses a poller that quits immediately.
    NoWait,
}

impl Default for ElementQueryWaitOptions {
    fn default() -> Self {
        Self::WaitDefault
    }
}

/// All options applicable to an ElementQuery.
///
/// These are stored in a separate struct so that they can be constructed
/// separately and applied to an ElementQuery in bulk if required.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct ElementQueryOptions {
    ignore_errors: Option<bool>,
    description: Option<Arc<str>>,
    wait: Option<ElementQueryWaitOptions>,
}

impl ElementQueryOptions {
    /// Set whether to ignore errors when querying elements.
    pub fn ignore_errors(mut self, ignore_errors: bool) -> Self {
        self.ignore_errors = Some(ignore_errors);
        self
    }

    /// Set whether to ignore errors when querying elements.
    pub fn set_ignore_errors(mut self, ignore_errors: Option<bool>) -> Self {
        self.ignore_errors = ignore_errors;
        self
    }

    /// Set the description to be used in error messages for this element query.
    pub fn description(mut self, description: impl IntoArcStr) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the description to be used in error messages for this element query.
    pub fn set_description<T: Into<Arc<str>>>(mut self, description: Option<T>) -> Self {
        self.description = description.map(|x| x.into());
        self
    }

    /// Set the wait options for this element query.
    pub fn wait(mut self, wait_option: ElementQueryWaitOptions) -> Self {
        self.wait = Some(wait_option);
        self
    }

    /// Set the wait options for this element query.
    pub fn set_wait(mut self, wait_option: Option<ElementQueryWaitOptions>) -> Self {
        self.wait = wait_option;
        self
    }
}

/// High-level interface for performing powerful element queries using a
/// builder pattern.
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
/// #         driver.goto("http://localhost:8000").await?;
/// // WebDriver::query() example.
/// let elem = driver.query(By::Css("div[data-section='section-buttons']")).first().await?;
/// // WebElement::query() example.
/// let elem_button = elem.query(By::Id("button1")).first().await?;
/// #         assert_eq!(elem_button.tag_name().await?, "button");
/// #         driver.quit().await?;
/// #         Ok(())
/// #     })
/// # }
/// ```
#[derive(Debug)]
pub struct ElementQuery {
    source: ElementQuerySource,
    poller: Arc<dyn IntoElementPoller + Send + Sync>,
    selectors: Vec<ElementSelector>,
    options: ElementQueryOptions,
}

macro_rules! disallow_empty {
    ($elements: expr, $self: expr) => {
        if $elements.is_empty() {
            let desc: &str = $self.options.description.as_deref().unwrap_or("");
            Err(no_such_element(&$self.selectors, desc))
        } else {
            Ok($elements)
        }
    };
}

impl ElementQuery {
    /// Create a new `ElementQuery`.
    ///
    /// See `WebDriver::query()` or `WebElement::query()` rather than instantiating
    /// this directly.
    pub fn new(
        source: ElementQuerySource,
        by: By,
        poller: Arc<dyn IntoElementPoller + Send + Sync>,
    ) -> Self {
        let selector = ElementSelector::new(by);
        Self {
            source,
            poller,
            selectors: vec![selector],
            options: ElementQueryOptions::default(),
        }
    }

    /// Provide the options to use with this query.
    pub fn options(mut self, options: ElementQueryOptions) -> Self {
        self.options = options;

        // Apply wait options.
        match self.options.wait {
            None | Some(ElementQueryWaitOptions::WaitDefault) => self,
            Some(ElementQueryWaitOptions::Wait {
                timeout,
                interval,
            }) => self.wait(timeout, interval),
            Some(ElementQueryWaitOptions::NoWait) => self.nowait(),
        }
    }

    /// Provide a name that will be included in the error message if the query was not successful.
    /// This is useful for providing more context about this particular query.
    pub fn desc(mut self, description: &str) -> Self {
        self.options = self.options.description(description);
        self
    }

    /// By default, a query will ignore any errors that occur while polling for the desired
    /// element(s).
    /// However, this behaviour can be modified so that the waiter will return
    /// early if an error is returned from thirtyfour.
    pub fn ignore_errors(mut self, ignore: bool) -> Self {
        self.options = self.options.ignore_errors(ignore);
        self
    }

    //
    // Poller / Waiter
    //

    /// Use the specified ElementPoller for this ElementQuery.
    /// This will not affect the default ElementPoller used for other queries.
    pub fn with_poller(mut self, poller: Arc<dyn IntoElementPoller + Send + Sync>) -> Self {
        self.poller = poller;
        self
    }

    /// Force this ElementQuery to wait for the specified timeout, polling once
    /// after each interval. This will override the poller for this
    /// ElementQuery only.
    pub fn wait(self, timeout: Duration, interval: Duration) -> Self {
        self.with_poller(Arc::new(ElementPollerWithTimeout::new(timeout, interval)))
    }

    /// Force this ElementQuery to not wait for the specified condition(s).
    /// This will override the poller for this ElementQuery only.
    pub fn nowait(self) -> Self {
        self.with_poller(Arc::new(ElementPollerNoWait))
    }

    //
    // Selectors
    //

    /// Add the specified selector to this ElementQuery. Callers should use
    /// the `or()` method instead.
    fn add_selector(mut self, selector: ElementSelector) -> Self {
        self.selectors.push(selector);
        self
    }

    /// Add a new selector to this ElementQuery. All conditions specified after
    /// this selector (up until the next `or()` method) will apply to this
    /// selector.
    pub fn or(self, by: By) -> Self {
        self.add_selector(ElementSelector::new(by))
    }

    //
    // Retrievers
    //

    /// Return true if an element matches any selector (including filters), otherwise false.
    pub async fn exists(&self) -> WebDriverResult<bool> {
        let elements = self.run_poller(true, false).await?;
        Ok(!elements.is_empty())
    }

    /// Return true if no element matches any selector (including filters), otherwise false.
    pub async fn not_exists(&self) -> WebDriverResult<bool> {
        let elements = self.run_poller(false, true).await?;
        Ok(elements.is_empty())
    }

    /// Return the first WebElement that matches any selector (including filters).
    ///
    /// Returns None if no elements match.
    pub async fn first_opt(&self) -> WebDriverResult<Option<WebElement>> {
        let elements = self.run_poller(true, false).await?;
        Ok(elements.into_iter().next())
    }

    /// Return only the first WebElement that matches any selector (including filters).
    ///
    /// Returns Err(WebDriverError::NoSuchElement) if no elements match.
    pub async fn first(&self) -> WebDriverResult<WebElement> {
        self.first_opt().await?.ok_or_else(|| {
            let desc: &str = self.options.description.as_deref().unwrap_or("");
            no_such_element(&self.selectors, desc)
        })
    }

    /// Return only a single WebElement that matches any selector (including filters).
    ///
    /// This method requires that only one element was found, and will return
    /// Err(WebDriverError::NoSuchElement) if the number of elements found after processing
    /// all selectors was not equal to 1.
    ///
    /// This is useful because sometimes your element query is not specific enough and
    /// might accidentally match multiple elements. This is a common source of bugs in
    /// automated tests, because the first element might not be the one you expect.
    ///
    /// By requiring that only one element is matched, you can be more sure that it is the
    /// one you intended.
    pub async fn single(&self) -> WebDriverResult<WebElement> {
        let mut elements = self.run_poller(false, false).await?;

        if elements.len() == 1 {
            Ok(elements.swap_remove(0))
        } else if !elements.is_empty() {
            let element_description = get_elements_description(
                Some(elements.len()),
                self.options.description.as_deref().unwrap_or(""),
            );

            Err(crate::error::no_such_element(format!(
                "too many elements received; found {count} {element_description} using selectors: {selectors}",
                count = elements.len(),
                selectors = get_selector_summary(&self.selectors)
            )))
        } else {
            let desc: &str = self.options.description.as_deref().unwrap_or("");
            let err = no_such_element(&self.selectors, desc);
            Err(err)
        }
    }

    /// Return all WebElements that match any selector (including filters).
    ///
    /// This will return when at least one element is found, after processing all selectors.
    ///
    /// Returns an empty Vec if no elements match.
    pub async fn any(&self) -> WebDriverResult<Vec<WebElement>> {
        self.run_poller(false, false).await
    }

    /// Return all WebElements that match any selector (including filters).
    ///
    /// This will return when at least one element is found, after processing all selectors.
    ///
    /// Returns Err(WebDriverError::NoSuchElement) if no elements match.
    pub async fn any_required(&self) -> WebDriverResult<Vec<WebElement>> {
        let elements = self.run_poller(false, false).await?;
        disallow_empty!(elements, self)
    }

    /// Return all WebElements that match any single selector (including filters).
    #[deprecated(since = "0.32.0", note = "use all_from_selector() instead")]
    pub async fn all(&self) -> WebDriverResult<Vec<WebElement>> {
        self.all_from_selector().await
    }

    /// Return all WebElements that match a single selector (including filters).
    ///
    /// This will return when at least one element is found from any selector, without
    /// processing other selectors afterwards.
    ///
    /// Returns an empty Vec if no elements match.
    pub async fn all_from_selector(&self) -> WebDriverResult<Vec<WebElement>> {
        self.run_poller(true, false).await
    }

    /// Return all WebElements that match any single selector (including filters).
    #[deprecated(since = "0.32.0", note = "use all_from_selector_required() instead")]
    pub async fn all_required(&self) -> WebDriverResult<Vec<WebElement>> {
        self.all_from_selector_required().await
    }

    /// Return all WebElements that match any single selector (including filters).
    ///
    /// This will return when at least one element is found from any selector, without
    /// processing other selectors afterwards.
    ///
    /// Returns Err(WebDriverError::NoSuchElement) if no elements match.
    pub async fn all_from_selector_required(&self) -> WebDriverResult<Vec<WebElement>> {
        let elements = self.run_poller(true, false).await?;
        disallow_empty!(elements, self)
    }

    /// Run the poller for this ElementQuery and return the Vec of WebElements matched.
    ///
    /// NOTE: This function doesn't return a no_such_element error and the caller must handle it.
    ///
    /// The parameters are as follows:
    /// - `short_circuit`:
    ///   - if true, return as soon as any selector meets the condition.
    ///     The elements returned will be only the elements from that selector.
    ///   - if false, only check the condition (and possibly return) after processing all selectors.
    /// - `stop_on_miss`:
    ///   - if true, the condition is true if no elements were found.
    ///   - if false, the condition is true if at least one element was found.
    ///
    async fn run_poller(
        &self,
        short_circuit: bool,
        stop_on_miss: bool,
    ) -> WebDriverResult<Vec<WebElement>> {
        let desc: &str = self.options.description.as_deref().unwrap_or("");
        let no_such_element_error = no_such_element(&self.selectors, desc);
        if self.selectors.is_empty() {
            return Err(no_such_element_error);
        }

        // Start the poller.
        let mut poller = self.poller.start();

        let mut elements = IndexMap::new();
        loop {
            for selector in &self.selectors {
                let mut new_elements =
                    match self.fetch_elements_from_source(selector.by.clone()).await {
                        Ok(x) => x,
                        Err(e) if matches!(*e, WebDriverErrorInner::NoSuchElement(_)) => Vec::new(),
                        Err(e) => return Err(e),
                    };

                if !new_elements.is_empty() {
                    new_elements = filter_elements(new_elements, &selector.filters).await?;
                }

                // Stop early?
                if short_circuit && (stop_on_miss == new_elements.is_empty()) {
                    return Ok(new_elements);
                }

                // Collect elements, excluding duplicates.
                for element in new_elements {
                    elements.insert(element.element_id(), element);
                }
            }

            // Once all selectors have been processed, check if we have a match.
            if stop_on_miss == elements.is_empty() {
                return Ok(elements.into_values().collect());
            }

            // On timeout, return any elements found so far.
            if !poller.tick().await {
                return Ok(elements.into_values().collect());
            }
        }
    }

    /// Execute the specified selector and return any matched WebElements.
    async fn fetch_elements_from_source(&self, by: By) -> WebDriverResult<Vec<WebElement>> {
        match &self.source {
            ElementQuerySource::Driver(driver) => driver.find_all(by).await,
            ElementQuerySource::Element(element) => element.find_all(by).await,
        }
    }

    //
    // Filters
    //

    /// Add the specified ElementPredicate to the last selector.
    pub fn with_filter(mut self, f: impl ElementPredicate + 'static) -> Self {
        if let Some(selector) = self.selectors.last_mut() {
            selector.add_filter(f);
        }
        self
    }

    //
    // Advance selectors
    //

    /// Only match elements that are enabled.
    pub fn and_enabled(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_enabled(ignore_errors))
    }

    /// Only match elements that are NOT enabled.
    pub fn and_not_enabled(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_not_enabled(ignore_errors))
    }

    /// Only match elements that are selected.
    pub fn and_selected(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_selected(ignore_errors))
    }

    /// Only match elements that are NOT selected.
    pub fn and_not_selected(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_not_selected(ignore_errors))
    }

    /// Only match elements that are displayed.
    pub fn and_displayed(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_displayed(ignore_errors))
    }

    /// Only match elements that are NOT displayed.
    pub fn and_not_displayed(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_not_displayed(ignore_errors))
    }

    /// Only match elements that are clickable.
    pub fn and_clickable(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_clickable(ignore_errors))
    }

    /// Only match elements that are NOT clickable.
    pub fn and_not_clickable(self) -> Self {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_is_not_clickable(ignore_errors))
    }

    //
    // By alternative helper selectors
    //

    /// Only match elements that have the specified text.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_text<N>(self, text: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_text(text, ignore_errors))
    }

    /// Only match elements that do not have the specified text.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_text<N>(self, text: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_text(text, ignore_errors))
    }

    /// Only match elements that have the specified id.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_id<N>(self, id: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(move |elem: WebElement| {
            let id = id.clone();
            async move {
                match elem.id().await {
                    Ok(Some(x)) => Ok(id.is_match(&x)),
                    Ok(None) => Ok(false),
                    Err(e) => handle_errors(Err(e), ignore_errors),
                }
            }
        })
    }

    /// Only match elements that do not have the specified id.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_id<N>(self, id: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(move |elem: WebElement| {
            let id = id.clone();
            async move {
                match elem.id().await {
                    Ok(Some(x)) => Ok(!id.is_match(&x)),
                    Ok(None) => Ok(true),
                    Err(e) => handle_errors(Err(e), ignore_errors),
                }
            }
        })
    }

    /// Only match elements that contain the specified class name.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_class<N>(self, class_name: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_class(class_name, ignore_errors))
    }

    /// Only match elements that do not contain the specified class name.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_class<N>(self, class_name: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_class(class_name, ignore_errors))
    }

    /// Only match elements that have the specified tag.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_tag<N>(self, tag_name: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(move |elem: WebElement| {
            let tag_name = tag_name.clone();
            async move {
                handle_errors(elem.tag_name().await.map(|x| tag_name.is_match(&x)), ignore_errors)
            }
        })
    }

    /// Only match elements that do not have the specified tag.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_tag<N>(self, tag_name: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(move |elem: WebElement| {
            let tag_name = tag_name.clone();
            async move {
                negate(elem.tag_name().await.map(|x| tag_name.is_match(&x)), ignore_errors)
            }
        })
    }

    /// Only match elements that have the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_value<N>(self, value: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_value(value, ignore_errors))
    }

    /// Only match elements that do not have the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_value<N>(self, value: N) -> Self
    where
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_value(value, ignore_errors))
    }

    /// Only match elements that have the specified attribute with the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_attribute<S, N>(self, attribute_name: S, value: N) -> Self
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_attribute(
            attribute_name.into(),
            value,
            ignore_errors,
        ))
    }

    /// Only match elements that do not have the specified attribute with the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_attribute<S, N>(self, attribute_name: S, value: N) -> Self
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_attribute(
            attribute_name.into(),
            value,
            ignore_errors,
        ))
    }

    /// Only match elements that have all the specified attributes with the specified values.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_attributes<S, N>(self, desired_attributes: impl IntoIterator<Item = (S, N)>) -> Self
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_attributes(
            collect_arg_slice(desired_attributes),
            ignore_errors,
        ))
    }

    /// Only match elements that do not have any of the specified attributes with the specified
    /// values. See the `Needle` documentation for more details on text matching rules.
    pub fn without_attributes<S, N>(
        self,
        desired_attributes: impl IntoIterator<Item = (S, N)>,
    ) -> Self
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_attributes(
            collect_arg_slice(desired_attributes),
            ignore_errors,
        ))
    }

    /// Only match elements that have the specified property with the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_property<S, N>(self, property_name: S, value: N) -> Self
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_property(
            property_name.into(),
            value,
            ignore_errors,
        ))
    }

    /// Only match elements that do not have the specified property with the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_property<S, N>(self, property_name: S, value: N) -> Self
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_property(
            property_name.into(),
            value,
            ignore_errors,
        ))
    }

    /// Only match elements that have all the specified properties with the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_properties<S, N>(self, desired_properties: impl IntoIterator<Item = (S, N)>) -> Self
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_properties(
            collect_arg_slice(desired_properties),
            ignore_errors,
        ))
    }

    /// Only match elements that do not have any of the specified properties with the specified
    /// value. See the `Needle` documentation for more details on text matching rules.
    pub fn without_properties<S, N>(
        self,
        desired_properties: impl IntoIterator<Item = (S, N)>,
    ) -> Self
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_properties(
            collect_arg_slice(desired_properties),
            ignore_errors,
        ))
    }

    /// Only match elements that have the specified CSS property with the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_css_property<S, N>(self, css_property_name: S, value: N) -> Self
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_css_property(
            css_property_name.into(),
            value,
            ignore_errors,
        ))
    }

    /// Only match elements that do not have the specified CSS property with the specified value.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_css_property<S, N>(self, css_property_name: S, value: N) -> Self
    where
        S: IntoArcStr,
        N: Needle + Clone + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_css_property(
            css_property_name.into(),
            value,
            ignore_errors,
        ))
    }

    /// Only match elements that have all the specified CSS properties with the
    /// specified values.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn with_css_properties<S, N>(
        self,
        desired_css_properties: impl IntoIterator<Item = (S, N)>,
    ) -> Self
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_has_css_properties(
            collect_arg_slice(desired_css_properties),
            ignore_errors,
        ))
    }

    /// Only match elements that do not have any of the specified CSS properties with the
    /// specified values.
    /// See the `Needle` documentation for more details on text matching rules.
    pub fn without_css_properties<S, N>(
        self,
        desired_css_properties: impl IntoIterator<Item = (S, N)>,
    ) -> Self
    where
        S: IntoArcStr,
        N: Needle + Send + Sync + 'static,
    {
        let ignore_errors = self.options.ignore_errors.unwrap_or_default();
        self.with_filter(conditions::element_lacks_css_properties(
            collect_arg_slice(desired_css_properties),
            ignore_errors,
        ))
    }
}

/// Trait for enabling the ElementQuery interface.
pub trait ElementQueryable {
    /// Start an element query using the specified selector.
    fn query(&self, by: By) -> ElementQuery;
}

impl ElementQueryable for WebElement {
    /// Return an ElementQuery instance for more executing powerful element queries.
    ///
    /// This uses the builder pattern to construct queries that will return one or
    /// more elements, depending on the method specified at the end of the chain.
    ///
    /// See [`ElementQuery`] for more documentation.
    fn query(&self, by: By) -> ElementQuery {
        ElementQuery::new(
            ElementQuerySource::Element(self.clone()),
            by,
            self.handle.config().poller.clone(),
        )
    }
}

impl ElementQueryable for Arc<SessionHandle> {
    /// Return an ElementQuery instance for more executing powerful element queries.
    ///
    /// This uses the builder pattern to construct queries that will return one or
    /// more elements, depending on the method specified at the end of the chain.
    ///
    /// See [`ElementQuery`] for more documentation.
    fn query(&self, by: By) -> ElementQuery {
        ElementQuery::new(
            ElementQuerySource::Driver(self.clone()),
            by,
            self.config().poller.clone(),
        )
    }
}

#[cfg(test)]
/// This function checks if the public async methods implement Send. It is not intended to be executed.
async fn _test_is_send() -> WebDriverResult<()> {
    use crate::prelude::*;

    // Helper methods
    fn is_send<T: Send>() {}
    fn is_send_val<T: Send>(_val: &T) {}

    // ElementSelector
    let selector = ElementSelector::new(By::Css("div"));
    is_send_val(&filter_elements(Vec::new(), &selector.filters));

    // Pre values
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:4444", caps).await?;

    // ElementQuery
    let query = driver.query(By::Css("div"));
    is_send_val(&query.exists());
    is_send_val(&query.not_exists());
    is_send_val(&query.first());
    is_send_val(&query.all_from_selector());
    is_send_val(&query.all_from_selector_required());

    Ok(())
}
