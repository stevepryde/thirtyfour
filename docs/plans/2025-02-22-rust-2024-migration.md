# Rust 2024 Edition Migration Plan for thirtyfour

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Update the thirtyfour library from Rust 2021 edition to 2024 edition by:
1. Removing ALL dynamic dispatch (dyn Trait patterns)
2. Using native async traits with generics instead
3. Running clippy in pedantic mode and fixing all warnings
4. Updating documentation

**Key Finding:** The `async_trait` crate serves two purposes: enabling async methods AND making them dyn-safe. Since Rust 2024 doesn't support native async + dyn together, we must eliminate ALL dynamic dispatch patterns.

**Architecture:**
- Make traits use concrete types via generics with `impl Trait`
- Update structs to be generic over trait implementations
- Use associated types or default type parameters for ergonomics

---

## Task Breakdown (Approach B: Remove All Dynamic Dispatch)

### Task 1: Update Cargo.toml files - Rust 2024 edition

**Files:**
- Modify: `thirtyfour/Cargo.toml` (done in previous commit)
- Verify: `thirtyfour-macros/Cargo.toml` (done in previous commit)

---

### Task 2: Refactor HttpClient trait to remove async_trait and dyn

**File:** `thirtyfour/src/session/http.rs`

**Changes required:**
1. Remove `#[async_trait::async_trait]` from trait definition (line 39)
2. Remove `#[async_trait::async_trait]` from reqwest impl (line 57)
3. Remove `#[async_trait::async_trait]` from null_client impl (line 134)
4. Change async fn signatures to use `-> impl Future<Output = ...>` 
5. The tricky part: handle the factory method that returns `Arc<dyn HttpClient>`

**Step 1: Read session/http.rs to understand full context**
```bash
cat thirtyfour/src/session/http.rs | head -160
```

**Step 2: Make edits to remove async_trait attributes**

**Step 3: Verify with cargo check**
Run: `cd thirtyfour && cargo check 2>&1 | head -30`
Expected: If this task alone doesn't compile, proceed to next tasks and fix together

---

### Task 3: Refactor SessionHandle to use generic HttpClient

**File:** `thirtyfour/src/session/handle.rs`

**Changes required:**
- Line 31: Change `pub client: Arc<dyn HttpClient>` → make struct generic
- Update constructors at lines 54, 63 to accept concrete types

**Pattern:**
```rust
// Before:
pub struct SessionHandle {
    pub client: Arc<dyn HttpClient>,
}

// After:
pub struct SessionHandle<C: HttpClient = reqwest::Client> {
    pub client: C,
}
```

---

### Task 4: Refactor session/create.rs for generic HttpClient

**File:** `thirtyfour/src/session/create.rs`

**Changes required:**
- Line 19: Change parameter from `&dyn HttpClient` to generic `&C`
- Update function signature to be generic over HttpClient type

---

### Task 5: Refactor WebDriver constructor for generic client

**File:** `thirtyfour/src/web_driver.rs`

**Context:** The main entry point needs to work with any HttpClient implementation
Changes needed around lines 100-130

**Step 1: Read web_driver.rs to find the new_with_config_and_client function**

---

### Task 6: Refactor ElementPoller traits (remove async_trait + Box<dyn>)

**File:** `thirtyfour/src/extensions/query/poller.rs`

**Changes required:**
1. Remove `#[async_trait::async_trait]` from ElementPoller trait (line 9)
2. Change async fn tick to return impl Future
3. Remove `#[async_trait::async_trait]` from implementations (lines 54, 88)
4. For IntoElementPoller::start() - change from Box<dyn> to either:
   - Return type `impl ElementPoller + Send + Sync`, OR
   - Use an enum wrapper for different poller types

**Step 1: Read the full file first**

---

### Task 7: Refactor ElementQuery and ElementWaiter to use concrete poller types

**Files:** 
- `thirtyfour/src/extensions/query/element_query.rs`
- `thirtyfour/src/extensions/query/element_waiter.rs`

