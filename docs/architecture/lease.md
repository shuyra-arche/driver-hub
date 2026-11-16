# Native Pend leases

`server::lease` owns one Task-local private Pend root and its receiver delivery.
It is an effect mechanism below Machine allocation coordination, not another
reservation table, platform Authority, or generic System Supply service.

A checked immutable Request correlates the exact device epoch, allocation and
attempt with an actual local source, recipient Task, narrowed rights and mark.
These identities describe intent; Kernel rechecks live Pend authority on every
effect. Unsafe construction requires the real current Task identity and a real
local source Pend. The owner is neither Send nor Sync and must stay in that
Task. Before each Accord, Return and absence query, the implementation also
checks public UnitQuery::Caller against the retained owner; missing, malformed,
failed or mismatched caller observations retain responsibility and fail closed.
The source owner retains its independent responsibilities.

The lease records intent before its first effect. RootPending derives a private
local root; DeliveryPending derives one recipient-local Pend; Delivered replays
the existing token without a second derivation. Receiver permissions always
exclude redistribution. Pages ranges are resolved relative to the source once
at the private root, then inherited by the recipient. Object and Pages scopes
are supported; Nest is rejected until its independent retirement contract is
validated. Kernel admission still enforces recipient/sender registration limits.

`finish` moves monotonically to Closing. Busy or errors retain the root and
exact unfinished responsibility. Successful Kernel Return proves this private
branch has settled. Return Denied alone proves nothing: only successful
capacity-one Collect in the same actual Task, strictly after root minus one,
with a validated absent-or-later token can establish that the never-transferred,
non-reusing private root is gone. Unexpected records, errors and a still-present
root fail closed. Closed is idempotent; dropping the Rust value is not cleanup.
Ancestor revocation can remove the root before finish and is covered by this
same absence proof.

This contract consumes Kernel `789ffd19b5359f3de172fb9aad8a61f6a7113a64`,
ABI/Ledger 13/13. Accord Busy publishes no new child registration, but other
source-side effects may already have occurred on memory/register paths. A retry
does not imply the source was unchanged. Return Busy can retain closed authority
and registration charges while cleanup remains incomplete.

Host tests check request rejection and rights/range boundaries. The CPU-only
[QEMU consumer](../../tests/target/handoff/README.md) exercises production lease
code with actual Kernel derivation, live target Tasks, ancestor revocation,
replay and withdrawal. It does not prove Pages mapping effects, device drain,
IRQ/External admission or full Hub boot. System Supply RPC integration is
a separate per-allocation [owner](supply.md) using these local root primitives.
Standalone Pend branch settlement never clears Machine duties or proves
Instance retirement. With the [Signal Authority](authority.md), Machine owns
the sole lease for an exact slot and attempt and may discharge only that
managed Authorization duty on Closed. Device, source and Instance owners still
provide their independent evidence; absent leases never imply no prior effect.
