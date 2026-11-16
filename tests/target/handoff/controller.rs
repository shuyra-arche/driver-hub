//! CPU-only consumer fixture injected into the exact System handoff User ELF.
//! System's test-only heap, ingress and Driver roles are reused; Hub production
//! code owns the actual Instance coordinator and native Pend lease branches.
use super::*;
use alloc::rc::Rc;
use core::cell::Cell;
use hub_server::{
    Machine,
    authority::native::{Import, Native as Authority},
    driver::{self, Native},
    lease::{self, Correlation, Lease, Request, State},
    machine::{DriverError, InstallError},
    supply as remote,
};

fn next(ids: &Cell<u64>) -> api::OperationId {
    let value = ids.get().checked_add(1).unwrap();
    ids.set(value);
    api::OperationId::new(value).unwrap()
}

fn cleanup<F: FnMut() -> Option<api::OperationId>>(native: &mut Native<F>) {
    while let Err(error) = native.finish() {
        assert!(
            matches!(&error, driver::NativeError::Cleanup(error) if error.is_busy()),
            "{error:?}"
        );
        tick(budget());
    }
}

type Hub = Machine<Authority<remote::Supply>>;

fn poll<F: FnMut() -> Option<api::OperationId>>(
    hub: &mut Hub,
    binding: hub_api::BindingId,
    native: &mut Native<F>,
    settled: bool,
) {
    loop {
        let progress = hub.driver(binding).unwrap().unwrap().progress();
        if if settled {
            progress.settled
        } else {
            progress.pending.is_none()
        } {
            break;
        }
        query_tick(budget());
        if let Err(error) = hub.poll_driver(binding, native, deadline()) {
            assert!(
                matches!(&error, DriverError::Instance(driver::Error::Transport(driver::NativeError::Cleanup(error))) if error.is_busy()),
                "{error:?}"
            );
        }
    }
    cleanup(native);
}

fn close_lease(lease: &mut Lease) {
    loop {
        match lease.finish() {
            Ok(State::Closed) => break,
            Err(lease::Error::Busy) => tick(budget()),
            other => panic!("lease retirement: {other:?}"),
        }
    }
    assert!(lease.delivery().is_none());
    assert_eq!(lease.advance().unwrap(), State::Closed);
    assert_eq!(lease.finish().unwrap(), State::Closed);
}

// An ancestor may remove our private root before we can deliver it. A denied
// derive must preserve responsibility until actual Kernel settlement is known.
fn cancelled_branch(owner: TaskId, request: Request, signal: PendId) {
    let mut cancelled = unsafe { Lease::new_for_current_task(owner, request) }.unwrap();
    close_lease(&mut cancelled); // no effect was submitted

    let mut limits = request.limits();
    limits.distributable = Distributable::Yes;
    let bytes = AccordRequest {
        src: signal,
        target: AccordTarget::Caller,
        limits,
        mark: Mark::new(DEVICE),
    }
    .to_bytes();
    let ancestor = pend::accord(VirtAddr::new(bytes.as_ptr().addr())).unwrap();
    let request = Request::new(
        request.correlation(),
        ancestor,
        request.recipient(),
        request.limits(),
        Mark::new(DEVICE),
    )
    .unwrap();
    let mut interrupted = unsafe { Lease::new_for_current_task(owner, request) }.unwrap();
    assert_eq!(interrupted.advance().unwrap(), State::DeliveryPending);
    retire(ancestor);
    assert!(matches!(
        interrupted.advance(),
        Err(lease::Error::Kernel(
            kernel_api::abi::call::pend::PendFail::Denied
        ))
    ));
    assert_eq!(interrupted.state(), State::DeliveryPending);
    assert!(interrupted.delivery().is_none());
    close_lease(&mut interrupted);
}

