# Provenance

## Source checkpoint

- Repository: `https://github.com/anreonyr/sqware.git`
- Public pull request: **#43**
- Frozen source HEAD: `89ecffd0f8ce2077cd757d9561be141c87b8ec3b`
- Frozen commit subject: `Share allocator mechanisms through a Kernel-owned package`
- Commit author: `anreonyr <anreonyr@gmail.com>`
- API subtree: `programs/src/service/hub/api`
- Test subtrees: `programs/tests/hub-api`, `programs/tests/hub-coordination`
- Toolchain copied from source: `nightly-2026-09-04`, edition 2024, resolver 3.

The source working tree was clean at this commit before and after this
snapshot. No source file was written by this preparation.

## Authors

Git history for the API subtree records these author identities:

- `anreonyr <anreonyr@gmail.com>`
- `anreonyr <88045926+anreonyr@users.noreply.github.com>`

The original history remains at `anreonyr/sqware`; this repository is a
fresh snapshot, not a history rewrite.

## License

No `LICENSE`, `COPYING`, or `NOTICE` file is tracked at the frozen source
commit. The source `Cargo.toml` declares neither `authors` nor `license`.
This snapshot therefore adds no license and asserts no copyright permission.
Do not infer a license from publication or from this document.

## API move

The source API is 19 Rust files. This provider has 34 Rust files: 13 source
files are byte-identical, and 6 source files were mechanically split into
one-type-per-file children with public re-exports preserving every original
public path. One item, `frame::dma::DmaSaid`, remains in its original
`dma_said.rs`.

Mechanical proof is recorded in
`evidence/pr21/provider/move-proof.json` (outside this repository):

- 13 / 13 unchanged Rust files have equal SHA-256 source and provider bytes.
- 18 / 18 moved item blocks occur byte-for-byte in their target files.
- 19 source Rust files map to 34 provider Rust files with no unaccounted file.

`api/Cargo.toml` is byte-identical to the source manifest. The two test
manifests differ only in the `hub-api` path (`../../src/service/hub/api` to
`../../api`).

### Split files

| Source file | Provider files | Source SHA-256 | Provider SHA-256 |
| --- | --- | --- | --- |
| `src/frame/mod.rs` | `frame/mod.rs`, `frame/{wire,said,deed,window}.rs`; Message impls appended to `frame/{bond,list,claim}.rs` | `382c692a6314289f31b3474641626fcfdfc31f49c513fc0d127533694272d993` | `5b2c58bacd026b470ee1bb352d25d00ce40bf8a07093285885e27f596bdf6f59` |
| `src/frame/inventory.rs` | `frame/inventory.rs`, `frame/inventory/snapshot.rs` | `fa0606319410687694f8926eea38540a0dce66b24ace0644b080b4a9ef86329c` | `84b44d862bf7f97fdffd37515c74d4e2f62ac0d3ff9ce50fdcc39aa908b6951b` |
| `src/frame/dma.rs` | `frame/dma.rs`, ten `frame/dma/*.rs` children | `2e29a5853aa906e75d23f1d7214a17d6c3e8586c5312dc29ffe3586dab3aeba2` | `89bc91e3420c6c1bce0c46df2d9380085042bf1247f16d6c12aaf198aa97cb0f` |
| `src/frame/bond.rs` | `frame/bond.rs` + `impl Message for Bond` | `772bc351be52f3777c56ee82252d0884fafe5edaa89017ad465d062928a76d79` | `8c134d0f028a1fe189501039ee045f27f4b5c9dd3442e5167cff72311fbe6fe8` |
| `src/frame/list.rs` | `frame/list.rs` + `impl Message for ListReq` | `ef568d67a1b8b8cddbc1f9ed4f54e7611fb2a33b3595e298c3ccb6e77056ab8a` | `761fc266a00c2b98c200a5845ff496a907d93ca4b06c7c4745a9d96c2d334371` |
| `src/frame/claim.rs` | `frame/claim.rs` + `impl Message for Claim` | `c5743dbcddef38be94991db20cb1f6f1fdd327a5b7e463ef4f27e68ea2fbbae9` | `2b05a8420ec3f7631fc8c95ac81ad332da0432a223bab55b1f7757fb2938c34f` |

Item-level moves inside the split files:

