//! Task-local, Kernel-checked native authority for CPU Sign resources.
//!
//! This deliberately does not interpret a Sign as an IRQ, device register, or
//! hardware quiescence proof. The imported token is a real Kernel Pend whose
//! provenance and canonical Signal description are supplied by trusted
//! platform code.

use core::marker::PhantomData;

use hub_api::{DeviceRef, Fail, Offer, ResourceKind, ResourceSpec, Retirement};
use kernel_api::{
    address::VirtAddr,
    model::{
        identity::{PendId, TaskId},
        modifiable::Modifiable,
        pend::{Distributable, Limits, Operations, PendInfo, PendScope, PendStatus, Target},
        sign::{SignId, SignRights},
        unit::{UnitInfo, UnitQuery, UnitTarget},
    },
};

use crate::{
    authority::{Authority, GrantRequest, SupplyAuthority},
    lease,
};

/// Platform-imported, canonical description of one real Signal Pend.
///
/// # Safety
/// Construction asserts provenance that cannot be recovered from the Pend:
/// the platform chose the unique canonical `resource` and `sign` mapping,
/// `provider` is the immediate delegating Task, `requester` is authorized to
/// reserve it, and `object_owner` is the trusted owner of the Sign object.
/// The token must be a
/// real Pend directly accorded into the Hub Task and must remain owned and
/// retained by this import's caller. This proves no interrupt, device, or
/// hardware semantics. The caller retains source duties after import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Import {
    resource: ResourceSpec,
    provider: TaskId,
    requester: TaskId,
    object_owner: TaskId,
    sign: SignId,
    token: PendId,
}

impl Import {
    /// Validate the canonical Signal shape. Kernel provenance is checked by
    /// Native whenever an offer or reservation is admitted and before start.
    ///
    /// # Safety
    /// The caller must uphold the provenance contract documented on `Import`.
    pub unsafe fn new(
        resource: ResourceSpec,
        provider: TaskId,
        requester: TaskId,
        object_owner: TaskId,
        sign: SignId,
        token: PendId,
    ) -> Result<Self, Fail> {
        let import = Self {
            resource,
            provider,
            requester,
            object_owner,
            sign,
            token,
        };
        import.valid().then_some(import).ok_or(Fail::Invalid)
    }

    fn valid(self) -> bool {
        self.resource.kind == ResourceKind::Signal
            && self.resource.domain != 0
            && self.resource.start == 0
            && self.resource.len == 1
            && self.resource.rights == SignRights::TRIGGER.bits()
            && self.resource.pend == self.token.get() as u64
            && self.provider.get() != 0
            && self.requester.get() != 0
            && self.object_owner.get() != 0
            && self.sign.get() != 0
            && self.token != PendId::NONE
    }

    pub const fn resource(self) -> ResourceSpec {
        self.resource
    }
}

/// The only supported native provider implementation. It is !Send/!Sync and
/// must remain in the Task owning each imported local Pend.
#[must_use = "retain Native and its imported source duties for their full lifetime"]
pub struct Native {
    owner: TaskId,
    imports: alloc::vec::Vec<Import>,
    _task_local: PhantomData<*mut ()>,
}

impl Native {
    /// Construct a task-local native authority. This constructor performs no
    /// Kernel effect; it retains the caller-supplied import descriptions.
    ///
    /// # Safety
    /// Call in `owner`'s actual Kernel Task with real local imported Pends.
    /// Keep this object there and retain all original source/retirement duties.
    pub unsafe fn new_for_current_task(
        owner: TaskId,
        imports: alloc::vec::Vec<Import>,
    ) -> Result<Self, Fail> {
        if owner.get() == 0 || imports.iter().any(|import| !import.valid()) {
            return Err(Fail::Invalid);
        }
        for (index, import) in imports.iter().enumerate() {
            if imports[..index].iter().any(|old| {
                old.resource.domain == import.resource.domain
                    || old.sign == import.sign
                    || old.token == import.token
            }) {
                return Err(Fail::Conflict);
            }
        }
        Ok(Self {
            owner,
            imports,
            _task_local: PhantomData,
        })
    }

    fn validate_import(&self, import: Import) -> Result<(), Fail> {
        ensure_caller(self.owner)?;
        let mut bytes = [0; PendInfo::SIZE];
        kernel_client::call::pend::inspect(import.token, VirtAddr::new(bytes.as_mut_ptr().addr()))
            .map_err(|error| map_pend(error.source))?;
        let info = PendInfo::from_bytes(&bytes).ok_or(Fail::Denied)?;
        if info.token != import.token
            || info.status != PendStatus::Active
            || info.target != Target::Sign(import.sign)
            || info.owner != import.object_owner
            || info.lender != import.provider
            || info.limits.scope != PendScope::Object
            || info.limits.operations != Operations::Sign(SignRights::TRIGGER)
            || info.limits.modifiable != Modifiable::No
            || info.limits.distributable != Distributable::Yes
        {
            return Err(Fail::Denied);
        }
        Ok(())
    }

