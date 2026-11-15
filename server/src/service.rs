//! Native Twine source and reply-route authentication precede dispatch. This
//! adapter does not supply a boot policy, Authority implementation or event loop.

use core::{cell::Cell, convert::Infallible};
use hub_api::protocol::machine;
use kernel_api::model::identity::{PendId, TaskId};
use twine::view::{
    clone::{Receiver, ReplyError, Sender},
    memory::Memory,
};
use weave::{FrameReceiver, FrameSender, server::ServeResult};

use crate::{Machine, authority::Authority};

/// Business dispatch only; `source` must be the Kernel/Twine metadata identity,
/// never a field from the request. Mutable access serializes reservation commits.
pub fn dispatch<A: Authority>(
    machine: &mut Machine<A>,
    source: TaskId,
    request: machine::DecodedRequest,
) -> machine::Response {
    match request {
        machine::DecodedRequest::Offer(r) => {
            machine::Response::Offer(machine.offer(source, r.header.operation, r.offer))
        }
        machine::DecodedRequest::Withdraw(r) => {
            machine::Response::Withdraw(machine.withdraw(source, r.header.operation, r.withdraw))
        }
        machine::DecodedRequest::Reserve(r) => {
            machine::Response::Reserve(machine.reserve(source, r.header.operation, r.reserve))
        }
        machine::DecodedRequest::Release(r) => {
            machine::Response::Release(machine.release(source, r.header.operation, r.release))
        }
        machine::DecodedRequest::Inspect(r) => {
            machine::Response::Inspect(machine.inspect(source, r.inspect))
        }
    }
}

pub type NativeResult<M> = ServeResult<Receiver<M>, Sender, ReplyError, Infallible>;

/// Serve one request. A failed route does not mutate Machine. A failed send may
/// follow a committed operation; retry/query its operation with a new reply route.
/// The caller retains Receiver and must settle/discard unfinished deliveries.
pub fn serve_once<M: Memory, A: Authority>(
    receiver: &mut Receiver<M>,
    machine: &mut Machine<A>,
    request_buffer: &mut [u8],
    response_buffer: &mut [u8],
) -> NativeResult<M> {
    serve_routed(
        receiver,
        machine,
        request_buffer,
        response_buffer,
        |request, source| {
            let raw = request.reply();
            let pend = PendId::from_bytes(&raw.to_le_bytes()).ok_or(ReplyError::InvalidGrant)?;
            Sender::reply(pend, source)
        },
    )
}

/// Compose a source-authenticated complete-frame receiver with a per-request
/// reply-route validator. Integrators must use Kernel metadata (as serve_once
/// does) and validate the destination before dispatching any effects.
pub fn serve_routed<R, S, E, A, F>(
    receiver: &mut R,
    machine: &mut Machine<A>,
    request_buffer: &mut [u8],
    response_buffer: &mut [u8],
    route: F,
) -> ServeResult<R, S, E, Infallible>
where
    R: FrameReceiver<Metadata = TaskId>,
    S: FrameSender,
    A: Authority,
    F: FnOnce(&machine::DecodedRequest, TaskId) -> Result<S, E>,
{
    let caller = Cell::new(TaskId::new(0));
    weave::server::serve_once::<machine::Interface, _, _, _, _>(
        receiver,
        |request, source| {
            let sender = route(request, source)?;
            caller.set(source);
            Ok(sender)
        },
        |request| Ok(dispatch(machine, caller.get(), request)),
        request_buffer,
        response_buffer,
    )
}
