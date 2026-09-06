mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use common::{MemorySource, empty_snapshot, seed_snapshot};
use oregon_contract_state::{DomainSnapshot, StateWrite, read_value};
use oregon_execution::{ExecutionJournalV1, JournalContextV1, JournalIntentV1, JournalLimitsV1};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct VectorDocument {
    version: u16,
    context: VectorContext,
    cases: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
struct VectorContext {
    chain_id: u64,
    height: u64,
    parent_block_hash_hex: String,
    txid_hex: String,
}

#[derive(Debug, Deserialize)]
struct VectorCase {
    name: String,
    domains: Vec<VectorDomain>,
    operations: Vec<VectorOperation>,
    intent: String,
    expected_domains: Vec<ExpectedDomain>,
    expected_transition_domains: Vec<u16>,
}

#[derive(Debug, Deserialize)]
struct VectorDomain {
    domain_id: u16,
    base_entries: Vec<VectorEntry>,
}

#[derive(Debug, Deserialize)]
struct VectorEntry {
    key_hex: String,
    value_hex: String,
}

#[derive(Debug, Deserialize)]
struct VectorOperation {
    op: String,
    domain_id: Option<u16>,
    key_hex: Option<String>,
    value_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExpectedDomain {
    domain_id: u16,
    old_root_hex: String,
    new_root_hex: String,
    final_entries: Vec<VectorEntry>,
}

fn decode_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("journal vectors require canonical lowercase hex"),
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0, "hex input must have an even length");
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (decode_nibble(pair[0]) << 4) | decode_nibble(pair[1]))
        .collect()
}