#[derive(Debug)]
enum Fault {
    Native(remote::NativeError),
    LostReply,
}
struct Probe<C> {
    inner: C,
    lose_next: bool,
    just_lost: bool,
    lost: usize,
    queries: usize,
    accepted: Option<supply::InstallId>,
}
// This wrapper never fabricates a receipt: it discards one real authenticated
// Install reply after native submission, preserving the SDK cleanup in inner.
unsafe impl<C: remote::Control<Error = remote::NativeError>> remote::Control for Probe<C> {
    type Error = Fault;
    fn peer(&self) -> system_protocol::TaskId {
        self.inner.peer()
    }
    fn caller(&self) -> system_protocol::TaskId {
        self.inner.caller()
    }
    fn allocate_registration(&mut self) -> Option<supply::RegistrationId> {
        self.inner.allocate_registration()
    }
    fn allocate_installation(&mut self) -> Option<supply::InstallId> {
        self.inner.allocate_installation()
    }
    fn execute(&mut self, command: remote::Command, wait: Wait) -> Result<remote::Response, Fault> {
        let installed = match &command {
            remote::Command::Install(request) => Some(request.id),
            _ => None,
        };
        if matches!(command, remote::Command::InspectInstall(_)) {
            self.queries += 1;
        }
        let response = self.inner.execute(command, wait).map_err(Fault::Native)?;
        if let remote::Response::Install(Outcome::Success(status)) = &response {
            if matches!(
                status.state,
                supply::InstallState::Pending | supply::InstallState::Granted
            ) {
                self.accepted = Some(status.id);
                if installed == Some(status.id) && self.lose_next {
                    self.lose_next = false;
                    self.just_lost = true;
                    self.lost += 1;
                    return Err(Fault::LostReply);
                }
            }
        }
        Ok(response)
    }
}
fn supply_step<C: remote::Control<Error = remote::NativeError>>(
    hub: &mut Hub,
    allocation: hub_api::AllocationId,
    control: &mut Probe<C>,
    closing: bool,
) -> remote::Progress {
    let result = if closing {
        hub.finish_system_supply(allocation, control, deadline())
    } else {
        hub.advance_system_supply(allocation, control, deadline())
    };
    if control.just_lost {
        assert!(matches!(
            result,
            Err(InstallError::Supply(remote::Error::Control(
                Fault::LostReply
            )))
        ));
        control.just_lost = false;
    } else if let Err(error) = result {
        assert!(
            matches!(&error, InstallError::Supply(remote::Error::Control(Fault::Native(error))) if error.is_busy()),
            "{error:?}"
        );
    }
    tick(budget());
    hub.system_supply(allocation).unwrap()
}

