mod support;

use hub_api::*;
use hub_server::Limits;
use kernel_api::model::identity::TaskId;
use std::sync::{Arc, Barrier, Mutex, atomic::Ordering};
use std::thread;
use support::*;

#[test]
fn concurrent_claims_have_one_commit_and_lost_reply_replay_keeps_identity() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    let shared = Arc::new(Mutex::new(machine));
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = [DRIVER, TaskId::new(10)]
        .into_iter()
        .map(|caller| {
            let machine = shared.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                (
                    caller,
                    machine
                        .lock()
                        .unwrap()
                        .reserve(caller, 1, Reserve { device }),
                )
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|(_, r)| r.failure == Fail::None)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|(_, r)| r.failure == Fail::Busy)
            .count(),
        1
    );
    let (caller, accepted) = results
        .iter()
        .find(|(_, r)| r.failure == Fail::None)
        .unwrap();
    let mut machine = shared.lock().unwrap();
    assert_eq!(machine.reserve(*caller, 1, Reserve { device }), *accepted);
    assert_eq!(
        machine.inspect(
            *caller,
            Inspect {
                operation: 1,
                device: DeviceRef::default()
            }
        ),
        *accepted
    );
    let mut changed = device;
    changed.epoch += 1;
    assert_eq!(
        machine
            .reserve(*caller, 1, Reserve { device: changed })
            .failure,
        Fail::Conflict
    );
}

#[test]
fn operations_are_source_scoped_and_rejected_ids_never_reexecute() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    let binding = reserve(&mut machine, device);
    assert_eq!(
        machine
            .release(TaskId::new(10), 2, Release { binding })
            .failure,
        Fail::Denied
    );
    assert_eq!(
        machine
            .inspect(
                TaskId::new(10),
                Inspect {
                    operation: 1,
                    device
                }
            )
            .failure,
        Fail::Unknown
    );
    machine.release(DRIVER, 3, Release { binding });
    assert_eq!(
        machine.reserve(DRIVER, 2, Reserve { device }).failure,
        Fail::Stale
    );
    let newer = machine.reserve(DRIVER, 4, Reserve { device });
    assert_eq!(newer.failure, Fail::None);
    assert_ne!(newer.snapshot.binding, binding);
    let replay = machine.reserve(DRIVER, 1, Reserve { device });
    assert_eq!(replay.snapshot.binding, binding);
    assert_eq!(replay.snapshot.state, BindingState::Closed);
}

#[test]
fn cancellation_before_effects_releases_but_preparation_retains_all_duties() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    let first = reserve(&mut machine, device);
    assert_eq!(
        machine
            .release(DRIVER, 2, Release { binding: first })
            .snapshot
            .state,
        BindingState::Closed
    );
    let second = machine
        .reserve(DRIVER, 3, Reserve { device })
        .snapshot
        .binding;
    let preparing = machine.begin_preparation(second).unwrap();
    assert_eq!(preparing.state, BindingState::Preparing);
    assert_eq!(preparing.pending, 7);
    assert_eq!(machine.begin_preparation(second), Ok(preparing));
    let closing = machine.release(DRIVER, 4, Release { binding: second });
    assert_eq!(closing.snapshot.state, BindingState::Closing);
    assert_eq!(closing.snapshot.pending, 7);
    assert_eq!(
        machine
            .reserve(TaskId::new(10), 1, Reserve { device })
            .failure,
        Fail::Busy
    );
    assert_eq!(
        machine.release(DRIVER, 4, Release { binding: second }),
        closing
    );
    assert_eq!(machine.begin_preparation(second), Err(Fail::Conflict));
}

#[test]
fn timeout_fault_settlement_and_provider_withdrawal_do_not_prove_quiescence() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    let binding = reserve(&mut machine, device);
    machine.begin_preparation(binding).unwrap();
    assert_eq!(machine.failed(binding, 17).unwrap().fault, 17);
    assert_eq!(machine.failed(binding, 0), Err(Fail::Invalid));
    assert_eq!(machine.failed(binding, 99).unwrap().fault, 17);
    assert_eq!(
        machine
            .withdraw(PROVIDER, 2, Withdraw { device })
            .snapshot
            .pending,
        7
    );
    assert_eq!(
        machine.offer(PROVIDER, 3, offer(2, 0)).failure,
        Fail::Conflict
    );
    let mut next = offer(1, 0);
    next.epoch = 2;
    assert_eq!(machine.offer(PROVIDER, 4, next).failure, Fail::Busy);
    assert_eq!(machine.settle(), 1);
    assert_eq!(machine.settle(), 1);
    assert_eq!(
        machine.reserve(DRIVER, 2, Reserve { device }).failure,
        Fail::Busy
    );
}

