# Signal import and allocation delivery

`authority::native::Native` is a Task-local Authority for CPU Signal resources.
It admits only `ResourceKind::Signal` (wire tag 7), with TRIGGER, Object scope,
no modification and no redistribution at the receiver. Every hardware kind
returns Unsupported; a Sign is not an IRQ route or a reset/power operation.

## Trusted import

The composition owner first really Accords a restricted distributable Signal
Pend into the Hub Task. Unsafe `Import::new` records its canonical domain,
object interval (start 0, len 1), exact local token, immediate provider,
permitted requester, expected SignId and target-object owner. These facts must
come from trusted provenance; wire descriptions and Pend metadata cannot create
them. Import fields are private. Native rejects duplicate domains, Sign objects
and local tokens. Original source duties remain with their owner; dropping the
import, Authority or Machine proves no retirement.

Offer, reservation, grant intent and fresh Start checks query actual
UnitQuery::Caller and Pend Inspect in this Task. They require exact token,
Active status, SignId, target-object owner, immediate lender, TRIGGER rights,
Object scope, no modification and distributability. The lender must match the
trusted direct provider; object owner may differ from that lender. PendInfo has
no full ancestor chain, physical address or canonical hardware alias mapping.
The private grant root keeps Kernel's actual revocation lineage; every Accord
rechecks live authorization rather than relying on these snapshots.

## One Binding owns each grant

After recording all preparation duties, `prepare_supply` queries the existing
Driver's current System Prepared observation and chooses that actual Task.
There is no wire recipient argument. `SupplyAuthority` produces only checked
intent; the Machine verifies correlation, recipient and mark, then stores its
Lease before the first possible native effect. One slot/attempt has one managed
branch. Repeated preparation replays the intent; changed or duplicate marks
conflict. `advance_supply` performs the native work, and repeated delivery
returns the same token.

Fresh Driver Start requires a complete delivered-slot mask and revalidated
imports/policy. Existing pending Driver operations keep their journal/recovery
path; fresh admission never replaces InspectOperation. Release, withdrawal,
failure and settle close the Binding before any subsequent supply advancement,
so advancement can only finish retained leases.

Only that exact owned Lease reaching Closed can clear its slot's Authorization
bit, under the unsafe sole-owner contract forbidding unrecorded parallel grants
for the same allocation/attempt. External Authorization receipts for a managed
slot are rejected. Missing leases, Busy, errors and source withdrawal retain
unknown duties; a failed preparation cannot assume no prior effects. Native
accepts no Device retirement receipts. System completion clears only Instance
bits. Quarantined Device responsibility therefore still blocks reuse.

Authority selects an associated Supply record: Native defaults to the actual
!Send/!Sync Lease, or `Native<supply::Supply>` stores the System-backed owner; external grant owners may use the uninhabited NoSupply. Binding
stores that type directly. Ordinary coordinators retain their Send behavior,
while Native Machine stays in its actual Kernel Task without an unsafe Send
implementation or a second allocation table.

## System boundary and evidence

This contract was rechecked against Kernel
`789ffd19b5359f3de172fb9aad8a61f6a7113a64`, ABI/Ledger 13/13. PendInfo
retains owner/lender and the same resolved-scope rules; Return/Collect retain
the exact same-Task settlement boundary. UnitQuery::Caller identifies the Task,
not its privilege level. User execution comes from the pinned ABI 13 startup
contract; the guest exercises actual ECALL-backed public operations rather
than reading privileged CSRs.


Fixed System `332d9d2e15f6bc4e1e8d94e4db2591b1a53ae613` supplies public
external Source Register/Withdraw/InspectSource and post-Prepared
Install/InspectInstall/CancelInstall operations. `prepare_system_supply` uses the
same trusted Authority request and actual bound Driver observation as direct
`prepare_supply`, then stores its per-allocation [Supply owner](supply.md) before
effects. Acquire remains self-only; the Hub never uses it to impersonate a
foreign receiver. System is the actual immediate lender of installed grants.

Host tests check import shape, codec, replay, partial cancellation, wrong phase,
retained unknown slots and journal recovery. They do not emulate Kernel effects.
The [native consumer](../../tests/target/handoff/README.md) uses real production
Machine/Authority/Supply and complete fixed System. Its trusted provider and Hub
are the same controller Task with actual self-Accorded imports; cross-Task
provider admission is not claimed. Two Driver Teams receive two independent
Signal slots each. A third Prepared Driver exercises imported ancestor
withdrawal after accepted registration/installation. A test transport drops one real
authenticated successful Install reply; recovery queries the original ID. Native replays, target selection,
receiver Sign Denied, exact Authorization/Instance settlement, retained Device
bits and denied rebinding are asserted with the original provider lifecycle
checker, budgets and actual root exit.

This is not a standalone Hub Unit, hardware import, device quiescence, Pages
mapping proof, full generic Supply coverage or Vanllia product acceptance.
ABI/Ledger/Capsule remain 13/13/2 and all provider revisions stay fixed. Protected
ECAM custody and IRQ/External admission require separate provider contracts.
