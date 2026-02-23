# Rust 2024 Edition Migration Plan for thirtyfour

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Update the thirtyfour library from Rust 2021 edition to 2024 edition, removing the `async-trait` dependency and converting dynamic dispatch patterns where appropriate. Also run clippy in pedantic mode, fix all warnings, update documentation, and ensure no panicking code or obscure APIs.

**Architecture:** 
- Remove `#[async_trait]` attributes - native Rust async traits work in edition 2024
- Convert `Box<dyn ElementPoller>` returns to `impl Trait` for cleaner ergonomics  
- Keep factory patterns (`Arc<dyn HttpClient>`) that need runtime polymorphism as-is

**Tech Stack:** Rust 2024 edition (requires Rust 1.85+), no external async-trait crate needed, clippy pedantic mode

---

## Task Breakdown

### Task 1: Update thirtyfour/Cargo.toml - change edition and remove async-trait dependency

**Files:**
- Modify: `thirtyfour/Cargo.toml`

**Context:** This is the main library crate. Need to update from Rust 2021 to 2024 edition and remove the async-trait dependency that was used for async trait support in older Rust editions.

**Changes required:**
1. Line 5: Change `edition = "2021"` to `edition = "2024"`
2. Remove line 42: `async-trait = "0.1.83"`

**Step 1: Make the edits**
- Edit thirtyfour/Cargo.toml with the above changes