| Source item / range | Provider target |
| --- | --- |
| `frame/mod.rs:33-47` `impl Message for Bond` | `frame/bond.rs` |
| `frame/mod.rs:49-63` `impl Message for ListReq` | `frame/list.rs` |
| `frame/mod.rs:65-81` `impl Message for Claim` | `frame/claim.rs` |
| `frame/mod.rs:83-124` `Wire` + impl | `frame/wire.rs` |
| `frame/mod.rs:126-155` `Said` + impl | `frame/said.rs` |
| `frame/mod.rs:157-213` `Deed` + impl | `frame/deed.rs` |
| `frame/mod.rs:215-275` `Window`, `WINDOW_LEN`, impl | `frame/window.rs` |
| `frame/inventory.rs:35-135` `Inventory` + impl | `frame/inventory/snapshot.rs` |
| `frame/dma.rs:21-27` `BindingId` | `frame/dma/binding_id.rs` |
| `frame/dma.rs:29-61` `BindingState` + impl | `frame/dma/binding_state.rs` |
| `frame/dma.rs:63-85` `Close` + impl | `frame/dma/close.rs` |
| `frame/dma.rs:87-93` `Failure` | `frame/dma/failure.rs` |
| `frame/dma.rs:102-112` `Enroll` | `frame/dma/enroll.rs` |
| `frame/dma.rs:117-129` `DmaClaim` | `frame/dma/dma_claim.rs` |
| `frame/dma.rs:131-139` `CloseBinding` | `frame/dma/close_binding.rs` |
| `frame/dma.rs:141-148` `Inspect` | `frame/dma/inspect.rs` |
| `frame/dma.rs:150-156` `Settle` | `frame/dma/settle.rs` |
| `frame/dma.rs:158-221` `DmaWire` + impl | `frame/dma/dma_wire.rs` |

The original `frame::dma.rs` keeps `Call`, the operation and party constants,
`DMA_BACK`, and the private `reply_to`, so the `mold::contract` expansion scope
and `frame::dma::Call` path are unchanged. `frame/mod.rs` re-exports
`frame::dma::Call as DmaCall`.

`frame/inventory.rs` keeps `InventoryItem`, `INVENTORY_MAX`, the private
constants, and `inventory_slot`. `InventoryItem::EMPTY` stays private inside
the parent module; the child `snapshot` module can read it without widening
visibility.

### Unchanged files and hashes

| File | SHA-256 (source = provider) |
| --- | --- |
| `src/coord.rs` | `d79ef21ddda4ac7bd9a717a376cd67e14c912a7fae66086379763dd30cc37133` |
| `src/coordination/call.rs` | `df99f28c1dbbbf79ea985389f8052ac1e753072188610c1d7107bc4a9973aa7c` |
| `src/coordination/mod.rs` | `8e306132963ca900b90c4d6305c9fc14c812eb2fa82d7d1b90a70e3e51b8a71f` |
| `src/coordination/prepare.rs` | `58734cf3b19064f68add50a8965a722f3663abfd1ceac3ac564782bc201e37e7` |
| `src/coordination/release.rs` | `2dcfe379b7d1cbb747b4ef896a51a441ae1ae74cab1ae2516f894986d451f793` |
| `src/coordination/wire.rs` | `05487d3ab5062abe4af04463005a3bff42b9d9cc4f2578a54479afa3fad6d116` |
| `src/declaration.rs` | `c556301963d5ab9b0d3ac2c58c3ba323b14ae3a7aee56af551d39cb303eb5c62` |
| `src/device_id.rs` | `3b5a727903524587571c5841b3cef3c34887ab92a16547f019e8ec47fa6215d5` |
| `src/frame/dma_said.rs` | `f2eb9dcb49f107700ba9adc476aee1562f13ce80bdb9f8e14332e9e0ad568f71` |
| `src/frame/vocab.rs` | `6442dcf76751fd0e04d24d6b33c7521ffce2a1acd4009457735c3e116598bc15` |
| `src/grant.rs` | `47eca29278fed3775531e942f659fb2f5069ba0810b0b6f2b93651d05bff377a` |
| `src/lib.rs` | `0f88b19e5ad71f08b9009fb50972a1614674e2e826e2d5d0d036ad54211f12ad` |
| `src/marks.rs` | `0623d4445617bf007b8bc4c6714529d7ba259f9e442aa8f9669f0e2886a9d820` |

`api/src/lib.rs` remains byte-identical, including the single inline
`mold::interface` declaration for `Channel` and `Grant`. The generated
`code_to_fail` / `fail_to_code` in `frame/vocab.rs` also remain unchanged.

