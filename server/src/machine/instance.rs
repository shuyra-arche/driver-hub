use super::Machine;
use crate::{
    authority::{Authority, SupplyRecord},
    driver::{self, Control, Driver, Progress},
};
use hub_api::{BindingId, BindingState, Fail};
use kernel_api::model::wait::Wait;
use system_api::{identity::Subject, loader::Input};

#[derive(Debug, PartialEq, Eq)]
pub enum DriverError<E> {
    Machine(Fail),
    Instance(driver::Error<E>),
}

impl<A: Authority> Machine<A> {
    /// Preserve the exact Binding/Device epoch and record duties before the
    /// first System call. The attached coordinator is the sole Instance owner
    /// here; it retains uncertain operation IDs, while System owns Instance life.
    pub fn create_driver<C: Control>(
        &mut self,
        id: BindingId,
        control: &mut C,
        input: Input,
        subject: Subject,
        wait: Wait,
    ) -> Result<Progress, DriverError<C::Error>> {
        let index = self.binding_index(id).map_err(DriverError::Machine)?;
        if let Some(driver) = &self.bindings[index].driver {
            let progress = driver.progress();
            if progress.pending.is_some()
                || progress.instance.is_some()
                || progress.closing
                || progress.settled
            {
                return Err(DriverError::Machine(Fail::Conflict));
            }
        }
        self.begin_preparation(id).map_err(DriverError::Machine)?;
        if self.bindings[index].driver.is_none() {
            let driver =
                Driver::new(id, control.peer()).ok_or(DriverError::Machine(Fail::Invalid))?;
            self.bindings[index].driver = Some(driver);
        }
        let result = self.bindings[index]
            .driver
            .as_mut()
            .unwrap()
            .create(control, input, subject, wait);
        self.synchronize_driver(index);
        result.map_err(DriverError::Instance)
    }

    /// Start completion is not readiness and does not make the hardware
    /// Binding Active. Device/Pend preparation still has independent owners.
    pub fn start_driver<C: Control>(
        &mut self,
        id: BindingId,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, DriverError<C::Error>> {
        let index = self.binding_index(id).map_err(DriverError::Machine)?;
        if self.bindings[index].state != BindingState::Preparing {
            return Err(DriverError::Machine(Fail::Conflict));
        }
        let driver = self.bindings[index]
            .driver
            .as_mut()
            .ok_or(DriverError::Machine(Fail::Unknown))?;
        if driver.progress().pending.is_some() {
            // Admission gates a new Start; poll_driver recovers an old one.
            return driver.start(control, wait).map_err(DriverError::Instance);
        }
        let binding = &self.bindings[index];
        let device = self
            .device_index(binding.device)
            .map_err(DriverError::Machine)?;
        let delivered = binding
            .supplies
            .iter()
            .enumerate()
            .fold(0, |mask, (slot, lease)| {
                mask | u8::from(lease.as_ref().is_some_and(SupplyRecord::delivered)) << slot
            });
        self.authority
            .check_start(
                binding.requester,
                self.devices[device].provider,
                &self.devices[device].offer,
                delivered,
            )
            .map_err(DriverError::Machine)?;
        let result = self.bindings[index]
            .driver
            .as_mut()
            .ok_or(DriverError::Machine(Fail::Unknown))?
            .start(control, wait);
        self.synchronize_driver(index);
        result.map_err(DriverError::Instance)
    }

    pub fn stop_driver<C: Control>(
        &mut self,
        id: BindingId,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, DriverError<C::Error>> {
        let index = self.binding_index(id).map_err(DriverError::Machine)?;
        self.bindings[index].close();
        let result = self.bindings[index]
            .driver
            .as_mut()
            .ok_or(DriverError::Machine(Fail::Unknown))?
            .stop(control, wait);
        self.synchronize_driver(index);
        result.map_err(DriverError::Instance)
    }

    pub fn close_driver<C: Control>(
        &mut self,
        id: BindingId,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, DriverError<C::Error>> {
        let index = self.binding_index(id).map_err(DriverError::Machine)?;
        self.bindings[index].close();
        let result = self.bindings[index]
            .driver
            .as_mut()
            .ok_or(DriverError::Machine(Fail::Unknown))?
            .close(control, wait);
        self.synchronize_driver(index);
        result.map_err(DriverError::Instance)
    }

    /// Advance one query/effect. Generic Release, withdrawal and Machine settle
    /// also request Driver cancellation here, without discarding pending work.
    pub fn poll_driver<C: Control>(
        &mut self,
        id: BindingId,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, DriverError<C::Error>> {
        let index = self.binding_index(id).map_err(DriverError::Machine)?;
        let closing = matches!(
            self.bindings[index].state,
            BindingState::Closing | BindingState::Retiring | BindingState::Closed
        );
        let driver = self.bindings[index]
            .driver
            .as_mut()
            .ok_or(DriverError::Machine(Fail::Unknown))?;
        let result = if closing {
            driver.close(control, wait)
        } else {
            driver.poll(control, wait)
        };
        self.synchronize_driver(index);
        result.map_err(DriverError::Instance)
    }

    pub fn retry_driver_retirement<C: Control>(
        &mut self,
        id: BindingId,
        control: &mut C,
        wait: Wait,
    ) -> Result<Progress, DriverError<C::Error>> {
        let index = self.binding_index(id).map_err(DriverError::Machine)?;
        let result = self.bindings[index]
            .driver
            .as_mut()
            .ok_or(DriverError::Machine(Fail::Unknown))?
            .retry_retirement(control, wait);
        self.synchronize_driver(index);
        result.map_err(DriverError::Instance)
    }

    pub fn driver(&self, id: BindingId) -> Result<Option<&Driver>, Fail> {
        self.binding_index(id)
            .map(|index| self.bindings[index].driver.as_ref())
    }

    fn synchronize_driver(&mut self, index: usize) {
        let binding = &mut self.bindings[index];
        let progress = binding.driver.as_ref().unwrap().progress();
        if progress.closing {
            binding.close();
        }
        if progress.settled {
            // The same System Instance underlies all resource slots. Its
            // terminal proof clears every Instance bit and no Device/Auth bit.
            for slot in 0..hub_api::MAX_RESOURCES {
                binding.pending &=
                    !super::binding::Binding::<A::Supply>::bit(slot as u8, hub_api::Duty::Instance);
            }
            binding.state = if binding.pending == 0 {
                BindingState::Closed
            } else {
                BindingState::Retiring
            };
        }
    }
}
