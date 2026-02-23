// This wrapper is a fairly direct port of the Select class from the python
// selenium library at:
// https://github.com/SeleniumHQ/selenium/blob/trunk/py/selenium/webdriver/support/select.py

// Copyright 2021 Stephen Pryde and the thirtyfour contributors
// Derived (and modified) from the Selenium project at https://github.com/SeleniumHQ/selenium.
//
// Copyright 2011-2020 Software Freedom Conservancy
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::error::{no_such_element, WebDriverErrorInner, WebDriverResult};
use crate::{By, WebElement};
use std::fmt::{Display, Formatter};

/// Set the selection state of the specified element.
async fn set_selected(element: &WebElement, select: bool) -> WebDriverResult<()> {
    if element.is_selected().await? != select {
        element.click().await?;
    }
    Ok(())
}

struct Escaped<'a>(&'a str);

impl Display for Escaped<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, substring) in self.0.split('\"').enumerate() {
            if i != 0 {
                f.write_str(", '\"', ")?;
            }
            write!(f, "\"{substring}\"",)?;
        }
        if self.0.ends_with('\"') {
            f.write_str(", '\"'")?;
        }
        Ok(())
    }
}

/// Escape the specified string for use in Css or `XPath` selector.
#[must_use]
pub fn escape_string(value: &str) -> String {
    let contains_single = value.contains('\'');
    let contains_double = value.contains('\"');
    if contains_single && contains_double {
        format!("concat({})", Escaped(value))
    } else if contains_double {
        format!("'{value}'")
    } else {
        format!("\"{value}\"")
    }
}

/// Get the longest word in the specified string.
fn get_longest_token(value: &str) -> &str {
    value.split(' ').max_by_key(|x| x.len()).unwrap_or("")
}

/// Convenience wrapper for `<select>` elements.
#[derive(Debug)]
pub struct SelectElement {
    element: WebElement,
    multiple: bool,
}

impl SelectElement {
    /// Instantiate a new `SelectElement` struct. The specified element must be a `<select>` element.
    ///
    /// # Errors
    /// Returns an error if the element is not a select element or if communication with the driver fails.
    pub async fn new(element: &WebElement) -> WebDriverResult<SelectElement> {
        let multiple = element.attr("multiple").await?.filter(|x| x != "false").is_some();
        let element = element.clone();
        Ok(SelectElement {
            element,
            multiple,
        })
    }

