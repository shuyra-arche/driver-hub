#![allow(dead_code)]

use hub_api::*;
use hub_server::{Limits, Machine, authority::Authority};
use kernel_api::model::identity::TaskId;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub const PROVIDER: TaskId = TaskId::new(7);
pub const DRIVER: TaskId = TaskId::new(9);
pub const OWNER: TaskId = TaskId::new(11);

#[derive(Clone, Default)]
pub struct Model {
    pub revoked: Arc<AtomicBool>,
    pub busy: Arc<AtomicBool>,
    pub admissions: Arc<AtomicUsize>,
}

// Host-only owner model: it tests the coordinator's boundary calls and retained
// duties. It deliberately does not claim Kernel/Pend or hardware validation.
unsafe impl Authority for Model {
    type Supply = hub_server::authority::NoSupply;

    fn check_offer(&mut self, provider: TaskId, offer: &Offer) -> Result<(), Fail> {
        self.admissions.fetch_add(1, Ordering::SeqCst);
        if provider != PROVIDER
            || offer.resources[..offer.count as usize]
                .iter()
                .any(|r| r.pend != 17)
        {
            return Err(Fail::Denied);
        }
        if self.revoked.load(Ordering::SeqCst) {
            return Err(Fail::Denied);
        }
        Ok(())
    }
    fn check_reservation(
        &mut self,
        requester: TaskId,
        provider: TaskId,
        _: DeviceRef,
        _: &Offer,
    ) -> Result<(), Fail> {
        if requester != DRIVER && requester != TaskId::new(10) {
            return Err(Fail::Denied);
        }
        if provider != PROVIDER || self.revoked.load(Ordering::SeqCst) {
            return Err(Fail::Denied);
        }
        Ok(())
    }
    fn check_start(
        &mut self,
        requester: TaskId,
        provider: TaskId,
        offer: &Offer,
        _delivered: u8,
    ) -> Result<(), Fail> {
        // This pre-existing Host owner models external preparation, not native
        // Lease delivery. Native's full-slot gate is verified in the guest.
        if (requester != DRIVER && requester != TaskId::new(10))
            || provider != PROVIDER
            || self.revoked.load(Ordering::SeqCst)
        {
            return Err(Fail::Denied);
        }
        if offer.resources[..offer.count as usize]
            .iter()
            .any(|resource| resource.pend != 17)
        {
            return Err(Fail::Denied);
        }
        Ok(())
    }
    fn check_retirement(
        &mut self,
        owner: TaskId,
        _: &ResourceSpec,
        evidence: &Retirement,
    ) -> Result<(), Fail> {
        if owner != OWNER || evidence.receipt != 100 + evidence.duty as u64 {
            return Err(Fail::Denied);
        }
        if self.busy.load(Ordering::SeqCst) {
            return Err(Fail::Busy);
        }
        Ok(())
    }
}

pub fn machine() -> Machine<Model> {
    with(Model::default(), Limits::default())
}
pub fn with(model: Model, limits: Limits) -> Machine<Model> {
    // Fresh isolated host model, with no hardware or prior allocation duties.
    unsafe { Machine::new(MachineId { incarnation: 1 }, model, limits) }.unwrap()
}
pub fn offer(key: u64, start: u64) -> Offer {
    Offer {
        key,
        epoch: 1,
        count: 1,
        resources: [
            ResourceSpec {
                kind: ResourceKind::Mmio,
                domain: 1,
                start,
                len: 4096,
                pend: 17,
                rights: 4,
            },
            ResourceSpec::EMPTY,
            ResourceSpec::EMPTY,
            ResourceSpec::EMPTY,
        ],
    }
}
pub fn enroll(machine: &mut Machine<Model>) -> DeviceRef {
    let reply = machine.offer(PROVIDER, 1, offer(1, 0));
    assert_eq!(reply.failure, Fail::None);
    reply.snapshot.device
}
pub fn reserve(machine: &mut Machine<Model>, device: DeviceRef) -> BindingId {
    let reply = machine.reserve(DRIVER, 1, Reserve { device });
    assert_eq!(reply.failure, Fail::None);
    reply.snapshot.binding
}
pub fn proof(snapshot: Snapshot, duty: Duty) -> Retirement {
    Retirement {
        device: snapshot.device,
        allocation: AllocationId {
            binding: snapshot.binding,
            slot: 0,
        },
        attempt: snapshot.attempt,
        duty,
        receipt: 100 + duty as u64,
    }
}