## Test provenance

The 14 original test functions are copied without body changes:

| Original file | Provider file | SHA-256 |
| --- | --- | --- |
| `programs/tests/hub-api/lib.rs` | `tests/hub-api/lib.rs` | `d7b1d2b46222d36ec0837cab404240568001f923be9717bd65f94c446006378f` |
| `programs/tests/hub-coordination/lib.rs` | `tests/hub-coordination/lib.rs` | `808f6705c8ce099827824995370e20fc38575fc8cd805d1d10a22e0cf5512dd4` |
| `programs/tests/hub-coordination/tests.rs` | `tests/hub-coordination/tests.rs` | `df3d8594f8018d065afb06cb327057229b1db096250ca6b5d18745590fedf304` |
| `programs/tests/hub-coordination/device_key.rs` | `tests/hub-coordination/device_key.rs` | `538fa04ff9bdb315a158cc9fb5f40dc31c4f75eb0743635a40e06ce558fd7c89` |

Four source-owned system-shape assertions were migrated into
`tests/hub-api/tests/source_contract.rs`. The remaining system-shape
composition/server assertions stay in the source tree and are not part of
this provider.

| Source assertion | Provider case | Intended check |
| --- | --- | --- |
| `system-shape/lib.rs:209` | `coordination_api_has_no_iova_ownership` | `api/src/coord.rs` contains no `IOVA`. |
| `system-shape/lib.rs:255` | `dma_claim_has_no_requester_trust_shape` | `DmaClaim` (now `frame/dma/dma_claim.rs`) and the whole `frame/dma` module have no `pub requester: u32,\n    pub sensor` shape. |
| `system-shape/lib.rs:575` | `hub_api_dependencies_stay_env_wire_mold_system_api` | `api/Cargo.toml` dependencies are only `env`, `wire`, `mold`, `system-api`. |
| `system-shape/lib.rs:710` | `hub_api_generates_interface_metadata` | `api/src/lib.rs` contains `mold::interface`. |

`tests/hub-api/tests/public_api.rs` is an additional compile-only
compatibility case. It names the original root, `frame`, `frame::dma`, and
`frame::inventory` paths, including `DmaCall`, `frame::dma::Call`,
`WINDOW_LEN`, `Call`, and the inventory exports, without constructing any
runtime or hardware state.

Test count: 14 legacy + 4 migrated source-contract cases + 1 compatibility
case = 19. The API test workspace runs 9 and the coordination test workspace runs 10.

## Lockfiles and verification

All three `Cargo.lock` files (`Cargo.lock`, `tests/hub-api/Cargo.lock`,
`tests/hub-coordination/Cargo.lock`) were generated by Cargo from the public
Git pins and then exercised with `--locked` and `--locked --offline`. No
source override, vendoring, path replacement, or hand-written lock entry was
used.

Verification artifacts are stored outside this repository at
`evidence/pr21/provider/`, including command lines, working directories,
exit codes, logs, metadata, and `move-proof.json`. This checkpoint is not a
commit, publication, Kernel-independence result, or Stage 3/4 completion at the worker
handoff. Root performs reviewed publication after sealing that checkpoint.


# Additive Hub Unit server/client/declaration and runtime-test extraction

The preceding canonical API provenance is preserved verbatim. This new Unit
slice extends the existing Hub repository history from
`cccef3f325198c8390007ae88b064bae0f21f93e`; all api/ and original19 canonical
API-test bytes remain unchanged. Runtime/client/declaration dependencies retain
that public canonical API revision, with no new API identity entering their
resolve closure. Source/vanllia is not a provider dependency.

Root copied the reviewed Driver8e6 Hub implementation/test checkpoint exactly;
the genuine worker adapts repository boundaries only. Hub owns declaration,
client, server and65 runtime cases (handoff21, retention39, capability-import5).
Existing hub-api9 and hub-coordination10 are reused unchanged, giving84 distinct
host cases. A copied coordination duplicate was removed only after exact snapshot
hash comparison; no original test/assertion was deleted.

Shared Peer is the single public driver-support package from Driver
`8e6de5a77aa52184bb6f7fbbcbc961f779e94629` with unchanged System Face5a.
This explicit bootstrap support revision avoids a release cycle. Pure Router API
comes from its separately published Unit revision
`94fec223f3c5df6c99a49527a65d016d88de3af4` (five Source API files unchanged;
Router runtime/client remain Source-pending). No System-peer publication,
private backlink, local overlay or unrelated SDK revision cascade is used.

