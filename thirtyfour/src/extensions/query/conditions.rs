use crate::error::WebDriverResult;
use crate::IntoArcStr;
use crate::{ElementPredicate, WebElement};
use std::sync::Arc;
use stringmatch::Needle;

pub(crate) fn handle_errors(
    result: WebDriverResult<bool>,
    ignore_errors: bool,
) -> WebDriverResult<bool> {
    match result {
        Ok(x) => Ok(x),
        Err(..) if ignore_errors => Ok(false),
        Err(e) => Err(e),
    }
}

pub(crate) fn negate(result: WebDriverResult<bool>, ignore_errors: bool) -> WebDriverResult<bool> {
    handle_errors(result.map(|x| !x), ignore_errors)
}

/// Predicate that returns true for elements that are enabled.
pub fn element_is_enabled(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { handle_errors(elem.is_enabled().await, ignore_errors) }
}

/// Predicate that returns true for elements that are not enabled.
pub fn element_is_not_enabled(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { negate(elem.is_enabled().await, ignore_errors) }
}

/// Predicate that returns true for elements that are selected.
pub fn element_is_selected(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { handle_errors(elem.is_selected().await, ignore_errors) }
}

/// Predicate that returns true for elements that are not selected.
pub fn element_is_not_selected(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { negate(elem.is_selected().await, ignore_errors) }
}

/// Predicate that returns true for elements that are displayed.
pub fn element_is_displayed(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { handle_errors(elem.is_displayed().await, ignore_errors) }
}

/// Predicate that returns true for elements that are not displayed.
pub fn element_is_not_displayed(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { negate(elem.is_displayed().await, ignore_errors) }
}

/// Predicate that returns true for elements that are clickable.
pub fn element_is_clickable(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { handle_errors(elem.is_clickable().await, ignore_errors) }
}

/// Predicate that returns true for elements that are not clickable.
pub fn element_is_not_clickable(ignore_errors: bool) -> impl ElementPredicate {
    move |elem: WebElement| async move { negate(elem.is_clickable().await, ignore_errors) }
}

/// Predicate that returns true for elements that have the specified class name.
/// See the `Needle` documentation for more details on text matching rules.
/// In particular, it is recommended to use StringMatch or Regex to perform a whole-word search.
pub fn element_has_class<N>(class_name: N, ignore_errors: bool) -> impl ElementPredicate
where
    N: Needle + Clone + Send + Sync + 'static,
{
    move |elem: WebElement| {
        let class_name = class_name.clone();
        async move {
            match elem.class_name().await {
                Ok(Some(x)) => Ok(class_name.is_match(&x)),
                Ok(None) => Ok(false),
                Err(e) => handle_errors(Err(e), ignore_errors),
            }
        }
    }
}

/// Predicate that returns true for elements that do not contain the specified class name.
/// See the `Needle` documentation for more details on text matching rules.
/// In particular, it is recommended to use StringMatch or Regex to perform a whole-word search.
pub fn element_lacks_class<N>(class_name: N, ignore_errors: bool) -> impl ElementPredicate
where
    N: Needle + Clone + Send + Sync + 'static,
{
    move |elem: WebElement| {
        let class_name = class_name.clone();
        async move {
            match elem.class_name().await {
                Ok(Some(x)) => Ok(!class_name.is_match(&x)),
                Ok(None) => Ok(true),
                Err(e) => handle_errors(Err(e), ignore_errors),
            }
        }
    }
}

/// Predicate that returns true for elements that have the specified text.
/// See the `Needle` documentation for more details on text matching rules.
pub fn element_has_text<N>(text: N, ignore_errors: bool) -> impl ElementPredicate
where
    N: Needle + Clone + Send + Sync + 'static,
{
    move |elem: WebElement| {
        let text = text.clone();
        async move { handle_errors(elem.text().await.map(|x| text.is_match(&x)), ignore_errors) }
    }
}

/// Predicate that returns true for elements that do not contain the specified text.
/// See the `Needle` documentation for more details on text matching rules.
pub fn element_lacks_text<N>(text: N, ignore_errors: bool) -> impl ElementPredicate
where
    N: Needle + Clone + Send + Sync + 'static,
{
    move |elem: WebElement| {
        let text = text.clone();
        async move { handle_errors(elem.text().await.map(|x| !text.is_match(&x)), ignore_errors) }
    }
}

/// Predicate that returns true for elements that have the specified value.
/// See the `Needle` documentation for more details on text matching rules.
pub fn element_has_value<N>(value: N, ignore_errors: bool) -> impl ElementPredicate
where
    N: Needle + Clone + Send + Sync + 'static,
{
    move |elem: WebElement| {
        let value = value.clone();
        async move {
            match elem.value().await {
                Ok(Some(x)) => Ok(value.is_match(&x)),
                Ok(None) => Ok(false),
                Err(e) => handle_errors(Err(e), ignore_errors),
            }
        }
    }
}

/// Predicate that returns true for elements that do not contain the specified value.
/// See the `Needle` documentation for more details on text matching rules.
pub fn element_lacks_value<N>(value: N, ignore_errors: bool) -> impl ElementPredicate
where
    N: Needle + Clone + Send + Sync + 'static,
{
    move |elem: WebElement| {
        let value = value.clone();
        async move {
            match elem.value().await {
                Ok(Some(x)) => Ok(!value.is_match(&x)),
                Ok(None) => Ok(true),
                Err(e) => handle_errors(Err(e), ignore_errors),
            }
        }
    }
}

