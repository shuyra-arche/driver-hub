# Per-allocation System Supply owner

The selected System revision is
`332d9d2e15f6bc4e1e8d94e4db2591b1a53ae613`, with public external Source
registration (#26) and post-Prepared installation (#27). Its
[contract](https://github.com/shuyra-arche/system/blob/332d9d2e15f6bc4e1e8d94e4db2591b1a53ae613/docs/supply.md)
is the authority for remote commit and retirement. Hub consumes only the public
API/Client. Kernel ABI/Ledger/Capsule remain 13/13/2.

## Admission and ownership

`Machine<authority::native::Native<supply::Supply>>` keeps one Task-local owner
directly in each Binding slot. `prepare_system_supply` validates the trusted
Signal import, selects the actual Task from its bound caller-owned Prepared
Instance, and records immutable intent before effects. System Instance and
Supply controls must name the same System and actual caller. The trusted source
name must match the Hub caller's Unit declaration; Machine derives the resource
name from allocation identity and attempt. Changed source or mark conflicts.

The owner first derives a private local offering root, then really Accords a
distributable child into the System Task. Register names that System-local
child, with narrowed maximum rights. Install names the exact registration,
Instance, Task, slot and rights. System independently checks actual caller,
immediate lender, namespace, source and fresh Prepared admission. Only CPU Object
Sign/Clone is supported; no memory, MMIO, Device, IRQ or External authority is
conferred. The Driver receives a non-distributable grant whose lender is System.

Registration and installation IDs are caller/System scoped, monotonic and
non-reusing, in distinct namespaces from Instance operation IDs. Their allocators
must be shared across every client in that caller. Provider history is bounded
(64 registrations and 256 installation records) and is never evicted; exhaustion
retains existing recovery/retirement, rather than reusing an identity.

## Uncertainty and closure

Successful Pending accepts work. An admission `Failure(Pending)` requests an
exact same-request retry before commit. Neither Unknown nor a lost response
proves absence. The owner retains possible-submission flags and queries/replays
the original IDs. It never allocates a replacement installation on uncertainty.
Closing is sticky: no error or subsequent observation reopens grant admission.

Finishing progresses through three independent obligations:

1. An accepted installation must reach Released with its original association;
   an unseen cancellation may produce the distinct Cancelled tombstone.
2. An accepted registration must reach Closed after imported branches retire;
   unseen withdrawal may produce Cancelled. Source closure alone does not prove
   installation retirement.
3. The Hub-local offering root must pass actual Return/strict Collect absence
   proof in its owning Task, after System reply-route cleanup completes.

Cancelled cannot downgrade an already accepted record. Lost CancelInstall or
Withdraw replies use exact-ID inspection; repeated `finish` preserves that
query phase. Tombstones prohibit late requests from resurrecting the operation.
Native verifies actual Kernel Caller before cleanup or source effects. Busy,
errors, invalid receipts and unknown state retain the owner and its duties.
Dropping a value proves none of these levels settled.

Only this exact owner reaching Closed clears its allocation's Authorization
bit. Generic external receipts cannot substitute for it. System Instance
completion clears only Instance bits; Device duties remain with the device
owner and continue to prevent rebinding. Parent-source withdrawal can also
retire a Prepared Instance; its original Create receipt remains historical.

## Evidence and remaining scope

Host models exercise replay, lost replies, accepted versus unseen cancellation,
late requests, wrong caller/target and retained local-root responsibility.
The native consumer runs the actual production Machine, Native adapter and full
System, registers sources, installs into actual Prepared Driver Tasks, checks
the System lender, discards one real authenticated Install reply and recovers
by its original ID. Driver Start, direct RPC, native withdrawal denial, explicit
cleanup and independent Device/Instance duties remain checked.

This resolves the former missing external registration/installation interface
for this CPU subset. A standalone Hub Unit, hardware import/ECAM custody,
IRQ/External admission, device quiescence, restart takeover and Vanllia product
pin/retirement acceptance remain separate work.
