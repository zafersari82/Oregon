use oregon_primitives::{
    Block, BlockHeader, DecodeLimits, Hash256, PrimitiveError, Transaction, write_varint,
};

use crate::{
    FRAME_HEADER_BYTES, FRAME_VERSION, FeatureSet, FrameHeader, GetHeaders, Hello, HelloAck,
    InventoryItem, InventoryKind, MAX_FRAME_PAYLOAD, MAX_GETDATA_ITEMS, MAX_HANDSHAKE_PAYLOAD,
    MAX_HEADERS_PER_MESSAGE, MAX_INV_ITEMS, MAX_LOCATOR_HASHES, Message, Negotiated, ProtocolError,
    TAG_BLOCK, TAG_GET_DATA, TAG_GET_HEADERS, TAG_HEADERS, TAG_HELLO, TAG_HELLO_ACK, TAG_INV,
    TAG_PING, TAG_PONG, TAG_TRANSACTION, build_frame_header, decode_message, encode_message,
    negotiate, network_magic, verify_frame_payload,
};

fn hash(byte: u8) -> Hash256 {
    Hash256::from_bytes([byte; 32])
}

fn hello() -> Hello {
    Hello {
        min_protocol_version: 1,
        max_protocol_version: 1,
        chain_id: hash(0x11),
        instance_nonce: [0x22; 16],
        offered_features: FeatureSet::HEADERS_SYNC | FeatureSet::BLOCK_RELAY | FeatureSet::TX_RELAY,
        required_features: FeatureSet::HEADERS_SYNC,
        best_height: 7,
        best_block_id: hash(0x33),
    }
}

fn header(nonce: u64) -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: hash(0x44),
        transaction_root: hash(0x55),
        timestamp: 1_800_000_000,
        difficulty_commitment: [0x66; 32],
        nonce,
    }
}

fn transaction(lock_time: u64) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![],
        lock_time,
    }
}

#[test]
fn exact_message_tags_and_fixed_payload_lengths_are_stable() {
    let ack = HelloAck {
        selected_protocol_version: 1,
        enabled_features: FeatureSet::HEADERS_SYNC,
        remote_nonce_echo: [0x77; 16],
    };

    let cases = [
        (Message::Hello(hello()), TAG_HELLO, 108),
        (Message::HelloAck(ack), TAG_HELLO_ACK, 26),
        (Message::Ping(0x0102_0304_0506_0708), TAG_PING, 8),
        (Message::Pong(0x1112_1314_1516_1718), TAG_PONG, 8),
    ];

    for (message, expected_tag, expected_len) in cases {
        let (tag, payload) = encode_message(&message).unwrap();
        assert_eq!(tag, expected_tag);
        assert_eq!(payload.len(), expected_len);
        assert_eq!(decode_message(tag, &payload).unwrap(), message);
    }

    assert_eq!(TAG_HELLO, 0x01);
    assert_eq!(TAG_HELLO_ACK, 0x02);
    assert_eq!(TAG_PING, 0x03);
    assert_eq!(TAG_PONG, 0x04);
    assert_eq!(TAG_INV, 0x10);
    assert_eq!(TAG_GET_DATA, 0x11);
    assert_eq!(TAG_GET_HEADERS, 0x20);
    assert_eq!(TAG_HEADERS, 0x21);
    assert_eq!(TAG_TRANSACTION, 0x30);
    assert_eq!(TAG_BLOCK, 0x31);
}

#[test]
fn fixed_messages_reject_truncation_and_trailing_bytes() {
    for message in [
        Message::Hello(hello()),
        Message::HelloAck(HelloAck {
            selected_protocol_version: 1,
            enabled_features: FeatureSet::HEADERS_SYNC,
            remote_nonce_echo: [0x88; 16],
        }),
        Message::Ping(9),
        Message::Pong(10),
    ] {
        let (tag, payload) = encode_message(&message).unwrap();
        assert!(decode_message(tag, &payload[..payload.len() - 1]).is_err());
        let mut trailing = payload;
        trailing.push(0);
        assert!(decode_message(tag, &trailing).is_err());
    }
}

