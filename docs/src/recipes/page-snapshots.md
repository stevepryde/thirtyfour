# Semantic Page Outlines

When a selector fails, a compact outline can show whether the target is absent,
renamed, disabled, or surrounded by a different user-visible state. The recipe
below runs through standard WebDriver script execution, so it does not require
CDP, BiDi, or an optional `thirtyfour` feature.

This is a **semantic DOM approximation**, not the browser's computed
accessibility tree and not an accessibility audit. It deliberately exposes a
small, reviewable set of DOM-derived hints for debugging and selector design.

## Capture A Bounded Outline

```rust,no_run
use thirtyfour::prelude::*;

const SEMANTIC_PAGE_OUTLINE_SCRIPT: &str = r##"
const LIMITS = Object.freeze({
  visitedElements: 2000,
  emittedNodes: 200,
  outlineLevels: 8,
  domDepth: 64,
  ancestryHops: 64,
  fieldBytes: 120,
  textScanNodes: 32,
  totalBytes: 20 * 1024,
  markerReserveBytes: 128,
});

const encoder = new TextEncoder();
const lines = [];
const reasons = new Set();
let visited = 0;
let emitted = 0;
let outputBytes = 0;
let stopped = false;

const byteLength = (text) => encoder.encode(text).length;

// Normalize, quote, escape, and UTF-8-bound a field without first copying an
// arbitrarily large attribute or text node into another unbounded string.
function boundedField(value, asciiLower = false) {
  let output = "";
  let pendingSpace = false;
  let truncated = false;
  for (const original of String(value ?? "")) {
    const character = asciiLower && original >= "A" && original <= "Z"
      ? String.fromCharCode(original.charCodeAt(0) + 32)
      : original;
    if (/\s/u.test(character)) {
      pendingSpace = output.length > 0;
      continue;
    }
    const escaped = character === "\\" || character === "\""
      ? `\\${character}`
      : character;
    const next = `${pendingSpace ? " " : ""}${escaped}`;
    if (byteLength(output) + byteLength(next) > LIMITS.fieldBytes) {
      truncated = true;
      break;
    }
    output += next;
    pendingSpace = false;
  }
  if (truncated) reasons.add("field-bytes");
  return output;
}

function isRedacted(element) {
  let current = element;
  for (let hops = 0; current instanceof Element; hops += 1) {
    if (hops >= LIMITS.ancestryHops) {
      reasons.add("ancestry");
      return true;
    }
    if (current.matches("[data-snapshot-redact]")
      || (current.localName === "input" && current.type === "password")) {
      return true;
    }
    current = current.parentElement
      || (current.getRootNode() instanceof ShadowRoot ? current.getRootNode().host : null);
  }
  return false;
}

function isHidden(element) {
  let current = element;
  for (let hops = 0; current instanceof Element; hops += 1) {
    if (hops >= LIMITS.ancestryHops) {
      reasons.add("ancestry");
      return true;
    }
    if (current.hidden || current.inert || current.getAttribute("aria-hidden") === "true") {
      return true;
    }
    const style = getComputedStyle(current);
    if (style.display === "none" || style.visibility === "hidden") return true;
    if (current.parentElement) {
      current = current.parentElement;
    } else {
      const root = current.getRootNode();
      current = root instanceof ShadowRoot ? root.host : null;
    }
  }
  return false;
}

function canReadText(element) {
  return !isRedacted(element)
    && !element.isContentEditable
    && !["input", "textarea", "select"].includes(element.localName);
}

// Inspect at most 32 DOM nodes for one derived text field. Redacted, hidden,
// form-value, and contenteditable subtrees are never entered.
function scannedText(root, budget = { remaining: LIMITS.textScanNodes }) {
  if (!canReadText(root)) return "";
  const walker = document.createTreeWalker(
    root,
    NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT,
  );
  let output = "";
  let node = walker.nextNode();
  while (node && budget.remaining > 0) {
    budget.remaining -= 1;
    if (node.nodeType === Node.TEXT_NODE) {
      const fragment = boundedField(node.nodeValue ?? "");
      output = boundedField(`${output} ${fragment}`);
    } else if (node instanceof Element
      && (isRedacted(node) || isHidden(node) || !canReadText(node))) {
      let next = walker.nextSibling();
      while (!next && walker.parentNode() && walker.currentNode !== root) {
        next = walker.nextSibling();
      }
      node = next;
      continue;
    }
    node = walker.nextNode();
  }
  if (node) reasons.add("text-scan");
  return output;
}

function labelledText(element, budget) {
  const references = (element.getAttribute("aria-labelledby") ?? "").slice(0, 1024);
  let output = "";
  let count = 0;
  for (const id of references.match(/\S+/gu) ?? []) {
    if (count >= 8) {
      reasons.add("label-references");
      break;
    }
    count += 1;
    const label = document.getElementById(id);
    if (label && !isRedacted(label) && !isHidden(label)) {
      output = boundedField(`${output} ${scannedText(label, budget)}`);
    }
  }
  return output;
}

function approximateName(element) {
  const budget = { remaining: LIMITS.textScanNodes };
  const ariaLabel = element.getAttribute("aria-label");
  if (ariaLabel) return boundedField(ariaLabel);

  const labelled = labelledText(element, budget);
  if (labelled) return labelled;

  if (element.labels) {
    let output = "";
    for (let index = 0; index < Math.min(element.labels.length, 4); index += 1) {
      const label = element.labels[index];
      if (!isRedacted(label) && !isHidden(label)) {
        output = boundedField(`${output} ${scannedText(label, budget)}`);
      }
    }
    if (output) return output;
  }

  const alt = element.getAttribute("alt");
  if (alt) return boundedField(alt);

  if (["button", "a", "summary", "h1", "h2", "h3", "h4", "h5", "h6"]
    .includes(element.localName)) {
    return scannedText(element, budget);
  }
  return "";
}

function implicitRole(element) {
  const tag = element.localName;
  if (tag === "a" && element.hasAttribute("href")) return "link";
  if (tag === "button") return "button";
  if (tag === "textarea") return "textbox";
  if (tag === "select") return element.multiple ? "listbox" : "combobox";
  if (tag === "img") return "img";
  if (["h1", "h2", "h3", "h4", "h5", "h6"].includes(tag)) return "heading";
  if (tag === "main") return "main";
  if (tag === "nav") return "navigation";
  if (tag === "table") return "table";
  if (["ul", "ol"].includes(tag)) return "list";
  if (tag === "li") return "listitem";
  if (tag !== "input") return "";
  if (["button", "submit", "reset"].includes(element.type)) return "button";
  if (element.type === "checkbox") return "checkbox";
  if (element.type === "radio") return "radio";
  if (element.type === "range") return "slider";
  return "textbox";
}

function normalizedRole(element) {
  const raw = element.getAttribute("role") || implicitRole(element);
  const match = boundedField(raw).toLowerCase().match(/^[a-z][a-z0-9-]*/u);
  return match ? match[0] : "";
}

function normalizedTag(element) {
  const tag = boundedField(element.localName, true);
  return tag || "element";
}

function stateTokens(element) {
  const tokens = [];
  for (const state of ["disabled", "checked", "selected", "required", "readonly"]) {
    if (element[state] === true) tokens.push(state);
  }
  for (const name of ["aria-busy", "aria-checked", "aria-current", "aria-expanded",
    "aria-invalid", "aria-pressed", "aria-selected"]) {
    const value = element.getAttribute(name);
    if (["true", "false", "mixed", "page", "step", "location", "date", "time"]
      .includes(value)) {
      tokens.push(`${name.slice(5)}=${value}`);
    }
  }
  return tokens;
}

function descriptor(element) {
  const role = normalizedRole(element);
  const name = approximateName(element);
  const testId = boundedField(element.getAttribute("data-testid") ?? "");
  const includeText = ["status", "alert", "log", "note"].includes(role)
    || ["p", "label", "section", "article", "dialog"].includes(element.localName);
  const text = includeText ? scannedText(element) : "";
  const parts = [normalizedTag(element)];
  if (role) parts.push(`role=${role}`);
  if (name) parts.push(`name=\"${name}\"`);
  if (text && text !== name) parts.push(`text=\"${text}\"`);
  if (testId) parts.push(`testid=\"${testId}\"`);
  if (["input", "button"].includes(element.localName) && element.type) {
    parts.push(`type=${boundedField(element.type)}`);
  }
  parts.push(...stateTokens(element));
  if (element.shadowRoot) parts.push("shadow=open");
  return { text: parts.join(" "), meaningful: parts.length > 1 };
}

