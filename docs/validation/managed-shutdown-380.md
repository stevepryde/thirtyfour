# Managed Chromium shutdown: issue #380

## Reproduction (2026-11-12)

Tested on macOS 27.0.1 / Apple Silicon, Chrome and chromedriver
153.0.8010.36, against main at `24f9114` and this change.
Each stress run launched 8 independent managed headless Chrome sessions per
batch, explicitly quit all sessions, and repeated 20 times (160 sessions).
`MAC_CHROMIUM_TMPDIR` pointed to a separate empty directory for each run.

| Observation | Before | After |
| --- | ---: | ---: |
| Recorded Chrome profiles remaining | 160 / 160 | 0 / 160 |
| Chromedriver `*.scoped_dir.*` directories remaining | 160 | 0 |
| Allocated space in redirected directory | ~1.3 GB | 0 bytes |

The single-session regression also failed before and passed after. The stress
assertion checks profile paths after a two-second settling period; the ordinary
regression checks immediately after `quit()` returns. Directory inventory was
checked separately after each stress run. Both runs also left 160 empty
`com.google.Chrome.*` directories without the `scoped_dir` name. Those are
reported separately, not claimed fixed or counted as chromedriver profile leaks.

On this Mac, requesting `/shutdown` and awaiting process exit allowed cleanup
to finish reliably in all 160 observed sessions. This is evidence for the fix,
not a guarantee for every Chromium version or OS. No Windows or Linux local run
was performed. The Chrome cleanup regression is included in the existing
Windows/Linux/macOS manager CI matrix.

## Other browsers and lifetime boundaries

A real Firefox headless session removed its `moz:profile` directory on explicit
quit with the existing force-kill path. Firefox shutdown is unchanged. Safari
was not tested: enabling Remote Automation would change the user's browser
configuration; Safari shutdown is unchanged. Edge shares the Chromium shutdown
path, but was not locally installed/tested.

The existing shared-manager, concurrent quit, interleaved launch/drop, implicit
drop and panic tests passed (13 manager tests, excluding Edge/Safari). New
synthetic Unix subprocess tests cover delayed cleanup after HTTP success/EOF,
a hanging request, a hanging child, cancellation, and exactly-one shutdown
status emission. Session-handle tests preserve DELETE-error retry behavior and
`leak()` ownership behavior. Unit tests: 119 passed. Doc tests: 127 passed,
10 ignored. Formatting, all-feature/all-target Clippy and all-feature rustdoc
checks passed.

Graceful shutdown is only used after the last managed session explicitly quits
successfully. Cancellation, session-creation failure, implicit drop and `leak()`
can still hit the synchronous kill fallback. This change does not delete files
itself, alter shared-session ownership, or redefine `leak()`.

## Re-running

```sh
cargo test -p thirtyfour --features manager-tests --test managed_cleanup
# Use an empty, dedicated directory. On Windows set TMP and TEMP instead;
# on Linux use TMPDIR. Do not mutate process environment within parallel tests.
MAC_CHROMIUM_TMPDIR=/absolute/dedicated/temp/root \
  cargo test -p thirtyfour --features manager-tests --test managed_cleanup \
  managed_cleanup_stress -- --ignored --nocapture
cargo test -p thirtyfour --features manager-tests --test managed_cleanup \
  managed_quit_removes_firefox_profile -- --ignored
```

The stress and Firefox tests are opt-in; normal CI runs the Chrome profile
regression. Normal explicit-quit shutdown grace is capped at 10 seconds and can
be configured using `shutdown_timeout`. That budget does not include the
preceding DELETE request or operating-system process reaping.
