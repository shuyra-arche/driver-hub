# Hub Unit

Standalone Driver Hub Unit. The repository extends the existing public Hub API
history at `cccef3f325198c8390007ae88b064bae0f21f93e` and contains the Hub API,
typed client, server, declaration, and owned host suites.

The current Unit input is the repository-owned [`unit.toml`](unit.toml), using
`unit::document::Unit` schema 1. It lives at the root because the declaration
belongs to the complete Hub Unit, alongside its API, server, and compatibility
declaration; it is not a Cluster deployment or a test scenario. Its stable id
and loader image are both `hub`.

## Layout

- `api/` — original canonical `hub-api`; retained bytes and tests.
- `client/` — `hub-client`, using the canonical external Hub API.
- `server/` — `hub-server` library and `hub` RV64 binary.
- `unit.toml` — current Hub Unit policy, compiled by the host to a binary record.
- `declaration/` — legacy Hub `UNIT` and `BUILD`, retained for existing callers
  and field-by-field migration checks; BUILD manifest `server/Cargo.toml`.
- `tests/unit-document/` — strict TOML/native-policy comparison, complete binary
  roundtrip, full-width mark checks, and Cargo binary/build-contract checks.
- `tests/hub-api/` — 9 canonical API tests.
- `tests/hub-coordination/` — 10 canonical coordination tests.
- `tests/handoff/` — 21 Hub runtime assertions.
- `tests/retention/` — 39 Hub runtime assertions.
- `tests/capability-import/` — 5 Hub runtime assertions.

The existing runtime suites are **65** owned assertions. Together with the 19
canonical API tests they form **84 distinct cases**; no removed duplicate
coordination suite is counted again. The new declaration suite has four
additional tests. All test directories are standalone host workspaces, excluded
from the runtime workspace; the existing scenarios have not been migrated.

## Unit lifecycle and policy

`creation = "startup"` creates Hub when the selected deployment starts.
`requires` selects the necessary Unit package closure; `after` waits for selected
instances to become ready. Inclusion through `requires` does not start every
included Unit: each declaration's `creation = "startup" | "requested"` decides
whether an instance starts with the deployment or waits for a creation request.
Both dependency lists are explicitly empty here: the old declaration
waited only for Operator and Identity bootstrap inside System. System must
complete its own bootstrap before creating Hub; these services are not separate
Unit ids. `system/initrd` supplies the machine inventory as a System-owned
resource. The Router SDK is a build dependency and does not imply a dependency
on the Router runtime Unit.

The ordered namespaces preserve service `hub/{bond,list,claim,retire}`, the
device subtree, and the DMA service capability. Identity keeps Found, Admit,
Expel, and Activate; startup readiness uses `hub-ready`, ending stays resident,
and retirement calls `hub/retire` in namespace 0. Construction, aliases, and
publication bindings remain empty; supervision is absent. Marks and authority
codes are checked against the actual Rust provider constants. Full-width marks
use quoted decimal strings; schema 1 also accepts quoted `0x` hexadecimal and
keeps native `u64` values in the binary record.

The host discovers the real Cargo binary at `server/Cargo.toml`: package
`hub-server`, bin/image `hub`, entry `server/src/main.rs`, target
`riscv64gc-unknown-none-elf`, release profile, default features enabled, and no
extra features. Build metadata is separate from the Unit policy. A deployment
is assembled in memory from selected declarations and dependencies for each
host invocation; this repository does not add a deployment file or a registry
of Units.

## Public identities

Runtime packages depend on the canonical external Hub API, never a copied local
substitute:

| Component | Repository | Revision |
| --- | --- | --- |
| `hub-api` | `https://github.com/shuyra-arche/hub.git` | `cccef3f325198c8390007ae88b064bae0f21f93e` |
| `driver-support` | `https://github.com/shuyra-arche/driver.git` | `8e6de5a77aa52184bb6f7fbbcbc961f779e94629` |
| `router-api` | `https://github.com/shuyra-arche/router.git` | `94fec223f3c5df6c99a49527a65d016d88de3af4` |
| `env` | `https://github.com/shuyra-arche/env.git` | `d3000253cd5d13d466e91fafd4dbb27a5ec5aaae` |
| `wire` | `https://github.com/shuyra-arche/wire.git` | `0ff77dc4573fa2620d2fe995a18c01949d9c7e57` |
| `system-api` | `https://github.com/shuyra-arche/system.git` | `9e5d63ca536a176583cf1b0054b4ecf7ddd88257` |
| `system-client` | `https://github.com/shuyra-arche/system.git` | `5a792ff2d39901a970925b1a6d63206adf6c16d8` |

## Build

```sh
cargo build --locked --target x86_64-unknown-linux-gnu --workspace --lib
cargo build --locked --target riscv64gc-unknown-none-elf --lib
cargo build --locked --release --target riscv64gc-unknown-none-elf --bin hub
```

The declaration suite uses public Unit revision
`ad50b803c3bd5cd2ef90010d80badf23551e88d5` for the document and binary record
APIs, with its own committed `tests/unit-document/Cargo.lock`. The `legacy-unit`
alias keeps revision `90396f0fda376e0693fb1b9553aa21864d241057` for comparing
the existing Rust declaration's native enums. Production dependency pins and the
repository lock stay on their existing revisions. No local Cargo patch is needed:

```sh
source /workspace/.vanllia-tools/env.sh
cargo test --locked --manifest-path tests/unit-document/Cargo.toml \
  --target-dir target/unit-document/build --target x86_64-unknown-linux-gnu
```

These host checks cover policy and binary encoding; QEMU and Source integration
acceptance remain separate work.

The Router SDK is a pure prerequisite for Hub; this Unit does not claim that the
Router runtime or client migrated. Driver aggregate intake and Source
`Kernel3aa`/`System9001` real DMA/reclaim acceptance remain pending and
Root-owned.

## Provenance and license

See `PROVENANCE.md` for the frozen Source commit, authors, file map, hash proof,
and source-contract mapping. No license file was present in the source tree at
the frozen commit; this repository does not add or imply a license or copyright
permission.
