# Native Hub CI

The Native Hub workflow runs formatting, Host contracts, dependency identity,
RV64 debug/release builds and the fixed System handoff. Pull requests with a
non-empty diff containing only `.md` paths keep the contracts job and skip the
QEMU handoff. A failed or incomplete diff classification defaults to running
the handoff. Pushes to `main` always run every job. Only outdated runs for the
same pull request are cancelled; main-branch runs are kept. Classification has
a five-minute bound, contracts has a 15-minute bound, and the existing handoff
bound remains 20 minutes.

The debug and release handoff jobs remain parallel and each runs all three Hart
counts. Cargo download storage is cached by runner OS/architecture, toolchain,
profile, all relevant lockfiles, pinned platform lock, and workflow contents.
The separate platform target cache is keyed for the fixed System revision,
release profile, RV64 target, lockfiles and workflow. Both caches have no
fallback restore key. Only the release handoff job saves a platform target
cache after a miss, avoiding same-key saves from both matrix jobs. The commands
still run on a cache hit. VM serial output, test results, inputs and evidence
remain outside both caches. Each job uploads its complete success or failure
evidence under a run-attempt-specific artifact name.

Cache entries are performance inputs only. A missing or incompatible entry
causes a normal build from the locked sources. The workflow logs report each
cache hit so warm-run savings can be measured from actual Actions runs.

The handoff uses System #24's immutable ABI 13 graph. Its public scenario
selector runs only `handoff`, without patching the provider runner. Source
revisions have separate ignored overlay directories; artifacts collect the
revision-qualified native evidence paths and include the guest's ABI/Ledger/
Capsule assertion. Existing release-Kernel role/VM bounds remain 60/90 seconds;
no old result is reused as evidence after the ABI migration.
