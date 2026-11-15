//! Intent-only Host tests. No native Kernel call or hardware proof is modeled.
use super::{
    recovery::{Script, input, instance, result, subject},
    support::PROVIDER,
};
use hub_api::*;
use hub_server::{
    Limits as MachineLimits, Machine,
    authority::{Authority, GrantRequest, SupplyAuthority},
    driver::Response,
    lease::{self, State},
    machine::SupplyError,
};
use kernel_api::model::{
    identity::{Mark, PendId, TaskId},
    modifiable::Modifiable,
    pend::{Distributable, Limits, Operations, PendScope},
    sign::SignRights,
    wait::Wait,
};
use std::{cell::Cell, rc::Rc};
use system_api::{
    control::instance as api,
    value::{Optional, Outcome},
};

#[derive(Clone, Default)]
struct IntentModel {
    requests: Rc<Cell<usize>>,
    denied: Rc<Cell<bool>>,
}

// The Host model supplies no native or device evidence. Tests below only
// prepare pure intent and cancel before the first possible native effect.
unsafe impl Authority for IntentModel {
    type Supply = lease::Lease;

    fn check_offer(&mut self, provider: TaskId, _: &Offer) -> Result<(), Fail> {
        if provider == PROVIDER {
            Ok(())
        } else {
            Err(Fail::Denied)
        }
    }
    fn check_reservation(
        &mut self,
        _: TaskId,
        _: TaskId,
        _: DeviceRef,
        _: &Offer,
    ) -> Result<(), Fail> {
        Ok(())
    }
    fn check_start(&mut self, _: TaskId, _: TaskId, _: &Offer, _: u8) -> Result<(), Fail> {
        Err(Fail::Unsupported)
    }
    fn check_retirement(
        &mut self,
        _: TaskId,
        _: &ResourceSpec,
        _: &Retirement,
    ) -> Result<(), Fail> {
        Err(Fail::Denied)
    }
}
unsafe impl SupplyAuthority for IntentModel {
    fn owner(&self) -> TaskId {
        TaskId::new(10)
    }
    fn lease_request(&mut self, request: GrantRequest) -> Result<lease::Request, Fail> {
        self.requests.set(self.requests.get() + 1);
        if self.denied.get() {
            return Err(Fail::Denied);
        }
        lease::Request::new(
            request.correlation,
            PendId::mint(17),
            request.recipient,
            Limits {
                operations: Operations::Sign(SignRights::TRIGGER),
                modifiable: Modifiable::No,
                distributable: Distributable::No,
                scope: PendScope::Object,
            },
            request.mark,
        )
        .map_err(|_| Fail::Invalid)
    }
}

fn ready(
    count: u8,
) -> (
    Machine<IntentModel>,
    Script,
    IntentModel,
    DeviceRef,
    AllocationId,
) {
    let model = IntentModel::default();
    let mut machine = unsafe {
        Machine::new(
            MachineId { incarnation: 1 },
            model.clone(),
            MachineLimits::default(),
        )
    }
    .unwrap();
    let mut resources = [ResourceSpec::EMPTY; MAX_RESOURCES];
    for (slot, resource) in resources[..usize::from(count)].iter_mut().enumerate() {
        *resource = ResourceSpec {
            kind: ResourceKind::Signal,
            domain: slot as u64 + 1,
            start: 0,
            len: 1,
            pend: 17,
            rights: SignRights::TRIGGER.bits(),
        };
    }
    let offer = machine.offer(
        PROVIDER,
        1,
        Offer {
            key: 1,
            epoch: 1,
            count,
            resources,
        },
    );
    assert_eq!(offer.failure, Fail::None);
    let device = offer.snapshot.device;
    let reserve = machine.reserve(TaskId::new(10), 1, Reserve { device });
    assert_eq!(reserve.failure, Fail::None);
    let allocation = AllocationId {
        binding: reserve.snapshot.binding,
        slot: 0,
    };
    let mut control = Script::default();
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Completed));
    machine
        .create_driver(
            allocation.binding,
            &mut control,
            input(),
            subject(),
            Wait::Poll,
        )
        .unwrap();
    (machine, control, model, device, allocation)
}
fn observed(control: &mut Script, phase: api::Phase, task: u64) {
    control
        .responses
        .push_back(Ok(Response::Observation(Outcome::Success(
            api::Observation {
                instance: instance(1),
                phase,
                ready: false,
                task: system_api::TaskId::new(task),
                team: system_api::TeamId::new(1000),
                reason: 0,
                failure: Optional(None),
                root: Optional(None),
            },
        ))));
}
fn snapshot(machine: &Machine<IntentModel>, device: DeviceRef) -> Snapshot {
    machine
        .inspect(
            TaskId::new(10),
            Inspect {
                operation: 0,
                device,
            },
        )
        .snapshot
}

