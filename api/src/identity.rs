//! Non-reusing identities; MachineId is an incarnation, not authorization.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, weave::Frame)]
pub struct MachineId {
    pub incarnation: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, weave::Frame)]
pub struct DeviceId {
    pub machine: MachineId,
    pub serial: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, weave::Frame)]
pub struct DeviceRef {
    pub id: DeviceId,
    pub epoch: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, weave::Frame)]
pub struct BindingId {
    pub machine: MachineId,
    pub serial: u64,
}

/// One full offered resource in one binding. Slots are never reassigned within
/// a binding; a new reservation always has a new BindingId.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, weave::Frame)]
pub struct AllocationId {
    pub binding: BindingId,
    pub slot: u8,
}
