#![no_std]

//! Hub v2: descriptions, exclusive reservations and retained retirement duties.
//! No response carries device access or System Instance authority.

pub mod binding;
pub mod device;
pub mod identity;
pub mod protocol;
pub mod resource;

pub use binding::{BindingState, Duty, Retirement, Snapshot};
pub use device::{Offer, Release, Reserve, Withdraw};
pub use identity::{AllocationId, BindingId, DeviceId, DeviceRef, MachineId};
pub use protocol::{Fail, Header, Inspect, Reply};
pub use resource::{MAX_RESOURCES, ResourceKind, ResourceSpec};
