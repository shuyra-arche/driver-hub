//! CPU-only consumer fixture injected into the exact System handoff User ELF.
//! System's test-only heap, ingress, Sign source and Driver roles are reused;
//! Hub production code supplies the actual Instance coordinator and native SDK.
use super::*;
use alloc::rc::Rc;
use core::cell::Cell;
use hub_server::driver::{self, Driver, Native};

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
    let mut source = source::Source::new("handoff").unwrap();
    let mut previous: Option<(
        Driver,
        operator::Resource,
        api::Observation,
        supply::LeaseId,
    )> = None;
    for round in 0..2 {
        cleanup(&mut native);
        // This fixture exercises only the production Instance coordinator;
        // these correlation IDs claim no machine/device or Pend authority.
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
        let key = alloc::format!("signal{round}");
        source
            .register(source::Resource {
                name: &key,
                source: signal,
                maximum,
                caller,
                target: Some(actual),
                description: b"cpu signal",
            })
            .unwrap();
        let lease = supply::LeaseId::new(round + 1).unwrap();
        source
            .acquire(
                caller,
                supply::Acquire {
                    lease,
                    target: observed.task,
                    binding: supply::Binding {
                        source: supply::Key {
                            name: String::from("handoff"),
                        },
                        resource: supply::Key { name: key },
                        slot: system_protocol::Mark::new(DEVICE),
                        rights: Rights(maximum),
                    },
                },
            )
            .unwrap();
        while source.inspect(caller, lease).unwrap().state != supply::State::Granted {
            source.advance().unwrap();
            tick(budget());
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
        if let Some((mut old, old_resource, old_observed, _old_lease)) = previous.take() {
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
            ping(&resource, &observed, license, context.clone(), false);
        }
        previous = Some((driver, resource, observed, lease));
    }
    let (mut driver, resource, observed, lease) = previous.take().unwrap();
    source.release(caller, lease).unwrap();
    while source.inspect(caller, lease).unwrap().state != supply::State::Released {
        source.advance().unwrap();
        tick(budget());
    }
    ping(&resource, &observed, license, context.clone(), true);
    retire(resource.token.try_into().unwrap());
    driver.close(&mut native, deadline()).unwrap();
    poll(&mut driver, &mut native, true);
    assert_eq!(
        driver.observe(&mut native, deadline()).unwrap().phase,
        api::Phase::Retired
    );
    cleanup(&mut native);
    while !source.finish().unwrap() {
        tick(budget());
    }
    retire(signal);
    let _ = writeln!(
        Console,
        "PASS hub-consumer native=1 system-instance=1 prepared=1 roots=1 stop-not-retire=1 retired=1"
    );
    let _ = writeln!(
        Console,
        "PASS handoff recovery=1 instance=1 roots=1 cas=1 rpc=1 supply=1 revoke=1 stop=1 retire=1"
    );
}
