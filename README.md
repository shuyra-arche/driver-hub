# Hub Unit

Standalone Driver Hub Unit. The repository extends the existing public Hub API
history at `cccef3f325198c8390007ae88b064bae0f21f93e` and contains the Hub API,
typed client, server, declaration, and owned host suites.

## Layout

- `api/` — original canonical `hub-api`; retained bytes and tests.
- `client/` — `hub-client`, using the canonical external Hub API.
- `server/` — `hub-server` library and `hub` RV64 binary.
- `declaration/` — Hub `UNIT` and `BUILD`; BUILD manifest `server/Cargo.toml`.
- `tests/hub-api/` — 9 canonical API tests.
- `tests/hub-coordination/` — 10 canonical coordination tests.
- `tests/handoff/` — 21 Hub runtime assertions.
- `tests/retention/` — 39 Hub runtime assertions.
- `tests/capability-import/` — 5 Hub runtime assertions.

The runtime suites are **65** owned assertions. Together with the 19 canonical
API tests they form **84 distinct cases**; no removed duplicate coordination
suite is counted again. All five test directories are standalone host
workspaces, excluded from the runtime workspace.

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

The Router SDK is a pure prerequisite for Hub; this Unit does not claim that the
Router runtime or client migrated. Driver aggregate intake and Source
`Kernel3aa`/`System9001` real DMA/reclaim acceptance remain pending and
Root-owned.

## Provenance and license

See `PROVENANCE.md` for the frozen Source commit, authors, file map, hash proof,
and source-contract mapping. No license file was present in the source tree at
the frozen commit; this repository does not add or imply a license or copyright
permission.
