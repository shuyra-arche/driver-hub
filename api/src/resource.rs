pub const MAX_RESOURCES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Code)]
#[repr(u8)]
pub enum ResourceKind {
    Mmio = 1,
    PciConfig = 2,
    Irq = 3,
    External = 4,
    Reset = 5,
    Power = 6,
    Signal = 7,
}

/// An owner-qualified range description. `pend` names an already Accorded
/// Hub-local Pend; it is untrusted input until the platform Authority checks it.
/// The trusted owner defines domain and coordinate units; hardware addresses
/// and logical Pend page coordinates must never be silently interchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, weave::Frame)]
pub struct ResourceSpec {
    #[frame(code)]
    pub kind: ResourceKind,
    pub domain: u64,
    pub start: u64,
    pub len: u64,
    pub pend: u64,
    pub rights: u32,
}

impl ResourceSpec {
    pub const EMPTY: Self = Self {
        kind: ResourceKind::Mmio,
        domain: 0,
        start: 0,
        len: 0,
        pend: 0,
        rights: 0,
    };

    pub fn valid(self) -> bool {
        self.domain != 0
            && self.pend != 0
            && self.rights != 0
            && self.len != 0
            && self.start.checked_add(self.len).is_some()
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.domain == other.domain
            && self.start < other.start.saturating_add(other.len)
            && other.start < self.start.saturating_add(self.len)
    }
}