#[test]
fn inventory_limits_accept_equality_and_reject_one_more() {
    let inventory = InventoryItem {
        kind: InventoryKind::Transaction,
        hash: hash(0x91),
    };

    let inv = Message::Inv(vec![inventory; MAX_INV_ITEMS]);
    let (tag, payload) = encode_message(&inv).unwrap();
    assert_eq!(tag, TAG_INV);
    assert_eq!(decode_message(tag, &payload).unwrap(), inv);
    assert!(matches!(
        encode_message(&Message::Inv(vec![inventory; MAX_INV_ITEMS + 1])),
        Err(ProtocolError::ListLimitExceeded {
            actual,
            max: MAX_INV_ITEMS
        }) if actual == MAX_INV_ITEMS + 1
    ));

    let get_data = Message::GetData(vec![inventory; MAX_GETDATA_ITEMS]);
    let (tag, payload) = encode_message(&get_data).unwrap();
    assert_eq!(tag, TAG_GET_DATA);
    assert_eq!(decode_message(tag, &payload).unwrap(), get_data);
    assert!(matches!(
        encode_message(&Message::GetData(vec![inventory; MAX_GETDATA_ITEMS + 1])),
        Err(ProtocolError::ListLimitExceeded {
            actual,
            max: MAX_GETDATA_ITEMS
        }) if actual == MAX_GETDATA_ITEMS + 1
    ));
}

#[test]
fn unknown_message_tag_and_inventory_kind_are_rejected() {
    assert_eq!(
        decode_message(0xff, &[]),
        Err(ProtocolError::UnknownMessageType(0xff))
    );

    let mut payload = vec![1, 0xff];
    payload.extend_from_slice(hash(0xaa).as_bytes());
    assert_eq!(
        decode_message(TAG_INV, &payload),
        Err(ProtocolError::UnknownInventoryKind(0xff))
    );
}

#[test]
fn canonical_varint_is_required_for_remote_lists() {
    let noncanonical_one = [0xfd, 0x01, 0x00];
    assert_eq!(
        decode_message(TAG_INV, &noncanonical_one),
        Err(ProtocolError::Primitive(
            oregon_primitives::PrimitiveError::NonCanonicalVarInt
        ))
    );
}

#[test]
fn remote_list_counts_are_bounded_before_item_decoding() {
    let cases = [
        (TAG_INV, MAX_INV_ITEMS),
        (TAG_GET_DATA, MAX_GETDATA_ITEMS),
        (TAG_GET_HEADERS, MAX_LOCATOR_HASHES),
        (TAG_HEADERS, MAX_HEADERS_PER_MESSAGE),
    ];

    for (tag, max) in cases {
        for declared in [(max + 1) as u64, u64::MAX] {
            let mut payload = Vec::new();
            write_varint(declared, &mut payload);

            assert_eq!(
                decode_message(tag, &payload),
                Err(ProtocolError::Primitive(
                    PrimitiveError::LengthLimitExceeded
                )),
                "tag {tag:#04x} accepted declared count {declared} with limit {max}"
            );
        }
    }
}

#[test]
fn locator_and_header_limits_accept_equality_and_reject_one_more() {
    let request = Message::GetHeaders(GetHeaders {
        locator: vec![hash(0xb1); MAX_LOCATOR_HASHES],
        stop: Some(hash(0xb2)),
    });
    let (tag, payload) = encode_message(&request).unwrap();
    assert_eq!(tag, TAG_GET_HEADERS);
    assert_eq!(decode_message(tag, &payload).unwrap(), request);
    assert!(matches!(
        encode_message(&Message::GetHeaders(GetHeaders {
            locator: vec![hash(0xb1); MAX_LOCATOR_HASHES + 1],
            stop: None,
        })),
        Err(ProtocolError::ListLimitExceeded {
            actual,
            max: MAX_LOCATOR_HASHES
        }) if actual == MAX_LOCATOR_HASHES + 1
    ));

    let headers = Message::Headers(vec![header(3); MAX_HEADERS_PER_MESSAGE]);
    let (tag, payload) = encode_message(&headers).unwrap();
    assert_eq!(tag, TAG_HEADERS);
    assert_eq!(decode_message(tag, &payload).unwrap(), headers);
    assert!(matches!(
        encode_message(&Message::Headers(vec![header(3); MAX_HEADERS_PER_MESSAGE + 1])),
        Err(ProtocolError::ListLimitExceeded {
            actual,
            max: MAX_HEADERS_PER_MESSAGE
        }) if actual == MAX_HEADERS_PER_MESSAGE + 1
    ));
}

