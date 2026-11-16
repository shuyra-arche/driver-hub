//! Native Kernel and System SDK adapter for one task-local Supply owner.

use super::{Command, Control, Response};
use core::marker::PhantomData;
use kernel_api::{
    abi::call::pend::PendFail,
    address::VirtAddr,
    model::{
        identity::{Mark, PendId, TaskId},
        pend::{AccordTarget, Limits, Operations, PendInfo, PendScope, PendStatus},
        unit::{UnitInfo, UnitQuery, UnitTarget},
    },
};
use system_api::{
    self as sys,
    supply::{self as api, operations as op},
};
use system_client::{Cleanup, Context};
use weave::{Codec, Contract, Operation};

#[derive(Debug)]
pub enum NativeError {
    Begin(system_client::Error),
    Call(system_client::Error),
    Cleanup(system_client::CleanupError),
    WrongPeer,
    Caller,
    Kernel(PendFail),
    InvalidSource,
}
impl NativeError {
    pub fn is_busy(&self) -> bool {
        match self {
            Self::Cleanup(error) => error.is_busy(),
            Self::Kernel(error) => error.is_busy(),
            Self::Begin(system_client::Error::Send(error)) => error.source.is_busy(),
            _ => false,
        }
    }
}

/// Caller-owned System Supply client plus the one CPU source lifecycle.
/// Cleanup and business duties survive errors and dropping this value is not
/// a remote cancellation or Pend retirement.
#[must_use = "retain native Supply control until every owner reports Closed"]
pub struct Native<F, G> {
    client: system_client::supply::Client,
    peer: TaskId,
    caller: TaskId,
    registrations: F,
    installations: G,
    registration_last: u64,
    installation_last: u64,
    cleanup: Option<Cleanup>,
    _task_local: PhantomData<*mut ()>,
}

impl<F, G> Native<F, G>
where
    F: FnMut() -> Option<api::RegistrationId>,
    G: FnMut() -> Option<api::InstallId>,
{
    /// Bind the real System Supply endpoint and caller-wide fresh ID sources.
    ///
    /// # Safety
    /// `endpoint`, `peer`, `license`, and `shared_context` must be trusted
    /// bootstrap values for this Task's System Supply client. `caller` is the
    /// actual Kernel Task identity, checked at each effect. The allocators must
    /// be monotonic, non-reusing namespaces shared with every other Supply
    /// client to this System in this Task. Keep this adapter in `caller`; its
    /// Kernel source roots and SDK cleanup belong to this Task.
    pub unsafe fn bind(
        endpoint: PendId,
        peer: TaskId,
        caller: TaskId,
        license: PendId,
        shared_context: Context,
        registrations: F,
        installations: G,
    ) -> Result<Self, NativeError> {
        if caller.get() == 0 || peer.get() == 0 {
            return Err(NativeError::Caller);
        }
        let client = system_client::supply::Client::bind(endpoint, peer, license, shared_context)
            .map_err(NativeError::Call)?;
        Ok(Self {
            client,
            peer,
            caller,
            registrations,
            installations,
            registration_last: 0,
            installation_last: 0,
            cleanup: None,
            _task_local: PhantomData,
        })
    }

    fn verify_caller(&self) -> Result<(), NativeError> {
        let mut bytes = [0; UnitInfo::WIDTH];
        let found = kernel_client::call::unit::inspect(
            UnitQuery::Caller,
            VirtAddr::new(bytes.as_mut_ptr().addr()),
        )
        .map_err(|_| NativeError::Caller)?;
        if !found {
            return Err(NativeError::Caller);
        }
        let info = UnitInfo::from_bytes(&bytes).ok_or(NativeError::Caller)?;
        if info.target == UnitTarget::Task(self.caller) {
            Ok(())
        } else {
            Err(NativeError::Caller)
        }
    }

    /// Finish SDK reply-route cleanup only. This does not withdraw a source,
    /// cancel an install, or retire any Kernel root.
    pub fn finish(&mut self) -> Result<(), NativeError> {
        if let Some(cleanup) = self.cleanup.as_mut() {
            cleanup.finish().map_err(NativeError::Cleanup)?;
            self.cleanup = None;
        }
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.cleanup.is_none()
    }

    fn call<O>(
        &mut self,
        request: O::Request,
        wait: kernel_api::model::wait::Wait,
    ) -> Result<<<O as Contract>::Response as Codec>::Decoded, NativeError>
    where
        O: Operation<Interface = op::Interface>,
    {
        self.finish()?;
        let mut pending = match self.client.begin::<O>(request, wait) {
            Ok(pending) => pending,
            Err(failure) => {
                self.cleanup = Some(failure.cleanup);
                return Err(NativeError::Begin(failure.error));
            }
        };
        let result = pending.wait();
        self.cleanup = Some(pending.into_cleanup());
        match result {
            Ok(response) if response.metadata == self.peer => Ok(response.value),
            Ok(_) => Err(NativeError::WrongPeer),
            Err(error) => Err(NativeError::Call(error)),
        }
    }

    fn accord(
        &self,
        source: PendId,
        target: AccordTarget,
        mark: Mark,
        limits: Limits,
    ) -> Result<PendId, NativeError> {
        self.verify_caller()?;
        // SAFETY: caller check above matches the task-local owner; the source
        // argument is either the source checked by CreateOffering or this
        // adapter's private root retained by its Supply.
        unsafe { crate::lease::accord_owned(self.caller, source, target, mark, limits) }.map_err(
            |error| match error {
                crate::lease::Error::Kernel(source) => NativeError::Kernel(source),
                crate::lease::Error::Busy => NativeError::Kernel(PendFail::Busy),
                _ => NativeError::Caller,
            },
        )
    }

    fn create_offering(
        &self,
        source: PendId,
        mark: Mark,
        limits: Limits,
    ) -> Result<PendId, NativeError> {
        self.verify_caller()?;
        let mut bytes = [0; PendInfo::SIZE];
        kernel_client::call::pend::inspect(source, VirtAddr::new(bytes.as_mut_ptr().addr()))
            .map_err(|error| NativeError::Kernel(error.source))?;
        let info = PendInfo::from_bytes(&bytes).ok_or(NativeError::InvalidSource)?;
        if info.status != PendStatus::Active
            || info.target.words()[0] != limits.operations.words()[0]
            || !info.limits.permits_child(limits)
            || limits.scope != PendScope::Object
            || !matches!(
                limits.operations,
                Operations::Sign(_) | Operations::Clone(_)
            )
        {
            return Err(NativeError::InvalidSource);
        }
        self.accord(source, AccordTarget::Caller, mark, limits)
    }
}

