# Changelog

All notable changes to Alea (on-chain drand BN254 verifier for Solana) are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Interface stability guarantees for the `verify` instruction are frozen at v1 per [ADR 0028](build-spec/decisions/0028-cpi-versioning.md).

## [Unreleased]

### Notes
- Pre-v0.1.0: specification frozen 2026-04-14 post-R3 audit (12-persona cold-read, mean composite 6.9). R3 fix-pass executed 2026-04-15 across 6 waves: build correctness, interface discipline, cryptographic correctness, security depth, OSS maintainability, polish. Phase 1 implementation next.

## [0.1.0] — TBD (target: before May 11, 2026 — Colosseum Frontier submission)

Initial release. Mainnet deployment on Solana.

### Added
- `alea-verifier` Anchor program — on-chain drand BN254 BLS verifier
- `verify(round: u64, signature: [u8; 64]) -> Result<[u8; 32]>` instruction
- `initialize` + `update_config` admin instructions
- Full on-chain SVDW hash-to-curve (no off-chain hinting)
- `AleaError` enum: codes 6000-6009
- `BeaconVerified` event with `{round, randomness, payer}`
- `alea-sdk` Rust CPI crate on crates.io
- `@alea/sdk` TypeScript SDK on npm (mirrors `drand` v1/v2 API + Solana tx construction)
- Interactive demo at docs site

### Security
- Apache 2.0 license
- Config PDA enforces `EXPECTED_EVMNET_CHAIN_HASH` (ADR 0031) — wrong-chain deploys rejected
- Deployer keypair authority (v1) → 90-day Squads 2-of-3 multisig transition (ADR 0009)
- `SECURITY.md` defines disclosure process (GitHub Security Advisory + email fallback)
- Published test vectors for rounds 1 + 9337227 (ADR 0029)
- `randomness = sha256(signature)` per drand `bls-bn254-unchained-on-g1` scheme — verified live against drand API (ADR 0036)

### Known Limitations
- v1 single deployer keypair is a single point of failure until the 90-day multisig transition (see [ADR 0009](build-spec/decisions/0009-upgrade-authority.md))
- CU budget TBD pending Phase 1.1.D empirical benchmark; SDK default 900,000 CU
- Supports drand `evmnet` chain only; mainnet/quicknet (BLS12-381) would require a separate deployment

## Versioning Policy

Per ADR 0028:
- **patch** (0.1.0 → 0.1.1): bug fixes only; no interface changes
- **minor** (0.1.x → 0.2.0): new instructions (additive); existing instructions unchanged
- **major** (0.x.y → 1.0.0): reserved; v1 `verify` semantics preserved
- Breaking changes to `verify` v1: forbidden. Would require a new mainnet program ID (new deployment, not an upgrade).
