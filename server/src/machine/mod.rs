mod binding;
mod device;
mod driver;
mod operation;

pub use driver::DriverError;

use alloc::vec::Vec;
use hub_api::{
    BindingId, BindingState, DeviceId, DeviceRef, Fail, Inspect, MAX_RESOURCES, MachineId, Offer,
    Release, Reply, Reserve, ResourceKind, ResourceSpec, Retirement, Snapshot, Withdraw,
};
use kernel_api::model::identity::TaskId;

use crate::authority::Authority;
use binding::Binding;
use device::Device;
use operation::{Intent, Operation};

/// Bounded retained state. Full history rejects new operations rather than
/// forgetting old identifiers or evicting unfinished duties.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub devices: usize,
    pub bindings: usize,
    pub operations: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            devices: 128,
            bindings: 256,
            operations: 1024,
        }
    }
}

/// One serialization boundary for conflicts spanning device resource ranges.
/// There is no separate inventory/claim/DMA ownership path. All transitions
/// require exclusive access; a service must hold that access across each call.
pub struct Machine<A: Authority> {
    id: MachineId,
    authority: A,
    limits: Limits,
    devices: Vec<Device>,
    bindings: Vec<Binding>,
    operations: Vec<Operation>,
    device_serial: u64,
    binding_serial: u64,
    settling: bool,
}

impl<A: Authority> Machine<A> {
    /// Construct a fresh domain, never recover from missing state by starting
    /// another empty domain over the same resources.
    ///
    /// # Safety
    /// The trusted boot/composition owner must assign a non-reused incarnation
    /// and establish that no previous Hub can mutate these resources and no
    /// previous allocation or hardware duty remains outstanding. Changing this
    /// number alone proves neither condition. Recovery/adoption of a lost Hub
    /// journal is unsupported; keep affected hardware quarantined instead.
    pub unsafe fn new(id: MachineId, authority: A, limits: Limits) -> Result<Self, Fail> {
        if id.incarnation == 0 {
            return Err(Fail::Invalid);
        }
        Ok(Self {
            id,
            authority,
            limits,
            devices: Vec::new(),
            bindings: Vec::new(),
            operations: Vec::new(),
            device_serial: 0,
            binding_serial: 0,
            settling: false,
        })
    }

    pub fn offer(&mut self, caller: TaskId, operation: u64, offer: Offer) -> Reply {
        self.execute(caller, operation, Intent::Offer(offer))
    }
    pub fn withdraw(&mut self, caller: TaskId, operation: u64, withdraw: Withdraw) -> Reply {
        self.execute(caller, operation, Intent::Withdraw(withdraw))
    }
    pub fn reserve(&mut self, caller: TaskId, operation: u64, reserve: Reserve) -> Reply {
        self.execute(caller, operation, Intent::Reserve(reserve))
    }
    pub fn release(&mut self, caller: TaskId, operation: u64, release: Release) -> Reply {
        self.execute(caller, operation, Intent::Release(release))
    }

    fn execute(&mut self, caller: TaskId, id: u64, intent: Intent) -> Reply {
        if caller.get() == 0 || id == 0 {
            return Reply::failed(Fail::Invalid);
        }
        if let Some(old) = self
            .operations
            .iter()
            .find(|o| o.caller == caller && o.id == id)
        {
            return if old.intent == intent {
                self.refresh(old.result)
            } else {
                Reply::failed(Fail::Conflict)
            };
        }
        if self
            .operations
            .iter()
            .any(|o| o.caller == caller && o.id > id)
        {
            return Reply::failed(Fail::Stale);
        }
        if self.operations.len() >= self.limits.operations
            || self.operations.try_reserve(1).is_err()
        {
            return Reply::failed(Fail::Full);
        }
        let result = match intent {
            Intent::Offer(offer) => self.offer_new(caller, offer),
            Intent::Withdraw(withdraw) => self.withdraw_new(caller, withdraw),
            Intent::Reserve(reserve) => self.reserve_new(caller, reserve),
            Intent::Release(release) => self.release_new(caller, release),
        }
        .map(Reply::observed)
        .unwrap_or_else(Reply::failed);
        self.operations.push(Operation {
            caller,
            id,
            intent,
            result,
        });
        result
    }

