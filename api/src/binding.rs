use crate::{AllocationId, BindingId, DeviceRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Code)]
#[repr(u8)]
pub enum BindingState {
    Reserving = 1,
    Preparing = 2,
    Active = 3,
    Closing = 4,
    Retiring = 5,
    Closed = 6,
}

/// Independent retirement owners, never inferred from task exit or timeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Code)]
#[repr(u8)]
pub enum Duty {
    Device = 1,
    Authorization = 2,
    Instance = 3,
}

/// Queryable coordination observation. The mask has three bits per resource,
/// ordered Device/Authorization/Instance. It carries no access authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Snapshot {
    pub device: DeviceRef,
    pub binding: BindingId,
    #[frame(code)]
    pub state: BindingState,
    pub pending: u16,
    pub attempt: u64,
    pub fault: u32,
}

/// Opaque provider evidence. Only the trusted Authority may validate its
/// meaning, owner, exact range, epoch and retirement completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retirement {
    pub device: DeviceRef,
    pub allocation: AllocationId,
    pub attempt: u64,
    pub duty: Duty,
    pub receipt: u64,
}
