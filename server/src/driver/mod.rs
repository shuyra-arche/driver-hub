//! Hub's operation ownership and association to a System-owned Instance.
//! System observations remain fresh queries; no Task lifecycle is mirrored here.

mod native;

pub use native::{Native, NativeError};
pub use system_api::control::instance::{InstanceId, OperationId};

use hub_api::BindingId;
use kernel_api::model::wait::Wait;
use system_api::{control::instance as api, identity::Subject, loader::Input, value::Outcome};

#[allow(
    clippy::large_enum_variant,
    reason = "move bounded Create intent without a new fallible heap allocation at submission"
)]
pub enum Command {
    Create(api::Create),
    Start(api::Change),
    Stop(api::Change),
    Retire(api::Change),
    Inspect(InstanceId),
    InspectOperation(OperationId),
    Cancel(OperationId),
}

pub enum Response {
    Receipt(Outcome<api::Receipt, api::Fail>),
    Observation(Outcome<api::Observation, api::Fail>),
}

/// Authenticated System operation boundary. Implemented by Native with the
/// fixed System SDK; no mock or permissive implementation ships in production.
///
/// # Safety
/// `peer` must identify the actual serving System Task; `caller` must identify
/// the actual native Task performing these calls. Responses must come
/// from that peer through a genuinely delivered caller-owned Instance endpoint
/// and exactly match the submitted operation variant. `allocate` must share a
/// non-reusing namespace with *all* Instance clients in this caller Task on this
/// peer (Create/Start/Stop/Retire/Publish use one System journal). A new backend
/// or endpoint must not restart that namespace. Do not return arbitrary public
/// Receipt values as evidence. Preserve native reply cleanup on every failure;
/// transport uncertainty must not be reported as a completed or absent effect.
pub unsafe trait Control {
    type Error;
    fn peer(&self) -> system_api::TaskId;
    /// Actual native caller owning this endpoint and operation journal scope.
    fn caller(&self) -> system_api::TaskId;
    fn allocate(&mut self) -> Option<OperationId>;
    fn execute(&mut self, command: Command, wait: Wait) -> Result<Response, Self::Error>;
    /// False is allowed only with positive local evidence that no frame for
    /// this call was submitted. Unknown submission must return true.
    fn may_have_submitted(&self, _: &Self::Error) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Create,
    Start,
    Stop,
    Retire,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operation {
    pub id: OperationId,
    pub kind: Kind,
    pub rejected: bool,
}

/// Hub coordination progress, not System Instance life. `settled` means this
/// owner has no remaining Instance responsibility after verified retirement or
/// positive absence proof. It says nothing about Device/Authorization duties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub binding: BindingId,
    pub peer: system_api::TaskId,
    pub caller: Option<system_api::TaskId>,
    pub instance: Option<InstanceId>,
    pub pending: Option<Operation>,
    pub closing: bool,
    pub settled: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Transport(E),
    Invalid,
    Conflict,
    Exhausted,
    WrongPeer,
    WrongCaller,
    WrongReceipt,
    Rejected(api::Fail),
    Unseen,
}

/// Retains accepted and potentially accepted work through reply loss or owner
/// cancellation. Dropping this record is never proof of System retirement.
/// Machine stores the same record directly inside its Binding.
#[must_use = "retain the Driver until System retirement is established"]
pub struct Driver {
    binding: BindingId,
    peer: system_api::TaskId,
    caller: Option<system_api::TaskId>,
    instance: Option<InstanceId>,
    pending: Option<Operation>,
    created: bool,
    closing: bool,
    settled: bool,
}

impl Driver {
    pub fn new(binding: BindingId, peer: system_api::TaskId) -> Option<Self> {
        (binding.serial != 0
            && binding.machine.incarnation != 0
            && peer != system_api::TaskId::NONE)
            .then_some(Self {
                binding,
                peer,
                caller: None,
                instance: None,
                pending: None,
                created: false,
                closing: false,
                settled: false,
            })
    }

    pub fn progress(&self) -> Progress {
        Progress {
            binding: self.binding,
            peer: self.peer,
            caller: self.caller,
            instance: self.instance,
            pending: self.pending,
            closing: self.closing,
            settled: self.settled,
        }
    }

