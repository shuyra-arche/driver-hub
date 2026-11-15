//! CPU-only consumer fixture injected into the exact System handoff User ELF.
//! System's test-only heap, ingress and Driver roles are reused; Hub production
//! code owns the actual Instance coordinator and native Pend lease branches.
use super::*;
use alloc::rc::Rc;
use core::cell::Cell;
use hub_server::{
    driver::{self, Driver, Native},
    lease::{self, Correlation, Lease, Request, State},
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

fn poll<F: FnMut() -> Option<api::OperationId>>(
    driver: &mut Driver,
    native: &mut Native<F>,
    settled: bool,
) {
    loop {
        let progress = driver.progress();
        if if settled {
            progress.settled
        } else {
            progress.pending.is_none()
        } {
            break;
        }
        query_tick(budget());
        if let Err(error) = driver.poll(native, deadline()) {
            assert!(
                matches!(&error, driver::Error::Transport(driver::NativeError::Cleanup(error)) if error.is_busy()),
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
    let signal = sign::create(license).unwrap();
    let mut previous: Option<(Driver, operator::Resource, api::Observation, Lease)> = None;
    for round in 0..2 {
        cleanup(&mut native);
        // These fixture correlation IDs claim no Machine/device or Pend
        // authority; production Instance and lease effects use real owners.
        let binding = hub_api::BindingId {
            machine: hub_api::MachineId {
                incarnation: caller.get() as u64,
            },
            serial: round + 1,
        };
        let mut driver = Driver::new(binding, endpoint.owner.into()).unwrap();
        driver
            .create(
                &mut native,
                loader::Input::Unit(loader::UnitInput {
                    name: Name::new("driver").unwrap(),
                    stack: 512 * 1024,
                    arguments: loader::Arguments { values: vec![5] },
                }),
                subject.clone(),
                deadline(),
            )
            .unwrap();
        poll(&mut driver, &mut native, false);
        let target = driver.progress().instance.unwrap();
        let observed = driver.observe(&mut native, deadline()).unwrap();
        assert_eq!(observed.phase, api::Phase::Prepared);
        assert!(!observed.ready);
        assert_ne!(observed.team.get(), info.team.get() as u64);
        assert_ne!(observed.task.get(), caller.get() as u64);
        cleanup(&mut native);
        let actual = TaskId::new(observed.task.get() as usize);
        let maximum = Limits {
            operations: Operations::Sign(SignRights::TRIGGER),
            modifiable: Modifiable::No,
            distributable: Distributable::No,
            scope: PendScope::Object,
        };
        let correlation = Correlation {
            device: hub_api::DeviceRef {
                id: hub_api::DeviceId {
                    machine: binding.machine,
                    serial: round + 1,
                },
                epoch: 1,
            },
            allocation: hub_api::AllocationId { binding, slot: 0 },
            attempt: 1,
        };
        let request =
            Request::new(correlation, signal, actual, maximum, Mark::new(DEVICE)).unwrap();
        if round == 0 {
            cancelled_branch(caller, request, signal);
        }
        // The signal is a real local Pend, and this owner stays in the actual
        // controller Task. Correlation IDs confer no Machine/device authority.
        let mut lease = unsafe { Lease::new_for_current_task(caller, request) }.unwrap();
        while lease.delivery().is_none() {
            match lease.advance() {
                Ok(_) => (),
                Err(lease::Error::Busy) => tick(budget()),
                other => panic!("lease delivery: {other:?}"),
            }
        }
        let delivered = lease.delivery().unwrap();
        assert_eq!(delivered.recipient, actual);
        assert_eq!(delivered.correlation, correlation);
        for _ in 0..4 {
            assert_eq!(lease.advance().unwrap(), State::Delivered);
            assert_eq!(lease.delivery(), Some(delivered));
        }
        driver.start(&mut native, deadline()).unwrap();
        poll(&mut driver, &mut native, false);
        let observed = loop {
            let observed = driver.observe(&mut native, deadline()).unwrap();
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
            sign::listen(signal, Wait::Poll).unwrap()
        else {
            panic!("missing supplied signal")
        };
        sign::consume(signal, sequence).unwrap();
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
        if let Some((mut old, old_resource, old_observed, mut old_lease)) = previous.take() {
            assert_ne!(old.progress().instance, Some(target));
            ping(
                &old_resource,
                &old_observed,
                license,
                context.clone(),
                false,
            );
            retire(old_resource.token.try_into().unwrap());
            old.stop(&mut native, deadline()).unwrap();
            assert!(
                !old.progress().settled,
                "Stop cannot discharge Instance retirement"
            );
            poll(&mut old, &mut native, true);
            assert_eq!(
                old.observe(&mut native, deadline()).unwrap().phase,
                api::Phase::Retired
            );
            cleanup(&mut native);
            close_lease(&mut old_lease);
            ping(&resource, &observed, license, context.clone(), false);
        }
        previous = Some((driver, resource, observed, lease));
    }
    let (mut driver, resource, observed, mut lease) = previous.take().unwrap();
    close_lease(&mut lease);
    ping(&resource, &observed, license, context.clone(), true);
    retire(resource.token.try_into().unwrap());
    driver.close(&mut native, deadline()).unwrap();
    poll(&mut driver, &mut native, true);
    assert_eq!(
        driver.observe(&mut native, deadline()).unwrap().phase,
        api::Phase::Retired
    );
    cleanup(&mut native);
    retire(signal);
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
