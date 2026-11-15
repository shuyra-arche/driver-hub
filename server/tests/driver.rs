mod support;

use hub_api::*;
use hub_server::{
    driver::{self, Command, Control, Driver, Kind, Response},
    machine::DriverError,
};
use kernel_api::model::wait::Wait;
use std::collections::VecDeque;
use support::*;
use system_api::{
    control::instance as api,
    identity::{PrincipalId, Subject},
    loader,
    value::{Optional, Outcome},
};

struct Script {
    peer: system_api::TaskId,
    caller: system_api::TaskId,
    next: u64,
    not_submitted: bool,
    responses: VecDeque<Result<Response, ()>>,
    seen: Vec<(&'static str, u64, Option<api::InstanceId>)>,
}
impl Default for Script {
    fn default() -> Self {
        Self {
            peer: system_api::TaskId::new(100),
            caller: system_api::TaskId::new(10),
            next: 0,
            not_submitted: false,
            responses: VecDeque::new(),
            seen: Vec::new(),
        }
    }
}
// Host-only authenticated-peer model, not Kernel/System execution evidence.
unsafe impl Control for Script {
    type Error = ();
    fn peer(&self) -> system_api::TaskId {
        self.peer
    }
    fn caller(&self) -> system_api::TaskId {
        self.caller
    }
    fn allocate(&mut self) -> Option<api::OperationId> {
        self.next = self.next.checked_add(1)?;
        api::OperationId::new(self.next)
    }
    fn execute(&mut self, command: Command, _: Wait) -> Result<Response, ()> {
        self.seen.push(match command {
            Command::Create(r) => ("create", r.id.get(), None),
            Command::Start(r) => ("start", r.id.get(), Some(r.instance)),
            Command::Stop(r) => ("stop", r.id.get(), Some(r.instance)),
            Command::Retire(r) => ("retire", r.id.get(), Some(r.instance)),
            Command::Cancel(id) => ("cancel", id.get(), None),
            Command::InspectOperation(id) => ("query", id.get(), None),
            Command::Inspect(id) => ("inspect", 0, Some(id)),
        });
        self.responses.pop_front().expect("unexpected effect")
    }
    fn may_have_submitted(&self, _: &()) -> bool {
        !self.not_submitted
    }
}
fn id(n: u64) -> api::OperationId {
    api::OperationId::new(n).unwrap()
}
fn instance(n: u64) -> api::InstanceId {
    api::InstanceId {
        system: system_api::TaskId::new(100),
        sequence: api::Sequence::new(n).unwrap(),
    }
}
fn input() -> loader::Input {
    loader::Input::Unit(loader::UnitInput {
        name: system_api::Name::new("driver").unwrap(),
        stack: 4096,
        arguments: loader::Arguments { values: vec![] },
    })
}
fn subject() -> Subject {
    Subject::new(
        PrincipalId {
            authority: system_api::TaskId::new(2),
            slot: 1,
        },
        &[],
    )
    .unwrap()
}
fn result(n: u64, target: Option<api::InstanceId>, state: api::State) -> Result<Response, ()> {
    Ok(Response::Receipt(Outcome::Success(api::Receipt {
        id: id(n),
        instance: Optional(target),
        state,
        failure: Optional(None),
    })))
}
fn rejected(n: u64, target: Option<api::InstanceId>) -> Result<Response, ()> {
    Ok(Response::Receipt(Outcome::Success(api::Receipt {
        id: id(n),
        instance: Optional(target),
        state: api::State::Failed,
        failure: Optional(Some(weave::Coded(api::Fail::Denied))),
    })))
}
fn binding() -> BindingId {
    BindingId {
        machine: MachineId { incarnation: 1 },
        serial: 1,
    }
}

#[test]
fn uncertain_create_queries_original_operation_and_unknown_does_not_forget_it() {
    let mut control = Script::default();
    let mut driver = Driver::new(binding(), control.peer).unwrap();
    control.responses.push_back(Err(()));
    assert_eq!(
        driver.create(&mut control, input(), subject(), Wait::Poll),
        Err(driver::Error::Transport(()))
    );
    control
        .responses
        .push_back(Ok(Response::Receipt(Outcome::Failure(api::Fail::Unknown))));
    assert_eq!(
        driver.poll(&mut control, Wait::Poll),
        Err(driver::Error::Unseen)
    );
    assert_eq!(driver.progress().pending.unwrap().id, id(1));
    assert!(!driver.progress().settled);
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Pending));
    driver.poll(&mut control, Wait::Poll).unwrap();
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Completed));
    driver.poll(&mut control, Wait::Poll).unwrap();
    assert_eq!(driver.progress().instance, Some(instance(1)));
    assert!(driver.progress().pending.is_none());
    assert_eq!(
        control.seen.iter().map(|call| call.0).collect::<Vec<_>>(),
        ["create", "query", "query", "query"]
    );
    assert_eq!(control.next, 1);
}