#[test]
fn false_busy_stale_and_wrong_owner_receipts_cannot_clear_duties() {
    let model = Model::default();
    let mut machine = with(model.clone(), Limits::default());
    let device = enroll(&mut machine);
    let binding = reserve(&mut machine, device);
    let preparing = machine.begin_preparation(binding).unwrap();
    machine.release(DRIVER, 2, Release { binding });
    let valid = proof(preparing, Duty::Device);
    assert_eq!(machine.retire(DRIVER, valid), Err(Fail::Denied));
    let mut wrong = valid;
    wrong.attempt += 1;
    assert_eq!(machine.retire(OWNER, wrong), Err(Fail::Stale));
    wrong = valid;
    wrong.device.epoch += 1;
    assert_eq!(machine.retire(OWNER, wrong), Err(Fail::Stale));
    wrong = valid;
    wrong.receipt = 5;
    assert_eq!(machine.retire(OWNER, wrong), Err(Fail::Denied));
    model.busy.store(true, Ordering::SeqCst);
    assert_eq!(machine.retire(OWNER, valid), Err(Fail::Busy));
    assert_eq!(
        machine
            .inspect(
                DRIVER,
                Inspect {
                    operation: 1,
                    device
                }
            )
            .snapshot
            .pending,
        7
    );
    model.busy.store(false, Ordering::SeqCst);
    let partial = machine.retire(OWNER, valid).unwrap();
    assert_eq!(partial.state, BindingState::Retiring);
    assert_eq!(partial.pending, 6);
    assert_eq!(machine.retire(OWNER, valid), Ok(partial));
    assert_eq!(
        machine.release(DRIVER, 3, Release { binding }).snapshot,
        partial
    );
    assert_eq!(machine.retire(DRIVER, valid), Err(Fail::Conflict));
    assert_eq!(
        machine
            .reserve(TaskId::new(10), 1, Reserve { device })
            .failure,
        Fail::Busy
    );
    machine
        .retire(OWNER, proof(preparing, Duty::Authorization))
        .unwrap();
    let closed = machine
        .retire(OWNER, proof(preparing, Duty::Instance))
        .unwrap();
    assert_eq!(closed.state, BindingState::Closed);
    assert_eq!(closed.pending, 0);
    assert_eq!(
        machine.retire(OWNER, proof(preparing, Duty::Instance)),
        Ok(closed)
    );
    let replacement = machine.reserve(DRIVER, 4, Reserve { device });
    assert_eq!(replacement.failure, Fail::None);
    assert_ne!(replacement.snapshot.binding, binding);
    assert_eq!(replacement.snapshot.fault, 0);
    assert_eq!(machine.retire(OWNER, valid), Ok(closed));
    assert_eq!(
        machine
            .inspect(
                DRIVER,
                Inspect {
                    operation: 4,
                    device
                }
            )
            .snapshot,
        replacement.snapshot
    );
}

#[test]
fn ranges_cannot_alias_via_device_names_or_resource_kinds() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    let mut alias = offer(2, 2048);
    alias.resources[0].kind = ResourceKind::PciConfig;
    assert_eq!(machine.offer(PROVIDER, 2, alias).failure, Fail::Conflict);
    assert_eq!(
        machine.offer(PROVIDER, 3, offer(2, 4096)).failure,
        Fail::None
    );
    machine.withdraw(PROVIDER, 4, Withdraw { device });
    alias.epoch = 2;
    alias.resources[0].start = 0;
    assert_eq!(machine.offer(PROVIDER, 5, alias).failure, Fail::Busy);
    alias.key = 3;
    assert_eq!(machine.offer(PROVIDER, 6, alias).failure, Fail::None);
}

#[test]
fn offer_epoch_reuse_rejects_old_bindings_and_machine_references() {
    let mut machine = machine();
    let old = enroll(&mut machine);
    machine.withdraw(PROVIDER, 2, Withdraw { device: old });
    let mut fresh = offer(1, 0);
    fresh.epoch = 2;
    let new = machine.offer(PROVIDER, 3, fresh).snapshot.device;
    assert_eq!(old.id, new.id);
    assert_eq!(
        machine.reserve(DRIVER, 1, Reserve { device: old }).failure,
        Fail::Stale
    );
    let mut foreign = new;
    foreign.id.machine.incarnation = 2;
    assert_eq!(
        machine
            .reserve(DRIVER, 2, Reserve { device: foreign })
            .failure,
        Fail::Stale
    );
    assert_eq!(
        machine.reserve(DRIVER, 3, Reserve { device: new }).failure,
        Fail::None
    );
}

