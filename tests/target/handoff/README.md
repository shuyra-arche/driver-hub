# Native Instance consumer

This scenario injects Hub's production Machine/Signal Authority/Driver
consumer into the ordinary
controller User image of System's fixed `332d9d2e15f6bc4e1e8d94e4db2591b1a53ae613`
handoff fixture. The complete production System runs unchanged, with its matching Kernel
`789ffd19b5359f3de172fb9aad8a61f6a7113a64` and Twine
`932454c07bd2673ac7d6d9612d36182aae71e8b2`. Ordinary Tasks follow the
ABI 13 User/ECALL startup contract. The guest asserts ABI/Ledger/Capsule
13/13/2, then exercises actual Caller, Pend, RPC and retirement calls. The controller
uses the real supplied Instance endpoint, a caller-wide operation sequence,
System-issued InstanceIds and native reply cleanup. The trusted controller
declaration also supplies the public System Supply endpoint at mark 8; Driver
declarations contain no device binding, so grants are installed after Prepared.

Three Driver Teams are prepared: one is closed before Start after its imported
source is revoked, and two are started with two independently imported Signal
slots each. Machine owns the reservation, Driver coordinator and each lease.
Actual CPU Sign delivery, ready
UnitRoots, publication CAS, direct consumer RPC, live replacement, withdrawal,
Stop admission versus Retire completion, and independent Team/root-exit evidence
are checked. Hub's native marker and the provider's typed lifecycle checker must
both pass. A new Hub Machine marker is also required in every fresh log.
Native Authority inspects actual self-Accorded imports in the controller Task,
checks the immediate lender and object owner separately, and rejects unsupported
hardware kinds. `prepare_system_supply` selects its actual target from the bound
System Prepared observation; Start requires every slot delivered.
Hub's production `supply::Supply` derives a private local offering root, really
Accords a distributable child to System, registers that System-local Pend, then
installs a non-distributable TRIGGER Pend into the actual Prepared Driver Task.
The Driver checks the immediate lender is System. Repeated preparation preserves
the same IDs/token. A test-only wrapper discards one authenticated successful
Install reply after the real native call; original-ID InspectInstall recovers
it without a second installation. It never synthesizes successful receipts.
Cancellation before effects closes locally; accepted work requires the exact
Released installation, Closed source and native local-root retirement.
An imported ancestor withdrawal while Prepared retains responsibility until
all branches retire. Final withdrawal occurs while the running Driver is still
alive and its next native trigger is denied. Each owner is explicitly finished,
independently of Instance retirement. Direct Lease cancellation and ancestor
revocation checks remain as separate native regressions.
Exact owned completion clears only its slot Authorization bit; completed System
retirement clears only Instance bits. Device bits remain retained (9 for two
slots, 1 for the failure case), fake Device receipts are Unsupported and rebinding
remains Busy. A mandatory `hub-system-supply` marker records registration,
Prepared installation, lost reply recovery, cancellation, withdrawal and the
actual System lender.

Hub production code uses only
public Kernel/System API and Client packages.

This selects the provider's main `handoff` scenario. Its other auth/cancel/death
VMs are not Hub-consumer results. Hub's Host suite separately checks uncertain
requests, cancellation, wrong identities and failed retirement. This scenario
does not boot a standalone Hub Unit, validate PCI/IRQ/External effects, prove device
quiescence or accept Toolchain std Units.

Use the declared toolchain, QEMU 10.0.13 and OpenSBI 1.6. Set `QEMU_RISCV64` and
`QEMU_OPENSBI` to their actual paths. Build System's instruction-qualified runtime
platform at that same revision, then run:

```sh
python3 tests/target/handoff/run.py /path/to/system-runtime-platform --system-checkout /path/to/fixed-system --profile release --harts 1 2 4
python3 tests/target/handoff/run.py /path/to/system-runtime-platform --system-checkout /path/to/fixed-system --profile debug --harts 1 2 4
```

The local System checkout is optional; otherwise the runner clones the provider.
It creates an ignored `target/system-<full-System-SHA>/` overlay, preserving
older revision evidence; checks its exact revision; and uses the
committed User lock. The overlay reuses the fixed System heap, Driver,
packer and checker; it adds only the controller's Supply endpoint declaration, replaces the main controller with the Hub consumer, asserts the System lender and strengthens the
Driver withdrawal probe to require native Sign Denied, rather than any non-Busy
error.
The provider runner is unchanged: its public `--scenarios handoff` selects
this scenario, and `--kernel-profile` records the actual selected profile and
uses the provider's corresponding execution bounds. Existing CI uses release
Kernel (60/90-second role/VM bounds) in both System profiles.
The provider main file still includes its private Source for unselected provider
roles, but the selected Hub controller does not call that implementation.
The User image stays release while `--profile` selects the full System profile.
The supplied Kernel profile is independent; pass `--kernel-profile debug`
when selecting an actually built debug platform. A profile argument does not
qualify an arbitrary Kernel binary.

Inputs/hashes and a pointer to exact ELF/archive/lock/serial/JSON evidence are
under `target/`. Failure is not retirement evidence. The provider runner's normal
budgets and behavioral checks remain in force; no missing root exit or timeout
is accepted as success. See its fixed README for the bounded console-loss policy.
