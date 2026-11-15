use hub_api::{DeviceRef, Fail, Offer, ResourceSpec, Retirement};
use kernel_api::model::identity::{Mark, TaskId};

use crate::lease;

pub mod native;

/// Read-only delivery progress, never a live authorization observation.
pub trait SupplyRecord {
    fn delivered(&self) -> bool;
}
impl SupplyRecord for lease::Lease {
    fn delivered(&self) -> bool {
        self.delivery().is_some()
    }
}

/// Uninhabited storage for external grant owners. Actual native Lease storage
/// stays Task-local; a coordinator without native grants can remain Send.
pub enum NoSupply {}
impl SupplyRecord for NoSupply {
    fn delivered(&self) -> bool {
        match *self {}
    }
}

/// A trusted executor's internal request to derive one offered resource for a
/// recipient selected from a fresh driver observation. This is not wire input;
/// the executor must correlate it with the current Machine allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrantRequest {
    pub requester: TaskId,
    pub provider: TaskId,
    pub correlation: lease::Correlation,
    pub resource: ResourceSpec,
    pub recipient: TaskId,
    pub mark: Mark,
}

/// Extension for trusted executors that request Machine-owned native grants.
///
/// # Safety
/// Returned requests must originate only from trusted live policy and an
/// imported source local to `owner`. The Machine executor must turn each
/// request into the unique lease for its exact `(device epoch, allocation,
/// slot, attempt)`. Implementations and executors must prevent parallel or
/// unrecorded grants for that correlation. Only under this sole-owner contract
/// may Machine discharge that exact slot's managed Authorization duty after
/// Lease::Closed; Device, source and Instance duties remain independent.
pub unsafe trait SupplyAuthority: Authority<Supply = lease::Lease> {
    fn owner(&self) -> TaskId;
    fn lease_request(&mut self, request: GrantRequest) -> Result<lease::Request, Fail>;
}

/// Trusted platform adapter, separate from descriptions and coordination state.
/// This repository supplies no permissive default implementation.
///
/// # Safety
/// Implementations must check real Hub-local Pends through the Kernel, never a
/// payload's copied PendInfo, before admitting an offer. Validate the actual
/// delegating provider, live authorization, target kind, exact scope, rights and
/// derivation lineage. Canonical conflict domains and coordinate units must come
/// from trusted machine provenance, including cross-kind hardware aliases.
/// Reservation must recheck current authority and caller policy. Kernel effects
/// must recheck live Pends at execution; an inspection is only a snapshot.
///
/// `retired` may succeed only after the named actual owner has completed the
/// exact resource/range, Device epoch, Allocation, attempt and duty. Task exit,
/// IRQ acknowledgement, a CQ reply, timeout or an untrusted receipt number alone
/// cannot establish quiescence. Busy/unknown/failed evidence must reject while
/// retaining responsibility. The adapter must hold or transfer unfinished
/// authority and hardware duties across failure; dropping Machine is no release.
pub unsafe trait Authority {
    type Supply: SupplyRecord;

    /// Pure authorization/provenance check. Must not acquire capabilities,
    /// change hardware, start an Instance or create any external responsibility.
    /// Existing imported Pends remain held by the adapter on either result.
    fn check_offer(&mut self, provider: TaskId, offer: &Offer) -> Result<(), Fail>;

    /// Revalidate live authority and requester policy without external effects.
    /// Actual grant/device preparation can begin only after begin_preparation
    /// has recorded every required retirement duty.
    fn check_reservation(
        &mut self,
        requester: TaskId,
        provider: TaskId,
        device: DeviceRef,
        offer: &Offer,
    ) -> Result<(), Fail>;

    /// Revalidate live source and policy before starting the Instance.
    /// `delivered` is a slot bitmask, not evidence of Kernel authority itself.
    fn check_start(
        &mut self,
        requester: TaskId,
        provider: TaskId,
        offer: &Offer,
        delivered: u8,
    ) -> Result<(), Fail>;

    /// Check completed owner evidence; this check must not initiate retirement
    /// effects whose uncertain completion would need a new responsibility.
    fn check_retirement(
        &mut self,
        owner: TaskId,
        resource: &ResourceSpec,
        evidence: &Retirement,
    ) -> Result<(), Fail>;
}
