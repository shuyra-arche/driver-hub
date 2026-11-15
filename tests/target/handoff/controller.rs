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
    machine::{DriverError, SupplyError},
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

type Hub = Machine<Authority>;

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

pub fn controller(license: PendId, context: Context) {
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
    hub.prepare_supply(
        failed_allocation,
        &mut native,
        Mark::new(DEVICE),
        deadline(),
    )
    .unwrap();
    cleanup(&mut native);
    assert_eq!(
        hub.advance_supply(failed_allocation).unwrap(),
        State::DeliveryPending
    );
    retire(imported[4]);
    assert!(matches!(
        hub.advance_supply(failed_allocation),
        Err(SupplyError::Lease(lease::Error::Kernel(
            kernel_api::abi::call::pend::PendFail::Denied
        )))
    ));
    assert_eq!(snapshot(&hub, caller, failed_device).pending, 7);
    close_supplies(&mut hub, failed_binding, 1);
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
            assert_eq!(
                hub.prepare_supply(allocation, &mut native, mark, deadline())
                    .unwrap(),
                State::RootPending
            );
            cleanup(&mut native);
            assert_eq!(
                hub.prepare_supply(allocation, &mut native, mark, deadline())
                    .unwrap(),
                State::RootPending
            );
            while hub.supply(allocation).unwrap().is_none() {
                match hub.advance_supply(allocation) {
                    Ok(_) => (),
                    Err(SupplyError::Lease(lease::Error::Busy)) => tick(budget()),
                    other => panic!("managed supply: {other:?}"),
                }
            }
            let delivered = hub.supply(allocation).unwrap().unwrap();
            assert_eq!(delivered.recipient.get() as u64, observed.task.get());
            assert_eq!(delivered.correlation.device, device);
            assert_eq!(delivered.correlation.allocation, allocation);
            for _ in 0..4 {
                assert_eq!(hub.advance_supply(allocation).unwrap(), State::Delivered);
                assert_eq!(hub.supply(allocation).unwrap(), Some(delivered));
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
            close_supplies(&mut hub, old, 2);
            retained(&mut hub, caller, &hub_ids, old_device, 9);
            ping(&resource, &observed, license, context.clone(), false);
        }
        previous = Some((binding, resource, observed, device));
    }
    let (binding, resource, observed, device) = previous.take().unwrap();
    close_supplies(&mut hub, binding, 2);
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
    assert_eq!(hub.settle(), 3); // three quarantined CPU devices, never reported reusable
    for token in &imported[..4] {
        retire(*token);
    }
    for signal in signals {
        retire(signal);
    }
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
fn close_supplies(hub: &mut Hub, binding: hub_api::BindingId, count: u8) {
    for slot in 0..count {
        let allocation = hub_api::AllocationId { binding, slot };
        loop {
            match hub.finish_supply(allocation) {
                Ok(State::Closed) => break,
                Err(SupplyError::Lease(lease::Error::Busy)) => tick(budget()),
                other => panic!("managed retirement: {other:?}"),
            }
        }
        assert_eq!(hub.supply(allocation).unwrap(), None);
        assert_eq!(hub.advance_supply(allocation).unwrap(), State::Closed);
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
