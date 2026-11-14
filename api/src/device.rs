use crate::{BindingId, DeviceRef, MAX_RESOURCES, ResourceSpec};

/// Source identity comes from transport metadata. `(source, key)` identifies
/// one device across increasing offer epochs. Parent/subprovider import awaits
/// a validated derivation contract and is deliberately absent from v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Offer {
    pub key: u64,
    pub epoch: u64,
    pub count: u8,
    #[frame(sequence(count = count, fill = ResourceSpec::EMPTY))]
    pub resources: [ResourceSpec; MAX_RESOURCES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Withdraw {
    pub device: DeviceRef,
}

/// Reserves all resources of exactly this offer epoch. This neither creates a
/// Driver Instance nor delivers a Pend; no partial-resource grant is implied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Reserve {
    pub device: DeviceRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct Release {
    pub binding: BindingId,
}
