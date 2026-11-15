//! Internal PCI configuration effects. Allocation stays with Machine; Kernel
//! retains IRQ/External authority. No PCI service or runtime ECAM grant is added.
mod bar;
mod ecam;

pub use bar::{Bar, Placement};
pub use ecam::Ecam;

const COMMAND: u16 = 4;
const BAR: u16 = 0x10;
const MASTER: u16 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Bounds,
    Access,
    Unsupported,
    Active,
    Pending,
    Malformed,
    Readback,
}

/// Configuration access to one exclusive physical function.
///
/// # Safety
/// Implementations must retain real configuration authority and the mapping,
/// enforce register bounds and platform ordering, and never redirect accesses to
/// another function. A failed write may have taken effect. Device reservations
/// and the Function journal must outlive all effects; dropping either cannot
/// authorize reassignment. MMIO snapshots and BDF numbers do not prove authority.
pub unsafe trait Configuration {
    fn read16(&mut self, register: u16) -> Result<u16, Fault>;
    fn read32(&mut self, register: u16) -> Result<u32, Fault>;
    fn write16(&mut self, register: u16, value: u16) -> Result<(), Fault>;
    fn write32(&mut self, register: u16, value: u32) -> Result<(), Fault>;
}

struct Probe {
    register: u16,
    low: u32,
    high: Option<u32>,
}

/// Retained device configuration effects, not a second resource allocator.
/// Programmed BARs remain recorded even when a later write/readback fails.
#[must_use = "configuration effects and reservations must survive until retirement"]
pub struct Function<C: Configuration> {
    config: C,
    probe: Option<Probe>,
    programmed: [Option<Placement>; 6],
}

impl<C: Configuration> Function<C> {
    pub fn new(config: C) -> Self {
        Self {
            config,
            probe: None,
            programmed: [None; 6],
        }
    }

    pub fn identity(&mut self) -> Result<Option<(u16, u16)>, Fault> {
        let id = self.config.read32(0)?;
        let vendor = id as u16;
        Ok((vendor != u16::MAX && vendor != 0).then_some((vendor, (id >> 16) as u16)))
    }

    /// Clear only Bus Master and verify its readback. This is not device drain
    /// evidence; memory decode, status bits and existing reservations stay intact.
    pub fn disable_master(&mut self) -> Result<(), Fault> {
        let command = self.config.read16(COMMAND)?;
        self.config.write16(COMMAND, command & !MASTER)?;
        if self.config.read16(COMMAND)? & MASTER != 0 {
            return Err(Fault::Readback);
        }
        Ok(())
    }

    pub fn restoration_pending(&self) -> bool {
        self.probe.is_some()
    }

    pub fn programmed(&self, index: u8) -> Option<Placement> {
        self.programmed.get(usize::from(index)).copied().flatten()
    }

    fn disabled(&mut self) -> Result<(), Fault> {
        if self.config.read16(COMMAND)? & 7 != 0 {
            return Err(Fault::Active);
        }
        Ok(())
    }

    fn bar_register(&mut self, index: u8) -> Result<(u16, u32), Fault> {
        // Endpoint headers only. Bridges need an explicit topology/bridge owner.
        if self.config.read16(0x0e)? & 0x7f != 0 || index >= 6 {
            return Err(Fault::Unsupported);
        }
        let mut at = 0;
        while at < index {
            let low = self.config.read32(BAR + u16::from(at) * 4)?;
            at += if low & 7 == 4 { 2 } else { 1 };
        }
        if at != index {
            return Err(Fault::Unsupported); // upper half of a 64-bit BAR
        }
        let register = BAR + u16::from(index) * 4;
        let low = self.config.read32(register)?;
        if low & 1 != 0 || matches!(low & 6, 2 | 6) || (low & 6 == 4 && index == 5) {
            return Err(Fault::Unsupported);
        }
        Ok((register, low))
    }

