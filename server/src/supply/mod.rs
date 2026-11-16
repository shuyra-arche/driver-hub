//! Per-allocation ownership of a real System Supply registration and install.
//!
//! The source registration, Prepared installation and the Hub-local offering
//! root have separate completion states. A System reply never stands in for
//! native Pend retirement, and closing one allocation does not close its
//! provider's original resource.

mod native;

use core::marker::PhantomData;
use kernel_api::model::{
    identity::{Mark, PendId},
    pend::{Distributable, Limits, Operations, PendScope},
    wait::Wait,
};
use system_api::{
    self as sys,
    control::instance::InstanceId,
    supply::{
        self as api, Fail as SupplyFail, InstallId, InstallState, InstallStatus, RegistrationId,
        SourceState, SourceStatus,
    },
    value::Outcome,
};

use crate::lease;

pub use native::{Native, NativeError};

/// Observable progress. Delivery is exposed only while the retained install
/// status is Granted; it is still a historical observation, not live authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub state: State,
    pub delivery: Option<lease::Delivery>,
    pub registration: RegistrationId,
    pub installation: InstallId,
    pub failure: Option<SupplyFail>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Offering,
    Registering,
    Installing,
    Granted,
    Closing,
    Closed,
}

/// Trusted local effects and System Supply RPC boundary.
///
/// # Safety
/// `caller` and `peer` must be the actual bound Task identities. ID allocators
/// must be non-reusing in their caller/System namespaces. `execute` must check
/// the real caller before every native effect and retain SDK response cleanup
/// until finished. Create/Delegate failures must mean no new CPU Object Pend
/// was published; Return Busy/errors retain the original root. A successful
/// `Retired` response must mean `lease::retire_owned_root` proved completion.
pub unsafe trait Control {
    type Error;

    fn peer(&self) -> sys::TaskId;
    fn caller(&self) -> sys::TaskId;
    fn allocate_registration(&mut self) -> Option<RegistrationId>;
    fn allocate_installation(&mut self) -> Option<InstallId>;
    fn execute(&mut self, command: Command, wait: Wait) -> Result<Response, Self::Error>;
}

/// One native effect or public Supply request. The only recipient is the
/// Machine's fresh Prepared observation recorded in the immutable lease request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    CreateOffering {
        source: PendId,
        mark: Mark,
        limits: Limits,
    },
    DelegateOffering {
        root: PendId,
        mark: Mark,
        limits: Limits,
    },
    RetireOffering(PendId),
    Register(api::Register),
    InspectSource(api::Registration),
    Withdraw(api::Registration),
    Install(api::Install),
    InspectInstall(api::Installation),
    CancelInstall(api::Installation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    OfferingRoot(PendId),
    Offered(sys::PendId),
    Retired,
    Source(Outcome<SourceStatus, SupplyFail>),
    Install(Outcome<InstallStatus, SupplyFail>),
}

#[derive(Debug)]
pub enum Error<E> {
    Invalid,
    Unsupported,
    Caller,
    IdentifierExhausted,
    Control(E),
    Lease(lease::Error),
    Remote(SupplyFail),
    InvalidObservation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Create,
    Delegate,
    Register,
    InspectSource,
    Install,
    InspectInstall,
    Granted,
    CloseInstall,
    CloseSource,
    CloseOffering,
    Closed,
}

/// The unique owner for one Machine allocation/slot/attempt.
///
/// It owns a local offering root, an immutable System registration and an
/// immutable System installation identity. Dropping it does not retire any
/// of them. It is task-local because its root belongs to the creating Task.
#[must_use = "retain Supply until progress().state is Closed"]
pub struct Supply {
    request: lease::Request,
    instance: InstanceId,
    source: api::Key,
    resource: api::Key,
    caller: sys::TaskId,
    peer: sys::TaskId,
    registration: RegistrationId,
    installation: InstallId,
    root: Option<PendId>,
    offered: Option<sys::PendId>,
    registration_possible: bool,
    installation_possible: bool,
    source_status: Option<SourceStatus>,
    install_status: Option<InstallStatus>,
    source_accepted: bool,
    install_accepted: bool,
    delivery: Option<lease::Delivery>,
    failure: Option<SupplyFail>,
    phase: Phase,
    closing: bool,
    _task_local: PhantomData<*mut ()>,
}

