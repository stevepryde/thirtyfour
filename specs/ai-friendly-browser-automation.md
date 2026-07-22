# AI-friendly browser automation

## Purpose

This specification owns the reliability contract for entry-level `thirtyfour`
examples, selector guidance, the AI/LLM quickstart, task-oriented recipes, and
translation guidance, and coding-agent guidance that humans and agents are
likely to copy. It also defines the browser-test runner, portable failure
artifacts, the interaction-helper design boundary, and semantic page-outline
guidance.

## Requirements

- **AI-AUTO-001 (confirmed):** Entry-level examples that manage a local browser
  must use `WebDriver::managed`; an example specifically teaching an external
  Selenium endpoint may use `WebDriver::new`. Every flow must explicitly await
  `driver.quit()` and propagate its error when no earlier flow error takes
  precedence.
- **AI-AUTO-002 (confirmed):** Normal element lookup in entry-level examples
  must use `WebDriver::query` or `WebElement::query`, so lookup polls for the
  required page state instead of making a one-shot request.
- **AI-AUTO-003 (confirmed):** Queries for important user-facing elements must
  include a human-readable description for timeout diagnostics.
- **AI-AUTO-004 (confirmed):** A query must use `single()` when exactly one
  match is part of the example's page contract, and `first()` only when
  first-match behavior is intentional.
- **AI-AUTO-005 (confirmed):** The entry-level documentation must describe
  `find()` and `find_all()` as lower-level, one-shot WebDriver operations rather
  than the normal automation path.
- **AI-AUTO-006 (observed):** The repository-root README is a symlink to the
  crate README, which directs readers to the `tokio_async` runnable example.
- **AI-AUTO-007 (confirmed):** The README, crate-level rustdoc, mdBook
  walkthrough, and matching runnable examples must stay behaviorally aligned.
- **AI-SEL-001 (confirmed):** Documentation must prefer stable, app-owned
  `data-testid` hooks through `By::Testid` when the application under test can
  provide them.
- **AI-SEL-002 (confirmed):** Selector guidance must recommend stable semantic
  CSS selectors when no test ID exists, and XPath only when CSS cannot
  reasonably express the target.
- **AI-SEL-003 (confirmed):** Visible-text matching is appropriate when the
  displayed copy is part of the behavior under test. It must not be presented
  as the default way to identify controls because copy, localization, and
  duplicate labels can make it brittle.
- **AI-SEL-004 (confirmed):** Examples against third-party sites must use
  selectors the real page exposes rather than pretending app-owned test hooks
  exist. The surrounding guidance must distinguish this constraint from the
  preferred practice for applications the reader controls.
- **AI-STYLE-001 (confirmed):** The mdBook must provide one concise checklist
  of reliability rules suitable for pasting into coding-agent instructions.
- **AI-STYLE-002 (confirmed):** The checklist must cover session setup and
  cleanup, query semantics and diagnostics, stable selectors, readiness waits,
  query scoping and components, session concurrency, and protocol-specific
  isolation and feature gates.
- **AI-STYLE-003 (confirmed):** Every major checklist rule must link to the
  deeper documentation that defines its behavior and must not claim that a
  planned API, such as the managed test harness, already exists.
- **AI-STYLE-004 (confirmed):** The checklist must be linked directly from the
  getting-started learning path.
- **AI-QS-001 (confirmed):** The mdBook must provide one compact AI/LLM
  quickstart with minimal dependencies and a starter test using
  `WebDriver::managed`, `query()`, readable descriptions, explicit readiness
  conditions, a user-visible outcome, and explicit session cleanup.
- **AI-QS-002 (confirmed):** The starter test must prefer app-owned test IDs,
  scoped queries, and a cleanup shape that still attempts `quit()` after the
  test body fails. A sample URL that is not runnable must be marked `no_run`.
- **AI-QS-003 (confirmed):** The quickstart must give direct selector,
  readiness, scoping, outcome, cleanup, and concurrency rules, while linking
  to the canonical reliability checklist rather than duplicating it.
- **AI-QS-004 (confirmed):** The quickstart must link to the query, waiting,
  component, manager, CDP, and BiDi guides and be part of the mdBook
  getting-started navigation.
- **AI-REC-001 (confirmed):** The mdBook must provide a dedicated recipes
  section with copyable, task-shaped examples for login, search, HTML modal or
  dialog interaction, iframes, shadow DOM, file uploads, screenshots on
  failure, table or list assertions, browser and driver logs, a simple CDP
  network or cache command, and a basic BiDi event subscription.
