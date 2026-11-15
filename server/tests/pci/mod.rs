use std::sync::{Arc, Mutex};

use hub_server::pci::{Configuration, Ecam, Fault, Function};

mod recovery;

struct Registers {
    words: [u32; 1024],
    masks: [u32; 6],
    probing: [bool; 6],
    writes: usize,
    fail_restore: bool,
    corrupt_restore_readback: bool,
    fail_program: bool,
    sticky_master: bool,
}

impl Registers {
    fn memory(wide: bool) -> Self {
        let mut words = [0; 1024];
        words[0] = 0x1042_1af4;
        words[4] = if wide { 0xc } else { 0x8 };
        Self {
            words,
            masks: [
                if wide { 0xffff_c00c } else { 0xffff_c008 },
                u32::MAX,
                0,
                0,
                0,
                0,
            ],
            probing: [false; 6],
            writes: 0,
            fail_restore: false,
            corrupt_restore_readback: false,
            fail_program: false,
            sticky_master: false,
        }
    }
}

#[derive(Clone)]
struct Model(Arc<Mutex<Registers>>);

// SAFETY: this model exclusively serializes one simulated function, honors PCI
// register widths, and preserves scripted effects even when a write reports error.
unsafe impl Configuration for Model {
    fn read16(&mut self, register: u16) -> Result<u16, Fault> {
        let r = self.0.lock().unwrap();
        Ok((r.words[usize::from(register / 4)] >> ((register & 2) * 8)) as u16)
    }

    fn read32(&mut self, register: u16) -> Result<u32, Fault> {
        let r = self.0.lock().unwrap();
        if (0x10..0x28).contains(&register) {
            let index = usize::from((register - 0x10) / 4);
            if r.probing[index] {
                return Ok(r.masks[index]);
            }
            if r.corrupt_restore_readback && r.writes != 0 && index == 0 {
                return Ok(r.words[usize::from(register / 4)] ^ 0x10);
            }
        }
        Ok(r.words[usize::from(register / 4)])
    }

    fn write16(&mut self, register: u16, value: u16) -> Result<(), Fault> {
        let mut r = self.0.lock().unwrap();
        r.writes += 1;
        let shift = (register & 2) * 8;
        let word = &mut r.words[usize::from(register / 4)];
        *word = (*word & !(0xffff << shift)) | (u32::from(value) << shift);
        if register == 4 && r.sticky_master {
            r.words[1] |= 4;
        }
        Ok(())
    }

    fn write32(&mut self, register: u16, value: u32) -> Result<(), Fault> {
        let mut r = self.0.lock().unwrap();
        r.writes += 1;
        let index = usize::from((register - 0x10) / 4);
        if value == u32::MAX {
            r.probing[index] = true;
            return Ok(());
        }
        if r.probing[index] && r.fail_restore {
            return Err(Fault::Access);
        }
        r.probing[index] = false;
        r.words[usize::from(register / 4)] = value;
        if r.fail_program && register == 0x10 {
            return Err(Fault::Access); // intentionally after the real write
        }
        Ok(())
    }
}

fn function(wide: bool) -> (Function<Model>, Arc<Mutex<Registers>>) {
    let registers = Arc::new(Mutex::new(Registers::memory(wide)));
    (Function::new(Model(registers.clone())), registers)
}

#[test]
fn discovers_identity_without_writes_and_rejects_absent_function() {
    let (mut f, registers) = function(false);
    assert_eq!(f.identity(), Ok(Some((0x1af4, 0x1042))));
    registers.lock().unwrap().words[0] = u32::MAX;
    assert_eq!(f.identity(), Ok(None));
    assert_eq!(registers.lock().unwrap().writes, 0);
}

#[test]
fn probes_both_widths_and_restores_prefetch_flags() {
    for wide in [false, true] {
        let (mut f, registers) = function(wide);
        let original = registers.lock().unwrap().words;
        // SAFETY: the simulated device has no consumers or in-flight accesses.
        let bar = unsafe { f.probe_bar(0) }.unwrap();
        assert_eq!(bar.size(), 16384);
        assert_eq!(bar.wide(), wide);
        assert!(bar.prefetchable());
        assert!(!f.restoration_pending());
        assert_eq!(registers.lock().unwrap().words, original);
        assert_eq!(registers.lock().unwrap().probing, [false; 6]);
    }
}

