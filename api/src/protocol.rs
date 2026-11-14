use crate::{BindingState, DeviceRef, Offer, Release, Reserve, Snapshot, Withdraw};

pub const REQUEST_CAPACITY: usize = 256;
pub const RESPONSE_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Code)]
#[repr(u8)]
pub enum Fail {
    None = 0,
    Invalid = 1,
    Denied = 2,
    Unknown = 3,
    Stale = 4,
    Conflict = 5,
    Busy = 6,
    Full = 7,
    Unsupported = 8,
}

/// Every mutation uses a nonzero, monotonically increasing operation in the
/// actual sender's namespace. A retry may use a new reply route but must retain
/// the same operation and complete business payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Header {
    pub operation: u64,
    pub reply: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Inspect {
    /// Nonzero queries this sender's retained operation outcome. Zero queries
    /// the current device observation, including any retained binding.
    pub operation: u64,
    pub device: DeviceRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Reply {
    #[frame(code)]
    pub failure: Fail,
    pub snapshot: Snapshot,
}

impl Reply {
    /// Failure has no observation. Zero IDs and Closed below are wire sentinels,
    /// never evidence that a real device or allocation has been retired.
    pub const fn failed(failure: Fail) -> Self {
        Self {
            failure,
            snapshot: Snapshot {
                device: DeviceRef {
                    id: crate::DeviceId {
                        machine: crate::MachineId { incarnation: 0 },
                        serial: 0,
                    },
                    epoch: 0,
                },
                binding: crate::BindingId {
                    machine: crate::MachineId { incarnation: 0 },
                    serial: 0,
                },
                state: BindingState::Closed,
                pending: 0,
                attempt: 0,
                fault: 0,
            },
        }
    }

    pub const fn observed(snapshot: Snapshot) -> Self {
        Self {
            failure: Fail::None,
            snapshot,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct OfferRequest {
    pub header: Header,
    pub offer: Offer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct WithdrawRequest {
    pub header: Header,
    pub withdraw: Withdraw,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct ReserveRequest {
    pub header: Header,
    pub reserve: Reserve,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct ReleaseRequest {
    pub header: Header,
    pub release: Release,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct InspectRequest {
    pub reply: u64,
    pub inspect: Inspect,
}

#[weave::interface(tag = u8, client)]
pub mod machine {
    use super::*;
    #[operation(code = 1, request = OfferRequest, response = Reply, call = offer)]
    pub struct Offer;
    #[operation(code = 2, request = WithdrawRequest, response = Reply, call = withdraw)]
    pub struct Withdraw;
    #[operation(code = 3, request = ReserveRequest, response = Reply, call = reserve)]
    pub struct Reserve;
    #[operation(code = 4, request = ReleaseRequest, response = Reply, call = release)]
    pub struct Release;
    #[operation(code = 5, request = InspectRequest, response = Reply, call = inspect)]
    pub struct Inspect;
}

impl machine::DecodedRequest {
    pub fn reply(&self) -> u64 {
        match self {
            Self::Offer(r) => r.header.reply,
            Self::Withdraw(r) => r.header.reply,
            Self::Reserve(r) => r.header.reply,
            Self::Release(r) => r.header.reply,
            Self::Inspect(r) => r.reply,
        }
    }
}