#[test]
fn replay_keeps_one_intent_and_pre_effect_cancel_clears_only_owned_authorization() {
    let (mut machine, mut control, model, device, allocation) = ready(1);
    observed(&mut control, api::Phase::Prepared, 23);
    assert_eq!(
        machine
            .prepare_supply(allocation, &mut control, Mark::new(7), Wait::Poll)
            .unwrap(),
        State::RootPending
    );
    assert_eq!(
        machine
            .prepare_supply(allocation, &mut control, Mark::new(7), Wait::Poll)
            .unwrap(),
        State::RootPending
    );
    assert!(matches!(
        machine.prepare_supply(allocation, &mut control, Mark::new(8), Wait::Poll),
        Err(SupplyError::Machine(Fail::Conflict))
    ));
    assert_eq!(model.requests.get(), 1);
    assert_eq!(snapshot(&machine, device).pending, 7);
    assert_eq!(machine.finish_supply(allocation).unwrap(), State::Closed);
    assert_eq!(machine.finish_supply(allocation).unwrap(), State::Closed);
    assert_eq!(snapshot(&machine, device).pending, 5); // Device + Instance
    assert_eq!(machine.supply(allocation), Ok(None));
    let state = snapshot(&machine, device);
    let forged = Retirement {
        device,
        allocation,
        attempt: state.attempt,
        duty: Duty::Authorization,
        receipt: 1,
    };
    assert_eq!(machine.retire(TaskId::new(10), forged), Err(Fail::Denied));
    assert_eq!(
        machine
            .reserve(TaskId::new(10), 2, Reserve { device })
            .failure,
        Fail::Busy
    );
    assert!(matches!(
        machine.prepare_supply(allocation, &mut control, Mark::new(7), Wait::Poll),
        Err(SupplyError::Machine(Fail::Conflict))
    ));
}

#[test]
fn rejected_or_unprepared_target_never_acquires_an_intent_or_discharges_unknown_duty() {
    let (mut machine, mut control, model, device, allocation) = ready(1);
    observed(&mut control, api::Phase::Running, 23);
    assert!(matches!(
        machine.prepare_supply(allocation, &mut control, Mark::new(7), Wait::Poll),
        Err(SupplyError::Machine(Fail::Conflict))
    ));
    observed(&mut control, api::Phase::Prepared, 0);
    assert!(matches!(
        machine.prepare_supply(allocation, &mut control, Mark::new(7), Wait::Poll),
        Err(SupplyError::Machine(Fail::Denied))
    ));
    assert_eq!(model.requests.get(), 0);
    model.denied.set(true);
    observed(&mut control, api::Phase::Prepared, 23);
    assert!(matches!(
        machine.prepare_supply(allocation, &mut control, Mark::new(7), Wait::Poll),
        Err(SupplyError::Machine(Fail::Denied))
    ));
    machine.release(
        TaskId::new(10),
        2,
        Release {
            binding: allocation.binding,
        },
    );
    assert!(matches!(
        machine.advance_supply(allocation),
        Err(SupplyError::Machine(Fail::Unknown))
    ));
    assert_eq!(snapshot(&machine, device).pending, 7);
}

#[test]
fn partial_slot_preparation_preserves_other_slots_and_duplicate_marks_are_rejected() {
    let (mut machine, mut control, model, device, allocation) = ready(2);
    observed(&mut control, api::Phase::Prepared, 23);
    machine
        .prepare_supply(allocation, &mut control, Mark::new(7), Wait::Poll)
        .unwrap();
    let second = AllocationId {
        slot: 1,
        ..allocation
    };
    assert!(matches!(
        machine.prepare_supply(second, &mut control, Mark::new(7), Wait::Poll),
        Err(SupplyError::Machine(Fail::Conflict))
    ));
    assert_eq!(model.requests.get(), 1);
    model.denied.set(true);
    observed(&mut control, api::Phase::Prepared, 23);
    assert!(matches!(
        machine.prepare_supply(second, &mut control, Mark::new(8), Wait::Poll),
        Err(SupplyError::Machine(Fail::Denied))
    ));
    assert_eq!(machine.finish_supply(allocation).unwrap(), State::Closed);
    assert_eq!(snapshot(&machine, device).pending, 61); // only slot 0 Authorization
    assert!(matches!(
        machine.advance_supply(second),
        Err(SupplyError::Machine(Fail::Unknown))
    ));
    assert_eq!(snapshot(&machine, device).pending, 61);
}

#[test]
fn revoked_start_policy_does_not_intercept_an_existing_operation_journal() {
    use super::support;
    use hub_server::{driver, machine::DriverError};

    let model = support::Model::default();
    let mut machine = support::with(model.clone(), MachineLimits::default());
    let device = support::enroll(&mut machine);
    let binding = support::reserve(&mut machine, device);
    let mut control = Script::default();
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Completed));
    machine
        .create_driver(binding, &mut control, input(), subject(), Wait::Poll)
        .unwrap();
    control.responses.push_back(Err(()));
    assert!(matches!(
        machine.start_driver(binding, &mut control, Wait::Poll),
        Err(DriverError::Instance(driver::Error::Transport(())))
    ));
    model
        .revoked
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        machine.start_driver(binding, &mut control, Wait::Poll),
        Err(DriverError::Instance(driver::Error::Conflict))
    ));
    control
        .responses
        .push_back(result(2, Some(instance(1)), api::State::Completed));
    let progress = machine
        .poll_driver(binding, &mut control, Wait::Poll)
        .unwrap();
    assert!(progress.pending.is_none());
    assert_eq!(progress.instance, Some(instance(1)));
    // A fresh Start is still gated by the now-revoked policy.
    assert!(matches!(
        machine.start_driver(binding, &mut control, Wait::Poll),
        Err(DriverError::Machine(Fail::Denied))
    ));
}
