use super::support::*;
use hub_api::*;
use hub_server::Limits;
use kernel_api::model::identity::TaskId;
use std::sync::atomic::Ordering;

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