    /// Return a vec of all options belonging to this select tag.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn options(&self) -> WebDriverResult<Vec<WebElement>> {
        self.element.find_all(By::Tag("option")).await
    }

    /// Return a vec of all selected options belonging to this select tag.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn all_selected_options(&self) -> WebDriverResult<Vec<WebElement>> {
        let mut selected = Vec::new();
        for option in self.options().await? {
            if option.is_selected().await? {
                selected.push(option);
            }
        }
        Ok(selected)
    }

    /// Return the first selected option in this select tag.
    ///
    /// # Errors
    /// Returns an error if no options are selected or if communication with the driver fails.
    pub async fn first_selected_option(&self) -> WebDriverResult<WebElement> {
        for option in self.options().await? {
            if option.is_selected().await? {
                return Ok(option);
            }
        }
        Err(no_such_element("No options are selected".to_string()))
    }

    /// Set selection state for all options.
    async fn set_selection_all(&self, select: bool) -> WebDriverResult<()> {
        for option in self.options().await? {
            set_selected(&option, select).await?;
        }
        Ok(())
    }

    /// Set the selection state of options matching the specified value.
    async fn set_selection_by_value(&self, value: &str, select: bool) -> WebDriverResult<()> {
        let selector = format!("option[value={}]", escape_string(value));
        let options = self.element.find_all(By::Css(&*selector)).await?;
        for option in options {
            set_selected(&option, select).await?;
            if !self.multiple {
                break;
            }
        }
        Ok(())
    }

    /// Set the selection state of the option at the specified index.
    async fn set_selection_by_index(&self, index: u32, select: bool) -> WebDriverResult<()> {
        let selector = format!("option:nth-of-type({})", index + 1);
        let option = self.element.find(By::Css(&*selector)).await?;
        set_selected(&option, select).await?;
        Ok(())
    }

    /// Set the selection state of options that display text matching the specified text.
    /// That is, when given "Bar" this would select an option like:
    ///
    /// `<option value="foo">Bar</option>`
    ///
    /// NOTE: This will attempt to first select by exact match.
    ///       However, if no exact match was found,
    ///       it will attempt to select options that contain the longest word in the
    ///       specified search text.
    ///       This particular behaviour is patterned after the python selenium library.
    async fn set_selection_by_visible_text(&self, text: &str, select: bool) -> WebDriverResult<()> {
        let mut xpath = format!(".//option[normalize-space(.) = {}]", escape_string(text));
        let options = match self.element.find_all(By::XPath(&*xpath)).await {
            Ok(elems) => elems,
            Err(e) if matches!(*e, WebDriverErrorInner::NoSuchElement(_)) => Vec::new(),
            Err(e) => return Err(e),
        };

        let mut matched = false;
        for option in &options {
            set_selected(option, select).await?;
            if !self.multiple {
                return Ok(());
            }
            matched = true;
        }

        if options.is_empty() && text.contains(' ') {
            let substring_without_space = get_longest_token(text);
            let candidates = if substring_without_space.is_empty() {
                self.options().await?
            } else {
                xpath =
                    format!(".//option[contains(.,{})]", escape_string(substring_without_space));
                self.element.find_all(By::XPath(&*xpath)).await?
            };
            for candidate in candidates {
                if text == candidate.text().await? {
                    set_selected(&candidate, select).await?;
                    if !self.multiple {
                        return Ok(());
                    }
                    matched = true;
                }
            }
        }

        if matched {
            Ok(())
        } else {
            Err(no_such_element(format!("Could not locate element with visible text: {text}")))
        }
    }

    /// Set the selection state of options that match the specified `XPath` condition.
    async fn set_selection_by_xpath_condition(
        &self,
        condition: &str,
        select: bool,
    ) -> WebDriverResult<()> {
        let xpath = format!(".//option[{condition}]");
        let options = self.element.find_all(By::XPath(&*xpath)).await?;
        if options.is_empty() {
            return Err(no_such_element(format!(
                "Could not locate element matching XPath condition: {xpath:?}"
            )));
        }

        for option in &options {
            set_selected(option, select).await?;
            if !self.multiple {
                break;
            }
        }

        Ok(())
    }

    /// Set the selection state of options that display text exactly matching the specified text.
    async fn set_selection_by_exact_text(&self, text: &str, select: bool) -> WebDriverResult<()> {
        let condition = format!("text() = {}", escape_string(text));
        self.set_selection_by_xpath_condition(&condition, select).await
    }

    /// Set the selection state of options that display text containing the specified substring.
    async fn set_selection_by_partial_text(&self, text: &str, select: bool) -> WebDriverResult<()> {
        let condition = format!("contains(text(), {})", escape_string(text));
        self.set_selection_by_xpath_condition(&condition, select).await
    }

    /// Select all options for this select tag.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    ///
    /// # Panics
    /// Panics if this is not a multi-select element.
    pub async fn select_all(&self) -> WebDriverResult<()> {
        assert!(self.multiple, "You may only select all options of a multi-select");
        self.set_selection_all(true).await
    }

    /// Select options matching the specified value.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the value.
    pub async fn select_by_value(&self, value: &str) -> WebDriverResult<()> {
        self.set_selection_by_value(value, true).await
    }

    /// Select the option matching the specified index. This is done by examining
    /// the "index" attribute of an element and not merely by counting.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the index.
    pub async fn select_by_index(&self, index: u32) -> WebDriverResult<()> {
        self.set_selection_by_index(index, true).await
    }

    /// Select options with visible text matching the specified text.
    /// That is, when given "Bar" this would select an option like:
    ///
    /// `<option value="foo">Bar</option>`
    ///
    /// This will attempt to select by exact match, but if no option is found it will also
    /// attempt to select based on the longest contiguous word in the text.
    /// See also `select_by_exact_text()` and `select_by_partial_text()`.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the text.
    pub async fn select_by_visible_text(&self, text: &str) -> WebDriverResult<()> {
        self.set_selection_by_visible_text(text, true).await
    }

    /// Select options matching the specified `XPath` condition.
    /// E.g. The specified condition replaces `{}` in this `XPath`: `.//option[{}]`
    ///
    /// The following example would match `.//option[starts-with(text(), 'pre')]`:
    /// ```ignore
    /// select_by_xpath_condition("starts-with(text(), 'pre')").await?;
    /// ```
    /// For multi-select, all options matching the condition will be selected.
    /// For single-select, only the first matching option will be selected.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the condition.
    pub async fn select_by_xpath_condition(&self, condition: &str) -> WebDriverResult<()> {
        self.set_selection_by_xpath_condition(condition, true).await
    }

    /// Select options with visible text exactly matching the specified text.
    /// For multi-select, all options whose text exactly matches will be selected.
    /// For single-select, only the first exact match will be selected.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the text.
    pub async fn select_by_exact_text(&self, text: &str) -> WebDriverResult<()> {
        self.set_selection_by_exact_text(text, true).await
    }

    /// Select options with visible text partially matching the specified text.
    /// For multi-select, all options whose text contains the specified substring will be selected.
    /// For single-select, only the first option containing the substring will be selected.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the text.
    pub async fn select_by_partial_text(&self, text: &str) -> WebDriverResult<()> {
        self.set_selection_by_partial_text(text, true).await
    }

    /// Deselect all options for this select tag.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    ///
    /// # Panics
    /// Panics if this is not a multi-select element.
    pub async fn deselect_all(&self) -> WebDriverResult<()> {
        assert!(self.multiple, "You may only deselect all options of a multi-select");
        self.set_selection_all(false).await
    }

    /// Deselect options matching the specified value.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the value.
    ///
    /// # Panics
    /// Panics if this is not a multi-select element.
    pub async fn deselect_by_value(&self, value: &str) -> WebDriverResult<()> {
        assert!(self.multiple, "You may only deselect options of a multi-select");
        self.set_selection_by_value(value, false).await
    }

    /// Deselect the option matching the specified index. This is done by examining
    /// the "index" attribute of an element and not merely by counting.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the index.
    ///
    /// # Panics
    /// Panics if this is not a multi-select element.
    pub async fn deselect_by_index(&self, index: u32) -> WebDriverResult<()> {
        assert!(self.multiple, "You may only deselect options of a multi-select");
        self.set_selection_by_index(index, false).await
    }

    /// Deselect options with visible text matching the specified text.
    /// That is, when given "Bar" this would deselect an option like:
    ///
    /// `<option value="foo">Bar</option>`
    ///
    /// See also `deselect_by_exact_text()` and `deselect_by_partial_text()`.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the text.
    ///
    /// # Panics
    /// Panics if this is not a multi-select element.
    pub async fn deselect_by_visible_text(&self, text: &str) -> WebDriverResult<()> {
        assert!(self.multiple, "You may only deselect options of a multi-select");
        self.set_selection_by_visible_text(text, false).await
    }

    /// Deselect options matching the specified `XPath` condition.
    /// E.g. The specified condition replaces `{}` in this `XPath`: `.//option[{}]`
    ///
    /// The following example would match `.//option[starts-with(text(), 'pre')]`:
    /// ```ignore
    /// deselect_by_xpath_condition("starts-with(text(), 'pre')").await?;
    /// ```
    /// For multi-select, all options matching the condition will be deselected.
    /// For single-select, only the first matching option will be deselected.
    ///
    /// # Errors
    /// Returns an error if this is not a multi-select or if communication with the driver fails.
    pub async fn deselect_by_xpath_condition(&self, condition: &str) -> WebDriverResult<()> {
        self.set_selection_by_xpath_condition(condition, false).await
    }

    /// Deselect all options with visible text exactly matching the specified text.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the text.
    ///
    /// # Panics
    /// Panics if this is not a multi-select element.
    pub async fn deselect_by_exact_text(&self, text: &str) -> WebDriverResult<()> {
        assert!(self.multiple, "You may only deselect options of a multi-select");
        self.set_selection_by_exact_text(text, false).await
    }

    /// Deselect all options with visible text partially matching the specified text.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or no option matches the text.
    ///
    /// # Panics
    /// Panics if this is not a multi-select element.
    pub async fn deselect_by_partial_text(&self, text: &str) -> WebDriverResult<()> {
        assert!(self.multiple, "You may only deselect options of a multi-select");
        self.set_selection_by_partial_text(text, false).await
    }
}
