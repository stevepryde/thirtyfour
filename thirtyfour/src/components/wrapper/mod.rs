mod resolver;

use crate::WebElement;

pub use resolver::*;

#[cfg(feature = "component")]
pub use thirtyfour_macros::Component;

/// The `Component` automatically implements the `Component` trait derive macro.
///
/// Anything that implements `Component + Clone + From<WebElement>` can be used with
/// `ElementResolver` to take the resolved element as input, and return the specific type.
///
/// There is also an implementation of `ElementResolver` for a Vec containing such types.
pub trait Component: Sized + From<WebElement> {
    /// Get the base element for this component.
    fn base_element(&self) -> WebElement;
}