    /// Record the original operation before the only Create submission. On
    /// any error, retain this Driver and query that operation on the same peer.
    pub fn create<C: Control>(
        &mut self,
        control: &mut C,
        input: Input,
        subject: Subject,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        self.check_peer(control)?;
        if control.caller() == system_api::TaskId::NONE {
            return Err(Error::Invalid);
        }
        if self.created || self.closing {
            return Err(Error::Conflict);
        }
        let id = control.allocate().ok_or(Error::Exhausted)?;
        self.caller = Some(control.caller());
        self.created = true;
        self.pending = Some(Operation {
            id,
            kind: Kind::Create,
            rejected: false,
        });
        self.submit(
            control,
            Command::Create(api::Create { id, input, subject }),
            wait,
        )
    }

    pub fn start<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        self.change(control, Kind::Start, wait)
    }

    /// Stop completion only admits native closing. Poll subsequently submits
    /// Retire and retains the Instance duty through actual cleanup.
    pub fn stop<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        self.check_peer(control)?;
        self.closing = true;
        self.change(control, Kind::Stop, wait)
    }

    /// Cancellation is monotonic. A completed Create/Start cannot be undone by
    /// Cancel; its known Instance then needs a new Retire operation.
    pub fn close<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        if self.created {
            self.check_peer(control)?;
        }
        self.closing = true;
        self.poll(control, wait)
    }

    pub fn poll<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        if self.closing && !self.created && self.instance.is_none() && self.pending.is_none() {
            // No submission was ever attempted. This local absence proof does
            // not apply after an uncertain Create or Unknown operation query.
            self.settled = true;
            return Ok(self.progress());
        }
        self.check_peer(control)?;
        if self.settled {
            return Ok(self.progress());
        }
        if let Some(operation) = self.pending {
            let command = if self.closing && operation.kind != Kind::Retire {
                Command::Cancel(operation.id)
            } else {
                Command::InspectOperation(operation.id)
            };
            return self.submit(control, command, wait);
        }
        if self.closing && self.instance.is_some() {
            return self.change(control, Kind::Retire, wait);
        }
        Ok(self.progress())
    }

    /// A terminal failed Retire is retained until an explicit new intent.
    /// Retrying uses a fresh caller-wide ID for the same exact Instance, while
    /// preserving its duty until that new retirement actually completes.
    pub fn retry_retirement<C: Control>(
        &mut self,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        self.check_peer(control)?;
        let previous = self.pending.ok_or(Error::Conflict)?;
        if previous.kind != Kind::Retire || !previous.rejected || self.settled {
            return Err(Error::Conflict);
        }
        let instance = self.instance.ok_or(Error::Invalid)?;
        let id = control.allocate().ok_or(Error::Exhausted)?;
        self.pending = Some(Operation {
            id,
            kind: Kind::Retire,
            rejected: false,
        });
        self.submit(control, Command::Retire(api::Change { id, instance }), wait)
    }

    /// Return a fresh authorized observation, including optional UnitRoot.
    /// The Task/Team/root coordinates are not resource or execution authority.
    pub fn observe<C: Control>(
        &self,
        control: &mut C,
        wait: Wait,
    ) -> Result<api::Observation, Error<C::Error>> {
        self.check_peer(control)?;
        let instance = self.instance.ok_or(Error::Invalid)?;
        let response = control
            .execute(Command::Inspect(instance), wait)
            .map_err(Error::Transport)?;
        let Response::Observation(response) = response else {
            return Err(Error::WrongReceipt);
        };
        let value = match response {
            Outcome::Success(value) => value,
            Outcome::Failure(fail) => return Err(Error::Rejected(fail)),
        };
        if value.instance != instance
            || value
                .root
                .0
                .as_ref()
                .is_some_and(|root| root.registration.instance != instance)
        {
            return Err(Error::WrongReceipt);
        }
        Ok(value)
    }

    fn check_peer<C: Control>(&self, control: &C) -> Result<(), Error<C::Error>> {
        if self.caller.is_some_and(|caller| caller != control.caller()) {
            Err(Error::WrongCaller)
        } else if control.peer() != self.peer {
            Err(Error::WrongPeer)
        } else {
            Ok(())
        }
    }

    fn change<C: Control>(
        &mut self,
        control: &mut C,
        kind: Kind,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        self.check_peer(control)?;
        if self.pending.is_some() || self.settled || (kind == Kind::Start && self.closing) {
            return Err(Error::Conflict);
        }
        let instance = self.instance.ok_or(Error::Invalid)?;
        let id = control.allocate().ok_or(Error::Exhausted)?;
        self.pending = Some(Operation {
            id,
            kind,
            rejected: false,
        });
        let change = api::Change { id, instance };
        let command = match kind {
            Kind::Start => Command::Start(change),
            Kind::Stop => Command::Stop(change),
            Kind::Retire => Command::Retire(change),
            Kind::Create => unreachable!(),
        };
        self.submit(control, command, wait)
    }

    fn submit<C: Control>(
        &mut self,
        control: &mut C,
        command: Command,
        wait: Wait,
    ) -> Result<Progress, Error<C::Error>> {
        let mutation = matches!(
            command,
            Command::Create(_) | Command::Start(_) | Command::Stop(_) | Command::Retire(_)
        );
        let response = match control.execute(command, wait) {
            Ok(response) => response,
            Err(error) => {
                if mutation && !control.may_have_submitted(&error) {
                    if self
                        .pending
                        .is_some_and(|operation| operation.kind == Kind::Create)
                    {
                        self.created = false;
                    }
                    self.pending = None;
                }
                return Err(Error::Transport(error));
            }
        };
        let Response::Receipt(response) = response else {
            return Err(Error::WrongReceipt);
        };
        let receipt = match response {
            Outcome::Success(receipt) => receipt,
            // In particular, Unknown from InspectOperation or Cancel is not a
            // tombstone for a submitted frame that could still arrive later.
            Outcome::Failure(api::Fail::Unknown) => return Err(Error::Unseen),
            Outcome::Failure(fail) => return Err(Error::Rejected(fail)),
        };
        let operation = self.pending.ok_or(Error::WrongReceipt)?;
        if receipt.id != operation.id {
            return Err(Error::WrongReceipt);
        }
        if let Some(instance) = receipt.instance.0 {
            if instance.system != self.peer || self.instance.is_some_and(|known| known != instance)
            {
                return Err(Error::WrongReceipt);
            }
        } else if self.instance.is_some() || receipt.state == api::State::Completed {
            return Err(Error::WrongReceipt);
        }
        // Validate the entire receipt before committing any observation.
        if receipt.state == api::State::Completed && receipt.failure.0.is_some() {
            return Err(Error::WrongReceipt);
        }
        if let Some(instance) = receipt.instance.0 {
            self.instance = Some(instance);
        }
        match receipt.state {
            api::State::Pending => (),
            api::State::Cancelled => {
                self.pending = None;
                self.closing = true;
                self.settled = true;
            }
            api::State::Completed if operation.kind == Kind::Retire => {
                self.pending = None;
                self.closing = true;
                self.settled = true;
            }
            api::State::Completed => {
                self.pending = None;
            }
            api::State::Failed => {
                self.pending.as_mut().unwrap().rejected = true;
                if operation.kind == Kind::Create
                    && self.instance.is_none()
                    && let Some(failure) = &receipt.failure.0
                {
                    // In the pinned System provider, a rejected Create with no
                    // attached Instance has committed no Instance record. This
                    // is narrower than generic failure or failed retirement.
                    self.pending = None;
                    self.closing = true;
                    self.settled = true;
                    return Err(Error::Rejected(failure.0));
                }
                // A failed operation is not generic retirement proof. A known
                // Instance can be explicitly retired on the next closing turn.
                self.closing = true;
                if self.instance.is_some() && operation.kind != Kind::Retire {
                    self.pending = None;
                }
                return Err(Error::Rejected(
                    receipt
                        .failure
                        .0
                        .map_or(api::Fail::Interrupted, |fail| fail.0),
                ));
            }
        }
        Ok(self.progress())
    }
}