function addLine(depth, text) {
  if (emitted >= LIMITS.emittedNodes) {
    reasons.add("nodes");
    stopped = true;
    return false;
  }
  const line = `${"  ".repeat(depth)}${text}`;
  const lineBytes = byteLength(line) + 1;
  if (outputBytes + lineBytes > LIMITS.totalBytes - LIMITS.markerReserveBytes) {
    reasons.add("bytes");
    stopped = true;
    return false;
  }
  lines.push(line);
  emitted += 1;
  outputBytes += lineBytes;
  return true;
}

function visit(element, outlineDepth, domDepth) {
  if (stopped) return;
  if (visited >= LIMITS.visitedElements) {
    reasons.add("visited");
    stopped = true;
    return;
  }
  visited += 1;
  if (domDepth >= LIMITS.domDepth) {
    reasons.add("dom-depth");
    return;
  }

  if (["script", "style", "template", "noscript"].includes(element.localName)
    || isHidden(element)) {
    return;
  }
  if (isRedacted(element)) {
    addLine(outlineDepth, `${normalizedTag(element)} [redacted]`);
    return;
  }

  const details = descriptor(element);
  const semantic = details.meaningful
    || ["main", "nav", "form", "table", "ul", "ol", "li", "section", "article",
      "p", "dialog"].includes(element.localName);
  let childDepth = outlineDepth;
  if (semantic) {
    if (!addLine(outlineDepth, details.text)) return;
    childDepth = outlineDepth + 1;
    if (childDepth >= LIMITS.outlineLevels) {
      childDepth = LIMITS.outlineLevels - 1;
      reasons.add("outline-depth");
    }
  }

  for (const child of element.children) {
    visit(child, childDepth, domDepth + 1);
    if (stopped) break;
  }
  if (element.shadowRoot && !stopped && addLine(childDepth, "#shadow-root (open)")) {
    let shadowDepth = childDepth + 1;
    if (shadowDepth >= LIMITS.outlineLevels) {
      shadowDepth = LIMITS.outlineLevels - 1;
      reasons.add("outline-depth");
    }
    for (const child of element.shadowRoot.children) {
      visit(child, shadowDepth, domDepth + 1);
      if (stopped) break;
    }
  }
}

