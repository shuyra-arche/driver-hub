# Native Instance consumer

This scenario injects Hub's production Driver/Native consumer into the ordinary
controller User image of System's fixed `f97524817ce240a0b0471be3b557ceb5a871fe14`
handoff fixture. The complete production System runs unchanged. The controller
uses the real supplied Instance endpoint, a caller-wide operation sequence,
System-issued InstanceIds and native reply cleanup.

Two Driver Teams are prepared and started. Actual CPU Sign delivery, ready
UnitRoots, publication CAS, direct consumer RPC, live replacement, withdrawal,
Stop admission versus Retire completion, and independent Team/root-exit evidence
are checked. Hub's native marker and the provider's typed lifecycle checker must
both pass. The fixture's private CPU Sign source is test-only; Hub production
code depends only on public API/Client packages.

This selects the provider's main `handoff` scenario. Its other auth/cancel/death
VMs are not Hub-consumer results. Hub's Host suite separately checks uncertain
requests, cancellation, wrong identities and failed retirement. This scenario
does not boot a full Hub Machine, validate PCI/IRQ/External effects, prove device
quiescence or accept Toolchain std Units.

Use the declared toolchain, QEMU 10.0.13 and OpenSBI 1.6. Set `QEMU_RISCV64` and
`QEMU_OPENSBI` to their actual paths. Build System's instruction-qualified runtime
platform at that same revision, then run:

```sh
python3 tests/target/handoff/run.py /path/to/system-runtime-platform --system-checkout /path/to/fixed-system --profile release --harts 1 2 4
python3 tests/target/handoff/run.py /path/to/system-runtime-platform --system-checkout /path/to/fixed-system --profile debug --harts 1 2 4
```

The local System checkout is optional; otherwise the runner clones the provider.
It creates a separate ignored overlay, checks its exact revision and uses the
committed User lock. The overlay reuses the fixed System heap, source, Driver,
packer and checker; it replaces only the main controller with the Hub consumer.
The User image stays release while `--profile` selects the full System profile.
The supplied Kernel profile is independent and must be recorded.

Inputs/hashes and a pointer to exact ELF/archive/lock/serial/JSON evidence are
under `target/`. Failure is not retirement evidence. The provider runner's normal
budgets and behavioral checks remain in force; no missing root exit or timeout
is accepted as success. See its fixed README for the bounded console-loss policy.
