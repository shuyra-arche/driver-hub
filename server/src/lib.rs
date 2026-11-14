#![no_std]

//! Production coordination library. There is deliberately no executable Unit
//! until the System/Supply and trusted machine import contracts are available.

extern crate alloc;

pub mod authority;
pub mod machine;
pub mod pci;
pub mod service;

pub use machine::{Limits, Machine};