    fn offer_new(&mut self, caller: TaskId, offer: Offer) -> Result<Snapshot, Fail> {
        if self.settling {
            return Err(Fail::Busy);
        }
        let count = usize::from(offer.count);
        if offer.key == 0
            || offer.epoch == 0
            || count == 0
            || count > MAX_RESOURCES
            || offer.resources[..count].iter().any(|r| !r.valid())
            || offer.resources[count..]
                .iter()
                .any(|r| *r != ResourceSpec::EMPTY)
        {
            return Err(Fail::Invalid);
        }
        if offer.resources[..count]
            .iter()
            .any(|r| matches!(r.kind, ResourceKind::Irq | ResourceKind::External))
        {
            return Err(Fail::Unsupported);
        }
        for (index, resource) in offer.resources[..count].iter().enumerate() {
            if offer.resources[..index]
                .iter()
                .any(|other| resource.overlaps(*other))
            {
                return Err(Fail::Conflict);
            }
        }
        let previous = self
            .devices
            .iter()
            .position(|d| d.provider == caller && d.offer.key == offer.key);
        if let Some(index) = previous {
            let old = &self.devices[index];
            if old.offer.epoch >= offer.epoch {
                return Err(Fail::Stale);
            }
            if old.provided || self.open(old.id) {
                return Err(Fail::Busy);
            }
        }
        for (index, device) in self.devices.iter().enumerate() {
            if previous == Some(index) || (!device.provided && !self.open(device.id)) {
                continue;
            }
            if device.offer.resources[..usize::from(device.offer.count)]
                .iter()
                .any(|old| {
                    offer.resources[..count]
                        .iter()
                        .any(|new| old.overlaps(*new))
                })
            {
                return Err(Fail::Conflict);
            }
        }
        let serial = if previous.is_none() {
            if self.devices.len() >= self.limits.devices {
                return Err(Fail::Full);
            }
            let next = self.device_serial.checked_add(1).ok_or(Fail::Full)?;
            self.devices.try_reserve(1).map_err(|_| Fail::Full)?;
            next
        } else {
            self.device_serial
        };
        self.authority.check_offer(caller, &offer)?;
        let index = if let Some(index) = previous {
            self.devices[index].offer = offer;
            self.devices[index].provided = true;
            index
        } else {
            self.device_serial = serial;
            self.devices.push(Device {
                id: DeviceId {
                    machine: self.id,
                    serial,
                },
                provider: caller,
                offer,
                provided: true,
            });
            self.devices.len() - 1
        };
        Ok(self.device_snapshot(index))
    }

    fn reserve_new(&mut self, caller: TaskId, reserve: Reserve) -> Result<Snapshot, Fail> {
        if self.settling {
            return Err(Fail::Busy);
        }
        let index = self.device_index(reserve.device)?;
        let device = &self.devices[index];
        if !device.provided {
            return Err(Fail::Denied);
        }
        if self.open(device.id) {
            return Err(Fail::Busy);
        }
        if self.bindings.len() >= self.limits.bindings {
            return Err(Fail::Full);
        }
        let serial = self.binding_serial.checked_add(1).ok_or(Fail::Full)?;
        self.bindings.try_reserve(1).map_err(|_| Fail::Full)?;
        self.authority
            .check_reservation(caller, device.provider, reserve.device, &device.offer)?;
        self.binding_serial = serial;
        let binding = Binding {
            id: BindingId {
                machine: self.id,
                serial,
            },
            device: reserve.device,
            requester: caller,
            state: BindingState::Reserving,
            pending: 0,
            attempt: 0,
            fault: 0,
            completed: [None; 12],
            driver: None,
        };
        let snapshot = binding.snapshot();
        self.bindings.push(binding);
        Ok(snapshot)
    }

    fn withdraw_new(&mut self, caller: TaskId, withdraw: Withdraw) -> Result<Snapshot, Fail> {
        let index = self.device_index(withdraw.device)?;
        if self.devices[index].provider != caller {
            return Err(Fail::Denied);
        }
        self.devices[index].provided = false;
        for binding in &mut self.bindings {
            if binding.device == withdraw.device {
                binding.close();
            }
        }
        Ok(self.device_snapshot(index))
    }

    fn release_new(&mut self, caller: TaskId, release: Release) -> Result<Snapshot, Fail> {
        let index = self.binding_index(release.binding)?;
        let binding = &mut self.bindings[index];
        if binding.requester != caller {
            return Err(Fail::Denied);
        }
        binding.close();
        Ok(binding.snapshot())
    }

    /// Trusted executor calls this before *any* external preparation effect.
    /// This only records responsibility; it neither constructs an Instance nor
    /// enables a device. Driver uses the fixed System Instance contract, while
    /// hardware/authorized delivery gates still prevent an Active transition.
    pub fn begin_preparation(&mut self, id: BindingId) -> Result<Snapshot, Fail> {
        if self.settling {
            return Err(Fail::Busy);
        }
        let index = self.binding_index(id)?;
        let binding = &self.bindings[index];
        if binding.state == BindingState::Preparing {
            return Ok(binding.snapshot());
        }
        if binding.state != BindingState::Reserving {
            return Err(Fail::Conflict);
        }
        let device = self.device_index(binding.device)?;
        if !self.devices[device].provided {
            return Err(Fail::Denied);
        }
        self.authority.check_reservation(
            binding.requester,
            self.devices[device].provider,
            binding.device,
            &self.devices[device].offer,
        )?;
        let pending = (1u16 << (u16::from(self.devices[device].offer.count) * 3)) - 1;
        let binding = &mut self.bindings[index];
        binding.attempt = binding.attempt.checked_add(1).ok_or(Fail::Full)?;
        binding.pending = pending;
        binding.state = BindingState::Preparing;
        Ok(binding.snapshot())
    }

