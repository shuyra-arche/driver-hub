#![no_std]

//! Production coordination library. It includes a Task-local native Pend
//! lease primitive, but no trusted machine import, native Authority, complete
//! resource Supply wiring or executable Unit. Driver consumes the pinned
//! System Instance SDK.

extern crate alloc;

pub mod authority;
pub mod driver;
pub mod lease;
pub mod machine;
pub mod pci;
pub mod service;

pub use machine::{Limits, Machine};
