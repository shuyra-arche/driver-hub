//! Allocation-local native delivery. System owns Instance identity; the
//! Authority owns imported provenance; this Binding owns its actual leases.

use super::{Machine, binding::Binding};
use crate::{
    authority::{Authority, GrantRequest, SupplyAuthority, SupplyRecord},
    driver::{self, Control},
    lease::{self, Correlation, Delivery, Lease, State},
};
use hub_api::{AllocationId, BindingState, Duty, Fail};
use kernel_api::model::{identity::Mark, identity::TaskId, wait::Wait};

#[derive(Debug)]
pub enum SupplyError<E = core::convert::Infallible> {
    Machine(Fail),
    Instance(driver::Error<E>),
    Lease(lease::Error),
}

impl<A: SupplyAuthority<Supply = Lease>> Machine<A> {
    /// Record one exact allocation's delivery intent, without deriving a Pend.
    /// The recipient is selected only from the bound System Driver's fresh
    /// Prepared observation, never from a requester-supplied Task number.
    pub fn prepare_supply<C: Control>(
        &mut self,
        allocation: AllocationId,
        control: &mut C,
        mark: Mark,
        wait: Wait,
    ) -> Result<State, SupplyError<C::Error>> {
        let indices = self
            .preparing_supply(allocation, mark)
            .map_err(SupplyError::Machine)?;
        let (index, _, slot) = indices;
        if let Some(lease) = &self.bindings[index].supplies[slot] {
            return Ok(lease.state());
        }
        let (request, _) = self
            .prepare_grant(allocation, indices, control, mark, wait)
            .map_err(|error| match error {
                super::DriverError::Machine(fail) => SupplyError::Machine(fail),
                super::DriverError::Instance(error) => SupplyError::Instance(error),
            })?;
        // SAFETY: SupplyAuthority verifies this actual local source and owner;
        // Control fixes the actual System caller and target observation. Store
        // the lease in its Binding before the first possible native effect.
        let lease = unsafe { Lease::new_for_current_task(self.authority.owner(), request) }
            .map_err(SupplyError::Lease)?;
        self.bindings[index].supplies[slot] = Some(lease);
        Ok(State::RootPending)
    }

    /// Advance one native effect. Closing/withdrawal/settle can only retire,
    /// even if the private root has not yet reached its receiver.
    pub fn advance_supply(&mut self, allocation: AllocationId) -> Result<State, SupplyError> {
        let (index, _, slot) = self
            .supply_index(allocation)
            .map_err(SupplyError::Machine)?;
        let binding = &mut self.bindings[index];
        let closing = matches!(
            binding.state,
            BindingState::Closing | BindingState::Retiring | BindingState::Closed
        );
        let lease = binding.supplies[slot]
            .as_mut()
            .ok_or(SupplyError::Machine(Fail::Unknown))?;
        let state = if closing {
            lease.finish()
        } else {
            lease.advance()
        }
        .map_err(SupplyError::Lease)?;
        if closing && state == State::Closed {
            // No foreign owner receipt substitutes for this managed branch.
            self.settled_authorization(index, allocation.slot);
        }
        Ok(state)
    }

    /// Close the whole Binding before withdrawing one slot. Other slots and
    /// the Driver keep their own retained state and need explicit advancement.
    pub fn finish_supply(&mut self, allocation: AllocationId) -> Result<State, SupplyError> {
        let (index, _, _) = self
            .supply_index(allocation)
            .map_err(SupplyError::Machine)?;
        self.bindings[index].close();
        self.advance_supply(allocation)
    }

    /// Historical delivery is exposed only while the Binding admits work.
    /// Kernel remains the live authorization owner when the token is used.
    pub fn supply(&self, allocation: AllocationId) -> Result<Option<Delivery>, Fail> {
        let (index, _, slot) = self.supply_index(allocation)?;
        let binding = &self.bindings[index];
        if !matches!(
            binding.state,
            BindingState::Preparing | BindingState::Active
        ) {
            return Ok(None);
        }
        Ok(binding.supplies[slot].as_ref().and_then(Lease::delivery))
    }
}

