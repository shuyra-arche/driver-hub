# Machine resource domain

Hub coordinates device discovery, exclusive resource reservations and binding
responsibilities. Kernel owns live Pend authorization, IRQ delivery, External
Nests and access retirement. System owns Instance identity, creation, supervision,
UnitRoot and generic Supply. Drivers own their device protocol and publish their
own services; consumer I/O does not pass through Hub.

This contract replaces the old inventory/claim/admission/DMA service family.
The first implementation is a native `no_std` provider library, not a bootable
Hub Unit. System integration and hardware effects require separate validated
adapters. No Instance identity or System call is invented here.

## Ownership and identity

| Concept | Authority and meaning |
| --- | --- |
| Machine | A particular Hub incarnation and its device-resource coordination state. Its identity must never be reused for a fresh, empty domain. |
| Device | A source-qualified device identity and epoch. A topology coordinate is descriptive; removal/replacement invalidates the old epoch. |
| Resource | A bounded, typed device resource held by its real provider. Addresses and requester/source numbers describe a conflict domain, not permission. |
| Allocation | One complete offered resource within a Binding, identified by its non-reusing BindingId and fixed resource slot. Failure cannot silently free the resource or its retained cleanup responsibility. |
| Binding | The device association and its preparation/retirement obligations. It does not describe whether a System Instance is alive. |
| Offer | A provider's discoverable description of available resources. Acceptance requires verification by the trusted authority adapter. |
| Operation | A requester-qualified identity with replayable intent and result. The requester comes from the actual transport, not the request body. |

**Owned** means actual retained resource authority and responsibility.
**Provided** means an Offer can be discovered. **Bound** means a device has an
association under coordination. None implies the other. Pend metadata and
Operator publication remain observations; Kernel rechecks live authority at the
effect boundary. A remote Task's Pend number is not a local authorization handle.
Any Pend accepted by an adapter must have been genuinely delegated to the Hub,
with source, recipient, scope, permissions and revocation lineage checked.
The initial Offer imports complete resources from their authenticated provider;
recursive subprovider import has no accepted derivation contract yet and is not
exposed by v2. A reservation covers all resources of one exact Offer epoch.

Machine mutations require exclusive access. A service executor must serialize
them; an earlier observation never authorizes a later commit. Non-reusing IDs
must reject exhaustion, not wrap. Conflicts cover both device identity and
overlapping resources in the same real conflict domain, including resources
offered under different device names.

## Commitment and failure

`Offer`/`Withdraw` describe provider publication. `Reserve`/`Release` express
allocation intent. `Inspect` observes progress. Withdrawal closes discovery and
must retain any allocation or child-provider responsibility still outstanding.
The old ordinary Claim path, which freed a device when its claimant died, cannot
coexist with the new exclusive reservation path.

The essential binding progression is:

```text
Reserving -> Preparing -> Active -> Closing -> Retiring -> Closed
                |                    ^
                +--------------------+
```

An unprepared reservation can close without hardware retirement. Once preparation
can have created an effect, responsibility is retained before calling an external
owner. Explicit cancellation or a failure policy enters retained cleanup; neither
proves the effect did not happen. A timeout or lost reply alone leaves the
operation uncertain and requires query/replay, not automatic cancellation. The
integrator must detect requester death and initiate retained cleanup. `Active`
requires both real delivery and the accepted System/driver contract. Until that
contract exists, activation is unsupported.

`Closing` prevents new work. `Retiring` retains the exact allocation, device epoch
and resource obligations while owners withdraw access and establish retirement.
Busy, errors and lost replies retain those obligations. Only exact, trusted
owner evidence may establish `Closed` and permit reuse. An IRQ Ack, queue reply,
driver Task exit, disabled Bus Master or a software epoch alone cannot prove
that old device accesses have drained.

Every accepted operation records its intent and result before the reply is
attempted. A duplicate identity with the same intent observes that result; a
changed intent conflicts. If history is bounded, an evicted identity must remain
below a retained rejection floor and must never execute again. Twine submission
success only establishes enqueueing. A lost response is resolved by querying or
replaying the same operation identity, not by creating a new reservation.