- **AI-REC-002 (confirmed):** Every recipe must use `WebDriver::managed` unless
  an external WebDriver service is essential, use polling queries and element
  waits for readiness, describe important queries, prefer stable selectors,
  and explicitly attempt session cleanup.
- **AI-REC-003 (confirmed):** Each recipe must be short enough to copy and
  adapt, use cardinality that matches the intended page contract, and assert a
  user-visible or protocol-visible outcome instead of treating a successful
  click as sufficient evidence.
- **AI-REC-004 (confirmed):** Recipes that depend on an application, local
  browser, filesystem fixture, or optional protocol feature must be marked
  `no_run` and state the required setup or feature gate. Recipes must not use a
  fixed sleep for page readiness.
- **AI-REC-005 (confirmed):** Protocol-specific recipes must keep portable
  WebDriver flow separate from Chromium-only CDP and opt-in BiDi behavior, and
  link to the deeper owning guides rather than duplicating their full API
  references.
- **AI-TRANS-001 (confirmed):** The mdBook must map Selenium element lookup,
  explicit waits, page objects, Grid setup, and local browser setup to the
  corresponding `thirtyfour` query, waiter, Component, remote-session, and
  managed-session APIs.
- **AI-TRANS-002 (confirmed):** The mdBook must map Playwright locators,
  auto-waiting, and browser-context habits to scoped queries, explicit
  readiness conditions, and isolated WebDriver sessions without implying that
  a cloned `WebDriver` creates a separate browser context.
- **AI-TRANS-003 (confirmed):** The translation guide must explicitly name
  common Selenium and Playwright APIs that `thirtyfour` does not provide and
  show side-by-side source and Rust examples where the mapping benefits from
  code.
- **AI-TRANS-004 (confirmed):** The guide must link to the canonical query,
  waiting, Component, manager, manual WebDriver, and reliable-test guidance and
  be discoverable from the mdBook navigation and AI quickstart.
- **AI-COMP-001 (confirmed):** When the `component` feature is enabled, the
  prelude must export the `Component` derive/trait name and `ElementResolver`
  so the recommended `use thirtyfour::prelude::*;` import supports the common
  Component path.
- **AI-COMP-002 (confirmed):** The `resolve!` and `resolve_present!` helper
  macros must remain explicit crate-root imports. They are optional shorthand,
  and adding generic macro names to the wildcard prelude would introduce
  unnecessary collision risk.
- **AI-COMP-003 (confirmed):** Component rustdoc, mdBook guidance, examples,
  and tests must use the preferred import style and describe its `component`
  feature gate consistently.
- **AI-RUN-001 (confirmed):** The crate must provide one browser-test runner
  that accepts managed and externally created session futures, retains control
  of cleanup, and asynchronously attempts `WebDriver::quit` after the body
  succeeds, returns an error, or unwinds with a panic.
- **AI-RUN-002 (confirmed):** Session setup, body, and cleanup failures must be
  distinguishable. When the body and cleanup both fail, the returned error must
  retain both failures rather than allowing cleanup to replace the test error.
- **AI-RUN-003 (confirmed):** After catching an unwind panic, the runner must
  attempt cleanup and resume the original panic. Documentation must state that
  `panic = "abort"` cannot provide this cleanup guarantee and that a cleanup
  failure or panic during unwind is reported through tracing. It must also
  state that cancelling the runner future can interrupt asynchronous cleanup.
- **AI-RUN-004 (confirmed):** The AI quickstart and reliable-test checklist
  must use the runner as the preferred test shape, while application examples
  may continue to call `quit()` directly.
- **AI-ART-001 (confirmed):** The crate must provide one canonical,
  best-effort failure-artifact collector for current URL, page title,
  screenshot bytes, page source, browser logs, and managed-driver-process logs.
  Failure to capture one field must not prevent attempts for the remaining
  fields.
- **AI-ART-002 (confirmed):** Source and log text must have configurable,
  UTF-8-safe hard bounds with conservative defaults. Log retention must prefer
  the newest entries, and the text report must never print screenshot bytes or
  base64 data.
- **AI-ART-003 (confirmed):** The collector must attach before the failing body
  to observe live managed-driver-process logs and capture before session
  cleanup. Process logs must be documented as potentially containing lines
  from another concurrent session when the manager reuses a process. Browser
  logging capability requirements, unsupported drivers, the `manager` feature
  boundary, and the portable baseline's exclusion of CDP/BiDi-specific
  diagnostics must be explicit.
