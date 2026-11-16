# Driver Instance coordination

System's caller-owned Instance v1 subset is available at immutable revision
`332d9d2e15f6bc4e1e8d94e4db2591b1a53ae613` (merged System #27, retaining the #19 subset). Its
[contract matrix](https://github.com/shuyra-arche/system/blob/332d9d2e15f6bc4e1e8d94e4db2591b1a53ae613/docs/instance.md)
defines the actual Create/Start/Stop/Retire/Inspect/InspectOperation/Cancel and
UnitRoot publication semantics. Hub consumes that public API/Client, not the
historical Loader authorization or System Server internals.

The matching graph selects Kernel API/Client
`789ffd19b5359f3de172fb9aad8a61f6a7113a64`, Twine
`932454c07bd2673ac7d6d9612d36182aae71e8b2` and Weave
`7a7d67fb3f190aeb43049d051b0972bcd53213bc`. Kernel main may be newer; the consumer
selects the verified System graph as a whole rather than mixing revisions.

## ABI 13 and execution

Kernel ABI/Ledger are 13/13 and Capsule remains 2. Ordinary external Tasks run
in User mode and use ECALL; Supervisor execution is Kernel-internal. ABI 11/12
ELFs and old Task descriptors require rebuilding with the selected graph.
System Loader owns the target stack, copied arguments, unmapped guard and
backing/mapping retirement. Hub does not allocate foreign Task stacks or infer
that their storage is reusable from Task exit. Retire completion remains the
Instance owner's evidence.

The selected System also publishes declaration-selected CreateFor/Owned; this
Hub coordinator keeps using the caller-owned subset. No owner transfer or
creation-policy bypass is implied by upgrading its provider graph.

## Independent ownership

Hub owns the Device/Allocation/Binding and its unfinished coordination duties.
System owns Instance identity and lifecycle. The association stores the exact
System-issued InstanceId and the original actual caller; it does not turn
Task/Team coordinates or a cached phase into execution authority. Responses from
another caller's journal on the same System cannot discharge this association.
One Hub caller uses one operation-ID allocation
domain across its System requests; unrelated bindings cannot reuse those IDs.

Create must record the potential Instance responsibility before submission.
Completed Create means a prepared dormant Task and installed declarations, not
ready service. Completed Start means resume committed, not readiness. Completed
Stop means closing was admitted, not physical retirement. Only the exact terminal
Retire/Cancelled contract can discharge the corresponding Instance duty.
Device and authorization duties remain separately retained.

`Machine::create_driver` records preparation duties and attaches one Driver
coordinator to the actual Binding before calling System. `start_driver`,
`stop_driver`, `close_driver` and `poll_driver` advance that same record. Generic
Release, Offer withdrawal and Machine settlement close admission; the integrator
must continue polling the retained Driver. Generic retirement evidence cannot
clear Instance duties once a Driver is attached. Authenticated terminal progress
clears the Instance bit for every resource slot, preserving Device/Authorization
bits. This does not transition a hardware Binding to Active.

A lost reply leaves the original operation uncertain. Query that operation on
the same System peer. Unknown is an observation, not proof that a submitted frame
cannot arrive later; do not allocate a second Create identity. Keep native
Pending or its transferred Cleanup until route retirement succeeds. Cancellation
of completed Create does not undo its receipt: retire the resulting Instance
through a distinct recorded operation. A changed issuer, stale sequence or
mismatched result must not clear the old responsibility.

A known rejection of Create with no Instance attached establishes absence only
under this exact provider's commit contract. A call proven never submitted can
also close locally. Neither case applies to Unknown or a lost reply. Failed
Retire remains outstanding; an explicit retry records a fresh operation for the
same Instance, retaining responsibility until actual completion. Native cleanup
failures block further calls and retain their cleanup owner.

## Resource and service boundaries

System's public Register/Install contract now carries Hub-owned CPU Object
Sign/Clone rights into an exact caller-owned Prepared Instance. Machine uses its
bound Driver observation as the target; it never accepts an arbitrary recipient
from the wire. The [Supply owner](supply.md) retains registration, installation
and local offering-root retirement separately from this Instance journal. Its
caller and System peer must match the Driver coordinator's. The System fixture's
private native Supply module remains outside Hub production code.

Drivers publish through their actual initial-Task UnitRoot Grant. Publication
and stable-name replacement require the provider's readiness and revision CAS;
changing discovery does not revoke old consumer Pends. Normal Consumer-to-Driver
RPC remains direct. The current Instance consumer does not establish hardware
quiescence, protected ECAM custody or permission for runtime IRQ/External sources.

The old claim that no fixed System Instance contract exists is historical.
System's remaining full S1/S2 checklist does not block consuming the published v1
subset. Hub consumer verification, boot integration, actual resource delivery,
device retirement and Vanllia acceptance remain separate evidence boundaries.
