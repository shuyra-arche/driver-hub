# Hub machine resource domain

Hub coordinates device Offers, exclusive resource reservations and retained
binding responsibilities. Kernel owns live Pend/IRQ/External authorization and
access retirement. System owns Instance supervision, UnitRoot and generic Supply.
Drivers publish their own services; normal consumer I/O bypasses Hub.

This is the native **v2 provider library** for [Vanllia Track H](https://github.com/shuyra-arche/vanllia/issues/39)
and [Hub #3](https://github.com/shuyra-arche/driver-hub/issues/3). It has no bootable
Hub binary or Unit declaration. The old API and runtime have been replaced, and
the product's pins remain unchanged. Full Hub/System/driver composition has not
been accepted.

## Packages and contracts

- `api/`: bounded Weave v2 frames for Offer/Withdraw, Reserve/Release and Inspect.
- `client/`: generated typed calls with real Twine Clone/one-shot Reply transport.
- `server/`: the production `no_std` Machine, authenticated native request adapter
  and internal PCI mechanism. Package integration tests live under `tests/`.
- [Driver Instance contract](docs/architecture/driver.md): consumption of the
  fixed System Instance v1 subset, operation uncertainty and independent duties.
- [Machine contract](docs/architecture/machine.md): state ownership, identities,
  commitment, recovery, provider evidence and integration gates.

Machine keeps the single resource and Binding commit boundary. Its
`machine/instance.rs` coordinates the Driver stored directly in that Binding;
`driver/native.rs` owns the System SDK calls and reply cleanup. The public
`authority.rs` stays at the Server root as the trusted platform contract. PCI
configuration remains an independent effect mechanism, without a second
resource allocator.

The Server registers these integration-test targets explicitly:

| Target | Entry | Responsibility |
| --- | --- | --- |
| `machine` | `tests/machine/mod.rs` | Reservation, identity and retirement |
| `allocation` | `tests/machine/allocation/mod.rs` | OOM injection in its own executable |
| `driver` | `tests/driver/mod.rs` | Instance recovery and caller isolation |
| `pci` | `tests/pci/mod.rs` | Geometry and partial-effect recovery |
| `service` | `tests/service.rs` | Authenticated ingress and reply routing |

API codec tests remain in `api/tests/`. Shared Host models remain under
`server/tests/support/`. The OOM target retains its own global allocator and
must not be folded into another test executable.

Offers describe up to four complete resources; a reservation covers that exact
device epoch. Descriptions, addresses and Pend numbers never confer authority.
The trusted `Authority` must validate genuinely imported Hub-local Pends and
recheck live authorization at the effect boundary. There is no permissive default
or native hardware Authority implementation. Recursive subprovider import awaits
a validated derivation contract. IRQ and External Offers currently return
`Unsupported`, including any claimed boot-admitted source.

Mutations serialize through Machine and retain requester-scoped operation intent
and results. Duplicate requests cannot create a second reservation; Inspect and
replay resolve a lost reply. New operations need increasing, nonzero IDs; Busy or
other rejected attempts remain recorded and need a new operation ID to retry.
History is bounded and never evicted: exhaustion rejects new operations, while
existing queries, replay and retirement can continue.

Preparation records retirement duties before effects. Cancellation, failure and
withdrawal preserve reservations until the exact owners establish device,
authorization and Instance retirement. System #19 now supplies real InstanceId
and the caller-owned Instance v1 contract; the Hub consumer uses that fixed
subset. Native resource Supply and restart takeover remain separate work. On Hub death, the
integration owner must retain obligations or quarantine resources until real
retirement; a new incarnation ID alone is insufficient.

The internal PCI module checks mapped ECAM bounds, probes/restores endpoint BARs,
plans aligned windows, journals programming before partial writes and disables
Bus Master with readback. It does not allocate resources, grant BAR access,
enable decoding/Bus Master or establish device quiescence. Its unsafe platform
boundary requires real exclusive configuration authority, MMIO ordering and
already quiescent ownership. The protected ECAM handoff to a runtime Hub remains
unimplemented; no PCI/Router/IOMMU Unit or repository is retired by this change.

## Build and verify

Install Rust through rustup. The checkout pins `nightly-2026-11-13`, Rust source,
rustfmt, Clippy and `riscv64gc-unknown-none-elf`. Git dependencies use immutable
revisions and one Kernel ABI/Ledger 11/11 identity; commit `Cargo.lock`.

```sh
cargo fmt --all -- --check
cargo test --workspace --locked --target x86_64-unknown-linux-gnu
cargo test --locked -p hub-server --test allocation --target x86_64-unknown-linux-gnu
cargo clippy --workspace --all-targets --locked --target x86_64-unknown-linux-gnu -- -D warnings
cargo metadata --locked --format-version 1
cargo tree --workspace --locked --edges normal,build --target riscv64gc-unknown-none-elf

CARGO_TARGET_RISCV64GC_UNKNOWN_NONE_ELF_RUSTFLAGS='-Crelocation-model=static -Cforce-frame-pointers=yes -Ccode-model=medium -Dwarnings' \
  cargo build --workspace --locked --target riscv64gc-unknown-none-elf
CARGO_TARGET_RISCV64GC_UNKNOWN_NONE_ELF_RUSTFLAGS='-Crelocation-model=static -Cforce-frame-pointers=yes -Ccode-model=medium -Dwarnings' \
  cargo build --workspace --locked --release --target riscv64gc-unknown-none-elf
```

Host tests exercise the real production crates: exclusive claims, canonical
resource conflicts, stale epochs, source-scoped replay, rejected/uncertain
requests, cancellation, retained retirement, allocation failure, exact codecs,
reply routing and partial PCI effects. Scripted Authority/transport/configuration
models do not prove real Pend delivery, hardware isolation or MMIO ordering.
RV64 library compilation proves neither a runnable Unit nor QEMU execution.
Vanllia owns the separate one/multi-Hart QEMU and hardware acceptance gates.

The [native Instance consumer](tests/target/handoff/README.md) runs Hub's
production Driver/Native coordinator against the fixed complete System in QEMU.
It checks two Driver Teams, real CPU Sign supply, UnitRoot publication, direct
RPC, replacement and physical Instance retirement. It is a CPU-only consumer
scenario, not full Hub startup or device retirement acceptance.

See [CI execution and cache boundaries](docs/ci.md) for PR change selection,
parallel handoff profiles and evidence retention. Cache hits still execute the
locked build and every selected VM; caches contain no prior VM results.

This is an incompatible 0.x API replacement. Consumers must migrate after the
provider contract is reviewed and fixed; no legacy forwarding layer is supplied.
See [source attribution](PROVENANCE.md) and the
[organization contribution guide](https://github.com/shuyra-arche/.github/blob/main/CONTRIBUTING.md).