impl Supply {
    /// Allocate IDs and retain immutable intent without Kernel or RPC effects.
    ///
    /// # Safety
    /// Call in the Task whose table contains `request`'s actual source. The
    /// caller must own this Machine allocation and the management authority for
    /// `instance`; source name must be its trusted Unit declaration and
    /// `resource` unique for this allocation/attempt. Keep this object in the
    /// Task until it reports Closed.
    pub unsafe fn new_for_current_task<C: Control>(
        request: lease::Request,
        instance: InstanceId,
        source: api::Key,
        resource: api::Key,
        control: &mut C,
    ) -> Result<Self, Error<C::Error>> {
        let peer = control.peer();
        let caller = control.caller();
        if peer.get() == 0 || caller.get() == 0 || instance.system != peer {
            return Err(Error::Invalid);
        }
        if !source.valid()
            || !resource.valid()
            || source.name.len() > 128
            || resource.name.len() > 128
        {
            return Err(Error::Invalid);
        }
        let limits = request.limits();
        if !limits.valid() || limits.distributable != Distributable::No {
            return Err(Error::Invalid);
        }
        if limits.scope != PendScope::Object {
            return Err(Error::Unsupported);
        }
        if !matches!(
            limits.operations,
            Operations::Sign(_) | Operations::Clone(_)
        ) {
            return Err(Error::Unsupported);
        }
        if limits.operations.words()[1] == 0 || request.mark() == Mark::NONE {
            return Err(Error::Invalid);
        }
        let registration = control
            .allocate_registration()
            .ok_or(Error::IdentifierExhausted)?;
        let installation = control
            .allocate_installation()
            .ok_or(Error::IdentifierExhausted)?;
        if registration.get() == 0 || installation.get() == 0 {
            return Err(Error::IdentifierExhausted);
        }
        Ok(Self {
            request,
            instance,
            source,
            resource,
            caller,
            peer,
            registration,
            installation,
            root: None,
            offered: None,
            registration_possible: false,
            installation_possible: false,
            source_status: None,
            install_status: None,
            source_accepted: false,
            install_accepted: false,
            delivery: None,
            failure: None,
            phase: Phase::Create,
            closing: false,
            _task_local: PhantomData,
        })
    }

    pub const fn request(&self) -> lease::Request {
        self.request
    }

    pub fn source_name(&self) -> &str {
        &self.source.name
    }

    pub fn resource_name(&self) -> &str {
        &self.resource.name
    }

    /// Return retained progress only. This never queries System or Kernel.
    pub fn progress(&self) -> Progress {
        Progress {
            state: self.state(),
            delivery: (self.phase == Phase::Granted)
                .then_some(self.delivery)
                .flatten(),
            registration: self.registration,
            installation: self.installation,
            failure: self.failure,
        }
    }

    fn state(&self) -> State {
        if self.closing {
            return if self.phase == Phase::Closed {
                State::Closed
            } else {
                State::Closing
            };
        }
        match self.phase {
            Phase::Create => State::Offering,
            Phase::Delegate | Phase::Register | Phase::InspectSource => State::Registering,
            Phase::Install | Phase::InspectInstall => State::Installing,
            Phase::Granted => State::Granted,
            Phase::CloseInstall | Phase::CloseSource | Phase::CloseOffering => State::Closing,
            Phase::Closed => State::Closed,
        }
    }