#[test]
fn get_headers_stop_flag_must_be_zero_or_one() {
    assert_eq!(
        decode_message(TAG_GET_HEADERS, &[0, 2]),
        Err(ProtocolError::InvalidStopFlag(2))
    );
}

#[test]
fn transaction_and_block_payloads_are_existing_canonical_bytes() {
    let tx = transaction(5);
    let tx_message = Message::Transaction(tx.clone());
    let (tag, payload) = encode_message(&tx_message).unwrap();
    assert_eq!(tag, TAG_TRANSACTION);
    assert_eq!(payload, tx.encode());
    assert_eq!(decode_message(tag, &payload).unwrap(), tx_message);

    let block = Block {
        header: header(6),
        transactions: vec![transaction(7)],
    };
    let block_message = Message::Block(block.clone());
    let (tag, payload) = encode_message(&block_message).unwrap();
    assert_eq!(tag, TAG_BLOCK);
    assert_eq!(payload, block.encode());
    assert_eq!(
        Block::decode(&payload, &DecodeLimits::default()).unwrap(),
        block
    );
    assert_eq!(decode_message(tag, &payload).unwrap(), block_message);
}

#[test]
fn negotiation_selects_highest_overlap_and_ignores_unknown_optional_bits() {
    let unknown_optional = FeatureSet::from_bits(1 << 63);
    let local = Hello {
        min_protocol_version: 1,
        max_protocol_version: 3,
        offered_features: FeatureSet::HEADERS_SYNC | FeatureSet::BLOCK_RELAY | unknown_optional,
        required_features: FeatureSet::HEADERS_SYNC,
        ..hello()
    };
    let remote = Hello {
        min_protocol_version: 2,
        max_protocol_version: 4,
        offered_features: FeatureSet::HEADERS_SYNC | FeatureSet::TX_RELAY | unknown_optional,
        required_features: FeatureSet::HEADERS_SYNC,
        ..hello()
    };

    assert_eq!(
        negotiate(&local, &remote).unwrap(),
        Negotiated {
            protocol_version: 3,
            features: FeatureSet::HEADERS_SYNC,
        }
    );
}

#[test]
fn negotiation_rejects_no_version_overlap_and_unsupported_requirements() {
    let local = Hello {
        min_protocol_version: 1,
        max_protocol_version: 1,
        offered_features: FeatureSet::HEADERS_SYNC,
        required_features: FeatureSet::HEADERS_SYNC,
        ..hello()
    };
    let no_overlap = Hello {
        min_protocol_version: 2,
        max_protocol_version: 3,
        ..hello()
    };
    assert_eq!(
        negotiate(&local, &no_overlap),
        Err(ProtocolError::NoCommonProtocolVersion)
    );

    let requires_tx = Hello {
        offered_features: FeatureSet::HEADERS_SYNC | FeatureSet::TX_RELAY,
        required_features: FeatureSet::TX_RELAY,
        ..hello()
    };
    assert_eq!(
        negotiate(&local, &requires_tx),
        Err(ProtocolError::UnsupportedRequiredFeatures(
            FeatureSet::TX_RELAY.bits()
        ))
    );

    let requires_unknown = Hello {
        offered_features: FeatureSet::HEADERS_SYNC | FeatureSet::from_bits(1 << 63),
        required_features: FeatureSet::from_bits(1 << 63),
        ..hello()
    };
    assert_eq!(
        negotiate(&local, &requires_unknown),
        Err(ProtocolError::UnsupportedRequiredFeatures(1 << 63))
    );
}

#[test]
fn negotiation_rejects_required_features_that_were_not_offered() {
    let inconsistent = Hello {
        offered_features: FeatureSet::HEADERS_SYNC,
        required_features: FeatureSet::TX_RELAY,
        ..hello()
    };

    assert_eq!(
        negotiate(&inconsistent, &hello()),
        Err(ProtocolError::RequiredFeaturesNotOffered(
            FeatureSet::TX_RELAY.bits()
        ))
    );
}