#[test]
fn cancellation_after_completed_create_uses_fresh_retirement_and_never_start() {
    let mut control = Script::default();
    let mut driver = Driver::new(binding(), control.peer).unwrap();
    control.responses.push_back(Err(()));
    driver
        .create(&mut control, input(), subject(), Wait::Poll)
        .unwrap_err();
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Completed));
    let completed_create = driver.close(&mut control, Wait::Poll).unwrap();
    assert!(!completed_create.settled);
    assert!(completed_create.closing);
    assert_eq!(
        driver.start(&mut control, Wait::Poll),
        Err(driver::Error::Conflict)
    );
    control
        .responses
        .push_back(result(2, Some(instance(1)), api::State::Pending));
    driver.poll(&mut control, Wait::Poll).unwrap();
    control
        .responses
        .push_back(result(2, Some(instance(1)), api::State::Completed));
    assert!(driver.poll(&mut control, Wait::Poll).unwrap().settled);
    assert_eq!(
        control.seen,
        [
            ("create", 1, None),
            ("cancel", 1, None),
            ("retire", 2, Some(instance(1))),
            ("query", 2, None)
        ]
    );
}

#[test]
fn cancelled_pending_operation_is_distinct_from_unknown_unseen_id() {
    let mut control = Script::default();
    let mut driver = Driver::new(binding(), control.peer).unwrap();
    control.responses.push_back(Err(()));
    driver
        .create(&mut control, input(), subject(), Wait::Poll)
        .unwrap_err();
    control
        .responses
        .push_back(Ok(Response::Receipt(Outcome::Failure(api::Fail::Unknown))));
    driver.close(&mut control, Wait::Poll).unwrap_err();
    assert!(!driver.progress().settled);
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Cancelled));
    assert!(driver.poll(&mut control, Wait::Poll).unwrap().settled);
    assert_eq!(control.next, 1);
}

#[test]
fn receipt_id_issuer_and_instance_mismatch_never_change_association() {
    let mut control = Script::default();
    let mut driver = Driver::new(binding(), control.peer).unwrap();
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Pending));
    driver
        .create(&mut control, input(), subject(), Wait::Poll)
        .unwrap();
    let before = driver.progress();
    control
        .responses
        .push_back(result(2, Some(instance(1)), api::State::Completed));
    assert_eq!(
        driver.poll(&mut control, Wait::Poll),
        Err(driver::Error::WrongReceipt)
    );
    control
        .responses
        .push_back(result(1, Some(instance(2)), api::State::Completed));
    assert_eq!(
        driver.poll(&mut control, Wait::Poll),
        Err(driver::Error::WrongReceipt)
    );
    let foreign = api::InstanceId {
        system: system_api::TaskId::new(200),
        ..instance(1)
    };
    control
        .responses
        .push_back(result(1, Some(foreign), api::State::Completed));
    assert_eq!(
        driver.poll(&mut control, Wait::Poll),
        Err(driver::Error::WrongReceipt)
    );
    control.responses.push_back(rejected(1, None));
    assert_eq!(
        driver.poll(&mut control, Wait::Poll),
        Err(driver::Error::WrongReceipt)
    );
    assert_eq!(driver.progress(), before);
}