Cross-owner progress is not a transaction. The trusted adapter owns live
authorization and real effect evidence; the Machine owns coordination and
exclusive reservations. Constructing the public retirement evidence fields does
not authorize cleanup: the trusted Authority must verify the real owner proof.
Host adapters can exercise this protocol but do not prove hardware isolation.

Hub restart cannot create a clean domain over resources that may still be in
use. The integration owner must retain the old domain/obligations or revoke the
old authority and prove complete device and Kernel retirement before provisioning
a new incarnation. The initial library has no durable recovery or takeover
adapter, so live takeover remains unsupported. A changed incarnation ID alone
does not authorize reassignment.

## Native Signal allocation delivery

The [Signal Authority](authority.md) and `machine/supply.rs` now connect real
imported CPU Sign rights to each Binding's owned native leases. The target is
queried from its bound System Prepared Instance. Driver Start requires all
Native resource slots Delivered and fresh source/policy checks. Release,
withdrawal, failure and settle prevent further delivery and require explicit
lease advancement; exact owned completion clears only that slot's Authorization
bit. No lease is evidence of unknown/unmanaged work, so missing slots retain
their duties. External receipts cannot replace a stored lease's completion.

Signal means a CPU resource associated with a device; it is not MMIO, IRQ,
reset, power or a quiescence receipt. Native rejects every hardware kind. It
accepts no Device retirement receipt, so the CPU consumer deliberately retains
Device bits and prevents rebinding even after every lease/Instance has retired.
This is an internal executor path, not a generic Hub Supply RPC or full Hub Unit.

## Selected native graph

Current consumption selects merged System #24
`470e97f38aa904c3702b665c0d225809b9f0d16e`, Kernel
`789ffd19b5359f3de172fb9aad8a61f6a7113a64` (ABI/Ledger 13/13), Twine
`932454c07bd2673ac7d6d9612d36182aae71e8b2` and unchanged Weave. All ordinary
Tasks use User mode and ECALL. The Signal Authority/Lease ownership contract
and unsupported hardware gates remain in force; this graph update confers no
new device authorization. The provider survey below is historical, not the
current build graph.

## Hardware authority survey (historical)

The following source revisions were inspected together. These are provider
identities for this contract review, not evidence of composed Vanllia acceptance.

| Provider | Immutable revision |
| --- | --- |
| Kernel | `b2eb37e2af134e4bb4248df454c437b06c1f93c0` |
| Twine | `bbab13f5df3e4bad24314cd5a18f19f26452ce66` |
| Weave | `7a7d67fb3f190aeb43049d051b0972bcd53213bc` |
| System main | `7f4389e000b266bad87cc40d325890cd87bc21f1` |
| System draft #12 head | `7a324761d632d870d18b63575133852ede8372c9` |
| PCI | `69d43c9018ce6c35f63a83648050e33fe815504c` |
| Router | `00ad67ef413d44d90d81191d7107da67c92efa5f` |
| IOMMU | `acc4f23f929d6fb7838d1e96398a07f2e818eab1` |

Kernel's revision is the Gamma **follow-up fix**, with ABI/Ledger 11/11 and
Capsule format 2. Twine selects that same Kernel/API/Client identity.

