//! System owns the registered source and installation executors; this Binding
//! retains the one caller-side owner for its exact allocation and offering.

use super::{DriverError, Machine};
use crate::{
    authority::SupplyAuthority,
    driver,
    supply::{self, Control, Progress, State, Supply},
};
use hub_api::{AllocationId, BindingState, Fail};
use kernel_api::model::{identity::Mark, wait::Wait};
use system_api::supply::Key;

#[derive(Debug)]
pub enum InstallError<I, S> {
    Machine(Fail),
    Instance(driver::Error<I>),
    Supply(supply::Error<S>),
}

impl<A: SupplyAuthority<Supply = Supply>> Machine<A> {
    /// Keep one immutable intent before any offering/Register/Install effect.
    /// The source name is the trusted Hub Unit declaration name; the resource
    /// name is derived from the exact allocation and attempt, never wire input.
    /// System rechecks caller, namespace, source and actual Prepared recipient.
    pub fn prepare_system_supply<I: driver::Control, S: Control>(
        &mut self,
        allocation: AllocationId,
        instance: &mut I,
        control: &mut S,
        source: &str,
        mark: Mark,
        wait: Wait,
    ) -> Result<Progress, InstallError<I::Error, S::Error>> {
        let indices = self
            .preparing_supply(allocation, mark)
            .map_err(InstallError::Machine)?;
        if instance.peer() != control.peer()
            || instance.caller() != control.caller()
            || self.authority.owner().get() as u64 != control.caller().get()
        {
            return Err(InstallError::Machine(Fail::Denied));
        }
        let (index, _, slot) = indices;
        if let Some(record) = &self.bindings[index].supplies[slot] {
            return if record.source_name() == source {
                Ok(record.progress())
            } else {
                Err(InstallError::Machine(Fail::Conflict))
            };
        }
        let key = Key {
            name: source.into(),
        };
        if !key.valid() || key.name.len() > 128 {
            return Err(InstallError::Machine(Fail::Invalid));
        }
        let (request, target) = self
            .prepare_grant(allocation, indices, instance, mark, wait)
            .map_err(|error| match error {
                DriverError::Machine(fail) => InstallError::Machine(fail),
                DriverError::Instance(error) => InstallError::Instance(error),
            })?;
        let resource = Key {
            name: alloc::format!(
                "allocation-{:x}-{:x}-{:x}-{:x}",
                allocation.binding.machine.incarnation,
                allocation.binding.serial,
                request.correlation().attempt,
                allocation.slot,
            ),
        };
        // SAFETY: the Authority checked this Task's exact imported source, and
        // Driver inspected this caller-owned System Instance and actual Task.
        // This is the only managed offering/installation for this slot/attempt.
        // Construction has no native/RPC effect; publish the owner first.
        let record =
            unsafe { Supply::new_for_current_task(request, target, key, resource, control) }
                .map_err(InstallError::Supply)?;
        let progress = record.progress();
        self.bindings[index].supplies[slot] = Some(record);
        Ok(progress)
    }

    /// Advance the same owner and IDs. Unknown outcomes retain intent and are
    /// queried/replayed. Closing admission permits only withdrawal/retirement.
    pub fn advance_system_supply<S: Control>(
        &mut self,
        allocation: AllocationId,
        control: &mut S,
        wait: Wait,
    ) -> Result<Progress, InstallError<core::convert::Infallible, S::Error>> {
        let (index, _, slot) = self
            .supply_index(allocation)
            .map_err(InstallError::Machine)?;
        let binding = &mut self.bindings[index];
        let closing = matches!(
            binding.state,
            BindingState::Closing | BindingState::Retiring | BindingState::Closed
        );
        let record = binding.supplies[slot]
            .as_mut()
            .ok_or(InstallError::Machine(Fail::Unknown))?;
        let result = if closing {
            record.finish(control, wait)
        } else {
            record.advance(control, wait)
        };
        let progress = record.progress();
        if matches!(progress.state, State::Closing | State::Closed) {
            // A source failure closes the Binding even while preparation was
            // otherwise admitted; its cause remains with the retained owner.
            self.bindings[index].close();
        }
        if progress.state == State::Closed {
            // Only this exact managed owner proves both remote levels and its
            // local offering retired. No Device or Instance duty is cleared.
            self.settled_authorization(index, allocation.slot);
        }
        result.map_err(InstallError::Supply)
    }

    pub fn finish_system_supply<S: Control>(
        &mut self,
        allocation: AllocationId,
        control: &mut S,
        wait: Wait,
    ) -> Result<Progress, InstallError<core::convert::Infallible, S::Error>> {
        let (index, _, _) = self
            .supply_index(allocation)
            .map_err(InstallError::Machine)?;
        self.bindings[index].close();
        self.advance_system_supply(allocation, control, wait)
    }

    /// Retained operation observation, not live authority. A closing Binding
    /// never exposes a fresh delivery even before remote revocation finishes.
    pub fn system_supply(&self, allocation: AllocationId) -> Result<Progress, Fail> {
        let (index, _, slot) = self.supply_index(allocation)?;
        let binding = &self.bindings[index];
        let record = binding.supplies[slot].as_ref().ok_or(Fail::Unknown)?;
        let mut progress = record.progress();
        if !matches!(
            binding.state,
            BindingState::Preparing | BindingState::Active
        ) {
            progress.delivery = None;
        }
        Ok(progress)
    }
}
