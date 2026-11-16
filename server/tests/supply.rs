#![allow(clippy::too_many_lines)]

use hub_api::{AllocationId, BindingId, DeviceId, DeviceRef, MachineId};
use hub_server::{
    lease,
    supply::{Command, Control, Error, NativeError, Response, State, Supply},
};
use kernel_api::model::{
    identity::{Mark, PendId, TaskId},
    modifiable::Modifiable,
    pend::{Distributable, Limits, Operations, PendScope},
    sign::SignRights,
    wait::Wait,
};
use std::vec::Vec;
use system_api::{
    self as sys,
    control::instance::{InstanceId, Sequence},
    supply::{
        self as api, Fail, InstallId, InstallState, InstallStatus, RegistrationId, SourceState,
        SourceStatus,
    },
    value::{Optional, Outcome},
};

const CALLER: TaskId = TaskId::new(31);
const PEER: TaskId = TaskId::new(32);
const TARGET: TaskId = TaskId::new(33);
const MARK: Mark = Mark::new(44);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MockError {
    LostReply,
    Busy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lose {
    None,
    Register,
    Install,
    Cancel,
}

struct Model {
    peer: sys::TaskId,
    caller: sys::TaskId,
    next_registration: u64,
    next_installation: u64,
    commands: Vec<Command>,
    lose: Lose,
    source: Option<SourceStatus>,
    install: Option<InstallStatus>,
    unknown_source_once: bool,
    unknown_install_once: bool,
    wrong_install_id: bool,
    busy_retire: bool,
    conflict_register: bool,
}

impl Model {
    fn new() -> Self {
        Self {
            peer: PEER.into(),
            caller: CALLER.into(),
            next_registration: 0,
            next_installation: 0,
            commands: Vec::new(),
            lose: Lose::None,
            source: None,
            install: None,
            unknown_source_once: false,
            unknown_install_once: false,
            wrong_install_id: false,
            busy_retire: false,
            conflict_register: false,
        }
    }

    fn source_status(&self, state: SourceState) -> SourceStatus {
        SourceStatus {
            id: RegistrationId::new(1).unwrap(),
            state,
        }
    }

    fn install_status(&self, state: InstallState) -> InstallStatus {
        let id = InstallId::new(if self.wrong_install_id { 99 } else { 1 }).unwrap();
        let delivered = sys::PendId::new(77);
        InstallStatus {
            id,
            state,
            instance: Optional(Some(instance())),
            target: TARGET.into(),
            slot: MARK.into(),
            delivery: Optional((state == InstallState::Granted).then_some(api::Delivery {
                token: delivered,
                description: Vec::new(),
            })),
            failure: Optional(None),
        }
    }
}

// SAFETY: this is an isolated Host protocol model. It has no Kernel effects
// and cannot claim that a real Pend root or System effect was retired.
unsafe impl Control for Model {
    type Error = MockError;
    fn peer(&self) -> sys::TaskId {
        self.peer
    }
    fn caller(&self) -> sys::TaskId {
        self.caller
    }
    fn allocate_registration(&mut self) -> Option<RegistrationId> {
        self.next_registration += 1;
        RegistrationId::new(self.next_registration)
    }
    fn allocate_installation(&mut self) -> Option<InstallId> {
        self.next_installation += 1;
        InstallId::new(self.next_installation)
    }
    fn execute(&mut self, command: Command, _: Wait) -> Result<Response, MockError> {
        self.commands.push(command.clone());
        Ok(match command {
            Command::CreateOffering { .. } => Response::OfferingRoot(PendId::mint(70)),
            Command::DelegateOffering { .. } => Response::Offered(sys::PendId::new(71)),
            Command::RetireOffering(_) => {
                if self.busy_retire {
                    return Err(MockError::Busy);
                }
                Response::Retired
            }
            Command::Register(_request) => {
                if self.conflict_register {
                    return Ok(Response::Source(Outcome::Failure(Fail::Conflict)));
                }
                let status = self.source_status(SourceState::Active);
                self.source = Some(status);
                if self.lose == Lose::Register {
                    self.lose = Lose::None;
                    return Err(MockError::LostReply);
                }
                Response::Source(Outcome::Success(status))
            }
            Command::InspectSource(_) => {
                if self.unknown_source_once {
                    self.unknown_source_once = false;
                    Response::Source(Outcome::Failure(Fail::Unknown))
                } else {
                    Response::Source(
                        self.source
                            .map_or(Outcome::Failure(Fail::Unknown), Outcome::Success),
                    )
                }
            }
            Command::Withdraw(_) => {
                let status = self.source_status(SourceState::Closed);
                self.source = Some(status);
                Response::Source(Outcome::Success(status))
            }
            Command::Install(_request) => {
                let status = self.install_status(InstallState::Granted);
                self.install = Some(status.clone());
                if self.lose == Lose::Install {
                    self.lose = Lose::None;
                    return Err(MockError::LostReply);
                }
                Response::Install(Outcome::Success(status))
            }
            Command::InspectInstall(_) => {
                if self.unknown_install_once {
                    self.unknown_install_once = false;
                    Response::Install(Outcome::Failure(Fail::Unknown))
                } else {
                    Response::Install(
                        self.install
                            .clone()
                            .map_or(Outcome::Failure(Fail::Unknown), Outcome::Success),
                    )
                }
            }
            Command::CancelInstall(_) => {
                let status = self.install_status(InstallState::Released);
                self.install = Some(status.clone());
                if self.lose == Lose::Cancel {
                    self.lose = Lose::None;
                    return Err(MockError::LostReply);
                }
                Response::Install(Outcome::Success(status))
            }
        })
    }
}

fn instance() -> InstanceId {
    InstanceId {
        system: PEER.into(),
        sequence: Sequence::new(5).unwrap(),
    }
}

fn request() -> lease::Request {
    lease::Request::new(
        lease::Correlation {
            device: DeviceRef {
                id: DeviceId {
                    machine: MachineId { incarnation: 1 },
                    serial: 2,
                },
                epoch: 3,
            },
            allocation: AllocationId {
                binding: BindingId {
                    machine: MachineId { incarnation: 1 },
                    serial: 4,
                },
                slot: 0,
            },
            attempt: 6,
        },
        PendId::mint(17),
        TARGET,
        Limits {
            operations: Operations::Sign(SignRights::TRIGGER),
            modifiable: Modifiable::No,
            distributable: Distributable::No,
            scope: PendScope::Object,
        },
        MARK,
    )
    .unwrap()
}

fn supply(model: &mut Model) -> Supply {
    let source = api::Key {
        name: "controller".into(),
    };
    let resource = api::Key {
        name: "binding-slot-0-attempt-6".into(),
    };
    // SAFETY: test model identities/names are stable and its counters are fresh.
    unsafe { Supply::new_for_current_task(request(), instance(), source, resource, model) }.unwrap()
}

fn advance_to_install(model: &mut Model, owner: &mut Supply) {
    owner.advance(model, Wait::Infinite).unwrap(); // private root
    owner.advance(model, Wait::Infinite).unwrap(); // System-local child
    owner.advance(model, Wait::Infinite).unwrap(); // register
}

#[test]
fn constructor_keeps_immutable_intent_and_unique_names() {
    let mut model = Model::new();
    let owner = supply(&mut model);
    assert_eq!(owner.request(), request());
    assert_eq!(owner.source_name(), "controller");
    assert_eq!(owner.resource_name(), "binding-slot-0-attempt-6");
    assert_eq!(owner.progress().state, State::Offering);
    assert_eq!(owner.progress().delivery, None);
    assert_eq!(model.commands, []);
}

#[test]
fn unknown_after_lost_register_replays_exact_registration() {
    let mut model = Model::new();
    model.lose = Lose::Register;
    model.unknown_source_once = true;
    let mut owner = supply(&mut model);
    owner.advance(&mut model, Wait::Infinite).unwrap();
    owner.advance(&mut model, Wait::Infinite).unwrap();
    assert!(matches!(
        owner.advance(&mut model, Wait::Infinite),
        Err(Error::Control(MockError::LostReply))
    ));
    assert_eq!(owner.progress().state, State::Registering);
    owner.advance(&mut model, Wait::Infinite).unwrap(); // inspect unknown
    owner.advance(&mut model, Wait::Infinite).unwrap(); // replay same id/request
    let registers: Vec<_> = model
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::Register(request) => Some(request),
            _ => None,
        })
        .collect();
    assert_eq!(registers.len(), 2);
    assert_eq!(registers[0], registers[1]);
    assert_eq!(registers[0].id, owner.progress().registration);
}

