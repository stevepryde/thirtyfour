use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::OnceCell;

use crate::components::Component;
use crate::error::WebDriverResult;
use crate::extensions::query::ElementQueryOptions;
use crate::prelude::ElementQueryable;
use crate::{By, DynElementQueryFn, ElementQueryFn, WebElement};

/// Type alias for `ElementResolver<WebElement>`, for convenience.
pub type ElementResolverSingle = ElementResolver<WebElement>;
/// Type alias for `ElementResolver<Vec<WebElement>>` for convenience.
pub type ElementResolverMulti = ElementResolver<Vec<WebElement>>;

/// `resolve!(x)` expands to `x.resolve().await?`
#[macro_export]
macro_rules! resolve {
    ($a:expr) => {
        $a.resolve().await?
    };
}

/// `resolve_present!(x)` expands to `x.resolve_present().await?`
#[macro_export]
macro_rules! resolve_present {
    ($a:expr) => {
        $a.resolve_present().await?
    };
}

/// Element resolver that can resolve a particular element or list of elements on demand.
///
/// Once resolved, the result will be cached for later retrieval until manually invalidated.
#[derive(Clone)]
pub struct ElementResolver<T> {
    base_element: WebElement,
    query_fn: Arc<DynElementQueryFn<T>>,
    element: Arc<ArcSwap<OnceCell<T>>>,
}

impl<T: Debug> Debug for ElementResolver<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let _guard = self.element.load();
        f.debug_struct("ElementResolver")
            .field("base_element", &self.base_element)
            .finish_non_exhaustive()
    }
}

impl<T: Clone + 'static> ElementResolver<T> {
    /// Create a new resolver using a custom resolver function.
    pub fn new_custom(
        base_element: WebElement,
        query_fn: impl ElementQueryFn<T> + 'static,
    ) -> Self {
        Self {
            base_element,
            query_fn: DynElementQueryFn::arc(query_fn),
            element: Arc::new(ArcSwap::from_pointee(OnceCell::new())),
        }
    }

    fn peek(&self) -> Option<T> {
        self.element.load().get().cloned()
    }

    /// Return the cached element(s) if any, otherwise run the query and return the result.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or if the element is not found.
    pub async fn resolve(&self) -> WebDriverResult<T> {
        self.element
            .load()
            .get_or_try_init(|| self.query_fn.call(self.base_element.clone()))
            .await
            .cloned()
    }

    /// Invalidate any cached element(s).
    pub fn invalidate(&self) {
        if self.element.load().initialized() {
            self.element.store(Arc::new(OnceCell::new()));
        }
    }

    /// Run the query, ignoring any cached element(s).
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or if the element is not found.
    pub async fn resolve_force(&self) -> WebDriverResult<T>
    where
        T: Clone,
    {
        self.invalidate();
        self.resolve().await
    }
}

mod sealed {
    use std::future::Future;

    use futures_util::{StreamExt, TryStreamExt};

    use crate::components::Component;
    use crate::error::WebDriverResult;
    use crate::WebElement;

    pub trait Resolve: Sized {
        fn is_present(&self) -> impl Future<Output = WebDriverResult<bool>> + Send;
    }

    impl Resolve for WebElement {
        async fn is_present(&self) -> WebDriverResult<bool> {
            self.is_present().await
        }
    }

    impl<T: Component + Sync> Resolve for T {
        async fn is_present(&self) -> WebDriverResult<bool> {
            self.base_element().is_present().await
        }
    }

    impl<T: Resolve + Sync> Resolve for Vec<T> {
        fn is_present(&self) -> impl Future<Output = WebDriverResult<bool>> + Send {
            futures_util::stream::iter(self)
                .map(Resolve::is_present)
                // 16 is arbitrary, just don't send too many requests at the same time
                .buffer_unordered(self.len().min(16))
                .try_all(std::future::ready)
        }
    }
}

/// Either an element or component, or a Vec of [`Resolve`]
pub trait Resolve: sealed::Resolve {}
impl<T: sealed::Resolve> Resolve for T {}

impl<T: Resolve + Clone + 'static> ElementResolver<T> {
    /// Validate that the cached component is present, and if so, return it.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or if the element is not found.
    pub async fn validate(&self) -> WebDriverResult<Option<T>> {
        match self.peek() {
            Some(component) => Ok(component.is_present().await?.then_some(component)),
            None => Ok(None),
        }
    }

    /// Validate the element or component and repeat the query if it is not present, returning the result.
    ///
    /// If the component is already present, the cached component will be returned without
    /// performing an additional query.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or if the element is not found.
    pub async fn resolve_present(&self) -> WebDriverResult<T> {
        match self.validate().await? {
            Some(component) => Ok(component),
            None => self.resolve_force().await,
        }
    }
}

