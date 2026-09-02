use oregon_primitives::{
    Amount, BlockHeader, Hash256, Transaction, TxInput, TxOutput, transaction_root, write_varint,
};
use serde_json::json;

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn varint_hex(value: u64) -> String {
    let mut bytes = Vec::new();
    write_varint(value, &mut bytes);
    hex(&bytes)
}

fn transaction_json(name: &str, transaction: &Transaction) -> serde_json::Value {
    json!({
        "name": name,
        "version": transaction.version,
        "inputs": transaction.inputs.iter().map(|input| json!({
            "previous_txid": input.previous_txid.to_string(),
            "previous_output_index": input.previous_output_index,
            "sequence": input.sequence,
            "witness_hex": input.witness.iter().map(|item| hex(item)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "outputs": transaction.outputs.iter().map(|output| json!({
            "value_base_units": output.value.base_units(),
            "locking_program_hex": hex(&output.locking_program),
        })).collect::<Vec<_>>(),
        "lock_time": transaction.lock_time,
        "canonical_hex": hex(&transaction.encode()),
        "txid": transaction.txid().to_string(),
    })
}

fn main() {
    let minimum = Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![],
        lock_time: 0,
    };

    let multi = Transaction {
        version: 1,
        inputs: vec![
            TxInput {
                previous_txid: Hash256::from_bytes([0x11; 32]),
                previous_output_index: 3,
                sequence: 7,
                witness: vec![vec![0xaa, 0xbb], vec![0xcc]],
            },
            TxInput {
                previous_txid: Hash256::from_bytes([0x22; 32]),
                previous_output_index: 5,
                sequence: 0xffff_fffe,
                witness: vec![vec![], vec![0x00, 0xff]],
            },
        ],
        outputs: vec![
            TxOutput {
                value: Amount::from_base_units(42).unwrap(),
                locking_program: vec![0x51, 0x21, 0x02],
            },
            TxOutput {
                value: Amount::from_base_units(123_456_789).unwrap(),
                locking_program: vec![0xde, 0xad, 0xbe, 0xef],
            },
        ],
        lock_time: 9,
    };

    let third = Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![],
        lock_time: 2,
    };

    let transactions = vec![minimum.clone(), multi.clone(), third.clone()];
    let one_root = transaction_root(&transactions[..1]).unwrap();
    let two_root = transaction_root(&transactions[..2]).unwrap();
    let three_root = transaction_root(&transactions).unwrap();

    let header = BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0x33; 32]),
        transaction_root: three_root,
        timestamp: 1_800_000_000,
        difficulty_commitment: [0x44; 32],
        nonce: 7,
    };

    let fixture = json!({
        "varints": [
            {"value": 0u64, "hex": varint_hex(0)},
            {"value": 0xfcu64, "hex": varint_hex(0xfc)},
            {"value": 0xfdu64, "hex": varint_hex(0xfd)},
            {"value": 0xffffu64, "hex": varint_hex(0xffff)},
            {"value": 0x1_0000u64, "hex": varint_hex(0x1_0000)},
            {"value": 0xffff_ffffu64, "hex": varint_hex(0xffff_ffff)},
            {"value": 0x1_0000_0000u64, "hex": varint_hex(0x1_0000_0000)},
            {"value": u64::MAX, "hex": varint_hex(u64::MAX)}
        ],
        "non_minimal_varints": [
            "fdfc00",
            "feffff0000",
            "ffffffffff00000000"
        ],
        "amounts": {
            "max_base_units": 100_000_000_000_000u64,
            "above_max_base_units": 100_000_000_000_001u64
        },
        "transactions": [
            transaction_json("minimum-v1", &minimum),
            transaction_json("multi-io-witness", &multi),
            transaction_json("third-for-odd-merkle", &third)
        ],
        "merkle": {
            "one_transaction_root": one_root.to_string(),
            "two_transaction_root": two_root.to_string(),
            "three_transaction_odd_promotion_root": three_root.to_string()
        },
        "block_header": {
            "version": header.version,
            "previous_block": header.previous_block.to_string(),
            "transaction_root": header.transaction_root.to_string(),
            "timestamp": header.timestamp,
            "difficulty_commitment": hex(&header.difficulty_commitment),
            "nonce": header.nonce,
            "canonical_hex": hex(&header.encode()),
            "block_id": header.block_id().to_string()
        }
    });

    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
