#![no_std]

//! Production coordination library. There is deliberately no executable Unit
//! until trusted machine import, owned native resource delivery and product
//! integration are available. Driver consumes the pinned System Instance SDK.

extern crate alloc;

pub mod authority;
pub mod driver;
pub mod machine;
pub mod pci;
pub mod service;

pub use machine::{Limits, Machine};
