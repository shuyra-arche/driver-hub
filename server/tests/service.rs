mod support;

use hub_api::{
    protocol::{OfferRequest, ReserveRequest, machine},
    *,
};
use hub_server::{Machine, service};
use kernel_api::model::identity::TaskId;
use std::{cell::RefCell, rc::Rc};
use support::*;
use weave::{
    Codec, FrameReceiver, FrameSender, ReceivedFrame, correlation::ReplyReceiver,
    server::ServeError,
};

struct Incoming {
    bytes: Vec<u8>,
    source: TaskId,
}
impl FrameReceiver for Incoming {
    type Error = ();
    type Metadata = TaskId;
    fn receive(&mut self, out: &mut [u8]) -> Result<ReceivedFrame<TaskId>, ()> {
        if self.bytes.len() > out.len() {
            return Err(());
        }
        out[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(ReceivedFrame {
            len: self.bytes.len(),
            metadata: self.source,
        })
    }
}

#[derive(Clone)]
struct ResponseQueue(Rc<RefCell<Option<Vec<u8>>>>);
impl FrameSender for ResponseQueue {
    type Error = ();
    fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        *self.0.borrow_mut() = Some(bytes.to_vec());
        Ok(())
    }
}
impl FrameReceiver for ResponseQueue {
    type Error = ();
    type Metadata = TaskId;
    fn receive(&mut self, out: &mut [u8]) -> Result<ReceivedFrame<TaskId>, ()> {
        let bytes = self.0.borrow_mut().take().ok_or(())?;
        if bytes.len() > out.len() {
            return Err(());
        }
        out[..bytes.len()].copy_from_slice(&bytes);
        Ok(ReceivedFrame {
            len: bytes.len(),
            metadata: TaskId::new(99),
        })
    }
}
impl ReplyReceiver for ResponseQueue {}

struct Sender {
    machine: Rc<RefCell<Machine<Model>>>,
    reply: ResponseQueue,
    source: TaskId,
    discard: bool,
}
impl FrameSender for Sender {
    type Error = ();
    fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let mut receiver = Incoming {
            bytes: bytes.to_vec(),
            source: self.source,
        };
        let mut reply = self.reply.clone();
        service::serve_routed(
            &mut receiver,
            &mut *self.machine.borrow_mut(),
            &mut [0; 256],
            &mut [0; 128],
            |request, source| {
                if source != self.source || request.reply() != 42 {
                    return Err(());
                }
                Ok(&mut reply)
            },
        )
        .unwrap();
        if self.discard {
            *self.reply.0.borrow_mut() = None;
        }
        Ok(())
    }
}

#[test]
fn typed_client_and_production_dispatch_recover_lost_reply_without_new_reservation() {
    let mut core = machine();
    let device = enroll(&mut core);
    let machine = Rc::new(RefCell::new(core));
    let mut reply = ResponseQueue(Rc::new(RefCell::new(None)));
    let sender = Sender {
        machine: machine.clone(),
        reply: reply.clone(),
        source: DRIVER,
        discard: true,
    };
    let mut request_buffer = [0; 256];
    let mut response_buffer = [0; 128];
    let mut client = hub_client::Client::new(sender, &mut request_buffer, &mut response_buffer);
    let request = ReserveRequest {
        header: Header {
            operation: 1,
            reply: 42,
        },
        reserve: Reserve { device },
    };
    assert!(client.reserve(&mut reply, request).is_err());
    let accepted = machine.borrow().inspect(
        DRIVER,
        Inspect {
            operation: 1,
            device,
        },
    );
    assert_eq!(accepted.failure, Fail::None);
    client.inner_mut().sender_mut().discard = false;
    let repeated = client.reserve(&mut reply, request).unwrap().value;
    assert_eq!(repeated, accepted);
    let competitor = machine
        .borrow_mut()
        .reserve(TaskId::new(10), 1, Reserve { device });
    assert_eq!(competitor.failure, Fail::Busy);
}

#[test]
fn invalid_route_and_malformed_frame_never_dispatch_business_state() {
    let mut machine = machine();
    let request = machine::Request::Offer(OfferRequest {
        header: Header {
            operation: 1,
            reply: 999,
        },
        offer: offer(1, 0),
    });
    let mut bytes = [0; 256];
    let used = request.encode(&mut bytes).unwrap();
    let mut incoming = Incoming {
        bytes: bytes[..used].to_vec(),
        source: PROVIDER,
    };
    let result = service::serve_routed::<_, ResponseQueue, _, _, _>(
        &mut incoming,
        &mut machine,
        &mut [0; 256],
        &mut [0; 128],
        |_, _| Err("route"),
    );
    assert!(matches!(result, Err(ServeError::Route("route"))));
    assert_eq!(
        machine
            .inspect(
                PROVIDER,
                Inspect {
                    operation: 1,
                    device: DeviceRef::default()
                }
            )
            .failure,
        Fail::Unknown
    );
    incoming.bytes.push(0);
    let result = service::serve_routed::<_, ResponseQueue, _, _, _>(
        &mut incoming,
        &mut machine,
        &mut [0; 256],
        &mut [0; 128],
        |_, _| Err("must not route"),
    );
    assert!(matches!(result, Err(ServeError::Receive(_))));
    assert_eq!(machine.offer(PROVIDER, 1, offer(1, 0)).failure, Fail::None);
}

#[test]
fn authority_uses_authenticated_transport_source_and_not_reply_route_identity() {
    let mut machine = machine();
    let request = machine::Request::Offer(OfferRequest {
        header: Header {
            operation: 1,
            reply: 42,
        },
        offer: offer(1, 0),
    });
    let mut bytes = [0; 256];
    let used = request.encode(&mut bytes).unwrap();
    let mut incoming = Incoming {
        bytes: bytes[..used].to_vec(),
        source: DRIVER,
    };
    let mut output = ResponseQueue(Rc::new(RefCell::new(None)));
    service::serve_routed(
        &mut incoming,
        &mut machine,
        &mut [0; 256],
        &mut [0; 128],
        |_, _| Ok::<_, ()>(&mut output),
    )
    .unwrap();
    let wire = output.0.borrow_mut().take().unwrap();
    assert!(
        matches!(machine::Response::decode_exact(&wire), Some(machine::DecodedResponse::Offer(reply)) if reply.failure == Fail::Denied)
    );
    assert_eq!(machine.offer(PROVIDER, 1, offer(1, 0)).failure, Fail::None);
}