    /// Probe and restore a memory BAR, retaining its original values before any
    /// write. On error, inspect restoration_pending and retry restore_probe.
    ///
    /// # Safety
    /// The actual device is quiescent, all old consumer/config authority has
    /// been withdrawn, and exclusive ownership lasts through restoration. A
    /// cleared MASTER bit alone is insufficient. Keep this Function and its
    /// reservations on failure; no new binding may use the device meanwhile.
    pub unsafe fn probe_bar(&mut self, index: u8) -> Result<Bar, Fault> {
        if self.probe.is_some() {
            return Err(Fault::Pending);
        }
        self.disabled()?;
        let (register, low) = self.bar_register(index)?;
        let high = if low & 6 == 4 {
            Some(self.config.read32(register + 4)?)
        } else {
            None
        };
        self.probe = Some(Probe {
            register,
            low,
            high,
        });
        let result = self.measure(index, register, low, high.is_some());
        let restored = self.restore_probe();
        match (result, restored) {
            (Ok(bar), Ok(())) => Ok(bar),
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }

    fn measure(&mut self, index: u8, register: u16, low: u32, wide: bool) -> Result<Bar, Fault> {
        if wide {
            self.config.write32(register + 4, u32::MAX)?;
        }
        self.config.write32(register, u32::MAX)?;
        let mask_low = self.config.read32(register)?;
        let mask_high = if wide {
            self.config.read32(register + 4)?
        } else {
            u32::MAX
        };
        if mask_low & 0xf != low & 0xf {
            return Err(Fault::Malformed);
        }
        let mask = (u64::from(mask_high) << 32) | u64::from(mask_low & !0xf);
        let size = (!mask).checked_add(1).ok_or(Fault::Unsupported)?;
        if mask_low & !0xf == 0 && (!wide || mask_high == 0) {
            return Err(Fault::Unsupported);
        }
        if size < 16 || !size.is_power_of_two() || (!wide && size > 1u64 << 32) {
            return Err(Fault::Malformed);
        }
        Ok(Bar {
            index,
            flags: low & 0xf,
            size,
        })
    }

    /// Retry exact original BAR restoration. A failed write/readback retains the
    /// journal, including when the write may already have happened.
    pub fn restore_probe(&mut self) -> Result<(), Fault> {
        if self.probe.is_none() {
            return Ok(());
        }
        self.disabled()?;
        let probe = self.probe.as_ref().expect("retained probe");
        if let Some(high) = probe.high {
            self.config.write32(probe.register + 4, high)?;
        }
        self.config.write32(probe.register, probe.low)?;
        if self.config.read32(probe.register)? != probe.low
            || match probe.high {
                Some(high) => self.config.read32(probe.register + 4)? != high,
                None => false,
            }
        {
            return Err(Fault::Readback);
        }
        self.probe = None;
        Ok(())
    }

    /// Program an already reserved exact BAR window, leaving all command enables
    /// off. The effect journal commits before the first potentially partial write.
    ///
    /// # Safety
    /// The caller holds the real Machine reservation and restricted root over
    /// this complete interval for the current device/binding epoch. Geometry was
    /// probed from this function during that same exclusive quiescent ownership.
    /// Keep the journal and reservation on every result. No raw config/old driver
    /// authority may race, and only real retirement can permit interval reuse.
    pub unsafe fn program(&mut self, placement: Placement) -> Result<(), Fault> {
        if self.probe.is_some() {
            return Err(Fault::Pending);
        }
        self.disabled()?;
        let bar = placement.bar;
        let (register, low) = self.bar_register(bar.index)?;
        if low & 0xf != bar.flags {
            return Err(Fault::Malformed);
        }
        for old in self.programmed.iter().flatten() {
            if old.bar.index == bar.index {
                if *old != placement {
                    return Err(Fault::Pending);
                }
            } else if placement.address < old.address + old.reserved_bytes()
                && old.address < placement.address + placement.reserved_bytes()
            {
                return Err(Fault::Bounds);
            }
        }
        self.programmed[usize::from(bar.index)] = Some(placement);
        if bar.wide() {
            self.config
                .write32(register + 4, (placement.address >> 32) as u32)?;
        }
        self.config
            .write32(register, placement.address as u32 | bar.flags)?;
        if self.config.read32(register)? != placement.address as u32 | bar.flags
            || (bar.wide() && self.config.read32(register + 4)? != (placement.address >> 32) as u32)
        {
            return Err(Fault::Readback);
        }
        Ok(())
    }
}
