//! Task-local ownership of a derived Kernel Pend branch.
//!
//! A lease delivers one narrowed Pend to one Task and retains its private
//! local root until Kernel return proves the whole branch has settled. This
//! module does not establish device readiness, hardware quiescence, Machine
//! Device/Authorization retirement, or a native platform Authority.

use core::marker::PhantomData;
use hub_api::{AllocationId, DeviceRef, MAX_RESOURCES};
use kernel_api::{
    abi::call::pend::PendFail,
    address::VirtAddr,
    model::{
        identity::{Mark, PendId, TaskId},
        pend::{
            AccordRequest, AccordTarget, Distributable, Limits, PendInfo, PendRange, PendScope,
        },
    },
};

/// Correlation for the Machine allocation that requested this effect.
/// These identities locate work; only Kernel Pends carry access authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Correlation {
    pub device: DeviceRef,
    pub allocation: AllocationId,
    pub attempt: u64,
}

impl Correlation {
    fn valid(self) -> bool {
        let machine = self.device.id.machine;
        machine.incarnation != 0
            && self.device.id.serial != 0
            && self.device.epoch != 0
            && self.allocation.binding.machine == machine
            && self.allocation.binding.serial != 0
            && usize::from(self.allocation.slot) < MAX_RESOURCES
            && self.attempt != 0
    }
}

/// Immutable, checked input. It is an intent, not a grant or proof of authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    correlation: Correlation,
    source: PendId,
    recipient: TaskId,
    limits: Limits,
    mark: Mark,
}

impl Request {
    pub fn new(
        correlation: Correlation,
        source: PendId,
        recipient: TaskId,
        limits: Limits,
        mark: Mark,
    ) -> Result<Self, Error> {
        if !correlation.valid()
            || source == PendId::NONE
            || recipient.get() == 0
            || limits.distributable != Distributable::No
        {
            return Err(Error::Invalid);
        }
        match limits.scope {
            PendScope::Object | PendScope::Pages(_) => (),
            PendScope::Nest(_) => return Err(Error::Unsupported),
        }
        if !limits.valid() || limits.operations.words()[1] == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            correlation,
            source,
            recipient,
            limits,
            mark,
        })
    }

    pub const fn correlation(self) -> Correlation {
        self.correlation
    }

    pub const fn recipient(self) -> TaskId {
        self.recipient
    }

    /// Requested Kernel rights, always non-distributable at the recipient.
    pub const fn limits(self) -> Limits {
        self.limits
    }

    /// The recipient inherits the private root's resolved range and cannot
    /// redistribute the derived Pend.
    pub const fn recipient_limits(self) -> Limits {
        let mut limits = self.limits;
        limits.scope = match self.limits.scope {
            PendScope::Pages(_) => PendScope::Pages(PendRange::Inherit),
            scope => scope,
        };
        limits.distributable = Distributable::No;
        limits
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    RootPending,
    DeliveryPending,
    Delivered,
    Closing,
    Closed,
}

/// Recipient-local token returned by a successful Kernel Accord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub token: PendId,
    pub recipient: TaskId,
    pub correlation: Correlation,
}

#[derive(Debug)]
pub enum Error {
    Invalid,
    Unsupported,
    Busy,
    Kernel(PendFail),
    InvalidObservation,
}

/// Owns one real private Pend root and its optional recipient branch.
///
/// The object is deliberately neither `Send` nor `Sync`; it must stay with the
/// Kernel Task whose local Pend table contains `root`. Dropping this value does
/// not return the root or establish retirement.
#[must_use = "retain the lease until finish reports State::Closed"]
pub struct Lease {
    owner: TaskId,
    request: Request,
    root: Option<PendId>,
    delivery: Option<PendId>,
    state: State,
    _task_local: PhantomData<*mut ()>,
}

