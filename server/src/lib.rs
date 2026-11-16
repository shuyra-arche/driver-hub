#![no_std]

//! Production coordination library with Task-local CPU Signal import/Authority,
//! allocation-owned native leases and the pinned System Instance consumer.
//! Hardware import/retirement, full System Supply integration and an executable
//! Hub Unit still require their own validated contracts.

extern crate alloc;

pub mod authority;
pub mod driver;
pub mod lease;
pub mod machine;
pub mod pci;
pub mod service;
pub mod supply;

pub use machine::{Limits, Machine};