#[test]
fn failed_retirement_retains_identity_until_explicit_fresh_retry_completes() {
    let mut control = Script::default();
    let mut driver = Driver::new(binding(), control.peer).unwrap();
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Completed));
    driver
        .create(&mut control, input(), subject(), Wait::Poll)
        .unwrap();
    control.responses.push_back(rejected(2, Some(instance(1))));
    driver.close(&mut control, Wait::Poll).unwrap_err();
    let failed = driver.progress();
    assert!(!failed.settled);
    assert_eq!(failed.pending.unwrap().kind, Kind::Retire);
    assert!(failed.pending.unwrap().rejected);
    control
        .responses
        .push_back(result(3, Some(instance(1)), api::State::Pending));
    driver.retry_retirement(&mut control, Wait::Poll).unwrap();
    assert!(!driver.progress().settled);
    control
        .responses
        .push_back(result(3, Some(instance(1)), api::State::Completed));
    assert!(driver.poll(&mut control, Wait::Poll).unwrap().settled);
    assert_eq!(control.seen[2], ("retire", 3, Some(instance(1))));
}

#[test]
fn never_submitted_driver_can_settle_locally_but_uncertain_driver_cannot() {
    let mut control = Script {
        next: u64::MAX,
        ..Script::default()
    };
    let mut driver = Driver::new(binding(), control.peer).unwrap();
    assert_eq!(
        driver.create(&mut control, input(), subject(), Wait::Poll),
        Err(driver::Error::Exhausted)
    );
    assert!(driver.close(&mut control, Wait::Poll).unwrap().settled);
    assert!(control.seen.is_empty());
    let mut control = Script::default();
    let mut driver = Driver::new(binding(), system_api::TaskId::new(200)).unwrap();
    assert_eq!(
        driver.create(&mut control, input(), subject(), Wait::Poll),
        Err(driver::Error::WrongPeer)
    );
    assert!(driver.close(&mut control, Wait::Poll).unwrap().settled);
    assert!(control.seen.is_empty());
}

#[test]
fn cleanup_blocked_before_submission_does_not_become_an_unqueryable_create() {
    let mut control = Script {
        not_submitted: true,
        ..Script::default()
    };
    let mut driver = Driver::new(binding(), control.peer).unwrap();
    control.responses.push_back(Err(()));
    driver
        .create(&mut control, input(), subject(), Wait::Poll)
        .unwrap_err();
    assert!(driver.progress().pending.is_none());
    assert!(driver.close(&mut control, Wait::Poll).unwrap().settled);
}

#[test]
fn another_caller_on_same_system_cannot_substitute_its_terminal_receipt() {
    let mut first = Script::default();
    let mut driver = Driver::new(binding(), first.peer).unwrap();
    first.responses.push_back(Err(()));
    driver
        .create(&mut first, input(), subject(), Wait::Poll)
        .unwrap_err();
    let original = driver.progress();
    let mut other = Script {
        caller: system_api::TaskId::new(20),
        ..Script::default()
    };
    other
        .responses
        .push_back(result(1, Some(instance(9)), api::State::Cancelled));
    assert_eq!(
        driver.close(&mut other, Wait::Poll),
        Err(driver::Error::WrongCaller)
    );
    assert_eq!(driver.progress(), original);
    assert!(other.seen.is_empty());
    first
        .responses
        .push_back(Ok(Response::Receipt(Outcome::Failure(api::Fail::Unknown))));
    assert_eq!(
        driver.close(&mut first, Wait::Poll),
        Err(driver::Error::Unseen)
    );
    assert!(!driver.progress().settled);
}