    fn offered_imports(&self, provider: TaskId, offer: &Offer) -> Result<(), Fail> {
        let count = usize::from(offer.count);
        if count == 0 || count > offer.resources.len() {
            return Err(Fail::Invalid);
        }
        if offer.resources[..count]
            .iter()
            .any(|resource| resource.kind != ResourceKind::Signal)
        {
            return Err(Fail::Unsupported);
        }
        for resource in &offer.resources[..count] {
            let import = self
                .imports
                .iter()
                .find(|item| item.provider == provider && item.resource == *resource)
                .copied()
                .ok_or(Fail::Denied)?;
            self.validate_import(import)?;
        }
        Ok(())
    }

    fn requester_imports(
        &self,
        requester: TaskId,
        provider: TaskId,
        offer: &Offer,
    ) -> Result<(), Fail> {
        self.offered_imports(provider, offer)?;
        let count = usize::from(offer.count);
        for resource in &offer.resources[..count] {
            let import = self
                .imports
                .iter()
                .find(|item| item.provider == provider && item.resource == *resource)
                .copied()
                .ok_or(Fail::Denied)?;
            if import.requester != requester {
                return Err(Fail::Denied);
            }
        }
        Ok(())
    }
}

unsafe impl Authority for Native {
    type Supply = lease::Lease;

    fn check_offer(&mut self, provider: TaskId, offer: &Offer) -> Result<(), Fail> {
        self.offered_imports(provider, offer)
    }

    fn check_reservation(
        &mut self,
        requester: TaskId,
        provider: TaskId,
        _device: DeviceRef,
        offer: &Offer,
    ) -> Result<(), Fail> {
        self.requester_imports(requester, provider, offer)
    }

    fn check_start(
        &mut self,
        requester: TaskId,
        provider: TaskId,
        offer: &Offer,
        delivered: u8,
    ) -> Result<(), Fail> {
        if offer.count == 0 || usize::from(offer.count) > offer.resources.len() {
            return Err(Fail::Invalid);
        }
        let expected = ((1u16 << offer.count) - 1) as u8;
        if delivered != expected {
            return Err(Fail::Busy);
        }
        self.requester_imports(requester, provider, offer)
    }

    fn check_retirement(
        &mut self,
        _owner: TaskId,
        _resource: &ResourceSpec,
        _evidence: &Retirement,
    ) -> Result<(), Fail> {
        Err(Fail::Unsupported)
    }
}

unsafe impl SupplyAuthority for Native {
    fn owner(&self) -> TaskId {
        self.owner
    }

    fn lease_request(&mut self, request: GrantRequest) -> Result<lease::Request, Fail> {
        ensure_caller(self.owner)?;
        let import = self
            .imports
            .iter()
            .find(|item| {
                item.resource == request.resource
                    && item.provider == request.provider
                    && item.requester == request.requester
            })
            .copied()
            .ok_or(Fail::Denied)?;
        if request.recipient.get() == 0 {
            return Err(Fail::Invalid);
        }
        self.validate_import(import)?;
        let limits = Limits {
            operations: Operations::Sign(SignRights::TRIGGER),
            modifiable: Modifiable::No,
            distributable: Distributable::No,
            scope: PendScope::Object,
        };
        lease::Request::new(
            request.correlation,
            import.token,
            request.recipient,
            limits,
            request.mark,
        )
        .map_err(|error| match error {
            lease::Error::Busy => Fail::Busy,
            lease::Error::Unsupported => Fail::Unsupported,
            _ => Fail::Invalid,
        })
    }
}

fn ensure_caller(owner: TaskId) -> Result<(), Fail> {
    let mut bytes = [0; UnitInfo::WIDTH];
    let found = kernel_client::call::unit::inspect(
        UnitQuery::Caller,
        VirtAddr::new(bytes.as_mut_ptr().addr()),
    )
    .map_err(|error| {
        if error.source.is_busy() {
            Fail::Busy
        } else {
            Fail::Denied
        }
    })?;
    if !found {
        return Err(Fail::Denied);
    }
    let info = UnitInfo::from_bytes(&bytes).ok_or(Fail::Denied)?;
    if info.target == UnitTarget::Task(owner) {
        Ok(())
    } else {
        Err(Fail::Denied)
    }
}

fn map_pend(error: kernel_api::abi::call::pend::PendFail) -> Fail {
    if error.is_busy() {
        Fail::Busy
    } else {
        Fail::Denied
    }
}
