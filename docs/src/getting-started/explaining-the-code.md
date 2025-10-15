# Understanding Web Browser Automation

So what did this code actually do?

> **NOTE:** This section goes much deeper into how web browser automation works.
> Feel free to skip ahead and come back when you're ready to dig deeper.

## How Thirtyfour Works

Well, the first thing to know is that `thirtyfour` doesn't talk directly to the Web Browser.
It simply fires off commands to the webdriver server as HTTP Requests.
The webdriver then talks to the Web Browser and tells it to execute each command, and then
returns the response from the Web Browser to `thirtyfour`. As long as the webdriver is
running, `thirtyfour` can do just about anything a human can do in a web browser.

## Explaining The Code

So let's go through the code and see what is going on.

```rust
    let caps = DesiredCapabilities::chrome();
    let server_url = "http://localhost:9515";
    start_webdriver_process(server_url, &caps, true)?;
    let driver = WebDriver::new(server_url, caps).await?;
```

This is where we actually make the initial connection to the webdriver and start a new
session in the web browser. This will open the browser in a new profile (so it won't add
anything to your history etc.) and navigate to the default start page.

## Capabilities

The way we tell it what browser we want is by using `DesiredCapabilities`. In this case,
we construct a new `ChromeCapabilities` instance. Each `*Capabilities` struct will have
additional helper methods for setting options like headless mode, proxy, and so on.
See the [documentation](https://docs.rs/thirtyfour/latest/thirtyfour/common/capabilities/chrome/struct.ChromeCapabilities.html) for more details.

> You may have heard of `selenium`. `Selenium` is simply a proxy that forwards requests to one or
> more webdriver servers. Using `Selenium` you can interact with multiple browsers at once. You simply
> tell `Selenium` where each of the webdrivers are, and some details of each browser, and then in your
> code you give `thirtyfour` the address of the selenium server, not the webdriver, and use the
> `DesiredCapabilities` to request a particular browser.

## Element Queries

Next, we tell the browser to navigate to Wikipedia:

```rust
    driver.goto("https://wikipedia.org").await?;
```

And then we look for an element on the page:

```rust
    let elem_form = driver.find(By::Id("search-form")).await?;
```

We search for elements using what we call a `selector` or `locator`. In this case we are looking for
an element with the id of `search-form`.

> If you actually navigate to [https://wikipedia.org](https://wikipedia.org) and open your browser's
> devtools (F12 in most browsers), then go to the `Inspector` tab, you will see the raw HTML for
> the page. In the search box at the top of the inspector, if you type `#search-form`
> (the # is how we specify an id) you will see that it highlights the search form element.

This is the container element that contains both the input field and the button itself.

But we want to type into the field, so we need to do another query:

```rust
    let elem_text = elem_form.find(By::Id("searchInput")).await?;
```

And again, if you search in the inspector for `#searchInput` you get an `<input />` element
which is the one we want to type into.

## Typing Text Into An Element

To type into a field, we just tell `thirtyfour` to send the keys we want to type:

```rust
    elem_text.send_keys("selenium").await?;
```

This will literally type the text into the input field.

Now we need to find the search button and click it. Finding the element means doing another
query:

```rust
    let elem_button = elem_form.find(By::Css("button[type='submit']")).await?;
```

This time we use a `CSS` selector to locate a `<button>` element with an attribute `type` that
has the value `submit`. To learn more about selectors, [click here](https://www.selenium.dev/documentation/webdriver/elements/locators/).

## Clicking The Button

Next, the call to `click()` tells `thirtyfour` to simulate a click on that element:

```rust
    elem_button.click().await?;
```

## Dealing With Page Loading Times

The page now starts loading the result of our search on Wikipedia. This brings us to our first
issue when automating a web browser. How does our code know when the page has finished loading?
If we had a slow internet connection, we might try to find an element on the page only for it
to fail because that element hasn't loaded yet.

**How do we solve this?**

Well, one option is to simply tell our code to wait for a few seconds and then try to find the
element we are looking for. But this is brittle and likely to fail. Don't do this. There are
cases where you are forced to use this approach, but it should be a last resort. Incidentally,
from a website testing perspective, it probably also means a human is going to be unsure of when
the page has actually finished loading, and this is an indicator of a poor user experience.

Instead, we usually want to look for an element on the page and wait until that element is visible.

```rust
driver.query(By::ClassName("firstHeading")).first().await?;
assert_eq!(driver.title().await?, "Selenium - Wikipedia");
```

## Better Element Queries

To have `thirtyfour` "wait" until an element has loaded, we use the `query()` method which provides
a "builder" interface for constructing more flexible queries. Again, this uses one of the `By`
selectors, and we tell it to return only the first matching element.

The `query()` method will poll every half-second until it finds the element. If it cannot find the
element within 30 seconds, it will timeout and return an error. The polling time and timeout duration
can be changed using `WebDriverConfig`, or you can also override them for a given query.

The `query()` method is the recommended way to search for elements. The `find()` and `find_all()`
methods exist only for compatibility with the webdriver spec. The `query()` method uses these
under the hood, but adds polling and other niceties on top, including giving you more details about
your query if an element was not found.

See the [`ElementQuery`](https://docs.rs/thirtyfour/latest/thirtyfour/extensions/query/struct.ElementQuery.html)
documentation for more details on the kinds of queries it supports.

## Closing The Browser

Finally we need to close the browser window. If we don't, it will remain open after our application
has exited.

```rust
    driver.quit().await?;
```

And this concludes the introduction.

Happy Browser Automation!