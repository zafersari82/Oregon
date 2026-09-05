use std::str::FromStr;

use oregon_primitives::Hash256;
use oregon_primitives::execution_envelope::ExecutionEnvelopeV1;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vector {
    name: String,
    canonical_hex: String,
    signing_hash_hex: String,
    txid_hex: String,
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn literal_vectors_pin_signing_hash_and_txid() {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!(
        "../../../tests/vectors/execution-envelope-v1.json"
    ))
    .unwrap();

    for vector in vectors {
        let bytes = decode_hex(&vector.canonical_hex);
        let envelope = ExecutionEnvelopeV1::decode(&bytes)
            .unwrap_or_else(|error| panic!("{} failed to decode: {error:?}", vector.name));
        assert_eq!(
            envelope.signing_hash(),
            Hash256::from_str(&vector.signing_hash_hex).unwrap(),
            "{} signing hash",
            vector.name
        );
        assert_eq!(
            envelope.txid(),
            Hash256::from_str(&vector.txid_hex).unwrap(),
            "{} txid",
            vector.name
        );
    }
}