visit(document.documentElement, 0, 0);
if (reasons.size > 0) {
  const marker = `[outline limited: ${[...reasons].sort().join(",")}]`;
  // The reason vocabulary is fixed and fits in the reserved 128 bytes.
  lines.push(marker);
}
return lines.join("\n");
"##;

async fn semantic_page_outline(driver: &WebDriver) -> WebDriverResult<String> {
    driver
        .execute(SEMANTIC_PAGE_OUTLINE_SCRIPT, Vec::new())
        .await?
        .convert()
}

#[tokio::test]
async fn diagnoses_a_missing_checkout_control() -> WebDriverResult<()> {
    let driver = WebDriver::managed(DesiredCapabilities::chrome()).await?;
    let test_result: WebDriverResult<()> = async {
        driver.goto("https://example.test/checkout").await?;
        driver
            .query(By::Testid("submit-order"))
            .and_clickable()
            .desc("place order button")
            .single()
            .await?;
        Ok(())
    }
    .await;

    if test_result.is_err() {
        match semantic_page_outline(&driver).await {
            Ok(outline) => eprintln!("{outline}"),
            Err(error) => eprintln!("semantic outline unavailable: {error}"),
        }
    }
    let quit_result = driver.quit().await;
    test_result?;
    quit_result
}
```

The function returns text shaped like this:

```text
main role=main
  form testid="checkout-form"
    input role=textbox name="Email" type=text required
    button role=button name="Place order" testid="submit-order" type=submit disabled
  p role=status text="Card declined"