#[test]
fn negotiation_rejects_zero_and_reversed_version_ranges() {
    for invalid in [
        Hello {
            min_protocol_version: 0,
            max_protocol_version: 1,
            ..hello()
        },
        Hello {
            min_protocol_version: 3,
            max_protocol_version: 2,
            ..hello()
        },
    ] {
        assert_eq!(
            negotiate(&invalid, &hello()),
            Err(ProtocolError::InvalidProtocolVersionRange {
                min: invalid.min_protocol_version,
                max: invalid.max_protocol_version,
            })
        );
    }
}

#[test]
fn frame_golden_vector_is_exact() {
    let magic = network_magic(hash(0x11));
    assert_eq!(magic, [0xca, 0x20, 0x34, 0xec]);

    let (tag, payload) = encode_message(&Message::Ping(0x0102_0304_0506_0708)).unwrap();
    let header = build_frame_header(magic, tag, &payload).unwrap();
    let mut frame = header.encode().to_vec();
    frame.extend_from_slice(&payload);

    assert_eq!(
        frame,
        vec![
            0xca, 0x20, 0x34, 0xec, 0x01, 0x03, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x55, 0x94,
            0x94, 0xc1, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
        ]
    );
    assert_eq!(
        FrameHeader::decode(&frame[..FRAME_HEADER_BYTES]).unwrap(),
        header
    );
    assert_eq!(verify_frame_payload(&header, magic, &payload), Ok(()));
}

#[test]
fn frame_validation_rejects_corruption_wrong_magic_and_length_mismatch() {
    let magic = network_magic(hash(0x11));
    let (tag, payload) = encode_message(&Message::Ping(9)).unwrap();
    let header = build_frame_header(magic, tag, &payload).unwrap();

    let mut corrupt = payload.clone();
    corrupt[0] ^= 1;
    assert_eq!(
        verify_frame_payload(&header, magic, &corrupt),
        Err(ProtocolError::ChecksumMismatch)
    );
    assert_eq!(
        verify_frame_payload(&header, [0; 4], &payload),
        Err(ProtocolError::WrongNetworkMagic)
    );
    assert!(matches!(
        verify_frame_payload(&header, magic, &payload[..7]),
        Err(ProtocolError::PayloadLengthMismatch {
            declared: 8,
            actual: 7
        })
    ));
}

#[test]
fn frame_header_rejects_truncation_oversize_flags_version_and_unknown_type() {
    let mut bytes = [0u8; FRAME_HEADER_BYTES];
    bytes[4] = FRAME_VERSION;
    bytes[5] = TAG_PING;
    bytes[8..12].copy_from_slice(&8u32.to_le_bytes());

    assert!(FrameHeader::decode(&bytes[..15]).is_err());

    let mut oversize = bytes;
    oversize[8..12].copy_from_slice(&((MAX_FRAME_PAYLOAD + 1) as u32).to_le_bytes());
    assert!(matches!(
        FrameHeader::decode(&oversize),
        Err(ProtocolError::PayloadTooLarge {
            actual,
            max: MAX_FRAME_PAYLOAD
        }) if actual == MAX_FRAME_PAYLOAD + 1
    ));

    let mut flags = bytes;
    flags[6..8].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        FrameHeader::decode(&flags),
        Err(ProtocolError::NonZeroFlags(1))
    );

    let mut version = bytes;
    version[4] = FRAME_VERSION + 1;
    assert_eq!(
        FrameHeader::decode(&version),
        Err(ProtocolError::UnsupportedFrameVersion(FRAME_VERSION + 1))
    );

    let mut unknown = bytes;
    unknown[5] = 0xff;
    assert_eq!(
        FrameHeader::decode(&unknown),
        Err(ProtocolError::UnknownMessageType(0xff))
    );
}

#[test]
fn handshake_frame_limit_is_stricter_than_general_frame_limit() {
    let payload = vec![0; MAX_HANDSHAKE_PAYLOAD + 1];
    assert_eq!(
        build_frame_header([0; 4], TAG_HELLO, &payload),
        Err(ProtocolError::PayloadTooLarge {
            actual: MAX_HANDSHAKE_PAYLOAD + 1,
            max: MAX_HANDSHAKE_PAYLOAD,
        })
    );
}
