use std::fs;

use oregon_primitives::{
    Amount, BlockHeader, Decoder, Hash256, PrimitiveError, Transaction, TxInput, TxOutput,
    transaction_root, write_varint,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ProtocolVectors {
    varints: Vec<VarIntVector>,
    non_minimal_varints: Vec<String>,
    amounts: AmountVectors,
    transactions: Vec<TransactionVector>,
    merkle: MerkleVectors,
    block_header: BlockHeaderVector,
}

#[derive(Debug, Deserialize)]
struct VarIntVector {
    value: u64,
    hex: String,
}

#[derive(Debug, Deserialize)]
struct AmountVectors {
    max_base_units: u64,
    above_max_base_units: u64,
}

#[derive(Debug, Deserialize)]
struct TransactionVector {
    name: String,
    version: u16,
    inputs: Vec<InputVector>,
    outputs: Vec<OutputVector>,
    lock_time: u64,
    canonical_hex: String,
    txid: String,
}

#[derive(Debug, Deserialize)]
struct InputVector {
    previous_txid: String,
    previous_output_index: u32,
    sequence: u32,
    witness_hex: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OutputVector {
    value_base_units: u64,
    locking_program_hex: String,
}

#[derive(Debug, Deserialize)]
struct MerkleVectors {
    one_transaction_root: String,
    two_transaction_root: String,
    three_transaction_odd_promotion_root: String,
}

#[derive(Debug, Deserialize)]
struct BlockHeaderVector {
    version: u16,
    previous_block: String,
    transaction_root: String,
    timestamp: u64,
    difficulty_commitment: String,
    nonce: u64,
    canonical_hex: String,
    block_id: String,
}

fn fixture() -> ProtocolVectors {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/vectors/protocol-v0.json"
    );
    let json = fs::read_to_string(path).expect("protocol-v0 golden fixture must exist");
    serde_json::from_str(&json).expect("protocol-v0 golden fixture must match schema")
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0, "hex strings must contain whole bytes");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "golden vectors must use lowercase hexadecimal"
    );

    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(pair, 16).unwrap()
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn transaction_from_vector(vector: &TransactionVector) -> Transaction {
    Transaction {
        version: vector.version,
        inputs: vector
            .inputs
            .iter()
            .map(|input| TxInput {
                previous_txid: input.previous_txid.parse::<Hash256>().unwrap(),
                previous_output_index: input.previous_output_index,
                sequence: input.sequence,
                witness: input
                    .witness_hex
                    .iter()
                    .map(|item| decode_hex(item))
                    .collect(),
            })
            .collect(),
        outputs: vector
            .outputs
            .iter()
            .map(|output| TxOutput {
                value: Amount::from_base_units(output.value_base_units).unwrap(),
                locking_program: decode_hex(&output.locking_program_hex),
            })
            .collect(),
        lock_time: vector.lock_time,
    }
}

#[test]
fn protocol_v0_golden_vectors_match_current_consensus_primitives() {
    let vectors = fixture();

    for vector in &vectors.varints {
        let mut encoded = Vec::new();
        write_varint(vector.value, &mut encoded);
        assert_eq!(encode_hex(&encoded), vector.hex);

        let mut decoder = Decoder::new(&encoded);
        assert_eq!(decoder.read_varint().unwrap(), vector.value);
        decoder.finish().unwrap();
    }

    for encoded in &vectors.non_minimal_varints {
        let bytes = decode_hex(encoded);
        let mut decoder = Decoder::new(&bytes);
        assert_eq!(
            decoder.read_varint(),
            Err(PrimitiveError::NonCanonicalVarInt)
        );
    }

    assert!(Amount::from_base_units(vectors.amounts.max_base_units).is_ok());
    assert_eq!(
        Amount::from_base_units(vectors.amounts.above_max_base_units),
        Err(PrimitiveError::AmountAboveMaximum)
    );

    assert_eq!(vectors.transactions.len(), 3);
    assert_eq!(vectors.transactions[0].name, "minimum-v1");
    assert_eq!(vectors.transactions[1].name, "multi-io-witness");

    let transactions: Vec<Transaction> = vectors
        .transactions
        .iter()
        .map(|vector| {
            let transaction = transaction_from_vector(vector);
            assert_eq!(encode_hex(&transaction.encode()), vector.canonical_hex);
            assert_eq!(transaction.txid().to_string(), vector.txid);
            transaction
        })
        .collect();

    assert_eq!(
        transaction_root(&transactions[..1]).unwrap().to_string(),
        vectors.merkle.one_transaction_root
    );
    assert_eq!(
        transaction_root(&transactions[..2]).unwrap().to_string(),
        vectors.merkle.two_transaction_root
    );
    assert_eq!(
        transaction_root(&transactions[..3]).unwrap().to_string(),
        vectors.merkle.three_transaction_odd_promotion_root
    );

    let difficulty_bytes = decode_hex(&vectors.block_header.difficulty_commitment);
    let difficulty_commitment: [u8; 32] = difficulty_bytes.try_into().unwrap();
    let header = BlockHeader {
        version: vectors.block_header.version,
        previous_block: vectors.block_header.previous_block.parse().unwrap(),
        transaction_root: vectors.block_header.transaction_root.parse().unwrap(),
        timestamp: vectors.block_header.timestamp,
        difficulty_commitment,
        nonce: vectors.block_header.nonce,
    };

    assert_eq!(
        header.transaction_root.to_string(),
        vectors.merkle.three_transaction_odd_promotion_root
    );
    assert_eq!(encode_hex(&header.encode()), vectors.block_header.canonical_hex);
    assert_eq!(header.block_id().to_string(), vectors.block_header.block_id);
}
