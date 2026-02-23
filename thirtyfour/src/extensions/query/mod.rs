//! Advanced query interface featuring powerful filtering and polling options.
//!
//! See examples for more details.
//!
//! ## Usage
//!
//! ### `ElementQuery`
//!
//! The `WebDriver::query()` and `WebElement::query()` methods work out-of-the-box with no
//! additional setup required.
//! However, you can customize some behaviour if needed.
//!
//! Now, using the query interface, you can do things like this:
//!
//! ```no_run
//! # use thirtyfour::prelude::*;
//! # use thirtyfour::support::block_on;
//! #
//! # fn main() -> WebDriverResult<()> {
//! #     block_on(async {
//! #         let caps = DesiredCapabilities::chrome();
//! #         let mut driver = WebDriver::new("http://localhost:4444", caps).await?;
//! // This will only poll once and then return a bool immediately.
//! let is_found = driver
//!     .query(By::Id("button1"))
//!     .nowait()
//!     .exists()
//!     .await?;
//!
//! // This will poll until either branch matches at least one element.
//! // Only the first matched element will be returned.
//! let elem = driver
//!     .query(By::Css("thiswont.match"))
//!     .or(By::Id("button1"))
//!     .first()
//!     .await?;
//! #         driver.quit().await?;
//! #         Ok(())
//! #     })
//! # }
//! ```
//!
//! This will execute both queries once per poll iteration and return the first one that matches.
//!
//! You can also filter on one or both query branches like this:
//!
//! ```no_run
//! # use thirtyfour::prelude::*;
//! # use thirtyfour::support::block_on;
//! use thirtyfour::stringmatch::StringMatch;
//! # use std::time::Duration;
//! #
//! # fn main() -> WebDriverResult<()> {
//! #     block_on(async {
//! #         let caps = DesiredCapabilities::chrome();
//! #         let mut driver = WebDriver::new("http://localhost:4444", caps).await?;
//! let elem = driver
//!     .query(By::Css("thiswont.match"))
//!     .with_text("testing")
//!     .or(By::Id("button1"))
//!     .with_class(StringMatch::new("pure-button").word())
//!     .and_enabled()
//!     .first()
//!     .await?;
//! #         driver.quit().await?;
//! #         Ok(())
//! #     })
//! # }
//! ```
//!
//! Note the use of `StringMatch` to provide a partial (whole-word) match on the class name.
//! See the documentation for [StringMatch](https://crates.io/crates/stringmatch) for more info.
//!
//! **NOTE:** Each filter will trigger an additional request to the `WebDriver` server for every poll
//! iteration.
//! It is therefore strongly recommended to use `By::*` selectors to perform filtering,
//! if possible.
//! The `By::Css` and `By::XPath` selectors may be required for more complex
//! filters.
//!
//! To fetch all matching elements instead of just the first one, change `first()` to `all()`
//! and you'll get a Vec instead.
//! Note that `all()` will return only the elements from the query
//! branch that first matched something.
//! In the above example, if the
//! `(By::Css("branch.one")).with_text("testing")` branch returned at least one element, then only
//! those elements will be returned from an `all()` call even if the other branch has
//! matched something.
//! If you want to fetch all elements matched by all branches,
//! it's probably best to execute multiple queries.
//!
//! All timeout, interval and `ElementPoller` details can be overridden on a per-call basis if
//! desired.
//! See the [`ElementQuery`] documentation for more details.
//!
//! [`ElementQuery`]: ElementQuery
//!
//! ### `ElementWaiter`
//!
//! With `ElementWaiter` you can do things like this:
//! ```ignore
//! elem.wait_until().displayed().await?;
//! // You can optionally provide a nicer error message like this.
//! elem.wait_until().error("Timed out waiting for element to disappear").not_displayed().await?;
//!
//! elem.wait_until().enabled().await?;
//! elem.wait_until().clickable().await?;
//! ```
//!
//! And so on.
//! See the [`ElementWaiter`] docs for the full
//! list of predicates available.
//!
//! [`ElementWaiter`]: ElementWaiter
//!
//! `ElementWaiter` also allows the user of custom predicates that take a `&WebElement` argument
//! and return a `WebDriverResult<bool>`.
//!
//! A range of pre-defined predicates are also supplied for convenience in the
//! `thirtyfour::extensions::query::conditions` module.
//!
//! ```ignore
//! use thirtyfour_query::conditions;
//!
//! elem.wait_until().conditions(vec![
//!     conditions::element_is_displayed(true),
//!     conditions::element_is_clickable(true)
//! ]).await?;
//! ```
//! Take a look at the `conditions` module for the full list of predicates available.
//! NOTE: Predicates require you to specify whether errors should be ignored.
//!
//! These predicates (or your own) can also be supplied as filters to `ElementQuery`.
//!
//! ### `ElementPoller`
//!
//! The polling strategy can be customized by implementing both [`ElementPoller`]
//! and [`IntoElementPoller`].
//!
//! See [`ElementPollerWithTimeout`] for more details about the default polling behaviour.
//!
//! [`ElementPoller`]: ElementPoller
//! [`IntoElementPoller`]: IntoElementPoller
//! [`ElementPollerWithTimeout`]: ElementPollerWithTimeout

/// Predicates to use for element conditions.
pub mod conditions;
mod element_query;
mod element_waiter;
mod poller;
pub use element_query::*;
pub use element_waiter::*;
pub use poller::*;