#[test]
fn delivery_is_exact_and_finish_closes_install_source_and_private_root() {
    let mut model = Model::new();
    let mut owner = supply(&mut model);
    advance_to_install(&mut model, &mut owner);
    let granted = owner.advance(&mut model, Wait::Infinite).unwrap();
    assert_eq!(granted.state, State::Granted);
    let delivery = granted.delivery.unwrap();
    assert_eq!(delivery.recipient, TARGET);
    assert_eq!(delivery.correlation, request().correlation());
    assert_eq!(delivery.token.get(), 77);

    assert_eq!(
        owner.finish(&mut model, Wait::Infinite).unwrap().state,
        State::Closing
    );
    assert_eq!(
        owner.advance(&mut model, Wait::Infinite).unwrap().state,
        State::Closing
    );
    assert_eq!(
        owner.advance(&mut model, Wait::Infinite).unwrap().state,
        State::Closed
    );
    assert!(matches!(
        model.commands.last(),
        Some(Command::RetireOffering(_))
    ));
}

#[test]
fn uncertain_install_recovery_is_sticky_closing_and_does_not_reinstall() {
    let mut model = Model::new();
    model.lose = Lose::Install;
    model.unknown_install_once = true;
    let mut owner = supply(&mut model);
    advance_to_install(&mut model, &mut owner);
    assert!(matches!(
        owner.advance(&mut model, Wait::Infinite),
        Err(Error::Control(MockError::LostReply))
    ));
    assert_eq!(
        owner.finish(&mut model, Wait::Infinite).unwrap().state,
        State::Closing
    );
    owner.advance(&mut model, Wait::Infinite).unwrap(); // inspect unknown
    owner.advance(&mut model, Wait::Infinite).unwrap(); // cancel creates tombstone
    owner.advance(&mut model, Wait::Infinite).unwrap(); // withdraw/close source
    owner.advance(&mut model, Wait::Infinite).unwrap(); // retire root
    assert_eq!(owner.progress().state, State::Closed);
    assert!(
        !model
            .commands
            .iter()
            .skip(5)
            .any(|command| matches!(command, Command::Install(_)))
    );
}

