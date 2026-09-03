use oregon_primitives::BlockHeader;

pub const POW_INPUT_DOMAIN: &[u8] = b"OREGON/POW/V1\0";

pub fn pow_input(header: &BlockHeader) -> Vec<u8> {
    let encoded = header.encode();
    debug_assert_eq!(encoded.len(), 114);

    let mut input = Vec::with_capacity(POW_INPUT_DOMAIN.len() + encoded.len());
    input.extend_from_slice(POW_INPUT_DOMAIN);
    input.extend_from_slice(&encoded);
    input
}