- [Nest ABI](https://github.com/shuyra-arche/kernel/blob/b2eb37e2af134e4bb4248df454c437b06c1f93c0/api/src/abi/call/nest.rs)
  creates Internal Nests; former Attach/Detach slots are retired. There is no
  Unit call that registers an arbitrary runtime requester.
- [Trusted boot](https://github.com/shuyra-arche/kernel/blob/b2eb37e2af134e4bb4248df454c437b06c1f93c0/server/src/entry/boot/init.rs)
  admits External routes only when the platform owns the complete isolation
  group, excludes bypass/ATS/PRI and retains genuine quiescence evidence.
  The current [External backend](https://github.com/shuyra-arche/kernel/blob/b2eb37e2af134e4bb4248df454c437b06c1f93c0/server/src/entry/boot/devices/external.rs)
  also limits admission to a declared controller and requester below 64.
- The same boot interface admits exclusively owned, level-high PLIC sources
  during single-Hart boot, before `boot::start` publishes grants. Shared,
  cascaded and MSI routes are unsupported.
  [Source attachment](https://github.com/shuyra-arche/kernel/blob/b2eb37e2af134e4bb4248df454c437b06c1f93c0/server/src/basis/room/interrupt/source/mod.rs)
  requires the receiver's matching whole writable register Share and a
  non-redistributable LISTEN/CONSUME Sign. A TaskId cannot reroute an IRQ.
- [Pend delegation](https://github.com/shuyra-arche/kernel/blob/b2eb37e2af134e4bb4248df454c437b06c1f93c0/docs/pend-delegation.md)
  supports unrelated Teams with bounded recipient/sender registration. Busy or
  failed withdrawal retains responsibility and registration charges.
- [Quiescence](https://github.com/shuyra-arche/kernel/blob/b2eb37e2af134e4bb4248df454c437b06c1f93c0/server/src/metal/generic/iommu.rs)
  is a trusted boot Rust contract, not a Unit acknowledgement ABI. Revocation of
  a derived Pend does not itself delete an External Nest or prove device drain.

Unknown runtime requester/IRQ relationships therefore fail closed with
`Unsupported`. The initial Machine rejects all IRQ and External Offers because
there is no accepted native adapter for even the boot-admitted subset.
Descriptions, DTB discovery and numeric hardware IDs cannot substitute for an
admitted, restricted Pend.

The inspected Vanllia revision
`b5468739f2d7e36d64198de743a13d6ccabc6666` protects ECAM when an IOMMU topology is
present. Its [resource plan](https://github.com/shuyra-arche/vanllia/blob/b5468739f2d7e36d64198de743a13d6ccabc6666/platform/riscv/src/resources/mod.rs)
withholds configuration control from device grants. Kernel's
[admission boundary](https://github.com/shuyra-arche/kernel/blob/b2eb37e2af134e4bb4248df454c437b06c1f93c0/api/src/boot/platform/resources.rs)
rejects protected overlaps. System S2 alone cannot create that missing ECAM
authority. A separately reviewed trusted configuration-custody or mediated
effect contract is needed; removing protection is not a migration strategy.

The internal PCI module implements the independently usable configuration
mechanism: checked ECAM function pages, endpoint BAR sizing/restoration, bounded
placement, retained partial programming and Bus Master disable/readback. It
records configuration effects, while Machine remains the allocation owner.
Programming leaves decoding and Bus Master off. No method releases a programmed
aperture or treats disabling Bus Master as retirement. Unsafe construction and
effects require exclusive real configuration authority, platform MMIO ordering,
actual device quiescence and the current allocation's retained interval.
Host tests validate geometry and partial-failure behavior, not those hardware
preconditions. The mechanism has no validated runtime supply or QEMU evidence.

## Migration and integration gates

| Previous responsibility | Successor and gate |
| --- | --- |
| Hub inventory/ordinary claim/class admission | Machine/Offer/Allocation/Binding; remove the early-free path and System lifecycle duplication. |
| PCI ECAM/BAR/Bus Master | Hub internal PCI mechanism. Preserve Bus Master disabled before preparation, exact BAR scope, readback and retained programmed apertures after partial failure. Disabled Bus Master alone is insufficient retirement evidence. |
| Router PLIC implementation | Kernel IRQ/Sign. Do not copy a second claim/complete/enable state machine into Hub. |
| User IOMMU tables/queues/fence | Kernel External Nest and trusted retirement provider. Do not move these hardware authorities into Hub. |
| Hub execution admission and fixed `/dev` publication | System Instance supervision/Supply and driver-owned UnitRoot services, after S2 is frozen. |

The initial provider survey above predates System #19. A bounded caller-owned
Instance v1 contract is now published at System
`f97524817ce240a0b0471be3b557ceb5a871fe14`; the
[Driver Instance contract](driver.md) records its selected graph and Hub consumer
boundary. Full System S1/S2 acceptance remains open, while the published subset
can be consumed now. Hardware configuration custody and actual Hub-owned resource
delivery are independent of that Instance contract.

The merge order is provider contract, Hub consumer, then Vanllia pins/lock and
integration. Vanllia #39 owns the unified progress and Host/RV64/QEMU/hardware
acceptance. Hub #3 owns local implementation details. This change does not alter
the product's pinned Hub or retire the PCI/Router/IOMMU repositories or Units.
Those removals require migrated consumers and real replacement/retirement
evidence; repository renaming and archival follow later.