impl ElementResolver<WebElement> {
    /// Create a new element resolver that must return a single element.
    #[must_use]
    pub fn new_single(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move { elem.query(by).single().await }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that must return a single element, with extra options.
    #[must_use]
    pub fn new_single_opts(base_element: WebElement, by: By, options: ElementQueryOptions) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move { elem.query(by).options(options).single().await }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns the first element.
    #[must_use]
    pub fn new_first(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move { elem.query(by).first().await }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns the first element, with extra options.
    #[must_use]
    pub fn new_first_opts(base_element: WebElement, by: By, options: ElementQueryOptions) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move { elem.query(by).options(options).first().await }
        };
        Self::new_custom(base_element, resolver)
    }
}

impl ElementResolver<Vec<WebElement>> {
    /// Create a new element resolver that returns all elements, if any.
    ///
    /// If no elements were found, this will resolve to an empty Vec.
    #[must_use]
    pub fn new_allow_empty(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move { elem.query(by).all_from_selector().await }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns all elements (if any), with extra options.
    #[must_use]
    pub fn new_allow_empty_opts(
        base_element: WebElement,
        by: By,
        options: ElementQueryOptions,
    ) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move { elem.query(by).options(options).all_from_selector().await }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns at least one element.
    ///
    /// If no elements were found, a `NoSuchElement` error will be returned by the resolver's
    /// `resolve()` method.
    #[must_use]
    pub fn new_not_empty(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move { elem.query(by).all_from_selector_required().await }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns at least one element, with extra options.
    ///
    /// If no elements were found, a `NoSuchElement` error will be returned by the resolver's
    /// `resolve()` method.
    #[must_use]
    pub fn new_not_empty_opts(
        base_element: WebElement,
        by: By,
        options: ElementQueryOptions,
    ) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move { elem.query(by).options(options).all_from_selector_required().await }
        };
        Self::new_custom(base_element, resolver)
    }
}

impl<T: Component + Clone + 'static> ElementResolver<T> {
    /// Create a new element resolver that must return a single component.
    #[must_use]
    pub fn new_single(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move {
                let elem = elem.query(by).single().await?;
                Ok(elem.into())
            }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that must return a single component, with extra options.
    #[must_use]
    pub fn new_single_opts(base_element: WebElement, by: By, options: ElementQueryOptions) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move {
                let elem = elem.query(by).options(options).single().await?;
                Ok(elem.into())
            }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns the first component.
    #[must_use]
    pub fn new_first(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move {
                let elem = elem.query(by).first().await?;
                Ok(elem.into())
            }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns the first component, with extra options.
    #[must_use]
    pub fn new_first_opts(base_element: WebElement, by: By, options: ElementQueryOptions) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move {
                let elem = elem.query(by).options(options).first().await?;
                Ok(elem.into())
            }
        };
        Self::new_custom(base_element, resolver)
    }
}

impl<T: Component + Clone + 'static> ElementResolver<Vec<T>> {
    /// Create a new element resolver that returns all components, if any.
    ///
    /// If no components were found, this will resolve to an empty Vec.
    #[must_use]
    pub fn new_allow_empty(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move {
                let elems = elem.query(by).all_from_selector().await?;
                Ok(elems.into_iter().map(T::from).collect())
            }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns all components (if any), with extra options.
    #[must_use]
    pub fn new_allow_empty_opts(
        base_element: WebElement,
        by: By,
        options: ElementQueryOptions,
    ) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move {
                let elems = elem.query(by).options(options).all_from_selector().await?;
                Ok(elems.into_iter().map(T::from).collect())
            }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns at least one component.
    ///
    /// If no components were found, a `NoSuchElement` error will be returned by the resolver's
    /// `resolve()` method.
    #[must_use]
    pub fn new_not_empty(base_element: WebElement, by: By) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            async move {
                let elems = elem.query(by).all_from_selector_required().await?;
                Ok(elems.into_iter().map(T::from).collect())
            }
        };
        Self::new_custom(base_element, resolver)
    }

    /// Create a new element resolver that returns at least one component, with extra options.
    ///
    /// If no components were found, a `NoSuchElement` error will be returned by the resolver's
    /// `resolve()` method.
    #[must_use]
    pub fn new_not_empty_opts(
        base_element: WebElement,
        by: By,
        options: ElementQueryOptions,
    ) -> Self {
        let resolver = move |elem: WebElement| {
            let by = by.clone();
            let options = options.clone();
            async move {
                let elems = elem.query(by).options(options).all_from_selector_required().await?;
                Ok(elems.into_iter().map(T::from).collect())
            }
        };
        Self::new_custom(base_element, resolver)
    }
}