Hub production143 function signatures/bodies, field order, Peers/_state/reclaim,
League, DTB hold, owner/mark checks, deadlines, fence/claim/close/inspect/settle/
reclaim ordering, capacities, errors and final-reclamation criteria retain their
original contract. The65 original runtime tests,145 fixture/test functions,
19 fixture field bodies and14 local production projections remain. The Unit
BUILD uses server/Cargo.toml, hub-server/hub, unchanged features/target/profile.
Public Mold/Execution provide entry/linker/allocator/panic; Root verifies actual
RV artifacts and unique providers rather than assuming them from static code.

Driver aggregate and Source composition/selected Deployment integration follow
only after public Unit proof. Mixed device-recovery/system-shape remain Source
for this slice. Host resource/transport mocks do not prove Kernel DMA reclaim;
actual Kernel3aa/System9001 SMP4/8 acceptance is a later gate. Earlier Driver
monorepo passes are historical, not new-layout runtime proof. No full Driver or
Router migration is claimed. No license is invented; Source headers and history
are retained.

## Original Source file hashes and attribution

All rows identify original pre-adaptation Source
https://github.com/anreonyr/sqware.git at
`293c15fc97a9d9a4295810d10fe804e4cdec63c4`.

- Serial Device <anreonyr@gmail.com>
- anreonyr <88045926+anreonyr@users.noreply.github.com>
- anreonyr <anreonyr@gmail.com>