pub fn controller(license: PendId, context: Context) {
    assert_eq!(kernel_api::abi::VERSION, 13);
    assert_eq!(kernel_api::boot::ledger::VERSION, 13);
    assert_eq!(kernel_api::boot::capsule::VERSION, 2);
    let endpoint = find(|info| info.mark == Mark::new(7));
    let info = own();
    let UnitTarget::Task(caller) = info.target else {
        panic!("Task")
    };
    let ids = Rc::new(Cell::new(0));
    let native_ids = ids.clone();
    // This controller's Native calls and its explicit Publish client below
    // share exactly one caller/System operation sequence through this Cell.
    let mut native = unsafe {
        Native::bind(
            endpoint.token,
            endpoint.owner,
            caller,
            license,
            context.clone(),
            move || Some(next(&native_ids)),
        )
    }
    .unwrap();
    let mut publications = system_client::instance::Client::bind(
        endpoint.token,
        endpoint.owner,
        license,
        context.clone(),
    )
    .unwrap();
    let resolver = find(|info| info.mark == Mark::of("system/identity/resolve"));
    let mut identity = system_client::identity::Client::bind(
        resolver.token,
        resolver.owner,
        license,
        context.clone(),
    )
    .unwrap();
    let subject = success(finish(
        identity
            .begin::<identity::operations::Resolve>(
                identity::Task {
                    task: caller.into(),
                },
                deadline(),
            )
            .unwrap(),
    ))
    .0
    .unwrap()
    .current;
    let supplied = find(|info| info.mark == Mark::new(8));
    assert_eq!(supplied.owner, endpoint.owner);
    let registrations = Rc::new(Cell::new(0u64));
    let installations = Rc::new(Cell::new(0u64));
    let reg_ids = registrations.clone();
    let install_ids = installations.clone();
    let supply_native = unsafe {
        remote::Native::bind(
            supplied.token,
            supplied.owner,
            caller,
            license,
            context.clone(),
            move || {
                let id = reg_ids.get().checked_add(1)?;
                reg_ids.set(id);
                supply::RegistrationId::new(id)
            },
            move || {
                let id = install_ids.get().checked_add(1)?;
                install_ids.set(id);
                supply::InstallId::new(id)
            },
        )
    }
    .unwrap();
    let mut supplied = Probe {
        inner: supply_native,
        lose_next: false,
        just_lost: false,
        lost: 0,
        queries: 0,
        accepted: None,
    };
    let mut discovery = discovery(license, context.clone());
    let mut signals = vec![];
    let mut imported = vec![];
    let mut imports = vec![];
    let mut resources = vec![];
    for _ in 0..5 {
        let signal = sign::create(license).unwrap();
        let source = inspect(signal);
        let Target::Sign(sign_id) = source.target else {
            panic!("Sign")
        };
        let bytes = AccordRequest {
            src: signal,
            target: AccordTarget::Caller,
            limits: Limits {
                operations: Operations::Sign(SignRights::TRIGGER),
                modifiable: Modifiable::No,
                distributable: Distributable::Yes,
                scope: PendScope::Object,
            },
            mark: Mark::new(DEVICE),
        }
        .to_bytes();
        let token = pend::accord(VirtAddr::new(bytes.as_ptr().addr())).unwrap();
        let resource = hub_api::ResourceSpec {
            kind: hub_api::ResourceKind::Signal,
            domain: sign_id.get() as u64,
            start: 0,
            len: 1,
            pend: token.get() as u64,
            rights: SignRights::TRIGGER.bits(),
        };
        // This fixture owns freshly-created direct CPU Signs. Their object IDs
        // are canonical domains within this Signal-only machine; no hardware
        // identity, IRQ route, or physical address is inferred from PendInfo.
        imports.push(
            unsafe { Import::new(resource, caller, caller, source.owner, sign_id, token) }.unwrap(),
        );
        signals.push(signal);
        imported.push(token);
        resources.push(resource);
    }
    let authority = unsafe { Authority::new_for_current_task(caller, imports) }.unwrap();
    // Fresh isolated CPU resources, no previous Hub domain or hardware duties.
    let mut hub = unsafe {
        Hub::new(
            hub_api::MachineId {
                incarnation: caller.get() as u64,
            },
            authority,
            hub_server::Limits::default(),
        )
    }
    .unwrap();
    let hub_ids = Cell::new(0);
    let probe = Request::new(
        Correlation {
            device: hub_api::DeviceRef {
                id: hub_api::DeviceId {
                    machine: hub_api::MachineId { incarnation: 99 },
                    serial: 99,
                },
                epoch: 1,
            },
            allocation: hub_api::AllocationId {
                binding: hub_api::BindingId {
                    machine: hub_api::MachineId { incarnation: 99 },
                    serial: 99,
                },
                slot: 0,
            },
            attempt: 1,
        },
        signals[0],
        caller,
        Limits {
            operations: Operations::Sign(SignRights::TRIGGER),
            modifiable: Modifiable::No,
            distributable: Distributable::No,
            scope: PendScope::Object,
        },
        Mark::new(DEVICE),
    )
    .unwrap();
    cancelled_branch(caller, probe, signals[0]);

    // Exercise managed failure after a private root exists: revoking its
    // imported ancestor must retain the Binding and settle only its own branch.
    let failed_device = offer(&mut hub, caller, &hub_ids, 3, &resources[4..5]);
    let failed_binding = reserve(&mut hub, caller, &hub_ids, failed_device);
    create(&mut hub, failed_binding, &mut native, subject.clone());
    let failed_allocation = hub_api::AllocationId {
        binding: failed_binding,
        slot: 0,
    };
    let pending = hub
        .prepare_system_supply(
            failed_allocation,
            &mut native,
            &mut supplied,
            "controller",
            Mark::new(DEVICE),
            deadline(),
        )
        .unwrap();
    cleanup(&mut native);
    while supplied.accepted != Some(pending.installation) {
        supply_step(&mut hub, failed_allocation, &mut supplied, false);
    }
    // This source was registered and Install was actually accepted before the
    // ancestor is revoked. System owns its partial definition/lease cleanup.
    retire(imported[4]);
    assert_eq!(snapshot(&hub, caller, failed_device).pending, 7);
    close_supplies(&mut hub, failed_binding, &mut supplied, 1);
    assert_eq!(snapshot(&hub, caller, failed_device).pending, 5);
    hub.close_driver(failed_binding, &mut native, deadline())
        .unwrap();
    poll(&mut hub, failed_binding, &mut native, true);
    retained(&mut hub, caller, &hub_ids, failed_device, 1);

    let mut previous: Option<(
        hub_api::BindingId,
        operator::Resource,
        api::Observation,
        hub_api::DeviceRef,
    )> = None;
    for round in 0..2 {
        cleanup(&mut native);
        let first = round as usize * 2;
        let device = offer(
            &mut hub,
            caller,
            &hub_ids,
            round + 1,
            &resources[first..first + 2],
        );
        // A numeric requester has no policy grant merely because it names a
        // real Task. This is policy rejection, not a transport-authentication test.
        assert_eq!(
            hub.reserve(endpoint.owner, 1 + round, hub_api::Reserve { device })
                .failure,
            hub_api::Fail::Denied
        );
        let binding = reserve(&mut hub, caller, &hub_ids, device);
        create(&mut hub, binding, &mut native, subject.clone());
        let target = hub
            .driver(binding)
            .unwrap()
            .unwrap()
            .progress()
            .instance
            .unwrap();
        let observed = hub
            .driver(binding)
            .unwrap()
            .unwrap()
            .observe(&mut native, deadline())
            .unwrap();
        assert_eq!(observed.phase, api::Phase::Prepared);
        assert!(!observed.ready);
        assert_ne!(observed.team.get(), info.team.get() as u64);
        assert_ne!(observed.task.get(), caller.get() as u64);
        cleanup(&mut native);
        assert!(matches!(
            hub.start_driver(binding, &mut native, deadline()),
            Err(DriverError::Machine(hub_api::Fail::Busy))
        ));
        for slot in 0..2 {
            let allocation = hub_api::AllocationId { binding, slot };
            let mark = Mark::new(DEVICE + u64::from(slot));
            let created = hub
                .prepare_system_supply(
                    allocation,
                    &mut native,
                    &mut supplied,
                    "controller",
                    mark,
                    deadline(),
                )
                .unwrap();
            assert_eq!(created.state, remote::State::Offering);
            cleanup(&mut native);
            assert_eq!(
                hub.prepare_system_supply(
                    allocation,
                    &mut native,
                    &mut supplied,
                    "controller",
                    mark,
                    deadline()
                )
                .unwrap(),
                created
            );
            if round == 0 && slot == 0 {
                supplied.lose_next = true;
            }
            while hub.system_supply(allocation).unwrap().delivery.is_none() {
                supply_step(&mut hub, allocation, &mut supplied, false);
            }
            let delivered = hub.system_supply(allocation).unwrap().delivery.unwrap();
            assert_eq!(delivered.recipient.get() as u64, observed.task.get());
            assert_eq!(delivered.correlation.device, device);
            assert_eq!(delivered.correlation.allocation, allocation);
            for _ in 0..4 {
                let replay = hub
                    .prepare_system_supply(
                        allocation,
                        &mut native,
                        &mut supplied,
                        "controller",
                        mark,
                        deadline(),
                    )
                    .unwrap();
                assert_eq!(replay.delivery, Some(delivered));
                assert_eq!(replay.registration, created.registration);
                assert_eq!(replay.installation, created.installation);
            }
            if slot == 0 {
                assert!(matches!(
                    hub.start_driver(binding, &mut native, deadline()),
                    Err(DriverError::Machine(hub_api::Fail::Busy))
                ));
            }
        }
        hub.start_driver(binding, &mut native, deadline()).unwrap();
        poll(&mut hub, binding, &mut native, false);
        let observed = loop {
            let observed = hub
                .driver(binding)
                .unwrap()
                .unwrap()
                .observe(&mut native, deadline())
                .unwrap();
            cleanup(&mut native);
            if observed.ready {
                break observed;
            }
            query_tick(budget());
        };
        assert_eq!(
            observed.root.0.as_ref().unwrap().registration.instance,
            target
        );
        assert_eq!(
            observed.root.0.as_ref().unwrap().phase,
            operator::unit::Phase::Active
        );
        let kernel_api::abi::call::sign::SignReply::Triggered { sequence } =
            sign::listen(signals[first], Wait::Poll).unwrap()
        else {
            panic!("missing supplied signal")
        };
        sign::consume(signals[first], sequence).unwrap();
        let publication = api::Publication {
            id: next(&ids),
            name: Name::new("driver").unwrap(),
            expected: round,
            instance: Optional(Some(target)),
        };
        let published = success(finish(
            publications
                .begin::<op::Publish>(publication, deadline())
                .unwrap(),
        ));
        assert_eq!(published.state, api::State::Completed);
        let resource = bind_resource(&mut discovery, target);
        ping(&resource, &observed, license, context.clone(), false);
        if let Some((old, old_resource, old_observed, old_device)) = previous.take() {
            assert_ne!(
                hub.driver(old).unwrap().unwrap().progress().instance,
                Some(target)
            );
            ping(
                &old_resource,
                &old_observed,
                license,
                context.clone(),
                false,
            );
            retire(old_resource.token.try_into().unwrap());
            hub.stop_driver(old, &mut native, deadline()).unwrap();
            assert!(
                !hub.driver(old).unwrap().unwrap().progress().settled,
                "Stop cannot discharge Instance retirement"
            );
            poll(&mut hub, old, &mut native, true);
            assert_eq!(
                hub.driver(old)
                    .unwrap()
                    .unwrap()
                    .observe(&mut native, deadline())
                    .unwrap()
                    .phase,
                api::Phase::Retired
            );
            cleanup(&mut native);
            close_supplies(&mut hub, old, &mut supplied, 2);
            retained(&mut hub, caller, &hub_ids, old_device, 9);
            ping(&resource, &observed, license, context.clone(), false);
        }
        previous = Some((binding, resource, observed, device));
    }
    let (binding, resource, observed, device) = previous.take().unwrap();
    close_supplies(&mut hub, binding, &mut supplied, 2);
    assert_eq!(snapshot(&hub, caller, device).pending, 45); // Device + Instance, both slots
    ping(&resource, &observed, license, context.clone(), true);
    retire(resource.token.try_into().unwrap());
    hub.close_driver(binding, &mut native, deadline()).unwrap();
    poll(&mut hub, binding, &mut native, true);
    assert_eq!(
        hub.driver(binding)
            .unwrap()
            .unwrap()
            .observe(&mut native, deadline())
            .unwrap()
            .phase,
        api::Phase::Retired
    );
    cleanup(&mut native);
    retained(&mut hub, caller, &hub_ids, device, 9);
    assert_eq!(supplied.lost, 1);
    assert!(supplied.queries > 0);
    while let Err(error) = supplied.inner.finish() {
        assert!(error.is_busy(), "{error:?}");
        tick(budget());
    }
    assert!(supplied.inner.is_finished());
    assert_eq!(hub.settle(), 3); // three quarantined CPU devices, never reported reusable
    for token in &imported[..4] {
        retire(*token);
    }
    for signal in signals {
        retire(signal);
    }
    let _ = writeln!(
        Console,
        "PASS hub-system-supply register=1 prepared-install=1 lost-reply=1 cancel=1 withdraw=1 system-lender=1"
    );
    let _ = writeln!(Console, "PASS hub-abi version=13 ledger=13 capsule=2");
    let _ = writeln!(
        Console,
        "PASS hub-machine native-authority=1 prepared-target=1 slots=2 managed-revoke=1 retained-device=1"
    );
    let _ = writeln!(
        Console,
        "PASS hub-lease native=1 replay=1 cancel=1 ancestor-revoke=1 withdrawal=1"
    );
    let _ = writeln!(
        Console,
        "PASS hub-consumer native=1 system-instance=1 prepared=1 roots=1 stop-not-retire=1 retired=1"
    );
    let _ = writeln!(
        Console,
        "PASS handoff recovery=1 instance=1 roots=1 cas=1 rpc=1 supply=1 revoke=1 stop=1 retire=1"
    );
}

