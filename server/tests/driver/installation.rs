//! Host coordination model; native permissions and RPCs are tested by QEMU.
use super::recovery::{Script, input, instance, result, subject};
use hub_api::*;
use hub_server::{
    Limits as MachineLimits, Machine,
    authority::{Authority, GrantRequest, SupplyAuthority},
    lease,
    machine::{DriverError, InstallError},
    supply::{self as remote, Command, Control, Response, State},
};
use kernel_api::model::{
    identity::{Mark, PendId, TaskId},
    modifiable::Modifiable,
    pend::{Distributable, Limits, Operations, PendScope},
    sign::SignRights,
    wait::Wait,
};
use system_api::{
    self as sys,
    control::instance as api,
    supply as wire,
    value::{Optional, Outcome},
};

struct Policy;
unsafe impl Authority for Policy {
    type Supply = remote::Supply;
    fn check_offer(&mut self, _: TaskId, _: &Offer) -> Result<(), Fail> {
        Ok(())
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
    fn check_start(
        &mut self,
        _: TaskId,
        _: TaskId,
        offer: &Offer,
        delivered: u8,
    ) -> Result<(), Fail> {
        if delivered == ((1 << offer.count) - 1) {
            Ok(())
        } else {
            Err(Fail::Busy)
        }
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
unsafe impl SupplyAuthority for Policy {
    fn owner(&self) -> TaskId {
        TaskId::new(10)
    }
    fn lease_request(&mut self, request: GrantRequest) -> Result<lease::Request, Fail> {
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

#[derive(Debug)]
struct Busy;
#[derive(Default)]
struct Provider {
    registrations: u64,
    installations: u64,
    commands: usize,
    registered: bool,
    install: Option<wire::Install>,
    busy_return: bool,
    wrong_status: bool,
    wrong_caller: bool,
}
impl Provider {
    fn status(&self, state: wire::InstallState) -> wire::InstallStatus {
        let request = self.install.as_ref().unwrap();
        wire::InstallStatus {
            id: request.id,
            state,
            instance: Optional(Some(request.instance)),
            target: if self.wrong_status {
                sys::TaskId::new(99)
            } else {
                request.target
            },
            slot: request.slot,
            failure: Optional(None),
            delivery: Optional(
                (state == wire::InstallState::Granted).then(|| wire::Delivery {
                    token: PendId::mint(40).into(),
                    description: vec![],
                }),
            ),
        }
    }
}
// Scripted owner receipts only: no real Kernel effects or physical grants.
unsafe impl Control for Provider {
    type Error = Busy;
    fn peer(&self) -> sys::TaskId {
        sys::TaskId::new(100)
    }
    fn caller(&self) -> sys::TaskId {
        sys::TaskId::new(if self.wrong_caller { 11 } else { 10 })
    }
    fn allocate_registration(&mut self) -> Option<wire::RegistrationId> {
        self.registrations += 1;
        wire::RegistrationId::new(self.registrations)
    }
    fn allocate_installation(&mut self) -> Option<wire::InstallId> {
        self.installations += 1;
        wire::InstallId::new(self.installations)
    }
    fn execute(&mut self, command: Command, _: Wait) -> Result<Response, Busy> {
        self.commands += 1;
        Ok(match command {
            Command::CreateOffering { .. } => Response::OfferingRoot(PendId::mint(18)),
            Command::DelegateOffering { .. } => Response::Offered(PendId::mint(19).into()),
            Command::RetireOffering(_) => {
                if self.busy_return {
                    self.busy_return = false;
                    return Err(Busy);
                }
                Response::Retired
            }
            Command::Register(request) => {
                self.registered = true;
                Response::Source(Outcome::Success(wire::SourceStatus {
                    id: request.id,
                    state: wire::SourceState::Active,
                }))
            }
            Command::InspectSource(request) => {
                Response::Source(Outcome::Success(wire::SourceStatus {
                    id: request.id,
                    state: if self.registered {
                        wire::SourceState::Active
                    } else {
                        wire::SourceState::Cancelled
                    },
                }))
            }
            Command::Withdraw(request) => {
                let state = if self.registered {
                    wire::SourceState::Closed
                } else {
                    wire::SourceState::Cancelled
                };
                self.registered = false;
                Response::Source(Outcome::Success(wire::SourceStatus {
                    id: request.id,
                    state,
                }))
            }
            Command::Install(request) => {
                self.install = Some(request);
                Response::Install(Outcome::Success(self.status(wire::InstallState::Granted)))
            }
            Command::InspectInstall(_) => {
                Response::Install(Outcome::Success(self.status(wire::InstallState::Granted)))
            }
            Command::CancelInstall(request) => {
                Response::Install(Outcome::Success(if self.install.is_some() {
                    self.status(wire::InstallState::Released)
                } else {
                    wire::InstallStatus::cancelled(request.id)
                }))
            }
        })
    }
}

fn prepared() -> (Machine<Policy>, Script, Provider, DeviceRef, AllocationId) {
    let mut hub = unsafe {
        Machine::new(
            MachineId { incarnation: 1 },
            Policy,
            MachineLimits::default(),
        )
    }
    .unwrap();
    let device = hub
        .offer(
            TaskId::new(7),
            1,
            Offer {
                key: 1,
                epoch: 1,
                count: 1,
                resources: [
                    ResourceSpec {
                        kind: ResourceKind::Signal,
                        domain: 1,
                        start: 0,
                        len: 1,
                        pend: 17,
                        rights: SignRights::TRIGGER.bits(),
                    },
                    ResourceSpec::EMPTY,
                    ResourceSpec::EMPTY,
                    ResourceSpec::EMPTY,
                ],
            },
        )
        .snapshot
        .device;
    let allocation = AllocationId {
        binding: hub
            .reserve(TaskId::new(10), 1, Reserve { device })
            .snapshot
            .binding,
        slot: 0,
    };
    let mut instance_control = Script::default();
    instance_control
        .responses
        .push_back(result(1, Some(instance(1)), api::State::Completed));
    hub.create_driver(
        allocation.binding,
        &mut instance_control,
        input(),
        subject(),
        Wait::Poll,
    )
    .unwrap();
    instance_control
        .responses
        .push_back(Ok(hub_server::driver::Response::Observation(
            Outcome::Success(api::Observation {
                instance: instance(1),
                phase: api::Phase::Prepared,
                ready: false,
                task: sys::TaskId::new(23),
                team: sys::TeamId::new(1000),
                reason: 0,
                failure: Optional(None),
                root: Optional(None),
            }),
        )));
    (
        hub,
        instance_control,
        Provider::default(),
        device,
        allocation,
    )
}
fn pending(hub: &Machine<Policy>, device: DeviceRef) -> u16 {
    hub.inspect(
        TaskId::new(10),
        Inspect {
            operation: 0,
            device,
        },
    )
    .snapshot
    .pending
}
fn close(hub: &mut Machine<Policy>, allocation: AllocationId, provider: &mut Provider) {
    for _ in 0..12 {
        if hub
            .finish_system_supply(allocation, provider, Wait::Poll)
            .unwrap()
            .state
            == State::Closed
        {
            return;
        }
    }
    panic!("model failed to close")
}

#[test]
fn system_intent_replay_and_pre_effect_cancel_preserve_independent_duties() {
    let (mut hub, mut instance_control, mut provider, device, allocation) = prepared();
    let first = hub
        .prepare_system_supply(
            allocation,
            &mut instance_control,
            &mut provider,
            "hub",
            Mark::new(7),
            Wait::Poll,
        )
        .unwrap();
    assert_eq!(first.state, State::Offering);
    assert_eq!(
        hub.prepare_system_supply(
            allocation,
            &mut instance_control,
            &mut provider,
            "hub",
            Mark::new(7),
            Wait::Poll
        )
        .unwrap(),
        first
    );
    assert!(matches!(
        hub.prepare_system_supply(
            allocation,
            &mut instance_control,
            &mut provider,
            "other",
            Mark::new(7),
            Wait::Poll
        ),
        Err(InstallError::Machine(Fail::Conflict))
    ));
    assert_eq!(
        (
            provider.registrations,
            provider.installations,
            provider.commands
        ),
        (1, 1, 0)
    );
    close(&mut hub, allocation, &mut provider);
    assert_eq!(provider.commands, 0);
    assert_eq!(pending(&hub, device), 5); // Device + Instance
    let proof = Retirement {
        device,
        allocation,
        attempt: 1,
        duty: Duty::Authorization,
        receipt: 1,
    };
    assert_eq!(hub.retire(TaskId::new(10), proof), Err(Fail::Denied));
    assert_eq!(
        hub.reserve(TaskId::new(10), 2, Reserve { device }).failure,
        Fail::Busy
    );
}

#[test]
fn remote_terminals_do_not_clear_authorization_until_local_root_settles() {
    let (mut hub, mut instance_control, mut provider, device, allocation) = prepared();
    hub.prepare_system_supply(
        allocation,
        &mut instance_control,
        &mut provider,
        "hub",
        Mark::new(7),
        Wait::Poll,
    )
    .unwrap();
    for _ in 0..4 {
        hub.advance_system_supply(allocation, &mut provider, Wait::Poll)
            .unwrap();
    }
    assert!(hub.system_supply(allocation).unwrap().delivery.is_some());
    provider.wrong_caller = true;
    assert!(matches!(
        hub.finish_system_supply(allocation, &mut provider, Wait::Poll),
        Err(InstallError::Supply(remote::Error::Caller))
    ));
    assert_eq!(pending(&hub, device), 7);
    assert_eq!(hub.system_supply(allocation).unwrap().delivery, None);
    provider.wrong_caller = false;
    provider.busy_return = true;
    loop {
        match hub.finish_system_supply(allocation, &mut provider, Wait::Poll) {
            Ok(progress) => assert_ne!(progress.state, State::Closed),
            Err(InstallError::Supply(remote::Error::Control(Busy))) => break,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(pending(&hub, device), 7);
    close(&mut hub, allocation, &mut provider);
    assert_eq!(pending(&hub, device), 5);
}

#[test]
fn wrong_install_target_cannot_admit_start_or_settle_a_slot() {
    let (mut hub, mut instance_control, mut provider, device, allocation) = prepared();
    hub.prepare_system_supply(
        allocation,
        &mut instance_control,
        &mut provider,
        "hub",
        Mark::new(7),
        Wait::Poll,
    )
    .unwrap();
    for _ in 0..3 {
        hub.advance_system_supply(allocation, &mut provider, Wait::Poll)
            .unwrap();
    }
    provider.wrong_status = true;
    assert!(matches!(
        hub.advance_system_supply(allocation, &mut provider, Wait::Poll),
        Err(InstallError::Supply(remote::Error::InvalidObservation))
    ));
    assert_eq!(pending(&hub, device), 7);
    assert_eq!(hub.system_supply(allocation).unwrap().delivery, None);
    assert!(matches!(
        hub.start_driver(allocation.binding, &mut instance_control, Wait::Poll),
        Err(DriverError::Machine(Fail::Busy))
    ));
    provider.wrong_status = false;
    close(&mut hub, allocation, &mut provider);
    assert_eq!(pending(&hub, device), 5);
}