impl Lease {
    /// Create a task-local effect owner without performing Kernel effects.
    ///
    /// # Safety
    /// The calling Kernel Task must have identity `owner`; `request.source`
    /// must be an actual local Pend token, never a number copied from an Offer
    /// or wire request. Keep and use this value only in that same Task until it
    /// reaches `State::Closed`. The source owner must retain its own duties.
    pub unsafe fn new_for_current_task(owner: TaskId, request: Request) -> Result<Self, Error> {
        if owner.get() == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            owner,
            request,
            root: None,
            delivery: None,
            state: State::RootPending,
            _task_local: PhantomData,
        })
    }

    pub const fn owner(&self) -> TaskId {
        self.owner
    }

    pub const fn correlation(&self) -> Correlation {
        self.request.correlation
    }

    pub const fn state(&self) -> State {
        self.state
    }

    /// Return the same recipient-local token after delivery; never derive twice.
    pub fn delivery(&self) -> Option<Delivery> {
        (self.state == State::Delivered).then(|| Delivery {
            token: self.delivery.unwrap(),
            recipient: self.request.recipient,
            correlation: self.request.correlation,
        })
    }

    /// Perform at most one Kernel effect. Errors retain the exact current state
    /// and root so the caller can retry or explicitly finish.
    pub fn advance(&mut self) -> Result<State, Error> {
        match self.state {
            State::RootPending => {
                let root = accord(
                    self.request.source,
                    AccordTarget::Caller,
                    self.request.mark,
                    root_limits(self.request.limits),
                )?;
                self.root = Some(root);
                self.state = State::DeliveryPending;
                Ok(self.state)
            }
            State::DeliveryPending => {
                // Request::new rejects Nest. The private root has already
                // resolved this relative window; inherit it without shifting.
                let token = accord(
                    self.root.ok_or(Error::Invalid)?,
                    AccordTarget::Task(self.request.recipient),
                    self.request.mark,
                    self.request.recipient_limits(),
                )?;
                self.delivery = Some(token);
                self.state = State::Delivered;
                Ok(self.state)
            }
            State::Closing => self.return_root(),
            State::Delivered | State::Closed => Ok(self.state),
        }
    }

    /// Monotonically close this lease and advance its exact root retirement.
    /// Repeated calls retry Busy cleanup and never forget the retained root.
    pub fn finish(&mut self) -> Result<State, Error> {
        if self.state == State::Closed {
            return Ok(State::Closed);
        }
        self.state = State::Closing;
        self.return_root()
    }

    fn return_root(&mut self) -> Result<State, Error> {
        let Some(root) = self.root else {
            self.state = State::Closed;
            return Ok(self.state);
        };
        match kernel_client::call::pend::return_(root) {
            Ok(()) => {
                self.root = None;
                self.state = State::Closed;
                Ok(self.state)
            }
            Err(error) if error.source.is_busy() => Err(Error::Busy),
            Err(error) if error.source == PendFail::Denied => {
                if root_is_absent(root)? {
                    self.root = None;
                    self.state = State::Closed;
                    Ok(self.state)
                } else {
                    Err(Error::Kernel(PendFail::Denied))
                }
            }
            Err(error) => Err(Error::Kernel(error.source)),
        }
    }
}

fn accord(
    source: PendId,
    target: AccordTarget,
    mark: Mark,
    limits: Limits,
) -> Result<PendId, Error> {
    let request = AccordRequest {
        src: source,
        target,
        limits,
        mark,
    }
    .to_bytes();
    kernel_client::call::pend::accord(VirtAddr::new(request.as_ptr().addr()))
        .map_err(|error| map_kernel(error.source))
}

fn root_limits(mut limits: Limits) -> Limits {
    limits.distributable = Distributable::Yes;
    limits
}

/// Denied from Return is ambiguous. Only a successful, checked local ledger
/// enumeration can prove that this exact never-transferred root is gone.
fn root_is_absent(root: PendId) -> Result<bool, Error> {
    let after = root
        .get()
        .checked_sub(1)
        .map(PendId::mint)
        .ok_or(Error::Invalid)?;
    let mut bytes = [0; PendInfo::SIZE];
    let count =
        kernel_client::call::pend::collect(after, VirtAddr::new(bytes.as_mut_ptr().addr()), 1)
            .map_err(|error| map_kernel(error.source))?;
    match count {
        0 => Ok(true),
        1 => {
            let info = PendInfo::from_bytes(&bytes).ok_or(Error::InvalidObservation)?;
            if info.token == root {
                Ok(false)
            } else if info.token.get() > root.get() {
                Ok(true)
            } else {
                Err(Error::InvalidObservation)
            }
        }
        _ => Err(Error::InvalidObservation),
    }
}

fn map_kernel(error: PendFail) -> Error {
    if error.is_busy() {
        Error::Busy
    } else {
        Error::Kernel(error)
    }
}
