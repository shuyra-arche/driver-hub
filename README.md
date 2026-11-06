# Hub Unit

Hub owns device inventory, claims, execution admission and DMA-binding coordination
for Shuyra. It records ownership and progress; PCI and IOMMU own their hardware effects.

## Layout

- `api/`: device, claim, coordination and DMA contracts.
- `client/`: typed Hub calls.
- `server/`: state machines, service adapters and the `hub` RV64 binary.
- `unit.toml`: runtime policy discovered by Vanllia.
- `declaration/`: compatibility Rust Unit and image declarations.
- `tests/`: independent host workspaces for API, coordination, handoff,
  retention, capability import and Unit-document contracts.

Hub starts after System has completed its internal bootstrap. Its machine inventory
is a System-owned resource. A build dependency on Router does not imply starting
the Router Unit. Binding closure and settlement must not be treated as proof that
an execution CPU or device has stopped; pending obligations retain their ownership.

## Build and test

Install Rust through rustup. The checkout pins nightly-2026-09-04 and the RV64 target.
Run each host suite using its own locked manifest:

```sh
for suite in hub-api hub-coordination handoff retention capability-import unit-document; do
  cargo test --locked --manifest-path "tests/$suite/Cargo.toml" --target x86_64-unknown-linux-gnu || exit
done
CARGO_TARGET_RISCV64GC_UNKNOWN_NONE_ELF_RUSTFLAGS='-Crelocation-model=static -Cforce-frame-pointers=yes -Ccode-model=medium' \
  cargo build --locked --release --target riscv64gc-unknown-none-elf -p hub-server --bin hub
```

Uncached dependencies require network access. Host fixtures exercise state and
protocol behavior; they do not prove real DMA isolation or reclamation.
Use [Vanllia](https://github.com/shuyra-arche/vanllia) for composed runtime validation.

See [source attribution](PROVENANCE.md) and the
[organization contribution guide](https://github.com/shuyra-arche/vanllia/blob/main/CONTRIBUTING.md).