#[test]
fn accepted_install_cannot_be_replaced_by_cancel_tombstone_and_bad_ids_keep_duty() {
    let mut model = Model::new();
    let mut owner = supply(&mut model);
    advance_to_install(&mut model, &mut owner);
    owner.advance(&mut model, Wait::Infinite).unwrap();
    model.wrong_install_id = true;
    assert!(matches!(
        owner.finish(&mut model, Wait::Infinite),
        Err(Error::InvalidObservation)
    ));
    assert_eq!(owner.progress().state, State::Closing);
    assert_ne!(owner.progress().state, State::Closed);
}

#[test]
fn native_busy_classifier_does_not_treat_arbitrary_errors_as_retryable() {
    assert!(!NativeError::WrongPeer.is_busy());
    assert!(!NativeError::Caller.is_busy());
    assert!(NativeError::Kernel(kernel_api::abi::call::pend::PendFail::Busy).is_busy());
}

#[test]
fn wrong_control_task_keeps_the_owner_unadvanced() {
    let mut model = Model::new();
    let mut owner = supply(&mut model);
    model.caller = TaskId::new(88).into();
    assert!(matches!(
        owner.advance(&mut model, Wait::Infinite),
        Err(Error::Caller)
    ));
    assert_eq!(owner.progress().state, State::Offering);
    assert!(model.commands.is_empty());
}

#[test]
fn busy_root_retirement_keeps_closing_duty_until_retry_succeeds() {
    let mut model = Model::new();
    let mut owner = supply(&mut model);
    advance_to_install(&mut model, &mut owner);
    owner.advance(&mut model, Wait::Infinite).unwrap();
    owner.finish(&mut model, Wait::Infinite).unwrap();
    owner.advance(&mut model, Wait::Infinite).unwrap(); // close source
    model.busy_retire = true;
    assert!(matches!(
        owner.advance(&mut model, Wait::Infinite),
        Err(Error::Control(MockError::Busy))
    ));
    assert_eq!(owner.progress().state, State::Closing);
    model.busy_retire = false;
    assert_eq!(
        owner.finish(&mut model, Wait::Infinite).unwrap().state,
        State::Closed
    );
}

#[test]
fn admission_conflict_closes_the_retained_registration_and_local_root() {
    let mut model = Model::new();
    model.conflict_register = true;
    let mut owner = supply(&mut model);
    owner.advance(&mut model, Wait::Infinite).unwrap();
    owner.advance(&mut model, Wait::Infinite).unwrap();
    assert!(matches!(
        owner.advance(&mut model, Wait::Infinite),
        Err(Error::Remote(Fail::Conflict))
    ));
    assert_eq!(owner.progress().state, State::Closing);
    model.conflict_register = false;
    for _ in 0..4 {
        if owner.progress().state == State::Closed {
            break;
        }
        owner.finish(&mut model, Wait::Infinite).unwrap();
    }
    assert_eq!(owner.progress().state, State::Closed);
}