- **AI-ART-004 (confirmed):** Documentation must warn that bounded artifacts
  can still contain secrets or personal data and require application-specific
  disabling or redaction before external upload.
- **AI-INT-001 (confirmed):** The canonical interaction flow must remain an
  explicit sequence of described query, deliberate cardinality, readiness,
  action, and user-visible outcome.
- **AI-INT-002 (confirmed):** Fresh targets use query readiness filters, while
  intentionally held targets use `wait_until()`. Documentation must define
  clickable as displayed plus enabled rather than a complete interactability
  guarantee.
- **AI-INT-003 (confirmed):** This design adds no generic click-and-type API.
  Selector-only helpers would hide cardinality, diagnostics, timeout, clearing,
  stale-element, retry, or outcome semantics, while a builder exposing those
  choices would duplicate `ElementQuery` without a stronger safety guarantee.
- **AI-INT-004 (confirmed):** Reusable interaction abstractions belong in
  application-specific Component intent methods, where their semantics and
  expected outcome are known.
- **AI-SNAP-001 (confirmed):** The book must provide a standard-WebDriver
  semantic page-outline recipe and distinguish its DOM-derived approximation
  from a browser-computed accessibility tree.
- **AI-SNAP-002 (confirmed):** Outline output must use a documented allowlist
  of tag, role, approximate name/text, test ID, normalized control type, shadow
  boundary, and fixed state fields. It must not capture values, URLs, storage,
  cookies, arbitrary attributes or classes, or raw markup.
- **AI-SNAP-003 (confirmed):** Traversal, emitted nodes, nesting, per-field
  UTF-8 bytes, text inspection, and total UTF-8 output must have conservative
  hard bounds with an in-bound truncation marker.
- **AI-SNAP-004 (confirmed):** Sensitive subtrees must support explicit
  redaction, password/form/contenteditable contents must be excluded, and the
  documentation must warn that remaining names and text can contain private
  data.
- **AI-SNAP-005 (confirmed):** Current-frame, iframe, open/closed shadow-root,
  portable WebDriver, Chromium CDP, and BiDi boundaries must be explicit. Raw
  CDP accessibility output must never be presented as safe to print directly.
- **AI-SNAP-006 (confirmed):** Examples must demonstrate both stable-selector
  selection and failed-query diagnosis from the outline.

## Acceptance criteria

- **AC-001:** The crate README, crate-level rustdoc, mdBook first-code page, and
  matching runnable examples contain no `find()` call in their Wikipedia user
  flow. Covers AI-AUTO-002.
- **AC-002:** The search form, search input, search button, and, where present,
  resulting article heading queries have readable descriptions and enforce
  their intended uniqueness. Covers AI-AUTO-003 and AI-AUTO-004.
- **AC-003:** The mdBook explanation teaches the query-based flow and explicitly
  preserves `find()` / `find_all()` as deliberate one-shot APIs. Covers
  AI-AUTO-002 and AI-AUTO-005.
- **AC-004:** The updated runnable examples compile with their existing required
  feature sets, and repository formatting, library tests, doc tests, clippy,
  and rustdoc validation pass. Covers AI-AUTO-001 through AI-AUTO-007.
- **AC-005:** The README, crate-level rustdoc, and getting-started path show
  `By::Testid` and explain the stable-selector priority without making the live
  Wikipedia flow inaccurate. Covers AI-SEL-001, AI-SEL-002, and AI-SEL-004.
- **AC-006:** The query documentation explains when text matching is meaningful
  versus brittle, shows the CSS escape hatch for custom test attributes, and
  reserves XPath for targets CSS cannot express. Covers AI-SEL-002 and
  AI-SEL-003.
- **AC-007:** The component documentation contains a concrete `testid` resolver
  example. Covers AI-SEL-001.
- **AC-008:** A standalone mdBook page contains a copyable checklist covering
  all AI-STYLE-002 topics without duplicating the deeper guides. Covers
  AI-STYLE-001 through AI-STYLE-003.
- **AC-009:** The mdBook summary and first-code page link to the checklist.
  Covers AI-STYLE-004.
- **AC-010:** A standalone AI/LLM quickstart contains the minimal dependency
  block and compile-checked or `no_run` starter test required by AI-QS-001 and
  AI-QS-002.
- **AC-011:** The starter test waits for an interactable control, asserts a
  user-visible result without a fixed sleep, and attempts explicit cleanup on
  both success and failure. Covers AI-QS-001 and AI-QS-002.