#[test]
fn machine_can_retry_known_unsent_create_and_can_close_without_submitted_work() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    let binding = reserve(&mut machine, device);
    let mut control = Script {
        not_submitted: true,
        ..Script::default()
    };
    control.responses.push_back(Err(()));
    machine
        .create_driver(binding, &mut control, input(), subject(), Wait::Poll)
        .unwrap_err();
    control.not_submitted = false;
    control
        .responses
        .push_back(result(2, Some(instance(1)), api::State::Pending));
    let retried = machine
        .create_driver(binding, &mut control, input(), subject(), Wait::Poll)
        .unwrap();
    assert_eq!(retried.pending.unwrap().id, id(2));

    let mut machine = support::machine();
    let device = enroll(&mut machine);
    let binding = reserve(&mut machine, device);
    let mut exhausted = Script {
        next: u64::MAX,
        ..Script::default()
    };
    machine
        .create_driver(binding, &mut exhausted, input(), subject(), Wait::Poll)
        .unwrap_err();
    assert!(
        machine
            .close_driver(binding, &mut exhausted, Wait::Poll)
            .unwrap()
            .settled
    );
    assert!(exhausted.seen.is_empty());
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
        3
    );
}

#[test]
fn exact_failed_create_without_instance_proves_only_absence_of_instance() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    let binding = reserve(&mut machine, device);
    let mut control = Script::default();
    control.responses.push_back(rejected(1, None));
    assert_eq!(
        machine.create_driver(binding, &mut control, input(), subject(), Wait::Poll),
        Err(DriverError::Instance(driver::Error::Rejected(
            api::Fail::Denied
        )))
    );
    let progress = machine.driver(binding).unwrap().unwrap().progress();
    assert!(progress.settled);
    let observation = machine.inspect(
        DRIVER,
        Inspect {
            operation: 1,
            device,
        },
    );
    assert_eq!(observation.snapshot.pending, 3);
    assert_eq!(observation.snapshot.state, BindingState::Retiring);
}

#[test]
fn system_retirement_clears_all_instance_slots_and_no_device_or_authorization_duty() {
    let mut machine = machine();
    let mut offer = offer(1, 0);
    offer.count = MAX_RESOURCES as u8;
    for slot in 1..MAX_RESOURCES {
        offer.resources[slot] = offer.resources[0];
        offer.resources[slot].start = slot as u64 * 4096;
    }
    let device = machine.offer(PROVIDER, 1, offer).snapshot.device;
    let binding = reserve(&mut machine, device);
    let mut control = Script::default();
    control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Completed));
    machine
        .create_driver(binding, &mut control, input(), subject(), Wait::Poll)
        .unwrap();
    control
        .responses
        .push_back(result(2, Some(instance(1)), api::State::Completed));
    machine
        .start_driver(binding, &mut control, Wait::Poll)
        .unwrap();
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
            .state,
        BindingState::Preparing
    );
    control
        .responses
        .push_back(result(3, Some(instance(1)), api::State::Completed));
    machine
        .stop_driver(binding, &mut control, Wait::Poll)
        .unwrap();
    let before_retire = machine
        .inspect(
            DRIVER,
            Inspect {
                operation: 1,
                device,
            },
        )
        .snapshot;
    assert_eq!(before_retire.pending, 0xfff);
    assert_eq!(
        machine.retire(OWNER, proof(before_retire, Duty::Instance)),
        Err(Fail::Denied)
    );
    control
        .responses
        .push_back(result(4, Some(instance(1)), api::State::Pending));
    machine
        .poll_driver(binding, &mut control, Wait::Poll)
        .unwrap();
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
        0xfff
    );
    control
        .responses
        .push_back(result(4, Some(instance(1)), api::State::Completed));
    machine
        .poll_driver(binding, &mut control, Wait::Poll)
        .unwrap();
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
        0x6db
    );
    for slot in 0..MAX_RESOURCES {
        for duty in [Duty::Device, Duty::Authorization] {
            let mut evidence = proof(before_retire, duty);
            evidence.allocation.slot = slot as u8;
            machine.retire(OWNER, evidence).unwrap();
        }
    }
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
            .state,
        BindingState::Closed
    );
}
