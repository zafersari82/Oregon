# Oregon (OREG)

Oregon is an experimental, independent proof-of-work blockchain project under active research and development.

Earlier M0A work in this repository explored monetary and founder-allocation rules as patches against Bitcoin Core. The current `oregon-v0-protocol` milestone is different: it is an independent Rust protocol foundation with its own canonical encoding, identifiers, Merkle commitment rules, block-header format, parsing limits, and golden vectors. It does not import Bitcoin implementation code.

## Working monetary design

- Native asset: OREG
- Maximum scheduled supply envelope: 1,000,000 OREG
- Founder allocation: 50,000 OREG (5%), one-time and publicly declared
- Mining allocation: up to 949,999.97 OREG under the currently approved integer halving design
- Initial mining subsidy design parameter: 2.375 OREG
- Halving interval design parameter: 200,000 blocks
- No continuing founder tax, admin mint, treasury tax, or hidden premine mechanism

The Rust protocol foundation currently freezes monetary representation and safety constants; it does not yet implement the final emission engine, proof-of-work, difficulty adjustment, chain state, or mining RPC.

## Protocol v0 foundation

Development branch: `oregon-v0-protocol`

- Design: `docs/superpowers/specs/2026-09-02-oregon-v0-protocol-design.md`
- Implementation plan: `docs/superpowers/plans/2026-09-02-oregon-v0-protocol-foundation.md`
- Golden vectors: `tests/vectors/protocol-v0.json`
- Acceptance record: `docs/checkpoints/OREGON_V0_PROTOCOL_FOUNDATION.md`

## Status

No production mainnet has been launched. There is no runnable P2P node, production genesis, final PoW/difficulty algorithm, wallet, mining RPC, or production founder key in this milestone.

The `main` branch is intended for verified checkpoints. Development work is isolated on milestone branches.

## Security

Never commit wallet seeds, private keys, API keys, signing secrets, or other credentials.
