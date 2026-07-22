# Recipe Conventions

These recipes are small, task-shaped starting points for common browser
automation. Copy the closest recipe, replace its URL and selectors with the
contract of your application, and keep the reliability shape intact.

Every example is marked `no_run` because it needs a local browser, an
application or fixture that is not part of this book, or an optional protocol
feature. The examples are otherwise complete tests. They all:

- use [`WebDriver::managed`](../features/manager.md) for a local browser;
- locate normal page elements with [`query()`](../features/queries.md), stable
  app-owned test IDs, readable descriptions, and intentional cardinality;
- wait for observable state with a query or
  [`wait_until()`](../features/waiting.md), never a fixed sleep;
- retain the task result so [`driver.quit()`](../getting-started/ai-quickstart.md)
  is attempted after a command failure; and
- assert a user-visible or protocol-visible result.

The placeholder `https://example.test` URLs and `data-testid` values describe
the application contract the recipe expects. Add those stable hooks to an app
you control. For a third-party page, replace them with the most stable semantic
selectors the real page exposes.

## Choose A Recipe

- [Forms And Page Content](./forms-and-content.md): login, search, HTML modal,
  and table/list assertions.
- [Frames, Shadow DOM, And Files](./browser-contexts.md): iframe switching,
  shadow-root queries, and file upload.
- [Failure Artifacts And Logs](./diagnostics.md): screenshot-on-failure and
  browser/driver logs.
- [Semantic Page Outlines](./page-snapshots.md): a bounded, redaction-aware DOM
  outline for selector design and failure diagnosis.
- [CDP And BiDi](./browser-protocols.md): a typed Chromium cache command and a
  cross-browser BiDi event subscription.

For generated code, apply the copyable
[Reliable AI-Generated Tests](../getting-started/reliable-tests.md) checklist
before accepting the result.
