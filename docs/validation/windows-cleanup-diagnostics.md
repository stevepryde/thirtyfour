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
dispatch against this branch. Manual dispatch selects one OS and log destination.

## Interpretation

- Residue in both arms, especially raw before shutdown: investigate ChromeDriver /
  browser / OS cleanup; library file deletion remains out of scope.
- Residue exclusively in managed sessions across sufficiently repeated matched
  runs: compare driver logs and teardown timing before proposing a production fix.
- Persistent files with no handles after quit: earlier contention is still possible;
  use a focused filesystem event trace around teardown in a follow-up experiment.
- Zero residue: report the sample size and versions, not a claim that the bug is fixed.

No production solution is claimed until the observations support one.
## Observations from diagnostic commit 3120221

The initial comparison run is
https://github.com/stevepryde/thirtyfour/actions/runs/43548363020.
All counts below refer to returned profile directories, not all temporary files.

| Environment | Chrome / driver | Raw completed / residue | Managed completed / residue | Result |
| --- | --- | --- | --- | --- |
| Windows Server 2025, image 20261103.298.1 | 156.0.8078.80 | 160 / 0 | 160 / 0 | Passed |
| Ubuntu, image 20261111.196.1 | 157.0.8103.50 | 160 / 0 | 160 / 0 | Passed |
| Hosted macOS, image 20261102.0473.1 | 156.0.8078.80 | 160 / 0 | 159 / 0 | Failed: one managed driver port collision before session creation |
| Local Apple Silicon macOS | 153.0.8010.36 | 160 / 0 | 160 / 0 | Passed |

The ordinary Windows stress run also passed with 160 recorded profiles and zero
residue: https://github.com/stevepryde/thirtyfour/actions/runs/43548362865.
Its profiles were under C:\Windows\SystemTemp despite redirected TMP/TEMP.
All nine manager matrix entries passed in that run.

The hosted macOS failed session (`managed-10-1`) logged `bind() failed: Address
already in use (48)` and `ERR_ADDRESS_IN_USE`; the manager reported its readiness
timeout. The harness retains that failure rather than retrying or weakening it.
The earlier local harness pilot likewise exposed reserve/release port races; raw
driver startup was corrected to use ChromeDriver-selected ports before the stable
commit. Pilot results are excluded from the table.

The logged InitSession requests match across arms. Browser/driver versions match
within each environment but differ across environments. Windows raw DELETE-to-
barrier time had a 50 ms median and 2150 ms maximum; 12 of 160 exceeded one second.
These are delayed-completion observations, not direct evidence of a retry count,
handle owner, or antivirus interference.

The original two main-branch failures had no preserved inventories or handles.
Their exact contents cannot be recovered retrospectively from path-only logs.
The first diagnostic run did not reproduce persistent residue, so no current
failure can be described as log-only and no handle-owner claim is supported.
Instrumentation and normal run-to-run variation may affect reproduction.

Artifacts are retained for 14 days; the run pages link the Windows, Linux, macOS,
and original stress artifacts. No driver-owned files were deleted, no cleanup
assertions were weakened, and no production changes or upstream report were made.

The follow-up stress instrumentation persists its record after quit (including
quit errors) to avoid adding disk I/O between creation and quit. The matched
comparison deliberately keeps its detailed pre-quit records and verbose logs.
See the draft PR for additional runs after the initial observations above.

## Reproduced residue and open handles

The unchanged comparison at 3120221 reproduced the issue in the repeat run:
https://github.com/stevepryde/thirtyfour/actions/runs/43549259924
(Windows job 132307690572, artifact 14095598893).
Windows image 20261103.298.1 and Chrome/ChromeDriver 156.0.8078.80 matched the
first run. All 320 sessions completed without protocol or startup errors; three
profiles remained in each arm. Each contained exactly one `chrome_debug.log`:

| Session | Profile under C:\Windows\SystemTemp | Bytes | Handle observed |
| --- | --- | ---: | --- |
| managed-19-2 | scoped_dir11548_1491098205 | 297 | Not caught |
| managed-19-3 | scoped_dir7896_509295196 | 295 | chrome.exe PID 7648, parent 3960 |
| managed-19-4 | scoped_dir11628_471127973 | 1573 | Not caught |
| raw-20-2 | scoped_dir5072_1132890783 | 296 | chrome.exe PID 3340, parent 6896 |
| raw-20-3 | scoped_dir12536_47907609 | 297 | chrome.exe PID 6840, parent 2692 |
| raw-20-7 | scoped_dir9780_239114701 | 1465 | Not caught |

Those parent PIDs match the primary Chrome `goog:processID` returned for the
sessions. The managed handle was observed after `quit()`; both raw handles were
observed after DELETE 200 and GET /session/{id}/url returned 404 `invalid session
id`, **before** /shutdown was sent. The raw driver processes then exited with
status zero. This reproduces the failure without Thirtyfour managing session
teardown and supports surviving Chrome child log handles as a mechanism in the
observed cases. It is not evidence that antivirus caused these occurrences.

All six files persisted through the final and delayed inventories, 38–66 seconds
after their first captured residue snapshot. By those later snapshots, Handle
found no matching handles; waiting did not make ChromeDriver repeat its abandoned
cleanup. The collector therefore caught transient ownership that a final-only
inventory would have missed. Exact inventories, safe log contents, PIDs, parent
PIDs, timings, and driver logs are retained in the artifact.

Relevant release sources:
- [Chrome 156 log opens omit FILE_SHARE_DELETE](https://github.com/chromium/chromium/blob/156.0.8078.80/base/logging.cc#L321).
- [ChromeDriver's finite deletion retries](https://github.com/chromium/chromium/blob/156.0.8078.80/chrome/test/chromedriver/chrome/scoped_temp_dir_with_retry.cc#L11).
- [ChromeDriver's supported stderr log routing](https://github.com/chromium/chromium/blob/156.0.8078.80/chrome/test/chromedriver/chrome_launcher.cc#L149).

The diagnostic workflow now accepts `chrome_logs=stderr` on manual dispatch,
adding `--enable-chrome-logs` to both arms and retaining the redirected output as
artifacts. The default `profile` mode preserves the reproduction and all cleanup
assertions. This separately tests an opt-in workaround without changing library
defaults or deleting any files. A logging-destination change cannot by itself
promise that every Chrome descendant has exited. No upstream report has been filed.
