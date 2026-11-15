use hub_api::{protocol::*, *};
use weave::Codec;

fn offer() -> Offer {
    Offer {
        key: 1,
        epoch: 1,
        count: MAX_RESOURCES as u8,
        resources: [ResourceSpec {
            kind: ResourceKind::Mmio,
            domain: 1,
            start: 4096,
            len: 4096,
            pend: 17,
            rights: 4,
        }; MAX_RESOURCES],
    }
}

#[test]
fn public_wire_rejects_unknown_tags_truncation_and_trailing_bytes() {
    let request = machine::Request::Offer(OfferRequest {
        header: Header {
            operation: 9,
            reply: 17,
        },
        offer: offer(),
    });
    let mut bytes = [0; REQUEST_CAPACITY];
    let used = request.encode(&mut bytes).unwrap();
    assert!(used < REQUEST_CAPACITY);
    assert!(
        matches!(machine::Request::decode_exact(&bytes[..used]), Some(machine::DecodedRequest::Offer(r)) if r.header.operation == 9 && r.offer == offer())
    );
    assert!(machine::Request::decode_exact(&bytes[..used - 1]).is_none());
    assert!(machine::Request::decode_exact(&bytes[..used + 1]).is_none());
    bytes[0] = 255;
    assert!(machine::Request::decode_exact(&bytes[..used]).is_none());
}

#[test]
fn count_bounds_and_unknown_resource_kind_fail_at_decode() {
    let request = machine::Request::Offer(OfferRequest {
        header: Header {
            operation: 9,
            reply: 17,
        },
        offer: offer(),
    });
    let mut bytes = [0; REQUEST_CAPACITY];
    let used = request.encode(&mut bytes).unwrap();
    // [tag][Header:16][key:8][epoch:8][count:1][ResourceSpec...]
    bytes[33] = 5;
    assert!(machine::Request::decode_exact(&bytes[..used]).is_none());
    bytes[33] = MAX_RESOURCES as u8;
    bytes[34] = 0;
    assert!(machine::Request::decode_exact(&bytes[..used]).is_none());
}

#[test]
fn tagged_error_response_has_no_authorizing_handle() {
    let reply = Reply::failed(Fail::Unsupported);
    let mut bytes = [0; RESPONSE_CAPACITY];
    let used = machine::Response::Reserve(reply)
        .encode(&mut bytes)
        .unwrap();
    assert!(
        matches!(machine::Response::decode_exact(&bytes[..used]), Some(machine::DecodedResponse::Reserve(r)) if r == reply)
    );
    assert_eq!(reply.snapshot.binding.serial, 0);
    assert_eq!(reply.snapshot.device.id.serial, 0);
}

#[test]
fn signal_resource_kind_roundtrips_as_its_own_tag() {
    let mut value = offer();
    value.count = 1;
    value.resources[0] = ResourceSpec {
        kind: ResourceKind::Signal,
        domain: 9,
        start: 0,
        len: 1,
        pend: 31,
        rights: 2,
    };
    let request = machine::Request::Offer(OfferRequest {
        header: Header {
            operation: 12,
            reply: 13,
        },
        offer: value,
    });
    let mut bytes = [0; REQUEST_CAPACITY];
    let used = request.encode(&mut bytes).unwrap();
    assert!(matches!(
        machine::Request::decode_exact(&bytes[..used]),
        Some(machine::DecodedRequest::Offer(decoded))
            if decoded.offer.resources[0].kind == ResourceKind::Signal
    ));
}
