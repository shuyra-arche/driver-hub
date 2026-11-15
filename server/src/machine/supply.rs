//! Allocation-local native delivery. System owns Instance identity; the
//! Authority owns imported provenance; this Binding owns its actual leases.

use super::{Machine, binding::Binding};
use crate::{
    authority::{GrantRequest, SupplyAuthority},
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

impl<A: SupplyAuthority> Machine<A> {
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
        let (index, device, slot) = self
            .supply_index(allocation)
            .map_err(SupplyError::Machine)?;
        let binding = &self.bindings[index];
        if self.settling || binding.state != BindingState::Preparing {
            return Err(SupplyError::Machine(Fail::Conflict));
        }
        if mark == Mark::NONE {
            return Err(SupplyError::Machine(Fail::Invalid));
        }
        if let Some(lease) = &binding.supplies[slot] {
            return if lease.request().mark() == mark {
                Ok(lease.state())
            } else {
                Err(SupplyError::Machine(Fail::Conflict))
            };
        }
        if binding
            .supplies
            .iter()
            .flatten()
            .any(|lease| lease.request().mark() == mark)
        {
            return Err(SupplyError::Machine(Fail::Conflict));
        }
        if !self.devices[device].provided {
            return Err(SupplyError::Machine(Fail::Denied));
        }
        let driver = binding
            .driver
            .as_ref()
            .ok_or(SupplyError::Machine(Fail::Unknown))?;
        let progress = driver.progress();
        if progress.pending.is_some() || progress.closing || progress.settled {
            return Err(SupplyError::Machine(Fail::Busy));
        }
        let observation = driver
            .observe(control, wait)
            .map_err(SupplyError::Instance)?;
        if observation.phase != system_api::control::instance::Phase::Prepared || observation.ready
        {
            return Err(SupplyError::Machine(Fail::Conflict));
        }
        let target = usize::try_from(observation.task.get())
            .map_err(|_| SupplyError::Machine(Fail::Invalid))?;
        if target == 0 || self.authority.owner().get() as u64 != control.caller().get() {
            return Err(SupplyError::Machine(Fail::Denied));
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
            .map_err(SupplyError::Machine)?;
        if request.correlation() != correlation
            || request.recipient() != recipient
            || request.mark() != mark
        {
            return Err(SupplyError::Machine(Fail::Denied));
        }
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
            // Exact completion of this owned branch clears only its slot's
            // authorization duty. Source/device/Instance duties are separate.
            binding.pending &= !Binding::<Lease>::bit(allocation.slot, Duty::Authorization);
            binding.state = if binding.pending == 0 {
                BindingState::Closed
            } else {
                BindingState::Retiring
            };
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

    fn supply_index(&self, allocation: AllocationId) -> Result<(usize, usize, usize), Fail> {
        let index = self.binding_index(allocation.binding)?;
        let device = self.device_index(self.bindings[index].device)?;
        let slot = usize::from(allocation.slot);
        if slot >= usize::from(self.devices[device].offer.count) {
            return Err(Fail::Invalid);
        }
        Ok((index, device, slot))
    }
}