**Step 2: Verify compilation still works**
Run: `cd thirtyfour && cargo check`
Expected: SUCCESS (may have other errors from edition change - we'll address those in later tasks)

---

### Task 2: Update thirtyfour-macros/Cargo.toml - change edition to 2024

**Files:**
- Modify: `thirtyfour-macros/Cargo.toml`

**Context:** This is the proc-macro crate for thirtyfour. It also needs to be updated to 2024 edition for consistency.

**Changes required:**
1. Line 5: Change `edition = "2021"` to `edition = "2024"`

**Step 1: Make the edit**
- Edit thirtyfour-macros/Cargo.toml

**Step 2: Verify compilation works**
Run: `cd thirtyfour && cargo check`
Expected: SUCCESS (may have warnings about async_trait not being used)

---

### Task 3: Remove #[async_trait] attributes from session/http.rs

**Files:**
- Modify: `thirtyfour/src/session/http.rs`

**Context:** The HttpClient trait currently uses the #[async_trait::async_trait] attribute to support async methods in traits. In Rust 2024 edition, this is no longer needed - native async traits work directly.

**Changes required:**
1. Line 39: Remove `#[async_trait::async_trait]` (before `pub trait HttpClient`)
2. Line 57: Remove `#[async_trait::async_trait]` (before `impl HttpClient for reqwest::Client`)
3. Line 134: Remove `#[async_trait::async_trait]` (before `impl HttpClient for NullHttpClient`)

**Step 1: Make the edits**
- Edit thirtyfour/src/session/http.rs to remove these three attributes

**Step 2: Run cargo check**
Run: `cd thirtyfour && cargo check`
Expected: SUCCESS - the async trait methods should work without the macro

---

### Task 4: Update poller.rs - remove async_trait and convert Box<dyn> to impl Trait

**Files:**
- Modify: `thirtyfour/src/extensions/query/poller.rs`

**Context:** The ElementPoller trait uses async_trait, and IntoElementPoller::start() returns a boxed trait object. We need to:
1. Remove the async_trait attributes (native async works in 2024 edition)
2. Change Box<dyn ElementPoller> return type to impl Trait

**Changes required:**
1. Line 9: Remove `#[async_trait::async_trait]` from trait definition
2. Line 54: Remove `#[async_trait::async_trait]` from impl ElementPollerWithTimeout  
3. Line 88: Remove `#[async_trait::async_trait]` from impl ElementPollerNoWait
4. Lines 20, 79, 96: Change `Box<dyn ElementPoller + Send + Sync>` to `impl ElementPoller + Send + Sync`

**Step 1: Make the edits**
- Edit thirtyfour/src/extensions/query/poller.rs with these changes

**Step 2: Check for any callers that might break**
Run grep to find places using IntoElementPoller:
```bash
grep -r "IntoElementPoller" --include="*.rs"
```
Check if any code depends on the specific Box<dyn> type.

**Step 3: Run cargo check**
Run: `cd thirtyfour && cargo check`
Expected: SUCCESS with all traits working natively

---

### Task 5: Verify compilation with edition 2024

**Files:**
- Check: All files in thirtyfour/ and thirtyfour-macros/

**Context:** After making the core changes, verify everything compiles correctly.

**Step 1: Run full cargo check**
Run: `cargo check --all-targets`
Expected: SUCCESS - no errors

---

### Task 6: Fix all clippy pedantic warnings

**Files:**
- All source files in thirtyfour/
- Remove any #[allow(...)] clippy annotations
- Fix the underlying issues

**Context:** Run clippy in pedantic mode to find all linting issues. This includes:
1. Remove any `#[allow(clippy::...)]` or other allow attributes for lints
2. Fix actual code issues that trigger warnings
3. Ensure no panicking code (.unwrap(), .expect(), etc.)
4. No hidden side effects (complex control flow that's hard to follow)
5. No obscure APIs (use well-known, documented std/lib functions)

**Step 1: Run clippy with pedantic mode**
Run: `cargo clippy -- -W clippy::pedantic -W clippy::nursery`
Capture all warnings

**Step 2: Find and remove allow attributes**
Run: `grep -rn "#\[allow" thirtyfour/src/`
Check each one and either fix the issue or remove the allow if it's no longer needed

**Step 3: Fix issues systematically**
For each warning category:
- unwrap/expect → proper error handling
- complex logic → simplify
- missing docs → add documentation  
- unsafe code → review necessity, add safety comments
- hidden side effects → make explicit

**Step 4: Run clippy again until clean**
Run: `cargo clippy -- -W clippy::pedantic`
Expected: No warnings

---

### Task 7: Run tests and ensure all pass

**Files:**
- All test files in thirtyfour/

**Context:** Ensure the migration hasn't broken any functionality. All existing tests must continue to pass.

**Step 1: Run full test suite**
Run: `cargo test --all-targets`
Expected: All tests PASS (100% success)

**Step 2: If any tests fail**
- Investigate why they failed
- Fix the issues (not by disabling tests)
- Re-run until all pass

---

### Task 8: Update documentation if needed

**Files:**
- thirtyfour/README.md
- Any CHANGELOG or docs/

**Context:** The migration is a breaking change that warrants documenting. Also check if any code comments need updating now that async_trait is removed.

**Step 1: Check README for edition info**
Run: `grep -i "edition\|rust" thirtyfour/README.md`
Update if needed

**Step 2: Update version/changelog**
If there's a CHANGELOG, add entry about:
- Rust 2024 edition requirement
- Removal of async-trait dependency  
- Breaking changes for custom poller implementations

---

## Summary of Verification Steps

After all tasks complete, verify:

1. ✅ `cargo check` succeeds without errors
2. ✅ `cargo clippy -- -W clippy::pedantic` shows no warnings
3. ✅ `cargo test --all-targets` passes 100%
4. ✅ No `#[allow(...)]` for clippy lints remain
5. ✅ Documentation reflects the changes

## Commit Strategy

Each task should be committed individually with a descriptive message:
- "chore: update edition to 2024 in thirtyfour"
- "chore: remove async-trait dependency"  
- "refactor: use native async traits instead of async_trait crate"
- "refactor: convert Box<dyn> to impl Trait for poller"
- "fix(clippy): address pedantic warnings and remove allow directives"
- "test: verify all tests pass with Rust 2024 edition"
- "docs: update README for Rust 2024 requirement"

---

**Ready for subagent-driven execution using superpowers:subagent-driven-development**