| Source path | Initial Unit ownership | Original SHA-256 |
| --- | --- | --- |
| `programs/src/service/hub/client/Cargo.toml` | `client/Cargo.toml` | `827d1a0b802189e8e7ac5e56f1bca89a02865b6c7f5d45f5d64d496d4e3bd7e3` |
| `programs/src/service/hub/client/src/activation_client.rs` | `client/src/activation_client.rs` | `461dcf49a85427041aa010a874f62fb476ce317903b494c4392128bfa4c10b7b` |
| `programs/src/service/hub/client/src/client.rs` | `client/src/client.rs` | `a7672dc4759296f427aa02ad0c91d91945d62d3e96be531b396802ababa19977` |
| `programs/src/service/hub/client/src/debug.rs` | `client/src/debug.rs` | `229562fb0a5f4046cabf6dac363f936d583c41b7e659a54799a6547d7f194244` |
| `programs/src/service/hub/client/src/lib.rs` | `client/src/lib.rs` | `e7aab0012673911cc638e2a7cd007da7d70a41478f3c5f873a11cf26089ab0f1` |
| `programs/src/service/hub/mod.rs` | `server/src/lib.rs` | `57f0f357116087f8660583b7c6ab5226ace2e96e25b7ee0b1e0adc3a91df259a` |
| `programs/src/service/hub/fault.rs` | `server/src/fault.rs` | `72eb7363ebc19983f68a9e90f8584c8cdb5413da63720ea19f4f9fb3a659292f` |
| `programs/src/service/hub/core/admission.rs` | `server/src/core/admission.rs` | `13f33d5cc8dd677fbbe7f86994e9a4c7edcc972a1687a6adeee81326399be00a` |
| `programs/src/service/hub/core/admissions.rs` | `server/src/core/admissions.rs` | `1a42904083eb28f614fd60c563ed665c3190e5f0a61d7871412f2177a88ef632` |
| `programs/src/service/hub/core/binding.rs` | `server/src/core/binding.rs` | `89d5b3792aa9fb9491cd726c61c954e558fb5cea22defc0efa961ed76679f38b` |
| `programs/src/service/hub/core/bindings.rs` | `server/src/core/bindings.rs` | `cf366bc732f972b42deb711a9d904be253e5962824f566e2628b1e3f7e031056` |
| `programs/src/service/hub/core/cell.rs` | `server/src/core/cell.rs` | `12d4d37109719cae9c8384025c965d49932f70feebd9e106b327c9532ecfb9ef` |
| `programs/src/service/hub/core/claim_request.rs` | `server/src/core/claim_request.rs` | `e735a840302804c57e3cbb75a789caa689cf2db60fdda2b14252291849ff8435` |
| `programs/src/service/hub/core/enrollment.rs` | `server/src/core/enrollment.rs` | `df452248fde587012343ec4d7fe5585be9247d8055bac5cc6b7141b70884cb7e` |
| `programs/src/service/hub/core/enrollment_effects.rs` | `server/src/core/enrollment_effects.rs` | `e906dd4a56433100606defd5eadc6ee890bb1ef7087ec79a6c7631c269f69a13` |
| `programs/src/service/hub/core/enrollment_step.rs` | `server/src/core/enrollment_step.rs` | `8876f9399389f23ad96380738ef5a68f074834cfe9f6c91fd3df1052916488bb` |
| `programs/src/service/hub/core/enrollments.rs` | `server/src/core/enrollments.rs` | `bf527f2248b03e9ceebd9f7b1e8792bf331cd7520f0d1213e41e196d0729dff4` |
| `programs/src/service/hub/core/entry.rs` | `server/src/core/entry.rs` | `0044177f836b44d31cc880239a6c5b15034d78b4c63663d79b668f87ad60a22c` |
| `programs/src/service/hub/core/league.rs` | `server/src/core/league.rs` | `191f324ddd398faf4cfbf9fb1bd32f61cd1b6fff978ba003e942319f35da1d7d` |
| `programs/src/service/hub/core/league_step.rs` | `server/src/core/league_step.rs` | `1bc325ff4ea7d00662750885742014d2c1308422eac0e123ec6f1e2fdfe5d388` |
| `programs/src/service/hub/core/leagues.rs` | `server/src/core/leagues.rs` | `33e0f22486f84cca76d918b63c0fa31be3769260065b5ad8423657a3dc5aee34` |
| `programs/src/service/hub/core/ledger.rs` | `server/src/core/ledger.rs` | `6af366b62761f014c7407d1602d773186bfe29373ac6109d554a17d76fdc07bd` |
| `programs/src/service/hub/core/mod.rs` | `server/src/core/mod.rs` | `411409070324492c6549342a2fc5395a210d32fe54f82d7cf90367117f8f919b` |
| `programs/src/service/hub/core/owner.rs` | `server/src/core/owner.rs` | `35a83bbddc20c0cc99f08b564a4078f2270f08863a0582046d60258fb1dbbe30` |
| `programs/src/service/hub/serve/binding_rpc.rs` | `server/src/serve/binding_rpc.rs` | `441887fb2d956f96da901bfbe454ad18148f08b48d17c15aa8de3b5148f94dde` |
| `programs/src/service/hub/serve/bond.rs` | `server/src/serve/bond.rs` | `d337f763b5dafc3a97f4033c00192a79ce1988221b17e98d023bdb2ba61183e4` |
| `programs/src/service/hub/serve/claim.rs` | `server/src/serve/claim.rs` | `7f28c012a8a08d889de8edb2f7352827cdd25e57ed7306d469555f0d6b5071aa` |
| `programs/src/service/hub/serve/coordination.rs` | `server/src/serve/coordination.rs` | `7aa87bc54c90f4bf4c86f3e1efae2f6ed5eb4f78d43d95a0e9f1d463633ddfa7` |
| `programs/src/service/hub/serve/directory.rs` | `server/src/serve/directory.rs` | `e2bc7a162850e205ad53d0e828e6f11516352c3fa92f6ec8ffde9e28ca85dbc9` |
| `programs/src/service/hub/serve/dma.rs` | `server/src/serve/dma.rs` | `65aaefccdcdcb803a6e7b575939266e06ad3f55e617e295e6f01265522b74e3c` |
| `programs/src/service/hub/serve/enrollment_failure.rs` | `server/src/serve/enrollment_failure.rs` | `96b8d247d28b85cef022cb9bc6ea6b1fc7e482da443e468a5e9e0b0b423c1325` |
| `programs/src/service/hub/serve/execution.rs` | `server/src/serve/execution.rs` | `1378e33b524098463ab16d1f310d196d60e7f86c349d6c866d7ee152c475cdd1` |
| `programs/src/service/hub/serve/grant.rs` | `server/src/serve/grant.rs` | `fe8d406f54d78b2ad770782a10a2849b7c10e7bc3630b5073d918911f51213a9` |
| `programs/src/service/hub/serve/launch.rs` | `server/src/serve/launch.rs` | `0064512f7b2f9a1a4fac51845c54adce10c019b51050263edada716a69031576` |
| `programs/src/service/hub/serve/launch_phase.rs` | `server/src/serve/launch_phase.rs` | `f092d061a8d76519e6c054bcd120c17c83dc88df140b187284ba0708bd92a402` |
| `programs/src/service/hub/serve/launch_reply.rs` | `server/src/serve/launch_reply.rs` | `72bd08d1d4364f455186b92797ff09a418c5298723ca760e683e9b01ab22702a` |
| `programs/src/service/hub/serve/list.rs` | `server/src/serve/list.rs` | `884d405a90a9b5186fcb3bc2752a2385edda80d1244e2a82ec8f296b7ccc7ec7` |
| `programs/src/service/hub/serve/mod.rs` | `server/src/serve/mod.rs` | `280df984c78fb13e7f06e165d28e93e327f70db8c153c6e77bc2a6fa9c882c51` |
| `programs/src/service/hub/serve/peers.rs` | `server/src/serve/peers.rs` | `7b19472a0c7d3249911f43dea193cf534d46a80ab86d4fa343980ede882d9db2` |
| `programs/src/service/hub/serve/return_reply.rs` | `server/src/serve/return_reply.rs` | `47599b9abc49485d5280e25b32a536deaf1616cc4404fb491eb99f281b94b6ec` |
| `programs/src/service/hub/serve/returned.rs` | `server/src/serve/returned.rs` | `5e45efc9cd956f07b04965c66034fe9774c469a3142102619a394941648a5b08` |
| `programs/src/service/hub/serve/sweep.rs` | `server/src/serve/sweep.rs` | `22e8b2df4f71dddda98624fb05060b752b06af79f06172f4098d77ae0db4726d` |
| `programs/src/service/hub/main.rs` | `server/src/main.rs` | `09f351ca1bb9bbf36856774ad081ba2cf3c1288669c3d30524e0282c1c7a6b16` |
| `crates/composition/src/units/hub.rs` | `declaration/src/lib.rs` | `969e3858d66c7297b6fbfa4b1f0c262a4fc59c58e838c0dcf04f076e4fa9ae74` |
| `programs/tests/hub-handoff/Cargo.lock` | `tests/handoff/Cargo.lock` | `37d3283b7c05c3a9ba778dd9a91fd66172f5bf193e2dae87f1a9fd7db86221e9` |
| `programs/tests/hub-handoff/Cargo.toml` | `tests/handoff/Cargo.toml` | `714d82bb51823c4378f0714dba7f007fe5436036acddf4b4e325bf9319d1f1e1` |
| `programs/tests/hub-handoff/backend.rs` | `tests/handoff/backend.rs` | `ab864212f56b7d1cf0870a4d24946805257c1a12ead804feaa7868f92667dc05` |
| `programs/tests/hub-handoff/cases.rs` | `tests/handoff/cases.rs` | `9a4ef4b8f7ce3e099d48a3f2a0cd68924badc5197d54c5e0dcf313b516e1b5b8` |
| `programs/tests/hub-handoff/coord_backend.rs` | `tests/handoff/coord_backend.rs` | `4cd92aef4469c1dacf246e3c844af298aadeaa623f6e1d34bc26bfb6e4bb753e` |
| `programs/tests/hub-handoff/enrollment.rs` | `tests/handoff/enrollment.rs` | `3261f8a504581cf0162cb1fb7a6a06aab445fbfb8baf5106b3ad598a869d5cd8` |
| `programs/tests/hub-handoff/lib.rs` | `tests/handoff/lib.rs` | `e4ac15abf55ee667586f15923920a422f1c594c37bcf32ef66244115c9ca05f6` |
| `programs/tests/hub-handoff/raw.rs` | `tests/handoff/raw.rs` | `de1919d0d3456d73d692e7c22a6c433b65fa677116d87e55952615a0ab5feb97` |
| `programs/tests/hub-handoff/rpc.rs` | `tests/handoff/rpc.rs` | `db6961a882ef342a5b3afa966974b0396398cad7411367a3df28dc580b809d65` |
| `programs/tests/hub-handoff/backend/kind.rs` | `tests/handoff/backend/kind.rs` | `3760d20c318cbfd90aae866a9097b6b12437012baaf754ba471308f5bd585bbd` |
| `programs/tests/hub-handoff/coord_backend/state.rs` | `tests/handoff/coord_backend/state.rs` | `91afc89049b57a8b115d84aa152994ba6a304c8e027800da2906ceed74d89568` |
| `programs/tests/hub-handoff/raw/hole.rs` | `tests/handoff/raw/hole.rs` | `22334d35996ed848d6dc4925d1d314a2b87eb6326cf17de7eb3ae915d4ed8605` |
| `programs/tests/hub-handoff/rpc/request.rs` | `tests/handoff/rpc/request.rs` | `23f11c5bf8dd6ddc1185059498cd07fea0cc309f1f0563f36c6d93de3d6fa0c9` |
| `programs/tests/hub-handoff/serve/adapter_cases.rs` | `tests/handoff/serve/adapter_cases.rs` | `08fa22def45c28cce492908131288c704a20afea942cfe60e67c47fb2fcb0eaf` |
| `programs/tests/hub-handoff/serve/mod.rs` | `tests/handoff/serve/mod.rs` | `5ac917edaad15b975b38e088e4c7d05dd07a164075e5c592195dee1052e3e72f` |
| `programs/tests/hub-handoff/serve/peers.rs` | `tests/handoff/serve/peers.rs` | `260b64a0cd438efbec996607551c6bc2440cd6d46fcfec122b8375a6a2eeef29` |
| `programs/tests/hub-handoff/serve/peers/control_face.rs` | `tests/handoff/serve/peers/control_face.rs` | `b0ca3c3cc254090d757f03436c68ef55b328940433d0803aeae95138337756ce` |
| `programs/tests/hub-handoff/serve/peers/coord_face.rs` | `tests/handoff/serve/peers/coord_face.rs` | `d8c7a603844dfb6b9ab42763105bd517b9cdf0decc93bea61c8a60b5ec709d8b` |
| `programs/tests/hub-handoff/serve/peers/instance.rs` | `tests/handoff/serve/peers/instance.rs` | `09714c2c51c8467351b515c0d629f2deec70ffcd3fba77431d11df44f2e52d58` |
| `programs/tests/retention/Cargo.lock` | `tests/retention/Cargo.lock` | `4472c0efeaeba727164e1ce30957303b618ecc090ab9fe1f1ff3fd146b72937d` |
| `programs/tests/retention/Cargo.toml` | `tests/retention/Cargo.toml` | `b3fa4b73ded58c5b42a83c61f726a904e318b6203a488e801a7f4520120ccebd` |
| `programs/tests/retention/enrollment.rs` | `tests/retention/enrollment.rs` | `d69735d472b62384935bd0430d922b676c92139709c6a7a0de9ed5357a46d42b` |
| `programs/tests/retention/execution.rs` | `tests/retention/execution.rs` | `cf86b4e8d296f9c166007554d2b82799d78e54a4e9748255dd8a375f424a9ba4` |
| `programs/tests/retention/lib.rs` | `tests/retention/lib.rs` | `31a9b48efd5cba41b378b7c6b43b6f55d411b7472807ad495e388d29198dfe8c` |
| `programs/tests/retention/protocol.rs` | `tests/retention/protocol.rs` | `917c7eff98c72b3e4685805b537725aac34637e2cf37636104b9a198314c0a8d` |
| `programs/tests/retention/enrollment/boundary.rs` | `tests/retention/enrollment/boundary.rs` | `cb5090a5082b85116a670cab378f295776f521a2251388630a806828b1300d5b` |
| `programs/tests/retention/enrollment/effects.rs` | `tests/retention/enrollment/effects.rs` | `e4f442178e75162edea6207146f4ac1790e260ac870cfad62b226d66ed6eff4e` |
| `programs/tests/retention/enrollment/failure.rs` | `tests/retention/enrollment/failure.rs` | `54d461d822de14485cd58789e3be6b520d82dca0169637dc2dffdc2fe83a3cb2` |
| `programs/tests/capability-import/Cargo.lock` | `tests/capability-import/Cargo.lock` | `3a62d164e4b60e8f712e9b5c167451dca3ed95b13d012c1f9ec41ee3dc36b4fd` |
| `programs/tests/capability-import/Cargo.toml` | `tests/capability-import/Cargo.toml` | `744200a959b006debdb77187045e2b5f278989d29384a99512258e0c36e5d003` |
| `programs/tests/capability-import/hub_serve.rs` | `tests/capability-import/hub_serve.rs` | `fe34b20e7656049b5e9453adec1b4824133fc8b7ae6c6d71c39a3c5f00a3a7b8` |
| `programs/tests/capability-import/lib.rs` | `tests/capability-import/lib.rs` | `734427e2d29a2d731628a444935ace0a3fac471a95c1985dd04d0fe81a86cd22` |