fn encode_hex(value: &[u8]) -> String {
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn parse_hash(value: &str) -> Hash256 {
    value.parse().expect("vector hash is canonical")
}

fn parse_domain(value: u16) -> CommitmentDomainId {
    CommitmentDomainId::try_from(value).expect("vector uses a known commitment domain")
}

fn required_domain(operation: &VectorOperation) -> CommitmentDomainId {
    parse_domain(
        operation
            .domain_id
            .expect("state operation has a domain_id"),
    )
}

fn required_key(operation: &VectorOperation) -> Vec<u8> {
    decode_hex(
        operation
            .key_hex
            .as_deref()
            .expect("state operation has a key_hex"),
    )
}

fn load_vectors() -> VectorDocument {
    serde_json::from_str(include_str!("../../../tests/vectors/journal-v1.json"))
        .expect("journal vector JSON is valid")
}

#[test]
fn independent_journal_vectors_match_runtime_trace_results() {
    let vectors = load_vectors();
    assert_eq!(vectors.version, 1);
    assert_eq!(vectors.cases.len(), 6);

    let context = JournalContextV1 {
        chain_id: vectors.context.chain_id,
        height: vectors.context.height,
        parent_block_hash: parse_hash(&vectors.context.parent_block_hash_hex),
        txid: parse_hash(&vectors.context.txid_hex),
    };

    for case in &vectors.cases {
        let mut source = MemorySource::default();
        let mut snapshots = Vec::with_capacity(case.domains.len());
        let mut observed_keys: BTreeMap<CommitmentDomainId, BTreeSet<Vec<u8>>> = BTreeMap::new();

        for configured in &case.domains {
            let domain = parse_domain(configured.domain_id);
            let mut writes = Vec::with_capacity(configured.base_entries.len());
            for item in &configured.base_entries {
                let key = decode_hex(&item.key_hex);
                let value = decode_hex(&item.value_hex);
                observed_keys.entry(domain).or_default().insert(key.clone());
                writes.push(StateWrite::put(key, value));
            }

            let snapshot = if writes.is_empty() {
                empty_snapshot(domain)
            } else {
                seed_snapshot(&mut source, domain, writes)
            };
            let expected = case
                .expected_domains
                .iter()
                .find(|expected| expected.domain_id == configured.domain_id)
                .expect("configured domain has expected roots");
            assert_eq!(
                snapshot.root,
                parse_hash(&expected.old_root_hex),
                "{}: independent base root differs from Rust Stage 2",
                case.name
            );
            snapshots.push(snapshot);
        }

        let mut journal =
            ExecutionJournalV1::new(&source, context, &snapshots, JournalLimitsV1::default())
                .expect("vector journal construction succeeds");

        for operation in &case.operations {
            match operation.op.as_str() {
                "begin" => journal.begin_frame().expect("begin succeeds"),
                "commit" => journal.commit_frame().expect("child commit succeeds"),
                "revert" => journal.revert_frame().expect("child revert succeeds"),
                "put" => {
                    let domain = required_domain(operation);
                    let key = required_key(operation);
                    let value = decode_hex(
                        operation
                            .value_hex
                            .as_deref()
                            .expect("put operation has value_hex"),
                    );
                    observed_keys.entry(domain).or_default().insert(key.clone());
                    journal.put(domain, &key, &value).expect("put succeeds");
                }
                "delete" => {
                    let domain = required_domain(operation);
                    let key = required_key(operation);
                    observed_keys.entry(domain).or_default().insert(key.clone());
                    journal.delete(domain, &key).expect("delete succeeds");
                }
                other => panic!("unknown journal vector operation: {other}"),
            }
        }

        let intent = match case.intent.as_str() {
            "committed" => JournalIntentV1::Committed,
            "reverted" => JournalIntentV1::Reverted,
            other => panic!("unknown journal vector intent: {other}"),
        };
        let result = journal
            .finalize(intent)
            .expect("journal vector finalization succeeds");
        assert_eq!(result.context, context, "{}: context changed", case.name);
        assert_eq!(
            result.roots.len(),
            case.expected_domains.len(),
            "{}: result domain count differs",
            case.name
        );

        let actual_order: Vec<u16> = result
            .roots
            .iter()
            .map(|roots| u16::from(roots.domain))
            .collect();
        let mut sorted_order = actual_order.clone();
        sorted_order.sort_unstable();
        assert_eq!(
            actual_order, sorted_order,
            "{}: result roots are not in numeric domain order",
            case.name
        );

        for (actual, expected) in result.roots.iter().zip(&case.expected_domains) {
            assert_eq!(
                u16::from(actual.domain),
                expected.domain_id,
                "{}: result domain differs",
                case.name
            );
            assert_eq!(
                actual.old_root,
                parse_hash(&expected.old_root_hex),
                "{}: old root differs",
                case.name
            );
            assert_eq!(
                actual.new_root,
                parse_hash(&expected.new_root_hex),
                "{}: new root differs",
                case.name
            );
        }

        let transition_domains: Vec<u16> = result
            .transitions
            .iter()
            .map(|transition| u16::from(transition.domain))
            .collect();
        assert_eq!(
            transition_domains, case.expected_transition_domains,
            "{}: transition domain set differs",
            case.name
        );

        let mut published = source.clone();
        for transition in &result.transitions {
            published.absorb(transition);
        }

        for expected in &case.expected_domains {
            let domain = parse_domain(expected.domain_id);
            let final_state: BTreeMap<Vec<u8>, Vec<u8>> = expected
                .final_entries
                .iter()
                .map(|item| (decode_hex(&item.key_hex), decode_hex(&item.value_hex)))
                .collect();
            let snapshot = DomainSnapshot {
                domain,
                root: parse_hash(&expected.new_root_hex),
            };
            for key in observed_keys.get(&domain).into_iter().flatten() {
                assert_eq!(
                    read_value(&published, snapshot, key).expect("published vector read succeeds"),
                    final_state.get(key).cloned(),
                    "{}: final value differs for domain {:#06x}, key {}",
                    case.name,
                    expected.domain_id,
                    encode_hex(key)
                );
            }
        }
    }
}