unsafe impl<F, G> Control for Native<F, G>
where
    F: FnMut() -> Option<api::RegistrationId>,
    G: FnMut() -> Option<api::InstallId>,
{
    type Error = NativeError;

    fn peer(&self) -> sys::TaskId {
        self.peer.into()
    }
    fn caller(&self) -> sys::TaskId {
        self.caller.into()
    }
    fn allocate_registration(&mut self) -> Option<api::RegistrationId> {
        let id = (self.registrations)()?;
        if id.get() == 0 || id.get() <= self.registration_last {
            return None;
        }
        self.registration_last = id.get();
        Some(id)
    }
    fn allocate_installation(&mut self) -> Option<api::InstallId> {
        let id = (self.installations)()?;
        if id.get() == 0 || id.get() <= self.installation_last {
            return None;
        }
        self.installation_last = id.get();
        Some(id)
    }
    fn execute(
        &mut self,
        command: Command,
        wait: kernel_api::model::wait::Wait,
    ) -> Result<Response, Self::Error> {
        // Settle any prior SDK reply route before a native or remote effect.
        // On error the Supply owner retains phase and IDs for recovery.
        self.finish()?;
        self.verify_caller()?;
        use Command::*;
        Ok(match command {
            CreateOffering {
                source,
                mark,
                limits,
            } => Response::OfferingRoot(self.create_offering(source, mark, limits)?),
            DelegateOffering { root, mark, limits } => Response::Offered(
                self.accord(root, AccordTarget::Task(self.peer), mark, limits)?
                    .into(),
            ),
            RetireOffering(root) => {
                // SAFETY: this root is created by CreateOffering, kept local,
                // never transferred, and is retired in its actual owner Task.
                match unsafe { crate::lease::retire_owned_root(self.caller, root) } {
                    Ok(true) => Response::Retired,
                    Ok(false) => return Err(NativeError::Kernel(PendFail::Denied)),
                    Err(crate::lease::Error::Busy) => {
                        return Err(NativeError::Kernel(PendFail::Busy));
                    }
                    Err(crate::lease::Error::Kernel(error)) => {
                        return Err(NativeError::Kernel(error));
                    }
                    Err(_) => return Err(NativeError::Caller),
                }
            }
            Register(request) => Response::Source(self.call::<op::Register>(request, wait)?),
            InspectSource(request) => {
                Response::Source(self.call::<op::InspectSource>(request, wait)?)
            }
            Withdraw(request) => Response::Source(self.call::<op::Withdraw>(request, wait)?),
            Install(request) => Response::Install(self.call::<op::Install>(request, wait)?),
            InspectInstall(request) => {
                Response::Install(self.call::<op::InspectInstall>(request, wait)?)
            }
            CancelInstall(request) => {
                Response::Install(self.call::<op::CancelInstall>(request, wait)?)
            }
        })
    }
}
