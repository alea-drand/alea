# Changelog

All notable changes to Alea (on-chain drand BN254 verifier for Solana) are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Interface stability guarantees for the `verify` instruction are frozen at v1 per [ADR 0028](build-spec/decisions/0028-cpi-versioning.md).

## [Unreleased]

Nothing yet.

## [0.1.0] — 2026-04-19 (devnet)

Initial release. Solana devnet only; mainnet deployment is the Phase 5 gate.

### Added — On-chain program (`alea-verifier`)

- `verify(round: u64, signature: [u8; 64]) -> Result<[u8; 32]>` instruction
- `initialize(config: InitializeParams) -> Result<()>` admin instruction with upgrade-authority gate (FENDER-002, `UnauthorizedInit` code 6012)
- `update_config(params: UpdateConfigParams) -> Result<()>` admin instruction with `has_one = authority` constraint
- Full on-chain SVDW hash-to-curve implementation on Solana BPF (no off-chain hinting)
- `AleaError` enum: 6000–6012 (frozen per ADR 0028 append-only)
- `BeaconVerified` event: `{round, randomness, payer}` (schema frozen per ADR 0028)
- Config PDA (seeds `[b"config"]`) with byte-equality guards against `EXPECTED_EVMNET_CHAIN_HASH` / `EXPECTED_EVMNET_G2_PUBKEY` / `EXPECTED_EVMNET_GENESIS_TIME` / `EXPECTED_EVMNET_PERIOD`

### Added — Rust CPI crate (`alea-sdk` on crates.io)

- `cpi::verify(alea_program, config, payer, round, signature) -> Result<[u8; 32]>` — one-line CPI helper with:
  - `#[must_use]` attribute — flags silent-empty-randomness footgun if return value ignored
  - Runtime owner check on `config` account (Phase 4.5 defense-in-depth for non-Anchor callers)
- `is_round_recent(round, &Config, &Clock, max_age_seconds) -> bool` — pure recency predicate with saturating arithmetic and negative-clock clamp
- `config_pda(&Pubkey) -> (Pubkey, u8)` — PDA derivation helper
- `PROGRAM_ID: Pubkey` — canonical vanity program ID constant
- Re-exports: `AleaVerifier` (Program type), `Config`, `AleaError`
- Dev-dep devnet integration tests (`#[ignore]` by default)

### Added — TypeScript SDK (`@alea-drand/sdk` on npm)

- ESM-only, browser-compatible (no Node built-ins at runtime; IDL inlined at build)
- `getVerifiedRandomness(options)` — fetches latest drand beacon, submits verify tx, returns 32 bytes
- `verifyDrandBeacon(args)` — accepts a pre-fetched beacon
- `fetchBeacon(round?, { signal? })` — drand API fetch with 5-endpoint fallback + 3 retries + round verification + response size cap + redirect blocking
- `isRoundRecent(round, config, clock, maxAge)` — TS symmetric with Rust SDK
- `createVerifyInstruction({ round, signature, payer, programId? })` — low-level ix builder
- `getConfigAddress(programId?)` — PDA derivation
- AbortSignal threading through fetch + tx construction (pre-broadcast cancel only)
- `skipPreflight` opt-out for local debugging (default `true`)
- Constants: `DRAND_CHAIN_HASH`, `DRAND_GENESIS_TIME`, `DRAND_PERIOD`, `DRAND_ENDPOINTS`, `DEVNET_PROGRAM_ID`, `MAINNET_PROGRAM_ID` (throw-proxy)
- `AleaError` class + `ERRORS` frozen map (on-chain 2001/3010/6000–6012 + SDK 6100–6103)

### Added — Canonical example consumer

- `programs/example-lottery/` — commit-reveal lottery demonstrating all mandatory + SHOULD constraints (`seeds::program`, `is_round_recent`, immediate return-data capture, minimum-future-round enforcement)
- Checked-arithmetic payout path (Phase 4.5 T1-16 rewrite) eliminates underflow class

### Security

- Apache 2.0 license, fully open source
- ADR 0028 freezes the v1 CPI interface: `verify` instruction signature, `Config` account layout (217 bytes), `Verify` Accounts struct, return-data format, `BeaconVerified` event schema. Breaking changes require a new program ID.
- ADR 0034 mandates `seeds::program = alea_program.key()` on consumer-side Config PDA — enforced via documentation, rustdoc examples, example-lottery pattern, and CI IDL-diff gate.
- Deployer keypair authority (v1) → 90-day Squads 2-of-3 multisig transition committed per ADR 0009 → eventual immutable (post-audit).
- Internal audit trail (15 rounds cumulative, 8.66/10 arbitrated) + Phase 4.5 pre-publish audit (8 personas + 4 red-team agents, 2026-04-19): zero exploitable crypto or replay vulnerabilities; 16 T1 publish blockers + ~28 T2 hygiene items resolved.
- 23.82B fuzz iterations across 3 parallel cargo-fuzz targets — 0 crashes, 0 memory errors. Proof tarballs: [v0.2.0-audit-passed](https://github.com/alea-drand/alea/releases/tag/v0.2.0-audit-passed).
- `.github/workflows/supply-chain.yml` enforces cargo-deny (licenses + advisories + bans + sources) + npm audit + gitleaks on every PR + weekly cron. Baseline clean as of 2026-04-19 with 5 documented Anchor/Solana 1.18.x transitive advisories ignored (see `deny.toml`).
- npm releases carry Sigstore provenance attestation via GitHub Actions `id-token: write` + `npm publish --provenance`. Verify with `npm audit signatures`.
- External paid audit is the Phase 5 gate before mainnet deployment.

### Known Limitations

- Devnet only. Mainnet pending Phase 5.
- v1 single deployer keypair is a single point of failure until the multisig transition (see [ADR 0009](build-spec/decisions/0009-upgrade-authority.md)).
- CU budget: Alea verify consumes up to ~454K CU; every tx MUST include `ComputeBudgetInstruction::set_compute_unit_limit(900_000)`. The TS SDK injects this automatically; Rust consumers must add it manually.
- Supports drand `evmnet` chain only. mainnet / quicknet (BLS12-381) would require a separate deployment.
- BPF 6006 `PairingError` None-arm runtime test is open (infrastructure-failure path, not attacker-reachable; Phase 5 acceptance item).
- External consumers building on-chain with `cargo build-sbf` may need to pin `constant_time_eq = "=0.4.2"` due to Solana BPF rustc lag ([learning](https://github.com/alea-drand/alea/issues?q=bpf-rustc-lag)). Documented in `sdk/rust/README.md` §Troubleshooting.

### Program Addresses

| Cluster | Program ID | Config PDA |
|---------|-----------|------------|
| Devnet | `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U` | `6anALRxD98Tw7zbA9d5i4NJfTvxDsNBHohHVJWxv2Xm8` |
| Mainnet | Pending Phase 5 | Pending Phase 5 |

## Versioning Policy

Per ADR 0028:
- **patch** (0.1.0 → 0.1.1): bug fixes only; no interface changes
- **minor** (0.1.x → 0.2.0): new instructions (additive); existing instructions unchanged
- **major** (0.x.y → 1.0.0): reserved; v1 `verify` semantics preserved
- Breaking changes to `verify` v1: forbidden. Would require a new mainnet program ID (new deployment, not an upgrade).
