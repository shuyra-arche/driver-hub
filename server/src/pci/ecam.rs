use core::{marker::PhantomData, ptr::NonNull};

use super::{Configuration, Fault};

/// An exclusive, already mapped configuration page. It never maps physical RAM
/// or obtains authority from a requester number.
pub struct Ecam<'a> {
    page: NonNull<u8>,
    lifetime: PhantomData<&'a mut [u8]>,
}

impl<'a> Ecam<'a> {
    /// Select one complete function in a contiguous, authorized ECAM mapping.
    /// `first` and `requester` are BDF coordinates, not IOMMU admission.
    ///
    /// # Safety
    /// The caller exclusively owns the actual configuration authority and the
    /// mapping for `'a`, including every outstanding effect. The virtual region
    /// has device memory attributes and platform ordering appropriate for PCI
    /// configuration accesses; all accesses are naturally aligned little-endian
    /// MMIO. It cannot be unmapped/revoked or accessed by another agent while this
    /// object exists. The platform must preserve the configuration/effect journal
    /// and its real resource reservations until genuine retirement, even if Hub
    /// dies. This is a trusted platform boundary, not a runtime Supply adapter.
    pub unsafe fn new(
        base: NonNull<u8>,
        bytes: usize,
        first: u16,
        requester: u16,
    ) -> Result<Self, Fault> {
        let functions = bytes / 4096;
        if !base.as_ptr().addr().is_multiple_of(4096)
            || bytes == 0
            || !bytes.is_multiple_of(4096)
            || functions > 65536 - usize::from(first)
            || base.as_ptr().addr().checked_add(bytes).is_none()
        {
            return Err(Fault::Bounds);
        }
        let index = requester.checked_sub(first).ok_or(Fault::Bounds)?;
        if usize::from(index) >= functions {
            return Err(Fault::Bounds);
        }
        // SAFETY: the caller owns the checked complete mapping for this lifetime.
        let page = unsafe { base.add(usize::from(index) * 4096) };
        Ok(Self {
            page,
            lifetime: PhantomData,
        })
    }

    fn at<T>(&self, register: u16) -> Result<*mut T, Fault> {
        let at = usize::from(register);
        let width = core::mem::size_of::<T>();
        if !at.is_multiple_of(width) || at + width > 4096 {
            return Err(Fault::Bounds);
        }
        // SAFETY: the checked aligned access lies within the retained page.
        Ok(unsafe { self.page.as_ptr().add(at).cast() })
    }
}

// SAFETY: construction requires exclusive live configuration ownership, device
// attributes and platform ordering. Accessors validate widths before any MMIO.
unsafe impl Configuration for Ecam<'_> {
    fn read16(&mut self, register: u16) -> Result<u16, Fault> {
        // SAFETY: at validates this access within the exclusively retained page.
        Ok(u16::from_le(unsafe {
            self.at::<u16>(register)?.read_volatile()
        }))
    }

    fn read32(&mut self, register: u16) -> Result<u32, Fault> {
        // SAFETY: at validates this access within the exclusively retained page.
        Ok(u32::from_le(unsafe {
            self.at::<u32>(register)?.read_volatile()
        }))
    }

    fn write16(&mut self, register: u16, value: u16) -> Result<(), Fault> {
        // SAFETY: at validates this access within the exclusively retained page.
        unsafe { self.at::<u16>(register)?.write_volatile(value.to_le()) };
        Ok(())
    }

    fn write32(&mut self, register: u16, value: u32) -> Result<(), Fault> {
        // SAFETY: at validates this access within the exclusively retained page.
        unsafe { self.at::<u32>(register)?.write_volatile(value.to_le()) };
        Ok(())
    }
}