    /// Advance at most one external command. Calling again after a System SDK
    /// error continues recovery using the same IDs and business requests.
    pub fn advance<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        self.check_control(control)?;
        match self.phase {
            Phase::Create if self.closing => self.close_offering(control, wait)?,
            Phase::Delegate if self.closing => self.close_offering(control, wait)?,
            Phase::Register if self.closing => self.close_source(control, wait)?,
            Phase::InspectSource if self.closing => self.inspect_source(control, wait, true)?,
            Phase::Install if self.closing => self.close_install(control, wait)?,
            Phase::InspectInstall if self.closing => self.inspect_install(control, wait, true)?,
            Phase::Create => self.create(control, wait)?,
            Phase::Delegate => self.delegate(control, wait)?,
            Phase::Register => self.register(control, wait)?,
            Phase::InspectSource => self.inspect_source(control, wait, false)?,
            Phase::Install => self.install(control, wait)?,
            Phase::InspectInstall => self.inspect_install(control, wait, false)?,
            Phase::Granted => {
                self.phase = Phase::InspectInstall;
                self.inspect_install(control, wait, false)?;
            }
            Phase::CloseInstall => self.close_install(control, wait)?,
            Phase::CloseSource => self.close_source(control, wait)?,
            Phase::CloseOffering => self.close_offering(control, wait)?,
            Phase::Closed => (),
        }
        Ok(self.progress())
    }

    /// Monotonically begin closure, then advance one cleanup command.
    pub fn finish<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        self.check_control(control)?;
        if !self.closing && self.phase != Phase::Closed {
            self.begin_closing();
        }
        self.advance(control, wait)
    }

    fn begin_closing(&mut self) {
        self.closing = true;
        if !matches!(self.phase, Phase::Closed) {
            self.phase = Phase::CloseInstall;
        }
    }

    fn check_control<C: Control>(&self, control: &C) -> Result<(), Error<C::Error>> {
        if control.caller() == self.caller && control.peer() == self.peer {
            Ok(())
        } else {
            Err(Error::Caller)
        }
    }

    fn offering_limits(&self) -> Limits {
        let mut limits = self.request.limits();
        limits.distributable = Distributable::Yes;
        limits
    }

    fn create<C: Control>(&mut self, control: &mut C, wait: Wait) -> Result<(), Error<C::Error>> {
        match control
            .execute(
                Command::CreateOffering {
                    source: self.request.source(),
                    mark: self.request.mark(),
                    limits: self.offering_limits(),
                },
                wait,
            )
            .map_err(Error::Control)?
        {
            Response::OfferingRoot(root) if root != PendId::NONE => {
                self.root = Some(root);
                self.phase = Phase::Delegate;
                Ok(())
            }
            _ => Err(Error::InvalidObservation),
        }
    }

    fn delegate<C: Control>(&mut self, control: &mut C, wait: Wait) -> Result<(), Error<C::Error>> {
        let root = self.root.ok_or(Error::InvalidObservation)?;
        match control
            .execute(
                Command::DelegateOffering {
                    root,
                    mark: self.request.mark(),
                    limits: self.offering_limits(),
                },
                wait,
            )
            .map_err(Error::Control)?
        {
            Response::Offered(offered) if offered != sys::PendId::NONE => {
                self.offered = Some(offered);
                self.phase = Phase::Register;
                Ok(())
            }
            _ => Err(Error::InvalidObservation),
        }
    }

    fn register<C: Control>(&mut self, control: &mut C, wait: Wait) -> Result<(), Error<C::Error>> {
        let request = self.register_request()?;
        self.registration_possible = true;
        let response = match control.execute(Command::Register(request), wait) {
            Ok(Response::Source(response)) => response,
            Ok(_) => {
                self.phase = Phase::InspectSource;
                return Err(Error::InvalidObservation);
            }
            Err(error) => {
                self.phase = Phase::InspectSource;
                return Err(Error::Control(error));
            }
        };
        match response {
            Outcome::Success(status) => self.accept_source(status),
            Outcome::Failure(SupplyFail::Pending) => {
                self.phase = Phase::Register;
                Ok(())
            }
            Outcome::Failure(failure) => self.reject(failure),
        }
    }

    fn register_request<E>(&self) -> Result<api::Register, Error<E>> {
        let offered = self.offered.ok_or(Error::InvalidObservation)?;
        Ok(api::Register {
            id: self.registration,
            source: self.source.clone(),
            resource: self.resource.clone(),
            offered,
            maximum: sys::value::Rights(self.offering_limits()),
        })
    }

    fn accept_source<E>(&mut self, status: SourceStatus) -> Result<(), Error<E>> {
        if status.id != self.registration {
            return Err(Error::InvalidObservation);
        }
        if let Some(previous) = self.source_status {
            let valid = matches!(
                (previous.state, status.state),
                (
                    SourceState::Active,
                    SourceState::Active | SourceState::Closing | SourceState::Closed,
                ) | (
                    SourceState::Closing,
                    SourceState::Closing | SourceState::Closed
                ) | (SourceState::Closed, SourceState::Closed)
                    | (SourceState::Cancelled, SourceState::Cancelled)
            );
            if !valid {
                return Err(Error::InvalidObservation);
            }
        }
        if status.state == SourceState::Cancelled && self.source_accepted {
            return Err(Error::InvalidObservation);
        }
        if matches!(status.state, SourceState::Active | SourceState::Closing) {
            self.source_accepted = true;
        }
        if status.state != SourceState::Active {
            self.begin_closing();
        }
        self.source_status = Some(status);
        match status.state {
            SourceState::Active => {
                self.registration_possible = true;
                self.phase = Phase::Install;
            }
            SourceState::Closing | SourceState::Closed | SourceState::Cancelled => {
                self.registration_possible = status.state == SourceState::Closing;
                self.phase = Phase::CloseInstall;
            }
        }
        Ok(())
    }

    fn inspect_source<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
        closing: bool,
    ) -> Result<(), Error<C::Error>> {
        let response = match control.execute(
            Command::InspectSource(api::Registration {
                id: self.registration,
            }),
            wait,
        ) {
            Ok(Response::Source(response)) => response,
            Ok(_) => return Err(Error::InvalidObservation),
            Err(error) => return Err(Error::Control(error)),
        };
        match response {
            Outcome::Success(status) => {
                self.accept_source(status)?;
                if closing && !matches!(status.state, SourceState::Closed | SourceState::Cancelled)
                {
                    self.phase = Phase::CloseSource;
                }
                Ok(())
            }
            Outcome::Failure(SupplyFail::Unknown) if closing => {
                self.phase = Phase::CloseSource;
                Ok(())
            }
            Outcome::Failure(SupplyFail::Unknown) => {
                // Unknown does not tombstone a delayed Register; replay the exact intent.
                self.phase = Phase::Register;
                Ok(())
            }
            Outcome::Failure(SupplyFail::Pending) => Ok(()),
            Outcome::Failure(failure) => Err(Error::Remote(failure)),
        }
    }

    fn install<C: Control>(&mut self, control: &mut C, wait: Wait) -> Result<(), Error<C::Error>> {
        let request = api::Install {
            id: self.installation,
            source: self.registration,
            instance: self.instance,
            target: self.request.recipient().into(),
            slot: self.request.mark().into(),
            rights: sys::value::Rights(self.request.limits()),
        };
        self.installation_possible = true;
        let response = match control.execute(Command::Install(request), wait) {
            Ok(Response::Install(response)) => response,
            Ok(_) => {
                self.phase = Phase::InspectInstall;
                return Err(Error::InvalidObservation);
            }
            Err(error) => {
                self.phase = Phase::InspectInstall;
                return Err(Error::Control(error));
            }
        };
        match response {
            Outcome::Success(status) => self.accept_install(status),
            Outcome::Failure(SupplyFail::Pending) => {
                self.phase = Phase::Install;
                Ok(())
            }
            Outcome::Failure(failure) => self.reject(failure),
        }
    }

    fn inspect_install<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
        closing: bool,
    ) -> Result<(), Error<C::Error>> {
        let response = match control.execute(
            Command::InspectInstall(api::Installation {
                id: self.installation,
            }),
            wait,
        ) {
            Ok(Response::Install(response)) => response,
            Ok(_) => return Err(Error::InvalidObservation),
            Err(error) => return Err(Error::Control(error)),
        };
        match response {
            Outcome::Success(status) => {
                let state = status.state;
                self.accept_install(status)?;
                if closing && !matches!(state, InstallState::Released | InstallState::Cancelled) {
                    self.phase = Phase::CloseInstall;
                }
                Ok(())
            }
            Outcome::Failure(SupplyFail::Unknown) if closing => {
                // Force a tombstone for any delayed Install under this exact ID.
                self.phase = Phase::CloseInstall;
                Ok(())
            }
            Outcome::Failure(SupplyFail::Unknown) => {
                self.phase = Phase::Install;
                Ok(())
            }
            Outcome::Failure(SupplyFail::Pending) => Ok(()),
            Outcome::Failure(failure) => Err(Error::Remote(failure)),
        }
    }

    fn accept_install<E>(&mut self, status: InstallStatus) -> Result<(), Error<E>> {
        if status.id != self.installation {
            return Err(Error::InvalidObservation);
        }
        if status.state == InstallState::Cancelled && self.install_accepted {
            return Err(Error::InvalidObservation);
        }
        if let Some(previous) = self.install_status.as_ref().map(|value| value.state) {
            let valid = matches!(
                (previous, status.state),
                (
                    InstallState::Pending,
                    InstallState::Pending
                        | InstallState::Granted
                        | InstallState::Releasing
                        | InstallState::Released,
                ) | (
                    InstallState::Granted,
                    InstallState::Granted | InstallState::Releasing | InstallState::Released,
                ) | (
                    InstallState::Releasing,
                    InstallState::Releasing | InstallState::Released
                ) | (InstallState::Released, InstallState::Released)
                    | (InstallState::Cancelled, InstallState::Cancelled)
            );
            if !valid {
                return Err(Error::InvalidObservation);
            }
        }
        if matches!(
            status.state,
            InstallState::Pending | InstallState::Granted | InstallState::Releasing
        ) {
            self.install_accepted = true;
        }
        let association_matches = status.instance.0 == Some(self.instance)
            && status.target == self.request.recipient().into()
            && status.slot == self.request.mark().into();
        if status.state == InstallState::Cancelled {
            if status != InstallStatus::cancelled(self.installation) {
                return Err(Error::InvalidObservation);
            }
            self.installation_possible = false;
            self.delivery = None;
            self.begin_closing();
            self.phase = Phase::CloseSource;
            self.install_status = Some(status);
            return Ok(());
        }
        if !association_matches {
            return Err(Error::InvalidObservation);
        }
        if let Some(failure) = status.failure.0 {
            self.failure.get_or_insert(failure.0);
        }
        match status.state {
            InstallState::Pending => {
                self.installation_possible = true;
                self.delivery = None;
                self.phase = Phase::Install;
            }
            InstallState::Granted => {
                let delivery = status
                    .delivery
                    .0
                    .as_ref()
                    .ok_or(Error::InvalidObservation)?;
                if !delivery.valid() {
                    return Err(Error::InvalidObservation);
                }
                self.installation_possible = true;
                self.delivery = Some(lease::Delivery {
                    token: PendId::try_from(delivery.token)
                        .map_err(|_| Error::InvalidObservation)?,
                    recipient: self.request.recipient(),
                    correlation: self.request.correlation(),
                });
                self.phase = Phase::Granted;
            }
            InstallState::Releasing => {
                self.installation_possible = true;
                self.delivery = None;
                self.begin_closing();
                self.phase = Phase::CloseInstall;
            }
            InstallState::Released => {
                self.installation_possible = false;
                self.delivery = None;
                self.begin_closing();
                self.phase = Phase::CloseSource;
            }
            InstallState::Cancelled => unreachable!(),
        }
        self.install_status = Some(status);
        Ok(())
    }

    fn close_install<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<(), Error<C::Error>> {
        if !self.installation_possible {
            self.phase = Phase::CloseSource;
            return Ok(());
        }
        let response = match control.execute(
            Command::CancelInstall(api::Installation {
                id: self.installation,
            }),
            wait,
        ) {
            Ok(Response::Install(response)) => response,
            Ok(_) => {
                self.phase = Phase::InspectInstall;
                return Err(Error::InvalidObservation);
            }
            Err(error) => {
                self.phase = Phase::InspectInstall;
                return Err(Error::Control(error));
            }
        };
        match response {
            Outcome::Success(status) => {
                self.accept_install(status)?;
                if matches!(
                    self.install_status.as_ref().map(|value| value.state),
                    Some(InstallState::Released | InstallState::Cancelled)
                ) {
                    self.installation_possible = false;
                    self.phase = Phase::CloseSource;
                } else {
                    self.phase = Phase::InspectInstall;
                }
                Ok(())
            }
            Outcome::Failure(SupplyFail::Pending) => Ok(()),
            Outcome::Failure(failure) => Err(Error::Remote(failure)),
        }
    }

    fn close_source<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<(), Error<C::Error>> {
        if !self.registration_possible {
            self.phase = Phase::CloseOffering;
            return Ok(());
        }
        let response = match control.execute(
            Command::Withdraw(api::Registration {
                id: self.registration,
            }),
            wait,
        ) {
            Ok(Response::Source(response)) => response,
            Ok(_) => {
                self.phase = Phase::InspectSource;
                return Err(Error::InvalidObservation);
            }
            Err(error) => {
                self.phase = Phase::InspectSource;
                return Err(Error::Control(error));
            }
        };
        match response {
            Outcome::Success(status) => {
                self.accept_source(status)?;
                if matches!(status.state, SourceState::Closed | SourceState::Cancelled) {
                    self.registration_possible = false;
                    self.phase = Phase::CloseOffering;
                } else {
                    self.phase = Phase::InspectSource;
                }
                Ok(())
            }
            Outcome::Failure(SupplyFail::Pending) => Ok(()),
            Outcome::Failure(failure) => Err(Error::Remote(failure)),
        }
    }

    fn close_offering<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<(), Error<C::Error>> {
        let Some(root) = self.root else {
            self.phase = Phase::Closed;
            return Ok(());
        };
        match control
            .execute(Command::RetireOffering(root), wait)
            .map_err(Error::Control)?
        {
            Response::Retired => {
                self.root = None;
                self.offered = None;
                self.phase = Phase::Closed;
                Ok(())
            }
            _ => Err(Error::InvalidObservation),
        }
    }

    fn reject<E>(&mut self, failure: SupplyFail) -> Result<(), Error<E>> {
        self.failure.get_or_insert(failure);
        self.begin_closing();
        Err(Error::Remote(failure))
    }
}
