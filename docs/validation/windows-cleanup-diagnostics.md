# Windows Chrome cleanup diagnostics (#380)

This diagnostic branch changes tests and CI only. Thirtyfour must never delete
profile files: ChromeDriver owns cleanup, and files left after quit are evidence.
No profile deletion, test exclusion, or relaxed cleanup assertion is introduced.

## Experiment

`cleanup_comparison` alternates raw HTTP and managed session batches, each with
eight independent ChromeDriver processes, for 20 rounds (160 sessions per arm).
A bootstrap managed session resolves the driver executable once; both arms then
use that exact executable. An explicit `THIRTYFOUR_CHROMEDRIVER` skips bootstrap.
Both arms request identical headless/no-sandbox/quitGracefully=true capabilities,
perform no navigation, and retain ChromeDriver verbose logs. Returned browser and
driver versions and profiles are recorded per session, before quit. Independent
manager instances match the existing cleanup stress test's ownership model.

The raw arm sends DELETE, confirms an invalid-session response to GET
/session/{id}/url, observes residue with the driver **still running**, then calls
/shutdown and awaits process exit with a ten-second bound. The managed arm uses
normal `quit()`. Raw-driver residue before /shutdown establishes that a managed
process release is not necessary for that occurrence. It does not identify which
process caused deletion to fail. The barrier observes a completed deletion
attempt; it does not prove that files disappeared.

A separate workflow runs Windows, macOS, and Linux controls. It has read-only
repository permissions, a twenty-minute limit per job, and uploads artifacts even
on test failure. Existing Windows stress coverage retains its 20x8 sessions and
assertions and now captures the actual returned paths rather than relying solely
on redirected TEMP. Windows may put profiles in C:\Windows\SystemTemp regardless
of TMP/TEMP.

## Evidence and limits

Artifacts contain per-session timestamps, requested/returned capabilities,
ChromeDriver logs, the exact profile path, and bounded residue inventories
(256 entries, six recursion levels, sizes, explicit truncation/errors). Only a
`chrome_debug.log` prefix (up to 16 KiB) is read from profile contents. No profile
archive, browser history, cookie database, environment dump, or arbitrary process
command line is collected. Profiles are generated solely by these blank-page
sessions. Symlinks/reparse points are not traversed beneath the profile.

On Windows, residue triggers a read-only Sysinternals Handle search for the exact
profile path and a Chrome/ChromeDriver process listing with PID, parent PID,
creation time, and whether its command line references that profile. Handle is
downloaded from Microsoft's official distribution into the ephemeral CI runner;
its EULA is accepted for that run. Handle output includes any matching owner,
including non-Chrome processes. It never closes handles. Missing/failed collection
is reported and is not evidence that no handle existed.

Inventories are taken immediately after quit and at the end; the comparison also
repeats residual observations after another 30 seconds. Observation timing is
recorded because inventory/handle queries perturb timing. A snapshot after quit
cannot exclude a transient handle during ChromeDriver's finite deletion retry
window. Negative handle results therefore cannot prove or disprove antivirus or
short-lived child-process interference. OS runs may have different browser
versions; compare the recorded versions before drawing an OS-specific conclusion.
Verbose logging is identical across comparison arms but can itself perturb timing.
The original stress test remains a separate control without verbose driver logs.

## Local invocation

Use dedicated absolute artifact and browser-temp directories; existing directories
and their contents are never removed by the harness. On macOS:

```sh
THIRTYFOUR_CLEANUP_ARTIFACTS=/tmp/cleanup-observations \
MAC_CHROMIUM_TMPDIR=/tmp/cleanup-browser-temp \
cargo test -p thirtyfour --features manager-tests --test cleanup_comparison \
  -- --ignored --nocapture
```

Use TMP/TEMP on Windows and TMPDIR on Linux. `THIRTYFOUR_CLEANUP_ROUNDS` supports
short harness smoke runs; CI uses the default 20. Keep separate artifact directories
for separate runs. Re-run the diagnostic workflow through its PR check or manual
dispatch once available on the default branch.

## Interpretation

- Residue in both arms, especially raw before shutdown: investigate ChromeDriver /
  browser / OS cleanup; library file deletion remains out of scope.
- Residue exclusively in managed sessions across sufficiently repeated matched
  runs: compare driver logs and teardown timing before proposing a production fix.
- Persistent files with no handles after quit: earlier contention is still possible;
  use a focused filesystem event trace around teardown in a follow-up experiment.
- Zero residue: report the sample size and versions, not a claim that the bug is fixed.

No production solution is claimed until the observations support one.
