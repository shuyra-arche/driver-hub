use super::Fault;

/// Geometry observed while an exclusively controlled function was quiescent.
/// This is not a resource reservation or a Kernel authorization handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    pub(super) index: u8,
    pub(super) flags: u32,
    pub(super) size: u64,
}

impl Bar {
    pub fn index(self) -> u8 {
        self.index
    }

    pub fn size(self) -> u64 {
        self.size
    }

    pub fn wide(self) -> bool {
        self.flags & 6 == 4
    }

    pub fn prefetchable(self) -> bool {
        self.flags & 8 != 0
    }

    /// Plan within an already reserved aperture. Planning does not reserve it.
    pub fn place(self, base: u64, bytes: u64) -> Result<Placement, Fault> {
        let alignment = self.size.max(4096);
        let address = base
            .checked_add(alignment - 1)
            .map(|rounded| rounded & !(alignment - 1))
            .ok_or(Fault::Bounds)?;
        let reserved_bytes = self.size.max(4096);
        let end = address.checked_add(reserved_bytes).ok_or(Fault::Bounds)?;
        if bytes == 0
            || end > base.checked_add(bytes).ok_or(Fault::Bounds)?
            || (!self.wide() && end > 1u64 << 32)
        {
            return Err(Fault::Bounds);
        }
        Ok(Placement { bar: self, address })
    }
}

/// A planned or programmed address; never proof of an Allocation or Pend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub(super) bar: Bar,
    pub(super) address: u64,
}

impl Placement {
    pub fn bar(self) -> Bar {
        self.bar
    }

    pub fn address(self) -> u64 {
        self.address
    }

    pub fn reserved_bytes(self) -> u64 {
        self.bar.size.max(4096)
    }
}
