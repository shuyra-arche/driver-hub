use hub_api::{AllocationId, BindingId, DeviceId, DeviceRef, MAX_RESOURCES, MachineId};
use hub_server::lease::{Correlation, Error, Request};
use kernel_api::model::{
    identity::{Mark, PendId, TaskId},
    modifiable::Modifiable,
    nest::ReservationId,
    pend::{
        Distributable, FlatRights, Limits, NestRights, NestScope, Operations, PendRange, PendScope,
    },
    sign::SignRights,
};

fn correlation(slot: u8) -> Correlation {
    Correlation {
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
            slot,
        },
        attempt: 5,
    }
}

fn object_limits(distributable: Distributable) -> Limits {
    Limits {
        operations: Operations::Sign(SignRights::TRIGGER),
        modifiable: Modifiable::No,
        distributable,
        scope: PendScope::Object,
    }
}

#[test]
fn request_keeps_object_rights_and_requires_non_distributable_delivery() {
    let request = Request::new(
        correlation(0),
        PendId::mint(17),
        TaskId::new(23),
        object_limits(Distributable::No),
        Mark::new(9),
    )
    .unwrap();

    assert_eq!(request.correlation(), correlation(0));
    assert_eq!(request.recipient(), TaskId::new(23));
    assert_eq!(request.limits(), object_limits(Distributable::No));
    assert_eq!(request.recipient_limits(), object_limits(Distributable::No));
}

#[test]
fn ranged_delivery_inherits_the_private_roots_resolved_window() {
    let limits = Limits {
        operations: Operations::Flat(FlatRights::INSPECT),
        modifiable: Modifiable::No,
        distributable: Distributable::No,
        scope: PendScope::Pages(PendRange::pages(3, 2).unwrap()),
    };
    let request = Request::new(
        correlation(1),
        PendId::mint(17),
        TaskId::new(23),
        limits,
        Mark::new(10),
    )
    .unwrap();
    let child = request.recipient_limits();

    assert_eq!(child.operations, limits.operations);
    assert_eq!(child.modifiable, limits.modifiable);
    assert_eq!(child.distributable, Distributable::No);
    assert_eq!(child.scope, PendScope::Pages(PendRange::Inherit));
}

#[test]
fn invalid_correlation_authority_and_unsupported_nest_fail_closed() {
    assert!(matches!(
        Request::new(
            correlation(MAX_RESOURCES as u8),
            PendId::mint(17),
            TaskId::new(23),
            object_limits(Distributable::No),
            Mark::NONE,
        ),
        Err(Error::Invalid)
    ));

    let mut mismatched = correlation(0);
    mismatched.allocation.binding.machine.incarnation = 2;
    assert!(matches!(
        Request::new(
            mismatched,
            PendId::mint(17),
            TaskId::new(23),
            object_limits(Distributable::No),
            Mark::NONE,
        ),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        Request::new(
            correlation(0),
            PendId::NONE,
            TaskId::new(23),
            object_limits(Distributable::No),
            Mark::NONE,
        ),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        Request::new(
            correlation(0),
            PendId::mint(17),
            TaskId::new(23),
            object_limits(Distributable::Yes),
            Mark::NONE,
        ),
        Err(Error::Invalid)
    ));

    let nest = Limits {
        operations: Operations::Nest(NestRights::whole()),
        modifiable: Modifiable::No,
        distributable: Distributable::No,
        scope: PendScope::Nest(NestScope::Whole),
    };
    assert!(matches!(
        Request::new(
            correlation(0),
            PendId::mint(17),
            TaskId::new(23),
            nest,
            Mark::NONE,
        ),
        Err(Error::Unsupported)
    ));

    let reservation = NestScope::Reservation {
        id: ReservationId::from_bytes(&1u64.to_le_bytes()).unwrap(),
        range: PendRange::Inherit,
    };
    assert!(matches!(
        Request::new(
            correlation(0),
            PendId::mint(17),
            TaskId::new(23),
            Limits {
                scope: PendScope::Nest(reservation),
                ..nest
            },
            Mark::NONE,
        ),
        Err(Error::Unsupported)
    ));
}
