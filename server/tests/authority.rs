use hub_api::{Fail, ResourceKind, ResourceSpec};
use hub_server::authority::native::Import;
use kernel_api::model::{
    identity::{PendId, TaskId},
    sign::{SignId, SignRights},
};

fn signal(pend: u64) -> ResourceSpec {
    ResourceSpec {
        kind: ResourceKind::Signal,
        domain: 7,
        start: 0,
        len: 1,
        pend,
        rights: SignRights::TRIGGER.bits(),
    }
}

#[test]
fn native_import_requires_exact_signal_resource_and_token_identity() {
    let canonical = unsafe {
        Import::new(
            signal(23),
            TaskId::new(1),
            TaskId::new(2),
            TaskId::new(3),
            SignId::new(5),
            PendId::mint(23),
        )
    }
    .unwrap();
    assert_eq!(canonical.resource().kind, ResourceKind::Signal);

    let wrong_token = unsafe {
        Import::new(
            signal(23),
            TaskId::new(1),
            TaskId::new(2),
            TaskId::new(3),
            SignId::new(5),
            PendId::mint(24),
        )
    };
    assert_eq!(wrong_token.err(), Some(Fail::Invalid));

    let hardware = unsafe {
        Import::new(
            ResourceSpec {
                kind: ResourceKind::Mmio,
                ..signal(23)
            },
            TaskId::new(1),
            TaskId::new(2),
            TaskId::new(3),
            SignId::new(5),
            PendId::mint(23),
        )
    };
    assert_eq!(hardware.err(), Some(Fail::Invalid));

    let wrong_rights = unsafe {
        Import::new(
            ResourceSpec {
                rights: SignRights::LISTEN.bits(),
                ..signal(23)
            },
            TaskId::new(1),
            TaskId::new(2),
            TaskId::new(3),
            SignId::new(5),
            PendId::mint(23),
        )
    };
    assert_eq!(wrong_rights.err(), Some(Fail::Invalid));
}