    /// A fault preserves its first cause and all already-recorded duties.
    pub fn failed(&mut self, id: BindingId, code: u32) -> Result<Snapshot, Fail> {
        if code == 0 {
            return Err(Fail::Invalid);
        }
        let index = self.binding_index(id)?;
        let binding = &mut self.bindings[index];
        if !binding.open() {
            return Ok(binding.snapshot());
        }
        if binding.fault == 0 {
            binding.fault = code;
        }
        binding.close();
        Ok(binding.snapshot())
    }

    /// Validate a provider receipt at the owner boundary before clearing one
    /// exact duty. Errors leave all duties and range conflicts retained.
    pub fn retire(&mut self, owner: TaskId, evidence: Retirement) -> Result<Snapshot, Fail> {
        let index = self.binding_index(evidence.allocation.binding)?;
        let binding = &self.bindings[index];
        if evidence.duty == hub_api::Duty::Instance && binding.driver.is_some() {
            // Bound System responsibility is settled only by this Binding's
            // authenticated Driver coordinator, never an arbitrary receipt ID.
            return Err(Fail::Denied);
        }
        if binding.device != evidence.device || binding.attempt != evidence.attempt {
            return Err(Fail::Stale);
        }
        let device = self.device_index(evidence.device)?;
        let slot = usize::from(evidence.allocation.slot);
        if slot >= usize::from(self.devices[device].offer.count) || evidence.receipt == 0 {
            return Err(Fail::Invalid);
        }
        let bit = Binding::bit(evidence.allocation.slot, evidence.duty);
        let completed = bit.trailing_zeros() as usize;
        if let Some(accepted) = binding.completed[completed] {
            return if accepted == (owner, evidence.receipt) {
                Ok(binding.snapshot())
            } else {
                Err(Fail::Conflict)
            };
        }
        if !matches!(
            binding.state,
            BindingState::Closing | BindingState::Retiring
        ) {
            return Err(Fail::Conflict);
        }
        if binding.pending & bit == 0 {
            return Err(Fail::Conflict);
        }
        self.authority.check_retirement(
            owner,
            &self.devices[device].offer.resources[slot],
            &evidence,
        )?;
        let binding = &mut self.bindings[index];
        binding.completed[completed] = Some((owner, evidence.receipt));
        binding.pending &= !bit;
        binding.state = if binding.pending == 0 {
            BindingState::Closed
        } else {
            BindingState::Retiring
        };
        Ok(binding.snapshot())
    }

    /// Stop admission and close all bindings. The returned count is retained
    /// responsibility, not a claim that hardware has stopped.
    pub fn settle(&mut self) -> usize {
        self.settling = true;
        for binding in &mut self.bindings {
            binding.close();
        }
        self.bindings.iter().filter(|b| b.open()).count()
    }

    pub fn inspect(&self, caller: TaskId, inspect: Inspect) -> Reply {
        if inspect.operation != 0 {
            return self
                .operations
                .iter()
                .find(|o| o.caller == caller && o.id == inspect.operation)
                .map(|o| self.refresh(o.result))
                .unwrap_or_else(|| Reply::failed(Fail::Unknown));
        }
        match self.device_index(inspect.device) {
            Ok(index) => {
                let device = &self.devices[index];
                let authorized = device.provider == caller
                    || self
                        .bindings
                        .iter()
                        .any(|b| b.device == inspect.device && b.requester == caller);
                if !authorized {
                    Reply::failed(Fail::Denied)
                } else {
                    Reply::observed(self.device_snapshot(index))
                }
            }
            Err(fail) => Reply::failed(fail),
        }
    }

    fn refresh(&self, reply: Reply) -> Reply {
        if reply.failure == Fail::None
            && reply.snapshot.binding.serial != 0
            && let Some(binding) = self
                .bindings
                .iter()
                .find(|b| b.id == reply.snapshot.binding)
        {
            return Reply::observed(binding.snapshot());
        }
        reply
    }
    fn device_index(&self, reference: DeviceRef) -> Result<usize, Fail> {
        if reference.id.machine != self.id {
            return Err(Fail::Stale);
        }
        let index = self
            .devices
            .iter()
            .position(|d| d.id == reference.id)
            .ok_or(Fail::Unknown)?;
        if self.devices[index].offer.epoch != reference.epoch {
            return Err(Fail::Stale);
        }
        Ok(index)
    }
    fn binding_index(&self, id: BindingId) -> Result<usize, Fail> {
        if id.machine != self.id {
            return Err(Fail::Stale);
        }
        self.bindings
            .iter()
            .position(|b| b.id == id)
            .ok_or(Fail::Unknown)
    }
    fn open(&self, id: DeviceId) -> bool {
        self.bindings.iter().any(|b| b.device.id == id && b.open())
    }
    fn device_snapshot(&self, index: usize) -> Snapshot {
        let reference = self.devices[index].reference();
        if let Some(binding) = self.bindings.iter().rev().find(|b| b.device == reference) {
            return binding.snapshot();
        }
        Snapshot {
            device: reference,
            binding: BindingId {
                machine: self.id,
                serial: 0,
            },
            state: BindingState::Closed,
            pending: 0,
            attempt: 0,
            fault: 0,
        }
    }
}
