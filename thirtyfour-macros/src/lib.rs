//! Thirtyfour is a Selenium / `WebDriver` library for Rust, for automated website UI testing.
//!
//! This crate provides proc macros for use with [thirtyfour](https://docs.rs/thirtyfour).
//!
//!

use crate::component::expand_component_derive;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod component;

macro_rules! bail {
    ($span: expr, $($fmt:tt)*) => {
        return Err(syn::Error::new($span, format_args!($($fmt)*)))
    };
}

pub(crate) use bail;

/// Derive macro for a wrapped `Component`.
///
/// A `Component` contains a base [`WebElement`] from which all element queries will be performed.
///
/// All elements in the component are descendents of the base element (or at least the
/// starting point for an element query, since `XPath` queries can access parent nodes).
///
/// Components perform element lookups via [`ElementResolver`]s, which lazily perform an
/// element query to resolve a [`WebElement`], and then cache the result for later access.
///
/// See the [`ElementResolver`] documentation for more details.
///
/// ## Attributes
///
/// ### `#[base]`
/// By default, the base element should be named `base` and be of type `WebElement`.
/// You can optionally use the `#[base]` attribute if you wish to name the base element
/// something other than `base`.
/// If you use this attribute, you cannot also have another
/// element named `base`.
///
/// ### `#[by(..)]`
/// Components use the `#[by(..)]` attribute to specify all the details of the query.
///
/// The only required attribute is the selector attribute, which can be one of the following:
/// - `id = "..."`: Select element by id.
/// - `tag = "..."`: Select element by tag name.
/// - `link = "..."`: Select element by link text.
/// - `css = "..."`: Select element by CSS.
/// - `xpath = "..."`: Select element by `XPath`.
/// - `name = "..."`: Select element by name.
/// - `class = "..."`: Select element by class name.
///
/// Optional attributes available within `#[by(..)]` include:
/// - `single`: (default, single element only) Return `NoSuchElement` if the number of elements
///   found is != 1.
/// - `first`: (single element only) Select the first element that matches the query.
///   By default, a query will return `NoSuchElement` if multiple elements match.
///   This default is designed to catch instances where a query is not specific enough.
/// - `not_empty`: (default, multi elements only) Return `NoSuchElement` if no elements were found.
/// - `allow_empty`: (multi elements only) Return an empty Vec if no elements were found.
///   By default a multi-element query will return `NoSuchElement` if no
///   elements were found.
/// - `description = "..."`: Set the element description to be displayed in `NoSuchElement` errors.
/// - `allow_errors`: Ignore errors such as stale elements while polling.
/// - `wait(timeout_ms = 10000, interval_ms=500)`: Override the default polling options.
/// - `nowait`: Turn off polling for this element query.
/// - `custom = "my_resolve_fn"`: Use the specified function to resolve the element or component.
///   **NOTE**: The `custom` attribute cannot be specified with any other
///   attribute.
///
/// See [`ElementQueryOptions`] for more details on how each option is used.
///
/// ### Custom resolver functions
///
/// When using `custom = "my_resolve_fn"`, your function signature should look something like this:
///
/// ```ignore
/// async fn my_resolve_fn(elem: &WebElement) -> WebDriverResult<T>
/// ```
///
/// where the `T` is the same as type `T` in `ElementResolver<T>`.
/// Also see the example below.
///
/// ## Example:
/// ```ignore
/// /// This component shows how to nest components inside others.
/// #[derive(Debug, Clone, Component)]
/// pub struct CheckboxSectionComponent {
///     base: WebElement,
///     #[by(tag = "label", allow_empty)]
///     boxes: ElementResolver<Vec<CheckboxComponent>>,
///     // Other fields will be initialised using `Default::default()`.
///     my_field: bool,
/// }
///
/// /// This component shows how to wrap a simple web component.
/// #[derive(Debug, Clone, Component)]
/// pub struct CheckboxComponent {
///     base: WebElement,
///     #[by(css = "input[type='checkbox']", first)]
///     input: ElementResolver<WebElement>,
///     #[by(name = "text-label", description = "text label")]
///     label: ElementResolver<WebElement>
///     #[by(custom = "my_custom_resolve_fn")]
///     button: ElementResolver<WebElement>
/// }
///
/// /// Use this function signature for your custom resolvers.
/// async fn my_custom_resolve_fn(elem: &WebElement) -> WebDriverResult<WebElement> {
///     // Do something with elem.
///     elem.query(By::ClassName("my-class")).and_displayed().first().await
/// }
///
/// impl CheckboxComponent {
///     /// Return true if the checkbox is ticked.
///     pub async fn is_ticked(&self) -> WebDriverResult<bool> {
///         // Equivalent to: let elem = self.input.resolve_present().await?;
///         let elem = resolve_present!(self.input);
///         let prop = elem.prop("checked").await?;
///         Ok(prop.unwrap_or_default() == "true")
///     }
/// }
/// ```
/// [`WebElement`]: https://docs.rs/thirtyfour/latest/thirtyfour/struct.WebElement.html
/// [`ElementResolver`]: https://docs.rs/thirtyfour/0.31.0-alpha.1/thirtyfour/components/struct.ElementResolver.html
/// [`ElementQueryOptions`]: https://docs.rs/thirtyfour/0.31.0-alpha.1/thirtyfour/extensions/query/struct.ElementQueryOptions.html
/// [`ElementQueryFn<T>`]: https://docs.rs/thirtyfour/0.31.0-alpha.1/thirtyfour/common/types/type.ElementQueryFn.html
#[proc_macro_derive(Component, attributes(base, by))]
pub fn derive_component_fn(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    expand_component_derive(ast).into()
}