```

That report suggests a stable `By::Testid("submit-order")` selector and also
explains why the clickable query timed out: the control exists but is disabled
while the page reports an error. The outline does not prove selector uniqueness,
so generated code must still choose `single()`, `first()`, or `all()`
intentionally and give important queries a description.

## Bounds And Privacy Contract

The copied script enforces hard limits of 2,000 visited elements, 200 emitted
nodes, eight outline levels, 64 DOM traversal levels, 64 ancestor checks per
visibility or redaction decision, 120 UTF-8 bytes per field, 32 inspected DOM
nodes per derived text field, and 20 KiB total output. The total includes a
reserved, in-bound marker identifying limits that changed or stopped the
result. An ancestry limit fails closed as hidden/redacted. Non-semantic wrapper
elements are traversed but do not use an outline level; deeper semantic nodes
flatten at the eighth level.

Output is allowlisted to an ASCII-normalized tag, explicit or small-map
implicit role, approximate name, useful text, `data-testid`, normalized
form-control type, open-shadow-root presence, and fixed state tokens. The
script never reads form values, password contents, `href`/`src`/`action`
values, the page URL, cookies, storage, headers, network data, arbitrary
attributes or classes, raw HTML, or generated IDs.

Add `data-snapshot-redact` to a sensitive container. The script emits one
`[redacted]` line and never enters that subtree; password controls receive the
same treatment. Label and text derivation also reject redacted ancestors, so an
ARIA reference cannot bypass the marker. Text is never derived from inputs,
textareas, selects, or contenteditable regions.

Bounds are not a privacy filter. Visible text, ARIA labels, associated labels,
alt text, and even test IDs can contain secrets or personal data. Keep dynamic
sensitive data out of test IDs, customize the allowlist for the application,
review output before external upload, and mark unsafe regions explicitly.
The recipe also assumes the page has not replaced standard JavaScript or DOM
intrinsics; it is a debugging aid for applications you trust, not a security
sandbox for hostile documents.

## Browser And Context Boundaries

`WebDriver::execute` runs in the current browsing context. The recipe uses
standard DOM APIs and works across conforming Chromium, Firefox, Safari, and
other WebDriver implementations that permit normal script execution. It covers
only the active document: switch into an iframe, capture again, and restore the
outer frame explicitly when frame content matters.

Open shadow roots are traversed and marked; closed roots are inaccessible. The
outline is a point-in-time observation and can become stale immediately as the
page changes. Its approximate roles and names can differ from the browser's
computed accessibility semantics.

For an actual Chromium accessibility tree, the experimental CDP
[`Accessibility.getFullAXTree`](https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/#method-getFullAXTree)
command is available through `driver.cdp().send_raw(...)` with the default-on
`cdp` feature and a Chromium session. Do not print that raw response: it can
contain values, URLs, names, descriptions, and unbounded breadth. Apply an
explicit allowlist, redaction, node/field limits, and a total byte bound first.

WebDriver BiDi can locate a known node by accessibility role and name, but it
does not provide this recipe with one portable complete-tree snapshot. This
recipe therefore does not require or imply the `bidi` feature.

Protocol references: [W3C WebDriver script execution](https://www.w3.org/TR/webdriver2/#execute-script),
[WebDriver BiDi](https://w3c.github.io/webdriver-bidi/), and the
[CDP Accessibility domain](https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/).
