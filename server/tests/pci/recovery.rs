use super::*;

#[test]
fn failed_restoration_blocks_further_probe_until_exact_recovery() {
    let (mut f, registers) = function(true);
    registers.lock().unwrap().fail_restore = true;
    // SAFETY: the retained simulated function remains exclusive across retries.
    assert_eq!(unsafe { f.probe_bar(0) }, Err(Fault::Access));
    assert!(f.restoration_pending());
    // SAFETY: no additional operation may change the retained device.
    assert_eq!(unsafe { f.probe_bar(2) }, Err(Fault::Pending));
    registers.lock().unwrap().fail_restore = false;
    assert_eq!(f.restore_probe(), Ok(()));
    assert!(!f.restoration_pending());
    assert_eq!(registers.lock().unwrap().words[4], 0xc);
    assert_eq!(registers.lock().unwrap().words[5], 0);
}

#[test]
fn restoration_readback_failure_retains_journal_until_verified() {
    let (mut f, registers) = function(false);
    registers.lock().unwrap().corrupt_restore_readback = true;
    // SAFETY: the simulated device remains quiescent and exclusively retained.
    assert_eq!(unsafe { f.probe_bar(0) }, Err(Fault::Readback));
    assert!(f.restoration_pending());
    registers.lock().unwrap().corrupt_restore_readback = false;
    assert_eq!(f.restore_probe(), Ok(()));
    assert!(!f.restoration_pending());
    assert_eq!(registers.lock().unwrap().words[4], 8);
}

#[test]
fn retains_programmed_aperture_after_uncertain_effect_and_retries_same_intent() {
    let (mut f, registers) = function(true);
    // SAFETY: the simulated device is exclusively quiescent throughout this test.
    let bar = unsafe { f.probe_bar(0) }.unwrap();
    let placement = bar.place(0x1_0000_0000, 0x4000).unwrap();
    registers.lock().unwrap().fail_program = true;
    // SAFETY: the test holds this exact interval; no other simulated device uses it.
    assert_eq!(unsafe { f.program(placement) }, Err(Fault::Access));
    assert_eq!(f.programmed(0), Some(placement));
    assert_eq!(registers.lock().unwrap().words[1] & 7, 0);
    let changed = bar.place(0x1_0000_4000, 0x4000).unwrap();
    // SAFETY: both candidate intervals are retained by the test owner.
    assert_eq!(unsafe { f.program(changed) }, Err(Fault::Pending));
    registers.lock().unwrap().fail_program = false;
    // SAFETY: retry retains the same exclusive device, geometry and reservation.
    assert_eq!(unsafe { f.program(placement) }, Ok(()));
    assert_eq!(registers.lock().unwrap().words[4], 0xc);
    assert_eq!(registers.lock().unwrap().words[5], 1);
    assert_eq!(registers.lock().unwrap().words[1] & 7, 0);
}