fn offer(
    hub: &mut Hub,
    caller: TaskId,
    ids: &Cell<u64>,
    key: u64,
    resources: &[hub_api::ResourceSpec],
) -> hub_api::DeviceRef {
    let mut slots = [hub_api::ResourceSpec::EMPTY; hub_api::MAX_RESOURCES];
    slots[..resources.len()].copy_from_slice(resources);
    let reply = hub.offer(
        caller,
        next(ids).get(),
        hub_api::Offer {
            key,
            epoch: 1,
            count: resources.len() as u8,
            resources: slots,
        },
    );
    assert_eq!(reply.failure, hub_api::Fail::None);
    reply.snapshot.device
}
fn reserve(
    hub: &mut Hub,
    caller: TaskId,
    ids: &Cell<u64>,
    device: hub_api::DeviceRef,
) -> hub_api::BindingId {
    let reply = hub.reserve(caller, next(ids).get(), hub_api::Reserve { device });
    assert_eq!(reply.failure, hub_api::Fail::None);
    reply.snapshot.binding
}
fn create<F: FnMut() -> Option<api::OperationId>>(
    hub: &mut Hub,
    binding: hub_api::BindingId,
    native: &mut Native<F>,
    subject: identity::Subject,
) {
    hub.create_driver(
        binding,
        native,
        loader::Input::Unit(loader::UnitInput {
            name: Name::new("driver").unwrap(),
            stack: 512 * 1024,
            arguments: loader::Arguments { values: vec![5] },
        }),
        subject,
        deadline(),
    )
    .unwrap();
    poll(hub, binding, native, false);
}
fn snapshot(hub: &Hub, caller: TaskId, device: hub_api::DeviceRef) -> hub_api::Snapshot {
    let reply = hub.inspect(
        caller,
        hub_api::Inspect {
            operation: 0,
            device,
        },
    );
    assert_eq!(reply.failure, hub_api::Fail::None);
    reply.snapshot
}
fn close_supplies<C: remote::Control<Error = remote::NativeError>>(
    hub: &mut Hub,
    binding: hub_api::BindingId,
    control: &mut Probe<C>,
    count: u8,
) {
    for slot in 0..count {
        let allocation = hub_api::AllocationId { binding, slot };
        while supply_step(hub, allocation, control, true).state != remote::State::Closed {}
        let progress = hub.system_supply(allocation).unwrap();
        assert_eq!(progress.delivery, None);
        assert_eq!(
            hub.advance_system_supply(allocation, control, deadline())
                .unwrap(),
            progress
        );
    }
}

fn retained(
    hub: &mut Hub,
    caller: TaskId,
    ids: &Cell<u64>,
    device: hub_api::DeviceRef,
    expected: u16,
) {
    let current = snapshot(hub, caller, device);
    assert_eq!(current.pending, expected);
    assert_eq!(current.state, hub_api::BindingState::Retiring);
    let proof = hub_api::Retirement {
        device,
        allocation: hub_api::AllocationId {
            binding: current.binding,
            slot: 0,
        },
        attempt: current.attempt,
        duty: hub_api::Duty::Authorization,
        receipt: 1,
    };
    assert_eq!(hub.retire(caller, proof), Err(hub_api::Fail::Denied));
    assert_eq!(
        hub.retire(
            caller,
            hub_api::Retirement {
                duty: hub_api::Duty::Device,
                ..proof
            }
        ),
        Err(hub_api::Fail::Unsupported)
    );
    assert_eq!(
        hub.reserve(caller, next(ids).get(), hub_api::Reserve { device })
            .failure,
        hub_api::Fail::Busy
    );
    assert_eq!(snapshot(hub, caller, device).pending, expected);
}
