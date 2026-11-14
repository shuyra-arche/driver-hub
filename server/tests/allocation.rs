mod support;

use hub_api::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use support::*;

struct Failing;
thread_local! { static FAIL_AT: Cell<usize> = const { Cell::new(0) }; }

// The failure injector affects only allocations explicitly armed on this test
// thread; the host test harness and other threads use the system allocator.
unsafe impl GlobalAlloc for Failing {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let fail = FAIL_AT
            .try_with(|count| {
                let value = count.get();
                count.set(value.saturating_sub(1));
                value == 1
            })
            .unwrap_or(false);
        if fail {
            core::ptr::null_mut()
        } else {
            unsafe { System.alloc(layout) }
        }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Failing = Failing;

#[test]
fn allocation_failure_before_journal_or_device_commit_leaves_no_partial_offer() {
    for allocation in [1, 2] {
        let mut machine = machine();
        FAIL_AT.with(|count| count.set(allocation));
        let failed = machine.offer(PROVIDER, 1, offer(1, 0));
        FAIL_AT.with(|count| count.set(0));
        assert_eq!(failed.failure, Fail::Full);
        // If the journal was reserved but the Device allocation failed, this is
        // a retained rejection; it cannot later turn into an accepted operation.
        let retried = machine.offer(PROVIDER, 1, offer(1, 0));
        if allocation == 1 {
            assert_eq!(retried.failure, Fail::None);
        } else {
            assert_eq!(retried.failure, Fail::Full);
        }
        let accepted = if allocation == 1 {
            retried
        } else {
            machine.offer(PROVIDER, 2, offer(1, 0))
        };
        assert_eq!(accepted.failure, Fail::None);
        assert_eq!(accepted.snapshot.device.id.serial, 1);
    }
}

#[test]
fn binding_storage_failure_does_not_consume_exclusive_reservation_or_id() {
    let mut machine = machine();
    let device = enroll(&mut machine);
    FAIL_AT.with(|count| count.set(1));
    let failed = machine.reserve(DRIVER, 1, Reserve { device });
    FAIL_AT.with(|count| count.set(0));
    assert_eq!(failed.failure, Fail::Full);
    let accepted = machine.reserve(DRIVER, 2, Reserve { device });
    assert_eq!(accepted.failure, Fail::None);
    assert_eq!(accepted.snapshot.binding.serial, 1);
}