#[test]
fn probing_rejects_active_decode_bridge_io_and_upper_half() {
    let (mut f, registers) = function(true);
    registers.lock().unwrap().words[1] = 2;
    // SAFETY: this rejection test never performs a probing write.
    assert_eq!(unsafe { f.probe_bar(0) }, Err(Fault::Active));
    registers.lock().unwrap().words[1] = 0;
    // SAFETY: the simulated function has no external accesses.
    assert_eq!(unsafe { f.probe_bar(1) }, Err(Fault::Unsupported));
    registers.lock().unwrap().words[3] = 1 << 16;
    // SAFETY: rejected bridge geometry is never written.
    assert_eq!(unsafe { f.probe_bar(0) }, Err(Fault::Unsupported));
    registers.lock().unwrap().words[3] = 0;
    registers.lock().unwrap().words[4] = 1;
    // SAFETY: rejected I/O geometry is never written.
    assert_eq!(unsafe { f.probe_bar(0) }, Err(Fault::Unsupported));
    assert_eq!(registers.lock().unwrap().writes, 0);
}

#[test]
fn rejects_malformed_mask_and_restores_original_configuration() {
    let (mut f, registers) = function(false);
    registers.lock().unwrap().masks[0] = 0xffff_a008;
    // SAFETY: the simulated device has no consumers or in-flight accesses.
    assert_eq!(unsafe { f.probe_bar(0) }, Err(Fault::Malformed));
    assert!(!f.restoration_pending());
    assert_eq!(registers.lock().unwrap().words[4], 8);
}

#[test]
fn plans_exact_aligned_bar_and_rejects_overflow_and_32bit_truncation() {
    let (mut f, _) = function(false);
    // SAFETY: there are no consumers in this simulated function.
    let bar = unsafe { f.probe_bar(0) }.unwrap();
    let placement = bar.place(0x8000_1000, 0x10000).unwrap();
    assert_eq!(placement.address(), 0x8000_4000);
    assert_eq!(placement.reserved_bytes(), 0x4000);
    assert_eq!(bar.place(0x8000_1000, 0x4000), Err(Fault::Bounds));
    assert_eq!(bar.place(1u64 << 32, 0x10000), Err(Fault::Bounds));
    assert_eq!(bar.place(u64::MAX - 0x1000, 0x10000), Err(Fault::Bounds));
}

#[test]
fn disabling_master_preserves_decode_and_status_and_checks_readback() {
    let (mut f, registers) = function(false);
    registers.lock().unwrap().words[1] = 0xffff_0007;
    assert_eq!(f.disable_master(), Ok(()));
    assert_eq!(registers.lock().unwrap().words[1], 0xffff_0003);
    registers.lock().unwrap().sticky_master = true;
    assert_eq!(f.disable_master(), Err(Fault::Readback));
}

#[test]
fn mapped_ecam_checks_function_window_and_register_width_before_access() {
    #[repr(C, align(4096))]
    struct Page([u8; 4096]);
    let mut page = Page([0; 4096]);
    let base = core::ptr::NonNull::new(page.0.as_mut_ptr()).unwrap();
    // SAFETY: this is exclusive, aligned simulated MMIO retained for the test.
    assert!(unsafe { Ecam::new(base, 4095, 0, 0) }.is_err());
    // SAFETY: construction rejects the out-of-mapping function without access.
    assert!(unsafe { Ecam::new(base, 4096, 0, 1) }.is_err());
    // SAFETY: the complete simulated page is exclusively held for this lifetime.
    let mut config = unsafe { Ecam::new(base, 4096, 7, 7) }.unwrap();
    assert_eq!(config.write32(0, 0x1042_1af4), Ok(()));
    assert_eq!(config.read16(2), Ok(0x1042));
    assert_eq!(config.read32(2), Err(Fault::Bounds));
    assert_eq!(config.write32(4096, 0), Err(Fault::Bounds));
    assert_eq!(config.read16(4095), Err(Fault::Bounds));
}