#[test]
fn forged_or_revoked_grants_and_unknown_hardware_fail_closed() {
    let model = Model::default();
    let mut machine = with(model.clone(), Limits::default());
    let mut forged = offer(1, 0);
    forged.resources[0].pend = 9;
    assert_eq!(machine.offer(PROVIDER, 1, forged).failure, Fail::Denied);
    for (op, kind) in [(2, ResourceKind::External), (3, ResourceKind::Irq)] {
        let mut unknown = offer(op, op * 4096);
        unknown.resources[0].kind = kind;
        assert_eq!(
            machine.offer(PROVIDER, op, unknown).failure,
            Fail::Unsupported
        );
    }
    let device = machine.offer(PROVIDER, 4, offer(1, 0)).snapshot.device;
    model.revoked.store(true, Ordering::SeqCst);
    assert_eq!(
        machine.reserve(DRIVER, 1, Reserve { device }).failure,
        Fail::Denied
    );
    model.revoked.store(false, Ordering::SeqCst);
    let binding = machine
        .reserve(DRIVER, 2, Reserve { device })
        .snapshot
        .binding;
    model.revoked.store(true, Ordering::SeqCst);
    assert_eq!(machine.begin_preparation(binding), Err(Fail::Denied));
}

#[test]
fn bounded_history_never_forgets_operations_or_unfinished_responsibility() {
    let model = Model::default();
    let mut machine = with(
        model.clone(),
        Limits {
            devices: 1,
            bindings: 1,
            operations: 4,
        },
    );
    let device = enroll(&mut machine);
    let binding = reserve(&mut machine, device);
    machine.begin_preparation(binding).unwrap();
    machine.release(DRIVER, 2, Release { binding });
    assert_eq!(
        machine.offer(PROVIDER, 2, offer(2, 4096)).failure,
        Fail::Full
    );
    assert_eq!(
        machine
            .reserve(TaskId::new(10), 1, Reserve { device })
            .failure,
        Fail::Full
    );
    assert_eq!(
        machine
            .reserve(DRIVER, 1, Reserve { device })
            .snapshot
            .binding,
        binding
    );
    assert_eq!(
        machine
            .inspect(
                DRIVER,
                Inspect {
                    operation: 1,
                    device
                }
            )
            .snapshot
            .pending,
        7
    );
    assert_eq!(model.admissions.load(Ordering::SeqCst), 1);
}

#[test]
fn malformed_ranges_and_offers_have_no_partial_admission() {
    let model = Model::default();
    let mut machine = with(model.clone(), Limits::default());
    let mut bad = offer(1, u64::MAX);
    assert_eq!(machine.offer(PROVIDER, 1, bad).failure, Fail::Invalid);
    bad = offer(1, 0);
    bad.count = 2;
    bad.resources[1] = bad.resources[0];
    assert_eq!(machine.offer(PROVIDER, 2, bad).failure, Fail::Conflict);
    assert_eq!(model.admissions.load(Ordering::SeqCst), 0);
    assert_eq!(machine.offer(PROVIDER, 3, offer(1, 0)).failure, Fail::None);
}

#[test]
fn all_resource_slots_require_their_own_exact_retirement_evidence() {
    let mut machine = machine();
    let mut offered = offer(1, 0);
    offered.count = MAX_RESOURCES as u8;
    for slot in 1..MAX_RESOURCES {
        offered.resources[slot] = offered.resources[0];
        offered.resources[slot].start = slot as u64 * 4096;
    }
    let device = machine.offer(PROVIDER, 1, offered).snapshot.device;
    let binding = reserve(&mut machine, device);
    let preparing = machine.begin_preparation(binding).unwrap();
    assert_eq!(preparing.pending, 0xfff);
    machine.release(DRIVER, 2, Release { binding });
    let mut wrong = proof(preparing, Duty::Device);
    wrong.allocation.slot = MAX_RESOURCES as u8;
    assert_eq!(machine.retire(OWNER, wrong), Err(Fail::Invalid));
    for slot in 0..MAX_RESOURCES {
        for duty in [Duty::Device, Duty::Authorization, Duty::Instance] {
            let mut evidence = proof(preparing, duty);
            evidence.allocation.slot = slot as u8;
            let observed = machine.retire(OWNER, evidence).unwrap();
            if slot + 1 == MAX_RESOURCES && duty == Duty::Instance {
                assert_eq!(observed.state, BindingState::Closed);
                assert_eq!(observed.pending, 0);
            } else {
                assert_eq!(observed.state, BindingState::Retiring);
                assert_ne!(observed.pending, 0);
                assert_eq!(
                    machine
                        .release(
                            DRIVER,
                            3 + slot as u64 * 3 + duty as u64,
                            Release { binding }
                        )
                        .snapshot
                        .state,
                    BindingState::Retiring
                );
            }
        }
    }
    assert_eq!(machine.failed(binding, 1).unwrap().fault, 0);
}