- **AC-012:** The quickstart contains concise prescriptive rules and links to
  the canonical review checklist plus every deeper guide named by AI-QS-004.
- **AC-013:** The mdBook summary links the quickstart from Getting Started.
  Covers AI-QS-004.
- **AC-014:** A Recipes section in the mdBook navigation contains all eleven
  task categories required by AI-REC-001. The examples are complete,
  independently copyable tests or programs, or clearly identify shared setup.
- **AC-015:** Every recipe is compile-checked or marked `no_run`, contains no
  fixed sleep, uses the recommended selector/query/wait shape where elements
  are involved, and explicitly attempts `driver.quit()` after its task result.
  Covers AI-REC-002 through AI-REC-004.
- **AC-016:** The logging, CDP, and BiDi recipes identify browser limitations,
  feature flags, and capability opt-ins and link to their deeper documentation.
  Covers AI-REC-005.
- **AC-017:** A standalone translation guide covers every Selenium and
  Playwright concept in AI-TRANS-001 and AI-TRANS-002 and uses `thirtyfour`
  APIs that exist in the current tree.
- **AC-018:** The guide includes source-to-Rust examples for lookup, waits,
  scoped targets, Components, and local or remote sessions, plus a dedicated
  table of common nonexistent APIs. Covers AI-TRANS-003.
- **AC-019:** The mdBook summary and AI quickstart link to the translation
  guide, which links onward to each deeper guide in AI-TRANS-004.
- **AC-020:** With default features, Component examples compile with
  `thirtyfour::prelude::*` supplying `Component` and `ElementResolver`, while
  resolver macros use explicit root imports. Crate rustdoc, mdBook Component
  guidance, the playground example, and Component integration tests show the
  same import contract. Covers AI-COMP-001 through AI-COMP-003.
- **AC-021:** Deterministic tests exercise successful setup/body/cleanup,
  setup failure without body execution, body-only failure, cleanup-only
  failure, simultaneous body and cleanup failure, and panics during body
  construction and polling, including cleanup failure or panic during a body
  panic. Covers AI-RUN-001 through AI-RUN-003.
- **AC-022:** Public rustdoc defines runner ownership, error precedence, panic
  behavior, and managed/remote session compatibility; the AI quickstart and
  reliability checklist use that API in compile-checked examples. Covers
  AI-RUN-004.
- **AC-023:** Deterministic tests verify independent capture after an endpoint
  failure, disabled fields issuing no requests, UTF-8-safe source truncation,
  newest-entry log bounds, and a display report that exposes screenshot size
  but not screenshot contents. Covers AI-ART-001 and AI-ART-002.
- **AC-024:** Public rustdoc and the diagnostics recipe show collector
  attachment before a test body, capture before runner cleanup, browser and
  manager limitations, portable-protocol scope, and privacy guidance. Covers
  AI-ART-003 and AI-ART-004.
- **AC-025:** Waiting documentation contains copyable click and text-entry
  patterns and explains both fresh-query and held-element readiness paths.
  Covers AI-INT-001 and AI-INT-002.
- **AC-026:** Waiting documentation names interactability, stale-element,
  clearing, retry, and outcome limitations and records the no-new-generic-API
  decision. Existing quickstart and recipe flows remain explicit. Covers
  AI-INT-003 and AI-INT-004.
- **AC-027:** A standalone recipe contains a complete `no_run` WebDriver
  example and is linked from navigation, recipes, diagnostics, quickstart, and
  `llms.txt`. Covers AI-SNAP-001.
- **AC-028:** The script enforces the specified visited/emitted/depth/text/
  field/total limits and returns an in-bound marker identifying applied limits.
  Covers AI-SNAP-003.
- **AC-029:** Review confirms the output allowlist and that no code path reads
  form values, URLs, cookies, storage, arbitrary attributes or classes, or raw
  markup; sensitive subtrees stop traversal. Covers AI-SNAP-002 and AI-SNAP-004.
- **AC-030:** The page covers current-frame execution, iframe switching,
  open/closed shadow roots, approximate semantics, portable WebDriver support,
  and experimental Chromium CDP limitations. Covers AI-SNAP-001 and AI-SNAP-005.
- **AC-031:** Example output leads to a described, cardinality-explicit
  `By::Testid` query and explains a concrete disabled-control failure state.
  Covers AI-SNAP-006.
- **AC-032:** Formatting, library/doc tests, clippy, rustdoc, mdBook, and a
  compile check of the new Rust example pass.