pub(crate) fn collect_arg_slice<S, N, I>(desired_attributes: I) -> Arc<[(Arc<str>, N)]>
where
    I: IntoIterator<Item = (S, N)>,
    S: IntoArcStr,
    N: Needle + Send + Sync + 'static,
{
    desired_attributes.into_iter().map(|(a, b)| (a.into(), b)).collect()
}

macro_rules! elem_matches {
    (@inner_many_opt $elm: expr, $desired: expr, $ignore_errors: expr, $field: ident, |$val: ident, $name: ident| $check: expr) => {async move {
        for (name, $val) in &*$desired {
            match $elm.$field(name).await {
                Ok(Some($name)) => {
                    if $check {
                        return Ok(false);
                    }
                }
                Ok(None) => return Ok(false),
                Err(e) => return handle_errors(Err(e), $ignore_errors),
            }
        }
        Ok(true)
    }};
    (@inner_many_css $elm: expr, $desired: expr, $ignore_errors: expr, $field: ident, |$val: ident, $name: ident| $check: expr) => {async move {
        for (name, $val) in &*$desired {
            match $elm.$field(name).await {
                Ok($name) => {
                    if $check {
                        return Ok(false);
                    }
                }
                Err(e) => return handle_errors(Err(e), $ignore_errors),
            }
        }
        Ok(true)
    }};
    (@inner_single_opt $elm: expr, $name: expr, $ignore_errors: expr , $field: ident, |$ret: ident| $check: expr) => {
        async move {
            match $elm.$field($name).await {
                Ok(Some($ret)) => Ok($check),
                Ok(None) => Ok(false),
                Err(e) => handle_errors(Err(e), $ignore_errors),
            }
        }
    };
    (@inner_single_css $elm: expr, $name: expr, $ignore_errors: expr , $field: ident, |$ret: ident| $check: expr) => {
        async move {
            handle_errors(
                $elm.$field($name).await.map(|$ret| $check),
                $ignore_errors,
            )
        }
    };
    (many [$name: ident] $field: ident => $plural: ident) => {
        pastey::paste! {
            #[doc = concat!("Predicate that returns true for elements that have all the ",stringify!($plural)," specified with the")]
            ///specified values. See the `Needle` documentation for more details on text matching rules.
            pub fn [<element_has_ $plural>]<N>(
                [<desired_ $plural>]: Arc<[(Arc<str>, N)]>,
                ignore_errors: bool,
            ) -> impl ElementPredicate
            where
                N: Needle + Send + Sync + 'static,
            {
                move |elem: WebElement| {
                    let desired = [<desired_ $plural>].clone();
                    elem_matches!(@[<inner_many_ $name>] elem, desired, ignore_errors, $field, |val, x| !val.is_match(&x))
                }
            }

            #[doc = concat!("Predicate that returns true for elements that do not have any of the specified ",stringify!($plural)," specified with the")]
            ///specified values. See the `Needle` documentation for more details on text matching rules.
            pub fn [<element_lacks_ $plural>]<N>(
                [<desired_ $plural>]: Arc<[(Arc<str>, N)]>,
                ignore_errors: bool,
            ) -> impl ElementPredicate
            where
                N: Needle + Send + Sync + 'static,
            {
                move |elem: WebElement| {
                    let desired = [<desired_ $plural>].clone();
                    elem_matches!(@[<inner_many_ $name>] elem, desired, ignore_errors, $field, |value, x| value.is_match(&x))
                }
            }
        }
    };
    (single [$name: ident] $field: ident => $single: ident) => {
        pastey::paste! {
            #[doc = concat!("Predicate that returns true for elements that have the specified ",stringify!($plural)," with the specified")]
            ///value. See the `Needle` documentation for more details on text matching rules.
            pub fn [<element_has_ $single>]<N>(
                [<$single _name>]: Arc<str>,
                value: N,
                ignore_errors: bool,
            ) -> impl ElementPredicate
            where
                N: Needle + Clone + Send + Sync + 'static,
            {
                move |elem: WebElement| {
                    let name = [<$single _name>].clone();
                    let value = value.clone();
                    elem_matches!(@[<inner_single_ $name>] elem, name, ignore_errors, $field, |x| value.is_match(&x))
                }
            }

            #[doc = concat!("Predicate that returns true for elements that do not contain the specified ",stringify!($plural)," with the specified")]
            /// value. See the `Needle` documentation for more details on text matching rules.
            pub fn [<element_lacks_ $single>]<N>(
                [<$single _name>]: Arc<str>,
                value: N,
                ignore_errors: bool,
            ) -> impl ElementPredicate
            where
                N: Needle + Clone + Send + Sync + 'static,
            {
                move |elem: WebElement| {
                    let name = [<$single _name>].clone();
                    let value = value.clone();
                    elem_matches!(@[<inner_single_ $name>] elem, name, ignore_errors, $field, |x| !value.is_match(&x))
                }
            }
        }
    };

    ([$name: ident] $field: ident => $single: ident, $plural: ident) => {
        elem_matches!(many   [$name] $field => $plural);
        elem_matches!(single [$name] $field => $single);
    }
}

elem_matches!([opt] attr => attribute, attributes);
elem_matches!([opt] prop => property,  properties);
elem_matches!([css] css_value => css_property,  css_properties);
