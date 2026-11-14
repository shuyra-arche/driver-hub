use hub_api::{DeviceId, DeviceRef, Offer};
use kernel_api::model::identity::TaskId;

pub(super) struct Device {
    pub id: DeviceId,
    pub provider: TaskId,
    pub offer: Offer,
    pub provided: bool,
}

impl Device {
    pub fn reference(&self) -> DeviceRef {
        DeviceRef {
            id: self.id,
            epoch: self.offer.epoch,
        }
    }
}