impl<A: Authority> Machine<A> {
    pub(super) fn supply_index(
        &self,
        allocation: AllocationId,
    ) -> Result<(usize, usize, usize), Fail> {
        let index = self.binding_index(allocation.binding)?;
        let device = self.device_index(self.bindings[index].device)?;
        let slot = usize::from(allocation.slot);
        if slot >= usize::from(self.devices[device].offer.count) {
            return Err(Fail::Invalid);
        }
        Ok((index, device, slot))
    }

    pub(super) fn preparing_supply(
        &self,
        allocation: AllocationId,
        mark: Mark,
    ) -> Result<(usize, usize, usize), Fail> {
        let indices = self.supply_index(allocation)?;
        let (index, device, slot) = indices;
        let binding = &self.bindings[index];
        if self.settling || binding.state != BindingState::Preparing {
            return Err(Fail::Conflict);
        }
        if mark == Mark::NONE {
            return Err(Fail::Invalid);
        }
        if let Some(record) = &binding.supplies[slot]
            && record.request().mark() != mark
        {
            return Err(Fail::Conflict);
        }
        if binding.supplies.iter().enumerate().any(|(other, record)| {
            other != slot
                && record
                    .as_ref()
                    .is_some_and(|record| record.request().mark() == mark)
        }) {
            return Err(Fail::Conflict);
        }
        if !self.devices[device].provided {
            return Err(Fail::Denied);
        }
        Ok(indices)
    }

    pub(super) fn settled_authorization(&mut self, index: usize, slot: u8) {
        let binding = &mut self.bindings[index];
        binding.pending &= !Binding::<A::Supply>::bit(slot, Duty::Authorization);
        binding.state = if binding.pending == 0 {
            BindingState::Closed
        } else {
            BindingState::Retiring
        };
    }
}

impl<A: SupplyAuthority> Machine<A> {
    pub(super) fn prepare_grant<C: Control>(
        &mut self,
        allocation: AllocationId,
        (index, device, slot): (usize, usize, usize),
        control: &mut C,
        mark: Mark,
        wait: Wait,
    ) -> Result<
        (lease::Request, system_api::control::instance::InstanceId),
        super::DriverError<C::Error>,
    > {
        use super::DriverError;
        let binding = &self.bindings[index];
        let driver = binding
            .driver
            .as_ref()
            .ok_or(DriverError::Machine(Fail::Unknown))?;
        let progress = driver.progress();
        if progress.pending.is_some() || progress.closing || progress.settled {
            return Err(DriverError::Machine(Fail::Busy));
        }
        let observation = driver
            .observe(control, wait)
            .map_err(DriverError::Instance)?;
        if observation.phase != system_api::control::instance::Phase::Prepared || observation.ready
        {
            return Err(DriverError::Machine(Fail::Conflict));
        }
        let target = usize::try_from(observation.task.get())
            .map_err(|_| DriverError::Machine(Fail::Invalid))?;
        if target == 0 || self.authority.owner().get() as u64 != control.caller().get() {
            return Err(DriverError::Machine(Fail::Denied));
        }
        let correlation = Correlation {
            device: binding.device,
            allocation,
            attempt: binding.attempt,
        };
        let recipient = TaskId::new(target);
        let request = self
            .authority
            .lease_request(GrantRequest {
                requester: binding.requester,
                provider: self.devices[device].provider,
                correlation,
                resource: self.devices[device].offer.resources[slot],
                recipient,
                mark,
            })
            .map_err(DriverError::Machine)?;
        if request.correlation() != correlation
            || request.recipient() != recipient
            || request.mark() != mark
        {
            return Err(DriverError::Machine(Fail::Denied));
        }
        Ok((request, observation.instance))
    }
}
