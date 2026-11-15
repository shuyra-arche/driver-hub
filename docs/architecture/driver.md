# Driver Instance coordination

System's caller-owned Instance v1 subset is available at immutable revision
`f97524817ce240a0b0471be3b557ceb5a871fe14` (merged System #19). Its
[contract matrix](https://github.com/shuyra-arche/system/blob/f97524817ce240a0b0471be3b557ceb5a871fe14/docs/instance.md)
defines the actual Create/Start/Stop/Retire/Inspect/InspectOperation/Cancel and
UnitRoot publication semantics. Hub consumes that public API/Client, not the
historical Loader authorization or System Server internals.

The matching graph selects Kernel API/Client
`684f479c45dc2d94d405b32b4ffc6334a4233fc9`, Twine
`8c68cbeffd024b4105a6a7c2f4baaab330994ca1` and Weave
`7a7d67fb3f190aeb43049d051b0972bcd53213bc`. Kernel main may be newer; the consumer
selects the verified System graph as a whole rather than mixing revisions.

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

System-owned catalog sources can be delivered through System Supply. A Hub-owned
source requires actual Kernel delegation and a retained lease owner checking
caller, target, scope and revocation lineage. The System fixture's private native
Supply module is not a public SDK and must not be imported into Hub production
code. The Instance consumer alone does not implement a native resource Authority.

Drivers publish through their actual initial-Task UnitRoot Grant. Publication
and stable-name replacement require the provider's readiness and revision CAS;
changing discovery does not revoke old consumer Pends. Normal Consumer-to-Driver
RPC remains direct. The current Instance consumer does not establish hardware
quiescence, protected ECAM custody or permission for runtime IRQ/External sources.

The old claim that no fixed System Instance contract exists is historical.
System's remaining full S1/S2 checklist does not block consuming the published v1
subset. Hub consumer verification, boot integration, actual resource delivery,
device retirement and Vanllia acceptance remain separate evidence boundaries.
