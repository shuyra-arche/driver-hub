#![no_std]

//! Typed Weave calls with native Twine endpoints. Prepare a fresh single-use
//! Reply with `grant(service)` and put that returned Hub-local Pend in Header.
//! All failed/timeout calls still require Reply::finish; uncertain completion is
//! recovered through Inspect or a replay with the same business operation.

pub use hub_api;
pub use hub_api::protocol::machine::Client;
pub use twine::view::clone::{Reply, ReplyError, Sender};

/// Construct the typed client over a real Kernel-authorized Clone sender.
/// No destination, lifetime or access rights are inferred from this number;
/// Kernel rechecks the Pend at each send.
pub fn native<'a>(
    endpoint: kernel_api::model::identity::PendId,
    request_buffer: &'a mut [u8],
    response_buffer: &'a mut [u8],
) -> Client<'a, Sender> {
    Client::new(Sender::new(endpoint), request_buffer, response_buffer)
}