**Changes required:**
- Change storage from `Arc<dyn IntoElementPoller>` to generic type parameter
- Update with_poller() methods accordingly

---

### Task 8: Refactor WebDriverConfig for generic poller

**File:** `thirtyfour/src/common/config.rs`

**Changes required:**
- Lines 20, 72, 101: Make WebDriverConfig generic over poller type
- Use default type parameter for backward compatibility

```rust
// Before:
pub struct WebDriverConfig {
    pub poller: Arc<dyn IntoElementPoller + Send + Sync>,
}

// After:
#[non_exhaustive]
pub struct WebDriverConfig<P: IntoElementPoller = DefaultPoller> {
    pub keep_alive: bool,
    pub poller: P,  // Concrete type instead of Arc<dyn>
}
```

---

### Task 9: Verify compilation after all refactoring

**Step 1: Run cargo check**
```bash
cd thirtyfour && cargo check 2>&1 | head -50
```

If errors remain:
- Fix each error systematically
- May need to adjust generic bounds or add where clauses

---

### Task 10: Run clippy in pedantic mode and fix all warnings

**Step 1: Run clippy**
```bash
cd thirtyfour && cargo clippy -- -W clippy::pedantic -W clippy::nursery 2>&1 | head -100
```

**Step 2: Find allow attributes**
```bash
grep -rn "#\[allow" thirtyfour/src/ | grep -v "TODO\|FIXME"
```

**Step 3: Fix each warning category:**
- Remove unnecessary allow directives
- Fix actual code issues:
  - unwrap/expect → proper error handling  
  - missing docs → add documentation
  - complex logic → simplify

**Step 4: Repeat until clean**
```bash
cargo clippy -- -W clippy::pedantic
```

---

### Task 11: Run full test suite and ensure all pass

**Step 1: Run tests**
```bash
cd thirtyfour && cargo test --all-targets 2>&1
```

**Step 2: Fix any failing tests** (not disable them)

---

### Task 12: Update documentation

**Files to check:**
- `thirtyfour/README.md`
- Any CHANGELOG file

**Changes needed:**
- Note Rust 2024 edition requirement
- Document breaking changes for custom HttpClient/ElementPoller implementations

---

## Summary of Verification Steps

After all tasks complete, verify:

1. ✅ `cargo check --all-targets` succeeds without errors
2. ✅ `cargo clippy -- -W clippy::pedantic` shows no warnings  
3. ✅ `cargo test --all-targets` passes 100%
4. ✅ No `#[allow(...)]` for clippy lints remain (except TODO/FIXME)
5. ✅ Documentation reflects the changes

---

## Files Requiring Changes (Complete List)

| File | Change Type |
|------|-------------|
| thirtyfour/src/session/http.rs | Refactor trait + remove async_trait |
| thirtyfour/src/session/handle.rs | Make SessionHandle generic |
| thirtyfour/src/session/create.rs | Make start_session generic |
| thirtyfour/src/web_driver.rs | Update constructors for generic client |
| thirtyfour/src/extensions/query/poller.rs | Refactor traits, change return types |
| thirtyfour/src/extensions/query/element_query.rs | Change storage to concrete types |
| thirtyfour/src/extensions/query/element_waiter.rs | Change storage to concrete types |
| thirtyfour/src/common/config.rs | Make config generic with defaults |

---

## Commit Strategy

Commit after each task that compiles successfully:
- `refactor(http): remove async_trait, make HttpClient trait use native async`
- `refactor(handle): make SessionHandle generic over HttpClient`
- `refactor(poller): convert Box<dyn> to impl Trait pattern`  
- `refactor(config): make WebDriverConfig generic with default type`
- `fix(clippy): address pedantic warnings and remove allow directives`
- `test: verify all tests pass with Rust 2024 edition`

---

**Ready for subagent-driven execution using superpowers:subagent-driven-development**

Each task should be executed as a separate subagent dispatch, with verification (cargo check) after each one. If compilation fails at any step, launch new subagents to fix the specific issues.