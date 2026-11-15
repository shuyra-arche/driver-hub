use hub_api::{BindingId, BindingState, DeviceRef, Duty, Snapshot};
use kernel_api::model::identity::TaskId;

pub(super) struct Binding {
    pub id: BindingId,
    pub device: DeviceRef,
    pub requester: TaskId,
    pub state: BindingState,
    pub pending: u16,
    pub attempt: u64,
    pub fault: u32,
    pub completed: [Option<(TaskId, u64)>; 12],
    pub driver: Option<crate::driver::Driver>,
}

impl Binding {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            device: self.device,
            binding: self.id,
            state: self.state,
            pending: self.pending,
            attempt: self.attempt,
            fault: self.fault,
        }
    }

    pub fn open(&self) -> bool {
        self.state != BindingState::Closed
    }

    pub fn close(&mut self) {
        if self.open() {
            self.state = if self.pending == 0 {
                BindingState::Closed
            } else if self.state == BindingState::Retiring {
                BindingState::Retiring
            } else {
                BindingState::Closing
            };
        }
    }

    pub fn bit(slot: u8, duty: Duty) -> u16 {
        1 << (u16::from(slot) * 3 + duty as u16 - 1)
    }
}
